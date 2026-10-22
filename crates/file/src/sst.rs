//! Streamlined Sales Tax (SST) rate and boundary files.
//!
//! The 24 SST member states publish two files each quarter, free, at
//! streamlinedsalestax.org: a *boundary* file saying which taxing
//! jurisdictions (state, county, city, special districts) apply at each ZIP
//! or ZIP+4, and a *rate* file giving each jurisdiction's rate history. Both
//! are headerless CSV. This module finds the jurisdictions for a ZIP and
//! turns their rates into dated sales tax rates, ready to import.
//!
//! Boundary columns used (1-based): 1 record type (`Z` whole ZIP, `4` ZIP+4,
//! `A` street address), 2-3 effective dates, 18-21 ZIP and ZIP+4 range,
//! 22 the state's composite location code, 23 state FIPS, 25 county FIPS,
//! 26 place (city) FIPS, and from 30 on, special-district triplets of
//! (class, code, type). Rate columns: 1 state FIPS, 2 jurisdiction type
//! (`45` state, `00` county, `01` city, others districts), 3 jurisdiction
//! code, 4 the general rate as a fraction (0.0485 = 4.85%), 8-9 effective
//! dates. Checked against Utah's files and its published combined rates.

use std::collections::BTreeMap;
use std::io::BufRead;

use ledgercraft_ledger::{Date, DatedRate, TaxRate};

/// The jurisdiction types the boundary file's fixed columns stand for.
const STATE: &str = "45";
const COUNTY: &str = "00";
const CITY: &str = "01";

/// One taxing jurisdiction: its type and code in the rate file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Jurisdiction {
    pub kind: String,
    pub code: String,
}

/// A set of jurisdictions that applies somewhere in a ZIP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// Two-digit state FIPS code, e.g. `49` for Utah.
    pub state: String,
    /// The state's own location code, e.g. Utah's `25090` (25-090, Provo).
    pub location_code: String,
    pub jurisdictions: Vec<Jurisdiction>,
    /// How many boundary rows (address ranges) point here: a guide to how
    /// much of the ZIP it covers.
    pub rows: usize,
}

/// One row of a rate file: the rate, from a day, to a day (or open-ended).
type RatePeriod = (Date, Option<Date>, TaxRate);

/// Locations seen in a boundary file, by (state, location code,
/// jurisdictions), with how many rows point at each.
type Seen = BTreeMap<(String, String, Vec<Jurisdiction>), usize>;

/// A jurisdiction in the rate file: (type, code).
type JurisdictionKey = (String, String);

/// The rate file: each jurisdiction's rate history.
#[derive(Clone, Debug, Default)]
pub struct Rates {
    /// Each jurisdiction's rate periods, in file order.
    history: BTreeMap<JurisdictionKey, Vec<RatePeriod>>,
}

/// One tax to create or update: a jurisdiction with its dated rates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedTax {
    /// Identifies the jurisdiction across imports: `49/00/049`.
    pub code: String,
    pub name: String,
    pub agency: String,
    /// Oldest first; a gap in the file's history is a 0% rate.
    pub rates: Vec<DatedRate>,
    /// The rate in force on the import date.
    pub rate: TaxRate,
}

/// What importing a location creates: its taxes and a group of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub group_name: String,
    pub taxes: Vec<PlannedTax>,
}

impl Plan {
    /// The combined rate on the import date.
    pub fn total_rate(&self) -> TaxRate {
        TaxRate::from_millionths(self.taxes.iter().map(|t| t.rate.millionths()).sum())
    }
}

/// `20261001` as a date; the far-future "no end" dates as `None`.
fn file_date(text: &str) -> Result<Option<Date>, String> {
    let t = text.trim();
    let bad = || format!("{t:?} is not a date (YYYYMMDD)");
    if t.len() != 8 || !t.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    let year: i32 = t[..4].parse().map_err(|_| bad())?;
    if year >= 2999 {
        return Ok(None);
    }
    let month: u32 = t[4..6].parse().map_err(|_| bad())?;
    let day: u32 = t[6..].parse().map_err(|_| bad())?;
    Date::from_ymd(year, month, day)
        .map(Some)
        .map_err(|e| e.to_string())
}

fn field<'a>(row: &[&'a str], column: usize) -> &'a str {
    row.get(column.saturating_sub(1))
        .copied()
        .map_or("", str::trim)
}

