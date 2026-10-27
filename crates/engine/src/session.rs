use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use ledgercraft_core::Date;
use ledgercraft_file::{FileError, Protection};
use ledgercraft_ledger::{AuditEntry, Company, FeedAction, LedgerError, TxnId};

use crate::Command;

const MAX_UNDO: usize = 200;

#[derive(Debug)]
pub enum EngineError {
    Ledger(LedgerError),
    File(FileError),
    BadCommand(String),
    NoPath,
    Internal(String),
    /// The change is to closed books.
    Closed(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ledger(e) => fmt::Display::fmt(e, f),
            Self::File(e) => fmt::Display::fmt(e, f),
            Self::BadCommand(e) => write!(f, "not a valid command: {e}"),
            Self::NoPath => f.write_str("choose where to save the company file first"),
            Self::Internal(e) => write!(f, "internal error, nothing was changed: {e}"),
            Self::Closed(e) => f.write_str(e),
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
    /// The closing date password was entered: closed books can be changed
    /// until the company is closed.
    closing_unlocked: bool,
    /// How many daily backups to keep beside the file (0: none).
    backups: usize,
    /// Why the last save could not make its backup, if it could not.
    backup_problem: Option<String>,
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
            closing_unlocked: false,
            backups: 0,
            backup_problem: None,
        }
    }

    /// Keep `keep` daily backups of the file, made before the first save of
    /// each day (0 turns backups off).
    pub fn set_backups(&mut self, keep: usize) {
        self.backups = keep;
    }

    /// Why the last save could not make its backup, once. The save itself
    /// went ahead.
    pub fn take_backup_problem(&mut self) -> Option<String> {
        self.backup_problem.take()
    }

    /// Makes today's backup of `path` if it is due. A backup that fails is
    /// reported, but never stops the save.
    fn backup(&mut self, path: &Path) {
        let result =
            ledgercraft_file::backup::backup_before_save(path, Date::today(), self.backups);
        if let Err(e) = result {
            self.backup_problem = Some(format!("The daily backup could not be made: {e}"));
        }
    }

    /// Forgets where the books came from: the next save asks where to save.
    /// Used when a backup is opened, so saving never overwrites the backup.
    pub fn detach(&mut self) {
        self.path = None;
        self.dirty = true;
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

    /// Whether the closing date password has been entered this session.
    pub fn closing_unlocked(&self) -> bool {
        self.closing_unlocked
    }

    /// Enters the closing date password: closed books can be changed until
    /// the company is closed. Without a password set there is nothing to
    /// unlock: the closing date itself has to move.
    pub fn unlock_closing(&mut self, password: &str) -> Result<(), EngineError> {
        let check = self
            .company
            .closing
            .as_ref()
            .and_then(|c| c.password.as_deref())
            .ok_or_else(|| EngineError::Closed("The closing date has no password.".into()))?;
        if ledgercraft_file::password_matches(password, check) {
            self.closing_unlocked = true;
            Ok(())
        } else {
            Err(EngineError::Closed(
                "That is not the closing date password.".into(),
            ))
        }
    }

    /// Refuses a change to transactions dated on or before the closing date,
    /// unless its password has been entered.
    fn check_closing(&self, cmd: &Command) -> Result<(), EngineError> {
        let Some(closing) = &self.company.closing else {
            return Ok(());
        };
        let has_password = closing.password.is_some();
        if has_password && self.closing_unlocked {
            return Ok(());
        }
        let txn_date = |id: &TxnId| self.company.txn(*id).map(|t| t.date);
        let dates: Vec<Date> = match cmd {
            Command::TxnPost { date, .. } => vec![*date],
            Command::TxnUpdate { id, date, .. } => {
                txn_date(id).into_iter().chain([*date]).collect()
            }
            Command::TxnVoid { id } => txn_date(id).into_iter().collect(),
            Command::BankImport { entries, .. } => entries
                .iter()
                .filter(|e| matches!(e.action, FeedAction::Add { .. }))
                .map(|e| e.line.date)
                .collect(),
            Command::CompanySetClosing { .. } if has_password => {
                return Err(EngineError::Closed(
                    "Enter the closing date password first (File > Closing Date…).".into(),
                ));
            }
            _ => Vec::new(),
        };
        if dates.iter().any(|d| *d <= closing.date) {
            let how = if has_password {
                "enter the closing date password (File > Closing Date…) to change them"
            } else {
                "move the closing date (File > Closing Date…) to change them"
            };
            return Err(EngineError::Closed(format!(
                "The books are closed through {}: transactions dated then or before are \
                 locked. To change one, {how}.",
                closing.date
            )));
        }
        Ok(())
    }

    /// Why a change that rewrote transactions is refused: it changed one
    /// dated on or before the closing date (compared with `before`), and the
    /// closing date password has not been entered.
    fn closed_changes(&self, before: &Company) -> Option<EngineError> {
        let closing = self.company.closing.as_ref()?;
        let has_password = closing.password.is_some();
        if has_password && self.closing_unlocked {
            return None;
        }
        let changed = before
            .transactions
            .iter()
            .filter(|t| t.date <= closing.date)
            .any(|t| self.company.txn(t.id).is_none_or(|now| now.body != t.body));
        changed.then(|| {
            let how = if has_password {
                "enter the closing date password (File > Closing Date…) first"
            } else {
                "move the closing date (File > Closing Date…) first"
            };
            EngineError::Closed(format!(
                "This merge would change transactions dated on or before the closing date \
                 ({}): {how}.",
                closing.date
            ))
        })
    }

    /// What an audit trail entry says a command was done to.
    fn audit_detail(&self, cmd: &Command) -> String {
        let describe = |date: Date, number: &str, body: &ledgercraft_ledger::TxnBody| {
            let mut parts = vec![date.to_string(), body.label().to_string()];
            if !number.trim().is_empty() {
                parts.push(number.trim().to_string());
            }
            if let Some(party) = body.party() {
                parts.push(self.company.party_name(party));
            }
            if let Some(total) = body.total() {
                parts.push(total.to_string());
            }
            parts.join(" · ")
        };
        match cmd {
            Command::TxnPost {
                date, number, body, ..
            } => describe(*date, number, body),
            Command::TxnUpdate {
                id,
                date,
                number,
                body,
                ..
            } => {
                let was = self
                    .company
                    .txn(*id)
                    .map(|t| describe(t.date, &t.number, &t.body))
                    .unwrap_or_default();
                format!("{} (was {was})", describe(*date, number, body))
            }
            Command::TxnVoid { id } => self
                .company
                .txn(*id)
                .map(|t| describe(t.date, &t.number, &t.body))
                .unwrap_or_default(),
            Command::CompanySetClosing { date: Some(d), .. } => format!("Closed through {d}"),
            _ => String::new(),
        }
    }

    /// Adds a line to the audit trail.
    fn audit(&mut self, action: String, detail: String) {
        self.company.audit.push(AuditEntry {
            at: now_text(),
            action,
            detail,
        });
    }

    /// Runs a command. On any error — including a bug that panics — the books
    /// are restored exactly as they were.
    pub fn execute(&mut self, cmd: Command) -> Result<Outcome, EngineError> {
        self.check_closing(&cmd)?;
        let id = cmd.id();
        let label = cmd.undo_label();
        let detail = self.audit_detail(&cmd);
        let merge = matches!(
            cmd,
            Command::AccountMerge { .. }
                | Command::CustomerMerge { .. }
                | Command::VendorMerge { .. }
                | Command::ItemMerge { .. }
        );
        let before = self.company.clone();
        let result = catch_unwind(AssertUnwindSafe(|| cmd.apply(&mut self.company)));
        // A merge rewrites the transactions that named what was merged away:
        // closed ones are as locked as for any other change.
        if merge && matches!(result, Ok(Ok(_))) {
            if let Some(e) = self.closed_changes(&before) {
                self.company = before;
                return Err(e);
            }
        }
        match result {
            Ok(Ok(created)) => {
                self.audit(label.clone(), detail);
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
    /// Undo and redo restore the books but not the audit trail, which only
    /// grows: they are recorded in it too.
    pub fn undo(&mut self) -> Option<String> {
        let (label, prev) = self.undo.pop()?;
        let log = std::mem::take(&mut self.company.audit);
        let current = std::mem::replace(&mut self.company, prev);
        self.company.audit = log;
        self.redo.push((label.clone(), current));
        self.audit(format!("Undo {label}"), String::new());
        self.dirty = true;
        Some(label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let (label, next) = self.redo.pop()?;
        let log = std::mem::take(&mut self.company.audit);
        let current = std::mem::replace(&mut self.company, next);
        self.company.audit = log;
        self.undo.push((label.clone(), current));
        self.audit(format!("Redo {label}"), String::new());
        self.dirty = true;
        Some(label)
    }

    pub fn save(&mut self) -> Result<(), EngineError> {
        let path = self.path.clone().ok_or(EngineError::NoPath)?;
        self.backup(&path);
        ledgercraft_file::save(&path, &self.company, self.protection.as_ref())?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), EngineError> {
        let path = ledgercraft_file::with_extension(path);
        self.backup(&path);
        ledgercraft_file::save(&path, &self.company, self.protection.as_ref())?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }
}

/// Now, as the audit trail writes it: `YYYY-MM-DD HH:MM UTC`.
fn now_text() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let time = secs % 86_400;
    format!(
        "{} {:02}:{:02} UTC",
        Date::today(),
        time / 3_600,
        time % 3_600 / 60
    )
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
    fn sales_tax_is_set_up_charged_and_undone_through_commands() {
        let mut s = Session::new(Company::with_default_accounts("Tax Co"));
        let state = s
            .execute_json(r#"{"command":"tax.create","name":"Utah State","agency":"Utah State Tax Commission","from":"2020-01-01","rate":"4.85"}"#)
            .unwrap()
            .created
            .unwrap();
        let id: u64 = state.trim_start_matches('X').parse().unwrap();
        s.execute_json(&format!(
            r#"{{"command":"tax.setRate","id":{id},"from":"2026-10-01","rate":5}}"#
        ))
        .unwrap();
        let group = s
            .execute_json(&format!(
                r#"{{"command":"taxGroup.create","name":"Statewide","components":[{id}]}}"#
            ))
            .unwrap()
            .created
            .unwrap();
        assert!(group.starts_with('G'));
        let c = s.company();
        let t = &c.tax_components[0];
        assert_eq!(t.rates.len(), 2);
        assert!(c.sales_tax_account().is_some());
        assert_eq!(s.undo().as_deref(), Some("New Tax Group Statewide"));
        assert!(s.company().tax_groups.is_empty());
        // Finding a customer's group imports it and assigns it in one step.
        let customer = s
            .execute_json(r#"{"command":"customer.create","contact":{"name":"Cafe"}}"#)
            .unwrap()
            .created
            .unwrap();
        let cid: u64 = customer.trim_start_matches('C').parse().unwrap();
        let find = format!(
            r#"{{"command":"tax.import","group":"Utah location 25-090","on":"2026-10-22","customer":{cid},
                "taxes":[{{"code":"49/45/49","name":"Utah State (file)","rates":[{{"from":"2019-04-01","rate":"4.85"}}]}}]}}"#
        );
        s.execute_json(&find).unwrap();
        let c = s.company();
        let group = c.customers[0]
            .tax
            .group
            .and_then(|g| c.tax_group(g))
            .map(|g| g.name.clone());
        assert_eq!(group.as_deref(), Some("Utah location 25-090"));
        assert_eq!(s.undo().as_deref(), Some("Find Tax Group"));
        assert_eq!(s.company().customers[0].tax.group, None);
        // A rate that is not a percentage is refused.
        let bad =
            format!(r#"{{"command":"tax.setRate","id":{id},"from":"2026-01-01","rate":"lots"}}"#);
        assert!(s.execute_json(&bad).is_err());
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
            .rev()
            .find(|t| !t.voided && matches!(t.body, ledgercraft_ledger::TxnBody::Invoice(_)))
            .unwrap()
            .clone();
        s.execute(crate::Command::TxnUpdate {
            id: t.id,
            date: t.date,
            number: t.number.clone(),
            memo: "Edited".into(),
            body: t.body.clone(),
            class: None,
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
        assert!(s.company().reconciliations_for(checking).is_empty());
    }

    /// Merging names on closed transactions is refused like editing them.
    #[test]
    fn merges_respect_the_closing_date() {
        let mut s = Session::new(crate::sample_company().unwrap());
        // Two vendors with January (closed) transactions.
        let c = s.company();
        let landlord = c.vendors.iter().find(|v| v.contact.name == "Riverside Properties").unwrap().id;
        let lumber = c.vendors.iter().find(|v| v.contact.name == "Canyon Lumber Supply").unwrap().id;
        let merge = crate::Command::VendorMerge {
            from: landlord,
            into: lumber,
        };
        assert!(matches!(s.execute(merge.clone()), Err(EngineError::Closed(_))));
        assert!(s.company().vendor(landlord).is_some(), "nothing changed");
        // Inactive Old Mill Finishes has no transactions: it merges fine.
        let old_mill = s.company().vendors.iter().find(|v| v.contact.name == "Old Mill Finishes").unwrap().id;
        s.execute(crate::Command::VendorMerge {
            from: old_mill,
            into: lumber,
        })
        .unwrap();
        assert!(s.company().vendor(old_mill).is_none());
        assert_eq!(s.undo().as_deref(), Some("Merge Vendors"));
        assert!(s.company().vendor(old_mill).is_some());
    }

    /// Closing the books through March locks the sample's January
    /// transactions until the closing date password is entered.
    #[test]
    fn reopening_the_books_has_to_be_asked_for() {
        let mut s = Session::new(crate::sample_company().unwrap());
        assert!(s.execute_json(r#"{"command":"company.setClosing"}"#).is_err());
        assert!(s.company().closing.is_some());
        s.execute_json(r#"{"command":"company.setClosing","date":null}"#)
            .unwrap();
        assert!(s.company().closing.is_none());
    }

    #[test]
    fn closed_books_are_locked() {
        use ledgercraft_core::Date;
        let mut s = Session::new(crate::sample_company().unwrap());
        let d = |t: &str| Date::parse(t).unwrap();
        s.execute(crate::Command::CompanySetClosing {
            date: Some(d("2026-03-31")),
            password: Some("year end".into()),
            keep_password: false,
        })
        .unwrap();
        let stored = s.company().closing.clone().unwrap();
        assert!(!stored.password.clone().unwrap().contains("year end"));
        let january = s
            .company()
            .transactions
            .iter()
            .find(|t| t.date < d("2026-02-01") && !t.voided)
            .unwrap()
            .clone();
        let edit = crate::Command::TxnUpdate {
            id: january.id,
            date: january.date,
            number: january.number.clone(),
            memo: "Changed".into(),
            body: january.body.clone(),
            class: None,
        };
        assert!(matches!(s.execute(edit.clone()), Err(EngineError::Closed(_))));
        assert!(matches!(
            s.execute(crate::Command::TxnVoid { id: january.id }),
            Err(EngineError::Closed(_))
        ));
        // Moving an edit out of the closed period does not get round it.
        let mut moved = edit.clone();
        if let crate::Command::TxnUpdate { date, .. } = &mut moved {
            *date = d("2026-05-01");
        }
        assert!(s.execute(moved).is_err());
        // Nor can the closing date change without the password.
        let reopen = crate::Command::CompanySetClosing {
            date: None,
            password: None,
            keep_password: false,
        };
        assert!(matches!(s.execute(reopen.clone()), Err(EngineError::Closed(_))));
        assert!(s.unlock_closing("wrong").is_err());
        s.unlock_closing("year end").unwrap();
        s.execute(edit).unwrap();
        s.execute(reopen).unwrap();
        assert!(s.company().closing.is_none());
    }

    #[test]
    fn every_change_goes_in_the_audit_trail() {
        let mut s = Session::new(crate::sample_company().unwrap());
        assert!(s.company().audit.is_empty());
        let t = s
            .company()
            .transactions
            .iter()
            .rev()
            .find(|t| !t.voided && matches!(t.body, ledgercraft_ledger::TxnBody::Check(_)))
            .unwrap()
            .clone();
        s.execute(crate::Command::TxnVoid { id: t.id }).unwrap();
        s.undo();
        s.redo();
        let log: Vec<&str> = s.company().audit.iter().map(|e| e.action.as_str()).collect();
        let void = format!("Void {}", t.id);
        let undo = format!("Undo {void}");
        let redo = format!("Redo {void}");
        assert_eq!(log, vec![void.as_str(), undo.as_str(), redo.as_str()]);
        let first = &s.company().audit[0];
        assert!(first.at.ends_with(" UTC") && first.date().is_some());
        assert!(first.detail.contains(&t.date.to_string()), "{}", first.detail);
    }

    #[test]
    fn saving_makes_the_daily_backup() {
        let dir = std::env::temp_dir().join(format!("lc-session-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Books.lcbooks");
        let mut s = Session::new(crate::sample_company().unwrap());
        s.set_backups(5);
        s.save_as(&path).unwrap();
        // The first save had no earlier file to copy.
        assert!(ledgercraft_file::backup::backups_of(&path).is_empty());
        s.save().unwrap();
        assert_eq!(ledgercraft_file::backup::backups_of(&path).len(), 1);
        assert!(s.take_backup_problem().is_none());
        // A backup opened and saved goes somewhere new, never over itself.
        let backup = ledgercraft_file::backup::backups_of(&path).remove(0);
        let mut restored = Session::open(&backup).unwrap();
        restored.detach();
        assert!(restored.path().is_none() && restored.is_dirty());
        assert!(matches!(restored.save(), Err(EngineError::NoPath)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sample_company_is_valid() {
        let c = crate::sample_company().unwrap();
        assert!(c.transactions.len() > 10);
    }

    /// The sample is meant to show off every feature: if one goes missing
    /// (or stops adding up), this says which.
    #[test]
    fn sample_company_shows_every_feature() {
        use ledgercraft_core::{Date, Money};
        use ledgercraft_ledger::{Party, TxnBody};
        let c = crate::sample_company().unwrap();
        let account = |name: &str| c.account_by_name(name).unwrap().id;
        let today = Date::parse("2026-10-22").unwrap();

        // Sales tax: taxed invoices, an exempt customer, a dated rate change,
        // a payment, and tax still owed.
        let taxed = c
            .transactions
            .iter()
            .filter(|t| matches!(&t.body, TxnBody::Invoice(i) if i.tax.is_some()))
            .count();
        assert!(taxed >= 8, "{taxed} taxed invoices");
        assert!(c.customers.iter().any(|x| x.tax.exempt));
        assert!(c.tax_components.iter().any(|t| t.rates.len() > 1));
        let owed = c.sales_tax_owed(today);
        assert!(owed.iter().any(|o| o.paid.is_positive() && o.owed.is_positive()));
        assert!(c.tax_review_due(today).is_none());

        // Every aging column has something in it, on both sides.
        let ar = aging_buckets(&c, today, true);
        assert!(ar.iter().all(|m| m.is_positive()), "A/R aging {ar:?}");
        let ap = aging_buckets(&c, today, false);
        assert!(ap[0].is_positive() && ap[1].is_positive(), "A/P aging {ap:?}");

        // Money waiting to be deposited, and reconciled card and savings
        // accounts, but Checking left for the Reconcile lesson.
        assert!(c.account_balance(account("Undeposited Funds"), None).is_positive());
        assert_eq!(c.reconciliations_for(account("Business Credit Card")).len(), 1);
        assert_eq!(c.reconciliations_for(account("Savings")).len(), 1);
        assert!(c.reconciliations_for(account("Checking")).is_empty());
        assert!(c.account_balance(account("Checking"), None).is_positive());

        // Sales paid on the spot, credits applied, refunded and still open.
        assert!(c.transactions.iter().any(|t| matches!(t.body, TxnBody::SalesReceipt(_))));
        assert!(c
            .transactions
            .iter()
            .any(|t| matches!(&t.body, TxnBody::CreditMemo(m) if !m.applied.is_empty())));
        assert!(c.transactions.iter().any(|t| matches!(t.body, TxnBody::Refund(_))));
        assert!(c
            .customers
            .iter()
            .any(|x| !c.open_credits_for(x.id, None).is_empty()));

        // Mistakes voided, things retired, rules for the bank download.
        assert!(c.transactions.iter().any(|t| t.voided && matches!(t.body, TxnBody::Check(_))));
        assert!(c.transactions.iter().any(|t| t.voided && matches!(t.body, TxnBody::Invoice(_))));
        assert!(c.customers.iter().any(|x| !x.active));
        assert!(c.vendors.iter().any(|x| !x.active));
        assert!(c.items.iter().any(|x| !x.active));
        assert!(!c.bank_rules.is_empty());
        assert!(!c.info.legal_name.is_empty() && !c.info.phone.is_empty());
        assert!(c.info.print.logo.is_some());
        assert!(c.closing.is_some());
        // A sub-account, and a duplicate customer to merge.
        assert!(c.accounts.iter().any(|a| a.parent.is_some()));
        assert!(c.customers.iter().any(|x| x.contact.name == "Maple St. Cafe"));

        // Classes: two in use (each on income), one inactive.
        assert!(c.classes.iter().any(|k| !k.active));
        for k in c.classes.iter().filter(|k| k.active) {
            assert!(
                c.transactions.iter().any(|t| t.class == Some(k.id)),
                "{} unused",
                k.name
            );
        }

        // A loan, partly repaid, and depreciation.
        let loan = c.account_balance(account("Loan Payable"), None);
        assert!(loan.is_positive() && loan < Money::from_dollars(12_000), "{loan}");
        assert!(!c
            .account_balance(account("Accumulated Depreciation"), None)
            .is_zero());
        assert!(c
            .transactions
            .iter()
            .any(|t| matches!(t.body.party(), Some(Party::Vendor(_)))
                && matches!(t.body, TxnBody::PayBill(_))));
    }

    /// Open invoices (or bills) on `as_of`, by how late: current, 1-30,
    /// 31-60, 61-90, over 90 days.
    fn aging_buckets(
        c: &ledgercraft_ledger::Company,
        as_of: ledgercraft_core::Date,
        receivable: bool,
    ) -> [ledgercraft_core::Money; 5] {
        use ledgercraft_ledger::TxnBody;
        let mut buckets = [ledgercraft_core::Money::ZERO; 5];
        for t in c.transactions.iter().filter(|t| !t.voided) {
            let due = match (&t.body, receivable) {
                (TxnBody::Invoice(i), true) => i.due_date,
                (TxnBody::Bill(b), false) => b.due_date,
                _ => continue,
            };
            let open = c.open_balance(t.id, Some(as_of));
            let i = match due.days_until(as_of) {
                i64::MIN..=0 => 0,
                1..=30 => 1,
                31..=60 => 2,
                61..=90 => 3,
                _ => 4,
            };
            buckets[i] = buckets[i].saturating_add(open);
        }
        buckets
    }
}
