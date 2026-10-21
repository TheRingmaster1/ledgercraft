//! L2 of LedgerCraft: the company file.
//!
//! A `.lcbooks` file is UTF-8 JSON with a small envelope:
//! `{"format":"ledgercraft","version":1,"company":{…}}`. It is documented in
//! `docs/file-format.md` so any program can read your books, forever.

pub mod import;

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ledgercraft_ledger::Company;
use serde::{Deserialize, Serialize};

pub const EXTENSION: &str = "lcbooks";
pub const FORMAT: &str = "ledgercraft";
pub const VERSION: u32 = 1;
/// Refuse to read files larger than this (hostile or corrupt input).
pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub enum FileError {
    Io(std::io::Error),
    Json(serde_json::Error),
    NotLedgerCraft,
    TooNew(u32),
    TooLarge(u64),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read or write the file: {e}"),
            Self::Json(e) => write!(f, "the file is damaged or not a company file: {e}"),
            Self::NotLedgerCraft => f.write_str("this is not a LedgerCraft company file"),
            Self::TooNew(v) => write!(
                f,
                "this file was saved by a newer LedgerCraft (format {v}); please update"
            ),
            Self::TooLarge(n) => write!(f, "the file is too large to open ({n} bytes)"),
        }
    }
}

impl std::error::Error for FileError {}

impl From<std::io::Error> for FileError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for FileError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

#[derive(Serialize)]
struct EnvelopeOut<'a> {
    format: &'a str,
    version: u32,
    company: &'a Company,
}

#[derive(Deserialize)]
struct EnvelopeIn {
    format: String,
    version: u32,
    company: Company,
}

pub fn to_json(company: &Company) -> Result<String, FileError> {
    let env = EnvelopeOut { format: FORMAT, version: VERSION, company };
    Ok(serde_json::to_string_pretty(&env)?)
}

pub fn from_json(text: &str) -> Result<Company, FileError> {
    // Peek at the envelope first so a newer file gives a clear message.
    let head: serde_json::Value = serde_json::from_str(text)?;
    if head.get("format").and_then(|v| v.as_str()) != Some(FORMAT) {
        return Err(FileError::NotLedgerCraft);
    }
    let version = head.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version > u64::from(VERSION) {
        return Err(FileError::TooNew(u32::try_from(version).unwrap_or(u32::MAX)));
    }
    let env: EnvelopeIn = serde_json::from_value(head)?;
    if env.format != FORMAT || env.version > VERSION {
        return Err(FileError::NotLedgerCraft);
    }
    let mut company = env.company;
    company.repair_next_id();
    Ok(company)
}

pub fn load(path: &Path) -> Result<Company, FileError> {
    let len = fs::metadata(path)?.len();
    if len > MAX_FILE_BYTES {
        return Err(FileError::TooLarge(len));
    }
    let text = fs::read_to_string(path)?;
    from_json(&text)
}

/// Saves atomically: writes a temporary file next to the target, flushes it
/// to disk, then renames it over the target, so a crash or power cut never
/// leaves half a company file.
pub fn save(path: &Path, company: &Company) -> Result<(), FileError> {
    let json = to_json(company)?;
    let tmp = temp_path(path);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
        if let Err(e) = replace_with_retry(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Renames `tmp` over `path`. On Windows, OneDrive, antivirus scanners and
/// search indexing often hold the target open for a moment right after a
/// write, and the rename then fails with "access denied". Those holds are
/// brief, so retry for up to about two seconds before giving up.
fn replace_with_retry(tmp: &Path, path: &Path) -> std::io::Result<()> {
    const ATTEMPTS: u32 = 20;
    let mut attempt = 1;
    loop {
        match fs::rename(tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < ATTEMPTS && is_transient_lock(&e) => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_transient_lock(e: &std::io::Error) -> bool {
    // ERROR_ACCESS_DENIED (5), ERROR_SHARING_VIOLATION (32), ERROR_LOCK_VIOLATION (33)
    e.kind() == std::io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}


fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Adds the `.lcbooks` extension when a path has none.
pub fn with_extension(path: &Path) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension(EXTENSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_disk() {
        let company = Company::with_default_accounts("Disk Co");
        let dir = std::env::temp_dir().join(format!("ledgercraft-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("books.lcbooks");
        save(&path, &company).unwrap();
        assert_eq!(load(&path).unwrap(), company);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_foreign_and_future_files() {
        assert!(matches!(from_json("{\"format\":\"other\"}"), Err(FileError::NotLedgerCraft)));
        assert!(matches!(
            from_json("{\"format\":\"ledgercraft\",\"version\":99,\"company\":{}}"),
            Err(FileError::TooNew(99))
        ));
        assert!(matches!(from_json("not json"), Err(FileError::Json(_))));
    }
}
