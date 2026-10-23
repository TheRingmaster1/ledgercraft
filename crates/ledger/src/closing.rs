//! Closing the books, and the record of every change made to them.

use ledgercraft_core::Date;
use serde::{Deserialize, Serialize};

/// Books closed through `date`: transactions dated then or before cannot be
/// added, changed or voided unless the closing date password is entered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Closing {
    pub date: Date,
    /// A one-way check of the closing date password, if one is set (the
    /// password itself is never stored).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// One change to the books, for the Audit Trail report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When, as `YYYY-MM-DD HH:MM UTC`.
    pub at: String,
    /// What was done: "Invoice", "Edit Bill", "Void T42", "Undo Check", …
    pub action: String,
    /// What it was done to: date, number, name and amount, when it was a
    /// transaction.
    #[serde(default)]
    pub detail: String,
}

impl AuditEntry {
    /// The date part of `at`, if it has one.
    pub fn date(&self) -> Option<Date> {
        Date::parse(self.at.get(..10)?).ok()
    }
}