/// Reads a rate file.
pub fn read_rates(text: &str) -> Result<Rates, String> {
    let mut rates = Rates::default();
    for (n, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Vec<&str> = line.split(',').collect();
        let at = |e: String| format!("rate file line {}: {e}", n + 1);
        let general = field(&row, 4);
        if general.is_empty() {
            continue;
        }
        let rate = TaxRate::from_fraction(general).map_err(at)?;
        let from = file_date(field(&row, 8))
            .map_err(at)?
            .ok_or_else(|| at("the start date is missing".into()))?;
        let to = file_date(field(&row, 9)).map_err(at)?;
        rates
            .history
            .entry((field(&row, 2).to_string(), field(&row, 3).to_string()))
            .or_default()
            .push((from, to, rate));
    }
    if rates.history.is_empty() {
        return Err("This is not an SST rate file: no rates were found.".into());
    }
    Ok(rates)
}

/// A ZIP as typed: `84601`, `84601-1234` or `846011234`.
pub fn parse_zip(text: &str) -> Result<(String, Option<String>), String> {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    let other = text
        .chars()
        .any(|c| !c.is_ascii_digit() && c != '-' && !c.is_whitespace());
    match digits.len() {
        5 if !other => Ok((digits, None)),
        9 if !other => Ok((digits[..5].to_string(), Some(digits[5..].to_string()))),
        _ => Err(format!(
            "{text:?} is not a ZIP or ZIP+4 (84601 or 84601-1234)"
        )),
    }
}

/// The ZIP+4 part as a number for range checks; files write `1` for `0001`.
fn plus4(text: &str) -> Option<u32> {
    text.trim().parse().ok()
}

/// Every set of jurisdictions in force at `zip` on `date`, most common first.
/// With a ZIP+4, ZIP+4 records that cover it win over whole-ZIP records.
pub fn locations(boundary: impl BufRead, zip: &str, date: Date) -> Result<Vec<Location>, String> {
    let (zip5, ext) = parse_zip(zip)?;
    let ext = ext.as_deref().and_then(plus4);
    let mut exact = Seen::new();
    let mut whole = Seen::new();
    let mut address_only = false;
    for (n, line) in boundary.lines().enumerate() {
        let line = line.map_err(|e| format!("boundary file line {}: {e}", n + 1))?;
        let line = line.trim_start_matches('\u{feff}');
        let row: Vec<&str> = line.split(',').collect();
        let kind = field(&row, 1).to_ascii_uppercase();
        let low = field(&row, 18);
        let high = field(&row, 20);
        if low.is_empty() || zip5.as_str() < low || zip5.as_str() > high {
            if kind == "A" && field(&row, 16).starts_with(zip5.as_str()) {
                address_only = true;
            }
            continue;
        }
        let at = |e: String| format!("boundary file line {}: {e}", n + 1);
        let from = file_date(field(&row, 2)).map_err(at)?;
        let to = file_date(field(&row, 3)).map_err(at)?;
        if from.is_some_and(|f| f > date) || to.is_some_and(|t| t < date) {
            continue;
        }
        let target = match kind.as_str() {
            "Z" => &mut whole,
            "4" => {
                if let Some(e) = ext {
                    let lo = (low, plus4(field(&row, 19)).unwrap_or(0));
                    let hi = (high, plus4(field(&row, 21)).unwrap_or(9999));
                    let me = (zip5.as_str(), e);
                    if me < lo || me > hi {
                        continue;
                    }
                }
                &mut exact
            }
            _ => {
                address_only = true;
                continue;
            }
        };
        let state = field(&row, 23).to_string();
        let mut juris = vec![Jurisdiction {
            kind: STATE.into(),
            code: state.clone(),
        }];
        for (kind, column) in [(COUNTY, 25), (CITY, 26)] {
            let code = field(&row, column);
            if !code.is_empty() {
                juris.push(Jurisdiction {
                    kind: kind.into(),
                    code: code.to_string(),
                });
            }
        }
        // Special districts: (class, code, type) from column 30. Type 45 is
        // the state again (some states list location ids that way): skip it.
        let mut column = 30;
        while column + 2 <= row.len() {
            let code = field(&row, column + 1);
            let kind = field(&row, column + 2);
            if !code.is_empty() && !kind.is_empty() && kind != STATE {
                juris.push(Jurisdiction {
                    kind: kind.to_string(),
                    code: code.to_string(),
                });
            }
            column += 3;
        }
        *target
            .entry((state, field(&row, 22).to_string(), juris))
            .or_default() += 1;
    }
    // A ZIP+4 covered by ZIP+4 records uses them; otherwise the whole-ZIP
    // records (and, for a bare ZIP, every record) are the choices.
    let found = if ext.is_some() && !exact.is_empty() {
        exact
    } else if ext.is_some() {
        whole
    } else {
        let mut all = whole;
        for (k, v) in exact {
            *all.entry(k).or_default() += v;
        }
        all
    };
    if found.is_empty() {
        return Err(if address_only {
            format!("This state's file locates {zip5} by street address, which LedgerCraft cannot read yet. Enter the rates by hand.")
        } else {
            format!("No tax jurisdictions were found for {zip} on {date} in this boundary file. Is it the right state's file?")
        });
    }
    let mut out: Vec<Location> = found
        .into_iter()
        .map(|((state, location_code, jurisdictions), rows)| Location {
            state,
            location_code,
            jurisdictions,
            rows,
        })
        .collect();
    out.sort_by(|a, b| {
        b.rows
            .cmp(&a.rows)
            .then_with(|| a.location_code.cmp(&b.location_code))
    });
    Ok(out)
}

