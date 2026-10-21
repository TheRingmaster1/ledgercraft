use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

const MIN_YEAR: i64 = 1;
const MAX_YEAR: i64 = 9999;

/// A calendar date (proleptic Gregorian), stored as days since 1970-01-01.
/// Serialized as `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Date {
    days: i32,
}

impl Date {
    pub fn from_ymd(year: i32, month: u32, day: u32) -> CoreResult<Self> {
        let invalid = || CoreError::InvalidDate(format!("{year:04}-{month:02}-{day:02}"));
        let y = i64::from(year);
        if !(MIN_YEAR..=MAX_YEAR).contains(&y) || !(1..=12).contains(&month) {
            return Err(invalid());
        }
        if day == 0 || day > days_in_month(year, month) {
            return Err(invalid());
        }
        let days = i32::try_from(days_from_civil(y, month, day)).map_err(|_| invalid())?;
        Ok(Self { days })
    }

    fn from_days_clamped(days: i64) -> Self {
        let lo = days_from_civil(MIN_YEAR, 1, 1);
        let hi = days_from_civil(MAX_YEAR, 12, 31);
        let clamped = days.clamp(lo, hi);
        Self {
            days: i32::try_from(clamped).unwrap_or(0),
        }
    }

    /// Today's date (UTC) from the system clock.
    pub fn today() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self::from_days_clamped(i64::try_from(secs / 86_400).unwrap_or(0))
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        let (y, m, d) = civil_from_days(i64::from(self.days));
        (i32::try_from(y).unwrap_or(1970), m, d)
    }

    pub fn year(self) -> i32 {
        self.ymd().0
    }

    pub fn month(self) -> u32 {
        self.ymd().1
    }

    pub fn day(self) -> u32 {
        self.ymd().2
    }

    /// Adds (or subtracts) days, clamped to years 1–9999.
    pub fn add_days(self, n: i64) -> Self {
        Self::from_days_clamped(i64::from(self.days).saturating_add(n))
    }

    /// Days from `self` to `later` (negative if `later` is earlier).
    pub fn days_until(self, later: Self) -> i64 {
        i64::from(later.days) - i64::from(self.days)
    }

    /// The first day of the fiscal year containing `self`, for a fiscal year
    /// starting on the first of `start_month` (1–12; out of range means January).
    pub fn fiscal_year_start(self, start_month: u32) -> Self {
        let start_month = if (1..=12).contains(&start_month) {
            start_month
        } else {
            1
        };
        let (y, m, _) = self.ymd();
        let year = if m >= start_month { y } else { y - 1 };
        Self::from_ymd(year, start_month, 1).unwrap_or(self)
    }

    pub fn first_of_month(self) -> Self {
        let (y, m, _) = self.ymd();
        Self::from_ymd(y, m, 1).unwrap_or(self)
    }

    /// Parses `YYYY-MM-DD` or US-style `M/D/YYYY`.
    pub fn parse(input: &str) -> CoreResult<Self> {
        let s = input.trim();
        let invalid = || CoreError::InvalidDate(input.to_string());
        let parts: Vec<&str> = if s.contains('-') {
            s.split('-').collect()
        } else {
            s.split('/').collect()
        };
        let [a, b, c] = parts.as_slice() else {
            return Err(invalid());
        };
        let num = |p: &str| p.trim().parse::<u32>().map_err(|_| invalid());
        let (y, m, d) = if s.contains('-') {
            (num(a)?, num(b)?, num(c)?)
        } else {
            (num(c)?, num(a)?, num(b)?)
        };
        let y = i32::try_from(y).map_err(|_| invalid())?;
        Self::from_ymd(y, m, d).map_err(|_| invalid())
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl TryFrom<String> for Date {
    type Error = CoreError;
    fn try_from(s: String) -> CoreResult<Self> {
        Self::parse(&s)
    }
}

impl From<Date> for String {
    fn from(d: Date) -> Self {
        d.to_string()
    }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

// The two conversions below are the well-known public-domain civil-calendar
// algorithms (H. Hinnant, "chrono-Compatible Low-Level Date Algorithms").

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = i64::from(m);
    let d = i64::from(d);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_and_round_trip() {
        let epoch = Date::from_ymd(1970, 1, 1).unwrap();
        assert_eq!(epoch.days, 0);
        for &(y, m, d) in &[
            (2000, 2, 29),
            (2026, 12, 31),
            (1, 1, 1),
            (9999, 12, 31),
            (1969, 12, 31),
        ] {
            assert_eq!(Date::from_ymd(y, m, d).unwrap().ymd(), (y, m, d));
        }
    }

    #[test]
    fn validation() {
        assert!(Date::from_ymd(2026, 2, 29).is_err());
        assert!(Date::from_ymd(2024, 2, 29).is_ok());
        assert!(Date::from_ymd(2026, 13, 1).is_err());
        assert!(Date::from_ymd(0, 1, 1).is_err());
    }

    #[test]
    fn parsing() {
        assert_eq!(Date::parse("2026-10-19").unwrap().to_string(), "2026-10-19");
        assert_eq!(Date::parse("10/19/2026").unwrap().to_string(), "2026-10-19");
        assert!(Date::parse("19.10.2026").is_err());
        assert!(Date::parse("2026-10").is_err());
    }

    #[test]
    fn arithmetic() {
        let d = Date::from_ymd(2026, 12, 31).unwrap();
        assert_eq!(d.add_days(1).to_string(), "2027-01-01");
        assert_eq!(Date::from_ymd(2026, 1, 1).unwrap().days_until(d), 364);
        assert_eq!(d.fiscal_year_start(1).to_string(), "2026-01-01");
        assert_eq!(
            Date::from_ymd(2026, 3, 5)
                .unwrap()
                .fiscal_year_start(7)
                .to_string(),
            "2025-07-01"
        );
    }

    #[test]
    fn serde_as_string() {
        let d = Date::from_ymd(2026, 4, 5).unwrap();
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "\"2026-04-05\"");
        let back: Date = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
        assert!(serde_json::from_str::<Date>("\"2026-02-30\"").is_err());
    }
}
