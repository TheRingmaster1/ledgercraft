use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{parse_scaled, CoreError, CoreResult, Qty, ScaleError};

/// The largest magnitude a single amount may have, in cents (90 trillion).
/// Keeping amounts far below `i64::MAX` leaves headroom for sums.
pub const MAX_CENTS: i64 = 9_000_000_000_000_000;

/// An exact amount of money in the company's currency, stored as cents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Self = Self(0);

    pub const fn from_cents(cents: i64) -> Self {
        Self(cents)
    }

    pub fn from_dollars(dollars: i64) -> Self {
        Self(dollars.saturating_mul(100))
    }

    pub const fn cents(self) -> i64 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub fn abs(self) -> Self {
        Self(self.0.saturating_abs())
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    pub fn checked_neg(self) -> Option<Self> {
        self.0.checked_neg().map(Self)
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn saturating_neg(self) -> Self {
        Self(self.0.saturating_neg())
    }

    /// Sums amounts, returning `None` on overflow.
    pub fn checked_sum<I: IntoIterator<Item = Self>>(iter: I) -> Option<Self> {
        iter.into_iter().try_fold(Self::ZERO, Self::checked_add)
    }

    /// `self × qty`, rounded half away from zero to the cent.
    pub fn times(self, qty: Qty) -> Option<Self> {
        let product = i128::from(self.0) * i128::from(qty.milli());
        let quotient = product / 1000;
        let remainder = product % 1000;
        let rounded = if remainder.abs() * 2 >= 1000 {
            quotient + product.signum()
        } else {
            quotient
        };
        i64::try_from(rounded)
            .ok()
            .filter(|v| v.unsigned_abs() <= MAX_CENTS.unsigned_abs())
            .map(Self)
    }

    /// Parses `1234.5`, `1,234.50`, `$1,234.50`, `-$12`, `$-12` or `(12.00)`.
    pub fn parse(input: &str) -> CoreResult<Self> {
        let mut s = input.trim();
        let mut negative = false;
        if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
            negative = true;
            s = inner.trim();
        }
        let cleaned = s.replacen('$', "", 1);
        let cents = parse_scaled(&cleaned, 2).map_err(|e| match e {
            ScaleError::Invalid => CoreError::InvalidMoney(input.to_string()),
            ScaleError::Overflow => CoreError::Overflow,
        })?;
        if cents.unsigned_abs() > MAX_CENTS.unsigned_abs() {
            return Err(CoreError::Overflow);
        }
        Ok(Self(if negative { -cents } else { cents }))
    }

    /// Like [`Money::parse`] but an empty field means zero.
    pub fn parse_or_zero(input: &str) -> CoreResult<Self> {
        if input.trim().is_empty() {
            Ok(Self::ZERO)
        } else {
            Self::parse(input)
        }
    }

    /// `-1234.56`: no separators, for CSV and editable fields.
    pub fn to_plain_string(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        format!("{sign}{}.{:02}", abs / 100, abs % 100)
    }
}

/// `-1,234.56`
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        let digits = (abs / 100).to_string();
        let len = digits.len();
        let mut grouped = String::with_capacity(len + len / 3);
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (len - i).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        write!(f, "{sign}{grouped}.{:02}", abs % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_forms() {
        assert_eq!(Money::parse("1234.5").unwrap().cents(), 123_450);
        assert_eq!(Money::parse("$1,234.50").unwrap().cents(), 123_450);
        assert_eq!(Money::parse("-$12").unwrap().cents(), -1200);
        assert_eq!(Money::parse("$-12").unwrap().cents(), -1200);
        assert_eq!(Money::parse("(12.00)").unwrap().cents(), -1200);
        assert!(Money::parse("12.345").is_err());
        assert!(Money::parse("abc").is_err());
        assert!(Money::parse("").is_err());
        assert_eq!(Money::parse_or_zero("  ").unwrap(), Money::ZERO);
    }

    #[test]
    fn display() {
        assert_eq!(Money::from_cents(0).to_string(), "0.00");
        assert_eq!(Money::from_cents(5).to_string(), "0.05");
        assert_eq!(Money::from_cents(123_456_789).to_string(), "1,234,567.89");
        assert_eq!(Money::from_cents(-100_000).to_string(), "-1,000.00");
        assert_eq!(Money::from_cents(-100_050).to_plain_string(), "-1000.50");
        assert!(Money::from_cents(i64::MIN).to_plain_string().len() > 3);
    }

    #[test]
    fn multiply_rounds_half_away_from_zero() {
        let rate = Money::from_cents(333); // 3.33
        assert_eq!(rate.times(Qty::from_milli(1500)), Some(Money::from_cents(500))); // 4.995 → 5.00
        assert_eq!(
            Money::from_cents(-333).times(Qty::from_milli(1500)),
            Some(Money::from_cents(-500))
        );
        assert_eq!(Money::from_cents(MAX_CENTS).times(Qty::from_whole(2)), None);
    }

    #[test]
    fn sums_detect_overflow() {
        assert_eq!(
            Money::checked_sum([Money::from_cents(1), Money::from_cents(2)]),
            Some(Money::from_cents(3))
        );
        assert_eq!(Money::checked_sum([Money::from_cents(i64::MAX), Money::from_cents(1)]), None);
    }
}