/// The taxes and group to import for a location, with rates as of `date`.
pub fn plan(rates: &Rates, location: &Location, date: Date) -> Result<Plan, String> {
    let (state_abbr, state_name, agency) = state_info(&location.state);
    let mut taxes = Vec::new();
    for j in &location.jurisdictions {
        let Some(history) = rates.history.get(&(j.kind.clone(), j.code.clone())) else {
            return Err(format!(
                "The rate file has no rates for jurisdiction {} (type {}): is it the same quarter and state as the boundary file?",
                j.code, j.kind
            ));
        };
        let mut sorted = history.clone();
        sorted.sort_by_key(|(from, _, _)| *from);
        let mut dated: Vec<DatedRate> = Vec::new();
        for (i, (from, to, rate)) in sorted.iter().enumerate() {
            dated.retain(|d| d.from != *from);
            dated.push(DatedRate {
                from: *from,
                rate: *rate,
            });
            // A gap before the next rate (or after the last) charges nothing.
            if let Some(end) = to {
                let next_day = end.add_days(1);
                let next = sorted.get(i + 1).map(|(f, _, _)| *f);
                if next != Some(next_day) {
                    dated.push(DatedRate {
                        from: next_day,
                        rate: TaxRate::ZERO,
                    });
                }
            }
        }
        dated.sort_by_key(|d| d.from);
        let rate = dated
            .iter()
            .filter(|d| d.from <= date)
            .max_by_key(|d| d.from)
            .map_or(TaxRate::ZERO, |d| d.rate);
        let name = match j.kind.as_str() {
            STATE => format!("{state_name} State"),
            COUNTY => format!("{state_abbr} County {}", j.code),
            CITY => format!("{state_abbr} City {}", j.code),
            _ => format!("{state_abbr} District {}", j.code),
        };
        taxes.push(PlannedTax {
            code: format!("{}/{}/{}", location.state, j.kind, j.code),
            name,
            agency: agency.to_string(),
            rates: dated,
            rate,
        });
    }
    let group_name = if location.location_code.is_empty() {
        format!("{state_name} combined")
    } else {
        format!(
            "{state_name} location {}",
            pretty_location(&location.location_code)
        )
    };
    Ok(Plan { group_name, taxes })
}

/// Utah-style `25090` as `25-090`; other codes as they are.
fn pretty_location(code: &str) -> String {
    if code.len() == 5 && code.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}", &code[..2], &code[2..])
    } else {
        code.to_string()
    }
}

