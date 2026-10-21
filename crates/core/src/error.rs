use std::fmt;

/// Errors from parsing or computing core values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    InvalidMoney(String),
    InvalidQty(String),
    InvalidDate(String),
    Overflow,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMoney(s) => write!(f, "{s:?} is not a valid amount"),
            Self::InvalidQty(s) => write!(f, "{s:?} is not a valid quantity"),
            Self::InvalidDate(s) => {
                write!(
                    f,
                    "{s:?} is not a valid date (use YYYY-MM-DD or MM/DD/YYYY)"
                )
            }
            Self::Overflow => f.write_str("the amount is too large"),
        }
    }
}

impl std::error::Error for CoreError {}

pub type CoreResult<T> = Result<T, CoreError>;
