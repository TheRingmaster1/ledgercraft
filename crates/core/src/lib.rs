//! L0 of LedgerCraft: the value types every other crate builds on.
//!
//! Money is stored as an exact integer number of cents and quantities as an
//! exact integer number of thousandths, so no floating-point rounding can ever
//! reach a ledger.

mod date;
mod error;
mod money;
mod qty;

pub use date::Date;
pub use error::{CoreError, CoreResult};
pub use money::{Money, MAX_CENTS};
pub use qty::Qty;

/// Errors from [`parse_scaled`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScaleError {
    Invalid,
    Overflow,
}

/// Parses a decimal such as `-1,234.5` into an integer scaled by `10^scale`.
///
/// Commas in the whole part are ignored. More fractional digits than `scale`
/// are accepted only when they are zeros, so nothing is silently rounded.
pub(crate) fn parse_scaled(input: &str, scale: u32) -> Result<i64, ScaleError> {
    let mut s = input.trim();
    let mut negative = false;
    if let Some(rest) = s.strip_prefix('-') {
        negative = true;
        s = rest.trim_start();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.trim_start();
    }
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    let mut value: i64 = 0;
    let mut seen_digit = false;
    for ch in whole.chars() {
        if ch == ',' {
            continue;
        }
        let d = ch.to_digit(10).ok_or(ScaleError::Invalid)?;
        seen_digit = true;
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(i64::from(d)))
            .ok_or(ScaleError::Overflow)?;
    }
    let mut used = 0u32;
    for ch in frac.chars() {
        let d = ch.to_digit(10).ok_or(ScaleError::Invalid)?;
        seen_digit = true;
        if used < scale {
            value = value
                .checked_mul(10)
                .and_then(|v| v.checked_add(i64::from(d)))
                .ok_or(ScaleError::Overflow)?;
            used += 1;
        } else if d != 0 {
            return Err(ScaleError::Invalid);
        }
    }
    while used < scale {
        value = value.checked_mul(10).ok_or(ScaleError::Overflow)?;
        used += 1;
    }
    if !seen_digit {
        return Err(ScaleError::Invalid);
    }
    Ok(if negative { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_parsing() {
        assert_eq!(parse_scaled("1,234.5", 2), Ok(123_450));
        assert_eq!(parse_scaled("-0.07", 2), Ok(-7));
        assert_eq!(parse_scaled(".5", 3), Ok(500));
        assert_eq!(parse_scaled("2.500", 2), Ok(250));
        assert_eq!(parse_scaled("2.505", 2), Err(ScaleError::Invalid));
        assert_eq!(parse_scaled("", 2), Err(ScaleError::Invalid));
        assert_eq!(parse_scaled("-", 2), Err(ScaleError::Invalid));
        assert_eq!(parse_scaled("1.2.3", 2), Err(ScaleError::Invalid));
        assert_eq!(
            parse_scaled("99999999999999999999", 2),
            Err(ScaleError::Overflow)
        );
    }
}
