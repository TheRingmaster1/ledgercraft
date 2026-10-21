//! L2 of LedgerCraft: the company file.
//!
//! A `.lcbooks` file is UTF-8 JSON with a small envelope:
//! `{"format":"ledgercraft","version":1,"company":{…}}`. A password-protected
//! file is version 2: the same JSON, encrypted (see [`crypto`]), inside
//! `{"format":"ledgercraft","version":2,"encryption":{…},"ciphertext":"…"}`.
//! Both are documented in `docs/file-format.md`.

pub mod crypto;
pub mod import;

pub use crypto::{check_new_password, Protection, MIN_PASSWORD_CHARS};

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ledgercraft_ledger::Company;
use serde::{Deserialize, Serialize};

pub const EXTENSION: &str = "lcbooks";
pub const FORMAT: &str = "ledgercraft";
/// Version of a plain (unprotected) file.
pub const VERSION: u32 = 1;
/// Version of a password-protected file.
pub const PROTECTED_VERSION: u32 = 2;
/// Refuse to read files larger than this (hostile or corrupt input).
pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub enum FileError {
    Io(std::io::Error),
    Json(serde_json::Error),
    NotLedgerCraft,
    TooNew(u32),
    TooLarge(u64),
    /// The file is password protected and no password was given.
    PasswordRequired,
    /// The password does not open the file (or the file was altered).
    WrongPassword,
    WeakPassword(String),
    Damaged(String),
    Crypto(String),
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
            Self::PasswordRequired => f.write_str("this company file is password protected"),
            Self::WrongPassword => {
                f.write_str("the password is not correct (or the file has been changed or damaged)")
            }
            Self::WeakPassword(why) => write!(f, "that password is too weak: {why}"),
            Self::Damaged(why) => write!(f, "the file cannot be opened: {why}"),
            Self::Crypto(why) => write!(f, "encryption failed: {why}"),
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

#[derive(Serialize)]
struct ProtectedOut<'a> {
    format: &'a str,
    version: u32,
    encryption: &'a crypto::Header,
    ciphertext: &'a str,
}

#[derive(Deserialize)]
struct ProtectedIn {
    encryption: crypto::Header,
    ciphertext: String,
}

/// The file's text as it is saved: plain JSON, or encrypted with `protection`.
pub fn to_file_text(company: &Company, protection: Option<&Protection>) -> Result<String, FileError> {
    let json = zeroize::Zeroizing::new(to_json(company)?);
    let Some(protection) = protection else {
        return Ok(json.to_string());
    };
    let (header, ciphertext) = protection.seal(json.as_bytes())?;
    let out = ProtectedOut {
        format: FORMAT,
        version: PROTECTED_VERSION,
        encryption: &header,
        ciphertext: &ciphertext,
    };
    Ok(serde_json::to_string_pretty(&out)?)
}

/// Reads a file's text. A protected file needs `password`; the protection to
/// save it with again comes back with the company.
pub fn from_file_text(text: &str, password: Option<&str>) -> Result<(Company, Option<Protection>), FileError> {
    let head: serde_json::Value = serde_json::from_str(text)?;
    if head.get("format").and_then(|v| v.as_str()) != Some(FORMAT) {
        return Err(FileError::NotLedgerCraft);
    }
    let version = head.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version != u64::from(PROTECTED_VERSION) {
        return from_json(text).map(|c| (c, None));
    }
    let Some(password) = password else {
        return Err(FileError::PasswordRequired);
    };
    let env: ProtectedIn = serde_json::from_value(head)?;
    let (plain, protection) = Protection::open(&env.encryption, &env.ciphertext, password)?;
    let plain = std::str::from_utf8(&plain).map_err(|_| FileError::Damaged("its contents are not text".into()))?;
    let company = from_json(plain)?;
    Ok((company, Some(protection)))
}

/// True when the file at `path` is password protected.
pub fn is_protected(path: &Path) -> bool {
    matches!(load(path, None), Err(FileError::PasswordRequired))
}

/// Opens a company file; a protected one needs `password`.
pub fn load(path: &Path, password: Option<&str>) -> Result<(Company, Option<Protection>), FileError> {
    let len = fs::metadata(path)?.len();
    if len > MAX_FILE_BYTES {
        return Err(FileError::TooLarge(len));
    }
    let text = zeroize::Zeroizing::new(fs::read_to_string(path)?);
    from_file_text(&text, password)
}

/// Saves atomically: writes a temporary file next to the target, flushes it
/// to disk, then renames it over the target, so a crash or power cut never
/// leaves half a company file. With `protection` only encrypted bytes are
/// ever written, including to the temporary file.
pub fn save(path: &Path, company: &Company, protection: Option<&Protection>) -> Result<(), FileError> {
    let json = to_file_text(company, protection)?;
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
    fn protected_files_need_the_right_password() {
        let company = Company::with_default_accounts("Secret Co");
        let protection = Protection::new("correct horse battery").unwrap();
        let text = to_file_text(&company, Some(&protection)).unwrap();
        // Nothing readable is left in the file.
        assert!(!text.contains("Secret Co"));
        assert!(!text.contains("Checking"));
        assert!(text.contains("\"version\": 2"));

        assert!(matches!(from_file_text(&text, None), Err(FileError::PasswordRequired)));
        assert!(matches!(from_file_text(&text, Some("wrong password")), Err(FileError::WrongPassword)));
        let (opened, again) = from_file_text(&text, Some("correct horse battery")).unwrap();
        assert_eq!(opened, company);

        // Saving again with the returned protection still opens with the password,
        // and uses a fresh nonce each time.
        let again = again.unwrap();
        let second = to_file_text(&company, Some(&again)).unwrap();
        assert_ne!(second, text);
        assert!(from_file_text(&second, Some("correct horse battery")).is_ok());
    }

    #[test]
    fn tampering_is_detected() {
        let company = Company::new("Tamper Co");
        let protection = Protection::new("a long passphrase").unwrap();
        let text = to_file_text(&company, Some(&protection)).unwrap();
        // Weaken the stored settings: the header is authenticated, so this fails.
        let tampered = text.replace("\"iterations\": 2", "\"iterations\": 1");
        assert_ne!(tampered, text);
        assert!(matches!(from_file_text(&tampered, Some("a long passphrase")), Err(FileError::WrongPassword)));
        // Absurd settings are refused before any work is done.
        let huge = text.replace("\"memory_kib\": 19456", "\"memory_kib\": 4000000000");
        assert!(matches!(from_file_text(&huge, Some("a long passphrase")), Err(FileError::Damaged(_))));
    }

    #[test]
    fn weak_passwords_are_refused() {
        assert!(matches!(Protection::new("short"), Err(FileError::WeakPassword(_))));
        assert!(matches!(Protection::new("        "), Err(FileError::WeakPassword(_))));
        assert!(check_new_password("long enough").is_ok());
    }

    #[test]
    fn round_trip_through_disk() {
        let company = Company::with_default_accounts("Disk Co");
        let dir = std::env::temp_dir().join(format!("ledgercraft-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("books.lcbooks");
        save(&path, &company, None).unwrap();
        assert_eq!(load(&path, None).unwrap().0, company);
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
