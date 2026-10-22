use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use ledgercraft_file::{FileError, Protection};
use ledgercraft_ledger::{Company, LedgerError};

use crate::Command;

const MAX_UNDO: usize = 200;

#[derive(Debug)]
pub enum EngineError {
    Ledger(LedgerError),
    File(FileError),
    BadCommand(String),
    NoPath,
    Internal(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ledger(e) => fmt::Display::fmt(e, f),
            Self::File(e) => fmt::Display::fmt(e, f),
            Self::BadCommand(e) => write!(f, "not a valid command: {e}"),
            Self::NoPath => f.write_str("choose where to save the company file first"),
            Self::Internal(e) => write!(f, "internal error, nothing was changed: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<LedgerError> for EngineError {
    fn from(e: LedgerError) -> Self {
        Self::Ledger(e)
    }
}

impl From<FileError> for EngineError {
    fn from(e: FileError) -> Self {
        Self::File(e)
    }
}

/// What a successful command did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub command: &'static str,
    /// The id of anything created, e.g. `T42`.
    pub created: Option<String>,
}

/// An open company file with its undo history.
pub struct Session {
    company: Company,
    path: Option<PathBuf>,
    undo: Vec<(String, Company)>,
    redo: Vec<(String, Company)>,
    dirty: bool,
    /// The file's encryption key, when it is password protected.
    protection: Option<Protection>,
}

impl Session {
    pub fn new(company: Company) -> Self {
        Self {
            company,
            path: None,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            protection: None,
        }
    }

    /// Opens a plain company file. A protected one fails with
    /// `FileError::PasswordRequired`; open it with [`Session::open_with_password`].
    pub fn open(path: &Path) -> Result<Self, EngineError> {
        Self::load(path, None)
    }

    pub fn open_with_password(path: &Path, password: &str) -> Result<Self, EngineError> {
        Self::load(path, Some(password))
    }

    fn load(path: &Path, password: Option<&str>) -> Result<Self, EngineError> {
        let (company, protection) = ledgercraft_file::load(path, password)?;
        let mut s = Self::new(company);
        s.path = Some(path.to_path_buf());
        s.protection = protection;
        Ok(s)
    }

    /// True when the company file is password protected.
    pub fn is_protected(&self) -> bool {
        self.protection.is_some()
    }

    /// Sets a new password (or removes it with `None`) and saves the file
    /// straight away if it has a location, so the change takes effect on disk
    /// immediately rather than at the next save.
    pub fn set_password(&mut self, password: Option<&str>) -> Result<(), EngineError> {
        self.protection = match password {
            Some(p) => Some(Protection::new(p)?),
            None => None,
        };
        if self.path.is_some() {
            self.save()
        } else {
            self.dirty = true;
            Ok(())
        }
    }

    pub fn company(&self) -> &Company {
        &self.company
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Window title: `Company — file.lcbooks*`.
    pub fn title(&self) -> String {
        let file = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unsaved".into());
        let star = if self.dirty { "*" } else { "" };
        format!("{} — {file}{star}", self.company.info.name)
    }

    /// Runs a command. On any error — including a bug that panics — the books
    /// are restored exactly as they were.
    pub fn execute(&mut self, cmd: Command) -> Result<Outcome, EngineError> {
        let id = cmd.id();
        let label = cmd.undo_label();
        let before = self.company.clone();
        let result = catch_unwind(AssertUnwindSafe(|| cmd.apply(&mut self.company)));
        match result {
            Ok(Ok(created)) => {
                self.undo.push((label, before));
                if self.undo.len() > MAX_UNDO {
                    self.undo.remove(0);
                }
                self.redo.clear();
                self.dirty = true;
                Ok(Outcome {
                    command: id,
                    created,
                })
            }
            Ok(Err(e)) => {
                self.company = before;
                Err(e.into())
            }
            Err(_) => {
                self.company = before;
                Err(EngineError::Internal(format!("{id} panicked")))
            }
        }
    }

    /// Parses and runs a JSON command.
    pub fn execute_json(&mut self, json: &str) -> Result<Outcome, EngineError> {
        let cmd: Command =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        self.execute(cmd)
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(l, _)| l.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(l, _)| l.as_str())
    }