/// The SST member states by FIPS code: abbreviation, name and tax agency.
fn state_info(fips: &str) -> (&'static str, &'static str, &'static str) {
    match fips.trim_start_matches('0') {
        "5" => (
            "AR",
            "Arkansas",
            "Arkansas Department of Finance and Administration",
        ),
        "13" => ("GA", "Georgia", "Georgia Department of Revenue"),
        "18" => ("IN", "Indiana", "Indiana Department of Revenue"),
        "19" => ("IA", "Iowa", "Iowa Department of Revenue"),
        "20" => ("KS", "Kansas", "Kansas Department of Revenue"),
        "21" => ("KY", "Kentucky", "Kentucky Department of Revenue"),
        "26" => ("MI", "Michigan", "Michigan Department of Treasury"),
        "27" => ("MN", "Minnesota", "Minnesota Department of Revenue"),
        "31" => ("NE", "Nebraska", "Nebraska Department of Revenue"),
        "32" => ("NV", "Nevada", "Nevada Department of Taxation"),
        "34" => ("NJ", "New Jersey", "New Jersey Division of Taxation"),
        "37" => (
            "NC",
            "North Carolina",
            "North Carolina Department of Revenue",
        ),
        "38" => (
            "ND",
            "North Dakota",
            "North Dakota Office of State Tax Commissioner",
        ),
        "39" => ("OH", "Ohio", "Ohio Department of Taxation"),
        "40" => ("OK", "Oklahoma", "Oklahoma Tax Commission"),
        "44" => ("RI", "Rhode Island", "Rhode Island Division of Taxation"),
        "46" => ("SD", "South Dakota", "South Dakota Department of Revenue"),
        "47" => ("TN", "Tennessee", "Tennessee Department of Revenue"),
        "49" => ("UT", "Utah", "Utah State Tax Commission"),
        "50" => ("VT", "Vermont", "Vermont Department of Taxes"),
        "53" => ("WA", "Washington", "Washington Department of Revenue"),
        "54" => ("WV", "West Virginia", "West Virginia Tax Division"),
        "55" => ("WI", "Wisconsin", "Wisconsin Department of Revenue"),
        "56" => ("WY", "Wyoming", "Wyoming Department of Revenue"),
        _ => ("", "State", "State tax agency"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDARY: &str = include_str!("../testdata/utah-boundary-sample.csv");
    const RATES: &str = include_str!("../testdata/utah-rates-sample.csv");

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn rate(s: &str) -> TaxRate {
        TaxRate::parse(s).unwrap()
    }

    #[test]
    fn zips_are_read_with_or_without_the_plus_four() {
        assert_eq!(parse_zip("84601").unwrap(), ("84601".into(), None));
        assert_eq!(
            parse_zip(" 84601-1100 ").unwrap(),
            ("84601".into(), Some("1100".into()))
        );
        assert_eq!(parse_zip("846011100").unwrap().1, Some("1100".into()));
        assert!(parse_zip("8460").is_err());
        assert!(parse_zip("84601-11").is_err());
        assert!(parse_zip("UT 84601").is_err());
    }

    #[test]
    fn a_provo_zip_plus_four_is_745_percent_like_utahs_rate_list() {
        let rates = read_rates(RATES).unwrap();
        let found = locations(BOUNDARY.as_bytes(), "84601-1100", d("2026-10-01")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].location_code, "25090");
        let plan = plan(&rates, &found[0], d("2026-10-01")).unwrap();
        assert_eq!(plan.group_name, "Utah location 25-090");
        // Utah 4.85% + Utah County 2.5% + Provo 0.1%, as in Utah's
        // "Combined Sales and Use Tax Rates" for October 1, 2026.
        assert_eq!(plan.total_rate(), rate("7.45"));
        let names: Vec<&str> = plan.taxes.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Utah State", "UT County 049", "UT City 62470"]);
        assert!(plan
            .taxes
            .iter()
            .all(|t| t.agency == "Utah State Tax Commission"));
        assert_eq!(plan.taxes[2].code, "49/01/62470");
    }

    #[test]
    fn a_bare_zip_offers_each_location_in_it() {
        let found = locations(BOUNDARY.as_bytes(), "84601", d("2026-10-01")).unwrap();
        let codes: Vec<&str> = found.iter().map(|l| l.location_code.as_str()).collect();
        assert!(
            codes.contains(&"25090") && codes.contains(&"25000"),
            "{codes:?}"
        );
        // A ZIP+4 with no ZIP+4 record falls back to the whole-ZIP record:
        // unincorporated Utah County, 7.35%.
        let rates = read_rates(RATES).unwrap();
        let county = locations(BOUNDARY.as_bytes(), "84601-9998", d("2026-10-01")).unwrap();
        assert_eq!(county[0].location_code, "25000");
        assert_eq!(
            plan(&rates, &county[0], d("2026-10-01"))
                .unwrap()
                .total_rate(),
            rate("7.35")
        );
    }

    #[test]
    fn rate_history_keeps_its_dates_and_gaps() {
        let rates = read_rates(RATES).unwrap();
        let found = locations(BOUNDARY.as_bytes(), "84601-1100", d("2026-10-01")).unwrap();
        let p = plan(&rates, &found[0], d("2026-10-01")).unwrap();
        let provo = &p.taxes[2];
        // 0.25% to 2011-09-30, nothing until 0.1% from 2016-07-01.
        let at = |day: &str| {
            provo
                .rates
                .iter()
                .filter(|r| r.from <= d(day))
                .max_by_key(|r| r.from)
                .map(|r| r.rate)
        };
        assert_eq!(at("2010-06-01"), Some(rate("0.25")));
        assert_eq!(at("2014-01-01"), Some(TaxRate::ZERO));
        assert_eq!(at("2026-10-01"), Some(rate("0.1")));
        let state = &p.taxes[0];
        assert_eq!(state.rate, rate("4.85"));
        assert!(state.rates.windows(2).all(|w| w[0].from < w[1].from));
    }

    #[test]
    fn the_wrong_files_say_so() {
        assert!(read_rates("hello\n").is_err());
        assert!(locations(BOUNDARY.as_bytes(), "10001", d("2026-10-01")).is_err());
        let rates = read_rates("49,45,49,0.0485,0.0485,0.03,0.03,20190401,99991231\n").unwrap();
        let found = locations(BOUNDARY.as_bytes(), "84601-1100", d("2026-10-01")).unwrap();
        assert!(
            plan(&rates, &found[0], d("2026-10-01")).is_err(),
            "county and city missing"
        );
    }
}
