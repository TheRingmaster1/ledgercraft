use std::fmt;

use ledgercraft_core::{CoreError, Money};

use crate::TxnId;

/// Why the ledger refused a change. Every message is written for the person
/// keeping the books, not for a programmer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    Core(CoreError),
    NotFound(String),
    Inactive(String),
    Duplicate(String),
    Required(&'static str),
    Unbalanced {
        debits: Money,
        credits: Money,
    },
    WrongAccountType {
        account: String,
        expected: &'static str,
    },
    OverApplied {
        txn: TxnId,
        open: Money,
        applied: Money,
    },
    NotPositive(&'static str),
    InUse(String),
    Invalid(String),
    Overflow,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(e) => fmt::Display::fmt(e, f),
            Self::NotFound(what) => write!(f, "{what} was not found"),
            Self::Inactive(what) => write!(f, "{what} is inactive"),
            Self::Duplicate(what) => write!(f, "{what} already exists"),
            Self::Required(what) => write!(f, "{what} is required"),
            Self::Unbalanced { debits, credits } => {
                write!(f, "debits ({debits}) and credits ({credits}) must be equal")
            }
            Self::WrongAccountType { account, expected } => {
                write!(f, "{account} must be {expected}")
            }
            Self::OverApplied { txn, open, applied } => {
                write!(f, "{applied} is more than the {open} still open on {txn}")
            }
            Self::NotPositive(what) => write!(f, "{what} must be greater than zero"),
            Self::InUse(what) => write!(f, "{what}"),
            Self::Invalid(what) => write!(f, "{what}"),
            Self::Overflow => f.write_str("the amounts are too large"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<CoreError> for LedgerError {
    fn from(e: CoreError) -> Self {
        Self::Core(e)
    }
}

pub type LedgerResult<T> = Result<T, LedgerError>;