    /// Undoes the last command, returning its label.
    pub fn undo(&mut self) -> Option<String> {
        let (label, prev) = self.undo.pop()?;
        let current = std::mem::replace(&mut self.company, prev);
        self.redo.push((label.clone(), current));
        self.dirty = true;
        Some(label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let (label, next) = self.redo.pop()?;
        let current = std::mem::replace(&mut self.company, next);
        self.undo.push((label.clone(), current));
        self.dirty = true;
        Some(label)
    }

    pub fn save(&mut self) -> Result<(), EngineError> {
        let path = self.path.clone().ok_or(EngineError::NoPath)?;
        ledgercraft_file::save(&path, &self.company, self.protection.as_ref())?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), EngineError> {
        let path = ledgercraft_file::with_extension(path);
        ledgercraft_file::save(&path, &self.company, self.protection.as_ref())?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file saved before encryption existed (format version 1) opens, takes
    /// a password, and is rewritten encrypted in place with its books intact.
    #[test]
    fn older_plain_files_upgrade_to_encrypted() {
        let dir = std::env::temp_dir().join(format!("lc-upgrade-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.lcbooks");
        let company = crate::sample_company().unwrap();
        // Exactly what LedgerCraft wrote before this change: plain version 1 JSON.
        std::fs::write(&path, ledgercraft_file::to_json(&company).unwrap()).unwrap();
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("\"version\": 1"));

        let mut s = Session::open(&path).unwrap();
        assert!(!s.is_protected());
        s.set_password(Some("upgrade me please")).unwrap();
        assert!(!s.is_dirty(), "the upgrade is saved straight away");

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"version\": 2"));
        assert!(!text.contains("Pine Ridge"));
        let reopened = Session::open_with_password(&path, "upgrade me please").unwrap();
        assert_eq!(reopened.company(), &company);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn protected_sessions_save_encrypted_and_reopen_with_the_password() {
        let dir = std::env::temp_dir().join(format!("lc-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("locked.lcbooks");
        let mut s = Session::new(Company::with_default_accounts("Locked Co"));
        s.set_password(Some("open sesame please")).unwrap();
        s.save_as(&path).unwrap();
        assert!(ledgercraft_file::is_protected(&path));
        assert!(matches!(
            Session::open(&path),
            Err(EngineError::File(FileError::PasswordRequired))
        ));
        assert!(Session::open_with_password(&path, "nope nope nope").is_err());
        let mut reopened = Session::open_with_password(&path, "open sesame please").unwrap();
        assert!(reopened.is_protected());
        assert_eq!(reopened.company().info.name, "Locked Co");
        // Removing the password writes a plain file at once.
        reopened.set_password(None).unwrap();
        assert!(!ledgercraft_file::is_protected(&path));
        assert!(Session::open(&path).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_commands_change_nothing_and_undo_works() {
        let mut s = Session::new(Company::with_default_accounts("Undo Co"));
        let before = s.company().clone();
        let bad = s.execute_json(r#"{"command":"account.create","name":"Checking","kind":"bank"}"#);
        assert!(bad.is_err());
        assert_eq!(s.company(), &before);
        assert!(!s.is_dirty());

        let ok = s
            .execute_json(
                r#"{"command":"account.create","number":"6700","name":"Postage","kind":"expense"}"#,
            )
            .unwrap();
        assert!(ok.created.is_some());
        assert!(s.company().account_by_name("Postage").is_some());
        assert_eq!(s.undo().as_deref(), Some("New Account Postage"));
        assert!(s.company().account_by_name("Postage").is_none());
        s.redo();
        assert!(s.company().account_by_name("Postage").is_some());
    }

    #[test]
    fn company_information_changes_and_undoes() {
        let mut s = Session::new(Company::with_default_accounts("Info Co"));
        let json = r#"{"command":"company.setInfo","name":" Info Co LLC ","phone":"555-0100",
            "address":{"line1":"1 Main St","city":"Provo","region":"UT"}}"#;
        s.execute_json(json).unwrap();
        assert_eq!(s.company().info.name, "Info Co LLC");
        assert_eq!(s.company().info.phone, "555-0100");
        assert_eq!(s.company().info.address.city, "Provo");
        assert_eq!(s.undo().as_deref(), Some("Company Information"));
        assert_eq!(s.company().info.name, "Info Co");
        assert!(s.company().info.address.is_empty());
        let blank = r#"{"command":"company.setInfo","name":"  "}"#;
        assert!(s.execute_json(blank).is_err());
    }

    #[test]
    fn every_command_rejects_empty_params_without_panicking() {
        for info in crate::CATALOG {
            let mut s = Session::new(Company::with_default_accounts("Fuzz"));
            let json = format!(r#"{{"command":"{}"}}"#, info.id);
            assert!(
                s.execute_json(&json).is_err(),
                "{} accepted empty params",
                info.id
            );
        }
    }

    #[test]
    fn editing_a_transaction_can_be_undone() {
        let mut s = Session::new(crate::sample_company().unwrap());
        let t = s
            .company()
            .transactions
            .iter()
            .find(|t| !t.voided && matches!(t.body, ledgercraft_ledger::TxnBody::Invoice(_)))
            .unwrap()
            .clone();
        s.execute(crate::Command::TxnUpdate {
            id: t.id,
            date: t.date,
            number: t.number.clone(),
            memo: "Edited".into(),
            body: t.body.clone(),
        })
        .unwrap();
        assert_eq!(s.company().txn(t.id).unwrap().memo, "Edited");
        assert_eq!(s.undo().as_deref(), Some("Edit Invoice"));
        assert_eq!(s.company().txn(t.id).unwrap().memo, t.memo);
    }

    #[test]
    fn reconciling_through_the_session_can_be_undone() {
        use ledgercraft_core::{Date, Money};
        let mut s = Session::new(crate::sample_company().unwrap());
        let checking = s.company().account_by_name("Checking").unwrap().id;
        let through = Date::parse("2099-12-31").unwrap();
        let items = s.company().unreconciled(checking, through);
        assert!(!items.is_empty());
        let ending = items
            .iter()
            .fold(Money::ZERO, |total, i| total.saturating_add(i.amount));
        s.execute(crate::Command::AccountReconcile {
            account: checking,
            statement_date: through,
            ending_balance: ending,
            cleared: items.iter().map(|i| i.txn).collect(),
        })
        .unwrap();
        assert_eq!(s.company().reconciled_balance(checking), ending);
        assert_eq!(s.undo().as_deref(), Some("Reconcile"));
        assert!(s.company().reconciliations.is_empty());
    }

    #[test]
    fn sample_company_is_valid() {
        let c = crate::sample_company().unwrap();
        assert!(c.transactions.len() > 10);
    }
}
