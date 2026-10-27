//! Daily backups of company files.
//!
//! Before the first save of a day, the company file as it was is copied into
//! a `LedgerCraft Backups` folder beside it, named for the day:
//! `Pine Ridge 2026-10-27.lcbooks`. Saving again that day does not make
//! another copy, so each backup is how the books stood at the start of a
//! day's work. The oldest are deleted to keep the number chosen. Backups are
//! byte-for-byte copies: a password-protected file stays encrypted.

use std::fs;
use std::path::{Path, PathBuf};

use ledgercraft_ledger::Date;

use crate::{FileError, EXTENSION};

/// The folder backups go in, beside the company file.
pub const BACKUP_FOLDER: &str = "LedgerCraft Backups";

/// Where `path`'s backups are kept.
pub fn backup_dir(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(BACKUP_FOLDER)
}

/// The company file's name without `.lcbooks`.
fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Company".into())
}

/// Whether `name` is one of `stem`'s backups: `<stem> YYYY-MM-DD.lcbooks`.
fn is_backup_of(name: &str, stem: &str) -> bool {
    name.strip_prefix(stem)
        .and_then(|rest| rest.strip_prefix(' '))
        .and_then(|rest| rest.strip_suffix(&format!(".{EXTENSION}")))
        .is_some_and(|date| date.len() == 10 && Date::parse(date).is_ok())
}

/// `path`'s backups, oldest first.
pub fn backups_of(path: &Path) -> Vec<PathBuf> {
    let stem = stem(path);
    let Ok(entries) = fs::read_dir(backup_dir(path)) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| is_backup_of(&n.to_string_lossy(), &stem))
        })
        .collect();
    // The date in the name sorts them.
    found.sort();
    found
}

/// Copies `path` as it is now into today's backup, unless today's backup is
/// already made or there is no file yet, then deletes the oldest backups
/// beyond `keep`. Returns the backup made, if one was.
pub fn backup_before_save(path: &Path, today: Date, keep: usize) -> Result<Option<PathBuf>, FileError> {
    if keep == 0 || !path.is_file() {
        return Ok(None);
    }
    let dir = backup_dir(path);
    let target = dir.join(format!("{} {today}.{EXTENSION}", stem(path)));
    let made = if target.exists() {
        None
    } else {
        fs::create_dir_all(&dir)?;
        fs::copy(path, &target)?;
        Some(target)
    };
    let all = backups_of(path);
    let extra = all.len().saturating_sub(keep);
    for old in all.iter().take(extra) {
        // A backup that cannot be removed is left; it does not stop the save.
        let _ = fs::remove_file(old);
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    #[test]
    fn one_backup_a_day_and_the_oldest_go() {
        let dir = std::env::temp_dir().join(format!("lc-backup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Pine Ridge.lcbooks");

        // No file yet (a first save): nothing to back up.
        assert_eq!(backup_before_save(&file, d("2026-10-01"), 3).unwrap(), None);

        fs::write(&file, "monday").unwrap();
        let made = backup_before_save(&file, d("2026-10-01"), 3).unwrap();
        assert_eq!(
            made,
            Some(dir.join(BACKUP_FOLDER).join("Pine Ridge 2026-10-01.lcbooks"))
        );
        // Saving again the same day keeps the morning's copy.
        fs::write(&file, "monday afternoon").unwrap();
        assert_eq!(backup_before_save(&file, d("2026-10-01"), 3).unwrap(), None);
        let first = fs::read_to_string(dir.join(BACKUP_FOLDER).join("Pine Ridge 2026-10-01.lcbooks"));
        assert_eq!(first.unwrap(), "monday");

        for day in ["2026-10-02", "2026-10-03", "2026-10-04"] {
            backup_before_save(&file, d(day), 3).unwrap();
        }
        // Other files in the folder are left alone.
        fs::write(dir.join(BACKUP_FOLDER).join("notes.txt"), "x").unwrap();
        fs::write(dir.join(BACKUP_FOLDER).join("Pine Ridge Two 2026-01-01.lcbooks"), "x").unwrap();
        let names: Vec<String> = backups_of(&file)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                "Pine Ridge 2026-10-02.lcbooks",
                "Pine Ridge 2026-10-03.lcbooks",
                "Pine Ridge 2026-10-04.lcbooks",
            ]
        );
        assert!(dir.join(BACKUP_FOLDER).join("notes.txt").exists());
        // Off: nothing is made.
        assert_eq!(backup_before_save(&file, d("2026-10-05"), 0).unwrap(), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
