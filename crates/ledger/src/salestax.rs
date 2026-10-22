//! Sales tax.
//!
//! A tax *component* is one rate charged for one agency (a state, county,
//! city or district) with its rate history: each rate has the date it starts.
//! A tax *group* is the components charged together at a location; invoices
//! and customers choose a group. When an invoice is recorded, the tax it
//! charges is stored on it — component, rate, taxable amount and tax — so
//! later rate changes never rewrite history, and the liability report adds up
//! exactly what was charged.
//!
//! Tax is worked out per component on the invoice's taxable total and rounded
//! once, half a cent away from zero.

use std::fmt;

use ledgercraft_core::{Date, Money};
use serde::{Deserialize, Serialize};

use crate::{
    AccountId, AccountType, Company, CustomerId, Invoice, ItemId, LedgerError, LedgerResult,
    TaxComponentId, TaxGroupId,
};

/// A reminder that sales tax rates may have changed since they were checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaxReview {
    /// When the new quarter's rates were published (the 1st of March, June,
    /// September or December).
    pub published: Date,
    /// When they take effect: the start of the next calendar quarter.
    pub takes_effect: Date,
    /// When the rates were last checked, if ever.
    pub last_checked: Option<Date>,
}

/// The most recent day, on or before `today`, by which states publish the
/// next quarter's rate changes: Streamlined Sales Tax states may change rates
/// only at the start of a calendar quarter, and post them by the first day of
/// the month before (March 1 for April 1, and so on).
pub fn rate_publication_date(today: Date) -> Date {
    let (year, month, _) = today.ymd();
    let (year, month) = match month {
        12 => (year, 12),
        9..=11 => (year, 9),
        6..=8 => (year, 6),
        3..=5 => (year, 3),
        _ => (year - 1, 12),
    };
    Date::from_ymd(year, month, 1).unwrap_or(today)
}

/// The name of the liability account tax is owed in.
pub const SALES_TAX_ACCOUNT: &str = "Sales Tax Payable";

/// A tax rate, held exactly in millionths of the amount: 7.125% is 71,250.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(try_from = "RateText", into = "String")]
pub struct TaxRate(u32);

/// A rate as written in a file or command: "7.125", "7.125%" or 7.125.
#[derive(Deserialize)]
#[serde(untagged)]
enum RateText {
    Text(String),
    Number(f64),
}

impl TryFrom<RateText> for TaxRate {
    type Error = String;

    fn try_from(value: RateText) -> Result<Self, Self::Error> {
        match value {
            RateText::Text(s) => Self::parse(&s),
            RateText::Number(n) => Self::parse(&n.to_string()),
        }
    }
}

impl From<TaxRate> for String {
    fn from(rate: TaxRate) -> Self {
        rate.percent_text()
    }
}

/// Millionths in 100%.
const MILLION: u32 = 1_000_000;

impl TaxRate {
    pub const ZERO: Self = Self(0);

    /// Millionths of the amount: 7.125% is 71,250.
    pub const fn from_millionths(m: u32) -> Self {
        Self(m)
    }

    pub const fn millionths(self) -> u32 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// A percentage such as `7.125`, `7.125%` or `0.5`, to four decimal places,
    /// from 0% to 100%.
    pub fn parse(text: &str) -> Result<Self, String> {
        let t = text.trim().trim_end_matches('%').trim();
        let bad = || format!("{text:?} is not a rate: type a percentage such as 7.125");
        let (whole, frac) = t.split_once('.').unwrap_or((t, ""));
        if whole.is_empty() && frac.is_empty() {
            return Err(bad());
        }
        if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return Err(bad());
        }
        let frac = frac.trim_end_matches('0');
        if frac.len() > 4 {
            return Err(format!("{text:?} has more than four decimal places"));
        }
        let whole: u32 = if whole.is_empty() {
            0
        } else {
            whole.parse().map_err(|_| bad())?
        };
        let mut padded = frac.to_string();
        while padded.len() < 4 {
            padded.push('0');
        }
        let frac: u32 = padded.parse().map_err(|_| bad())?;
        let m = whole
            .checked_mul(10_000)
            .and_then(|w| w.checked_add(frac))
            .ok_or_else(bad)?;
        if m > MILLION {
            return Err(format!("{text:?} is more than 100%"));
        }
        Ok(Self(m))
    }

    /// A fraction of one, as rate files write it: `0.06875` is 6.875%.
    pub fn from_fraction(text: &str) -> Result<Self, String> {
        let t = text.trim();
        let bad = || format!("{text:?} is not a rate");
        let (whole, frac) = t.split_once('.').unwrap_or((t, ""));
        if !whole.chars().all(|c| c.is_ascii_digit())
            || !frac.chars().all(|c| c.is_ascii_digit())
            || t.is_empty()
        {
            return Err(bad());
        }
        let frac = frac.trim_end_matches('0');
        if frac.len() > 6 {
            return Err(format!("{text:?} is more precise than a millionth"));
        }
        let whole: u32 = if whole.is_empty() {
            0
        } else {
            whole.parse().map_err(|_| bad())?
        };
        let mut padded = frac.to_string();
        while padded.len() < 6 {
            padded.push('0');
        }
        let frac: u32 = padded.parse().map_err(|_| bad())?;
        let m = whole
            .checked_mul(MILLION)
            .and_then(|w| w.checked_add(frac))
            .ok_or_else(bad)?;
        if m > MILLION {
            return Err(format!("{text:?} is more than 100%"));
        }
        Ok(Self(m))
    }

    /// `7.125`: the percentage with no trailing zeros and no % sign.
    pub fn percent_text(self) -> String {
        let whole = self.0 / 10_000;
        let frac = self.0 % 10_000;
        if frac == 0 {
            whole.to_string()
        } else {
            let digits = format!("{frac:04}");
            format!("{whole}.{}", digits.trim_end_matches('0'))
        }
    }

    /// The tax on `amount`, rounded to the cent, half a cent away from zero.
    pub fn tax_on(self, amount: Money) -> Option<Money> {
        let product = i128::from(amount.cents()).checked_mul(i128::from(self.0))?;
        let half = i128::from(MILLION / 2);
        let rounded = if product >= 0 {
            (product + half) / i128::from(MILLION)
        } else {
            (product - half) / i128::from(MILLION)
        };
        i64::try_from(rounded).ok().map(Money::from_cents)
    }
}

/// `7.125%`
impl fmt::Display for TaxRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.percent_text())
    }
}

/// A rate and the day it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatedRate {
    pub from: Date,
    pub rate: TaxRate,
}

/// One rate charged for one agency, with its history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxComponent {
    pub id: TaxComponentId,
    /// e.g. "Utah State" or "Provo City".
    pub name: String,
    /// Who the tax is paid to, e.g. "Utah State Tax Commission".
    #[serde(default)]
    pub agency: String,
    /// The jurisdiction code from a state rate file, when imported.
    #[serde(default)]
    pub code: String,
    /// Oldest first.
    pub rates: Vec<DatedRate>,
    pub active: bool,
}

impl TaxComponent {
    /// The rate in force on `date`: the latest one starting on or before it.
    pub fn rate_on(&self, date: Date) -> Option<TaxRate> {
        self.rates
            .iter()
            .filter(|r| r.from <= date)
            .max_by_key(|r| r.from)
            .map(|r| r.rate)
    }
}

/// The components charged together at a location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxGroup {
    pub id: TaxGroupId,
    /// e.g. "Provo combined".
    pub name: String,
    pub components: Vec<TaxComponentId>,
    pub active: bool,
}

/// How a customer is taxed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomerTax {
    /// The group their invoices start with.
    pub group: Option<TaxGroupId>,
    /// Exempt customers (resellers, nonprofits) are not charged tax.
    pub exempt: bool,
    /// Their exemption certificate number or reason, printed on invoices.
    pub exemption: String,
}

/// One component's tax on an invoice, as charged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxLine {
    pub component: TaxComponentId,
    /// The component's name when the invoice was recorded.
    pub name: String,
    pub rate: TaxRate,
    /// The invoice's taxable amount.
    pub taxable: Money,
    pub amount: Money,
}

/// The tax an invoice charges, stored on it when recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceTax {
    pub group: TaxGroupId,
    /// The liability account the tax is owed in.
    pub account: AccountId,
    pub lines: Vec<TaxLine>,
}

impl InvoiceTax {
    pub fn total(&self) -> Option<Money> {
        Money::checked_sum(self.lines.iter().map(|l| l.amount))
    }
}

impl Company {
    pub fn tax_component(&self, id: TaxComponentId) -> Option<&TaxComponent> {
        self.tax_components.iter().find(|t| t.id == id)
    }

    pub fn tax_group(&self, id: TaxGroupId) -> Option<&TaxGroup> {
        self.tax_groups.iter().find(|g| g.id == id)
    }

    /// The account sales tax is owed in: the liability account named
    /// "Sales Tax Payable".
    pub fn sales_tax_account(&self) -> Option<AccountId> {
        self.accounts
            .iter()
            .find(|a| {
                a.name.trim().eq_ignore_ascii_case(SALES_TAX_ACCOUNT)
                    && matches!(
                        a.kind,
                        AccountType::OtherCurrentLiability | AccountType::LongTermLiability
                    )
            })
            .map(|a| a.id)
    }

    /// The Sales Tax Payable account, made if the chart has none.
    pub fn ensure_sales_tax_account(&mut self) -> LedgerResult<AccountId> {
        match self.sales_tax_account() {
            Some(id) => Ok(id),
            None => self.add_account(
                "",
                SALES_TAX_ACCOUNT,
                AccountType::OtherCurrentLiability,
                "Sales tax collected, owed to the tax agencies",
            ),
        }
    }

    fn check_component_name(
        &self,
        name: &str,
        except: Option<TaxComponentId>,
    ) -> LedgerResult<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LedgerError::Required("A tax name"));
        }
        if self
            .tax_components
            .iter()
            .any(|t| Some(t.id) != except && t.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!(
                "A sales tax named {name:?}"
            )));
        }
        Ok(name.to_string())
    }

    /// Adds a tax component with its first rate, and makes the Sales Tax
    /// Payable account if there is none.
    pub fn add_tax_component(
        &mut self,
        name: &str,
        agency: &str,
        code: &str,
        from: Date,
        rate: TaxRate,
    ) -> LedgerResult<TaxComponentId> {
        let name = self.check_component_name(name, None)?;
        self.ensure_sales_tax_account()?;
        let id = TaxComponentId(self.alloc());
        self.tax_components.push(TaxComponent {
            id,
            name,
            agency: agency.trim().to_string(),
            code: code.trim().to_string(),
            rates: vec![DatedRate { from, rate }],
            active: true,
        });
        Ok(id)
    }

    pub fn update_tax_component(
        &mut self,
        id: TaxComponentId,
        name: &str,
        agency: &str,
        code: &str,
    ) -> LedgerResult<()> {
        let name = self.check_component_name(name, Some(id))?;
        let t = self.tax_component_mut(id)?;
        t.name = name;
        t.agency = agency.trim().to_string();
        t.code = code.trim().to_string();
        Ok(())
    }

    fn tax_component_mut(&mut self, id: TaxComponentId) -> LedgerResult<&mut TaxComponent> {
        self.tax_components
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Sales tax {id}")))
    }

    /// Sets the rate starting on `from`, replacing one that starts that day.
    /// Invoices already recorded keep the rate they were charged.
    pub fn set_tax_rate(
        &mut self,
        id: TaxComponentId,
        from: Date,
        rate: TaxRate,
    ) -> LedgerResult<()> {
        let t = self.tax_component_mut(id)?;
        match t.rates.iter_mut().find(|r| r.from == from) {
            Some(r) => r.rate = rate,
            None => {
                t.rates.push(DatedRate { from, rate });
                t.rates.sort_by_key(|r| r.from);
            }
        }
        Ok(())
    }

    /// Removes the rate starting on `from`; a component keeps at least one.
    pub fn remove_tax_rate(&mut self, id: TaxComponentId, from: Date) -> LedgerResult<()> {
        let t = self.tax_component_mut(id)?;
        if t.rates.len() <= 1 {
            return Err(LedgerError::Invalid(format!(
                "{} needs at least one rate; change it instead",
                t.name
            )));
        }
        let before = t.rates.len();
        t.rates.retain(|r| r.from != from);
        if t.rates.len() == before {
            return Err(LedgerError::NotFound(format!("A rate starting {from}")));
        }
        Ok(())
    }

    pub fn set_tax_component_active(
        &mut self,
        id: TaxComponentId,
        active: bool,
    ) -> LedgerResult<()> {
        self.tax_component_mut(id)?.active = active;
        Ok(())
    }

    fn check_group(
        &self,
        name: &str,
        components: &[TaxComponentId],
        except: Option<TaxGroupId>,
    ) -> LedgerResult<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LedgerError::Required("A tax group name"));
        }
        if self
            .tax_groups
            .iter()
            .any(|g| Some(g.id) != except && g.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!(
                "A tax group named {name:?}"
            )));
        }
        if components.is_empty() {
            return Err(LedgerError::Required("At least one sales tax in the group"));
        }
        for (i, c) in components.iter().enumerate() {
            if self.tax_component(*c).is_none() {
                return Err(LedgerError::NotFound(format!("Sales tax {c}")));
            }
            if components[..i].contains(c) {
                return Err(LedgerError::Invalid(format!(
                    "Sales tax {c} is in the group twice"
                )));
            }
        }
        Ok(name.to_string())
    }

    pub fn add_tax_group(
        &mut self,
        name: &str,
        components: Vec<TaxComponentId>,
    ) -> LedgerResult<TaxGroupId> {
        let name = self.check_group(name, &components, None)?;
        let id = TaxGroupId(self.alloc());
        self.tax_groups.push(TaxGroup {
            id,
            name,
            components,
            active: true,
        });
        Ok(id)
    }

    pub fn update_tax_group(
        &mut self,
        id: TaxGroupId,
        name: &str,
        components: Vec<TaxComponentId>,
    ) -> LedgerResult<()> {
        let name = self.check_group(name, &components, Some(id))?;
        let g = self.tax_group_mut(id)?;
        g.name = name;
        g.components = components;
        Ok(())
    }

    fn tax_group_mut(&mut self, id: TaxGroupId) -> LedgerResult<&mut TaxGroup> {
        self.tax_groups
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Tax group {id}")))
    }

    pub fn set_tax_group_active(&mut self, id: TaxGroupId, active: bool) -> LedgerResult<()> {
        self.tax_group_mut(id)?.active = active;
        Ok(())
    }

    /// Sets how a customer is taxed.
    pub fn set_customer_tax(&mut self, id: CustomerId, tax: CustomerTax) -> LedgerResult<()> {
        if let Some(g) = tax.group {
            if self.tax_group(g).is_none() {
                return Err(LedgerError::NotFound(format!("Tax group {g}")));
            }
        }
        let c = self
            .customers
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Customer {id}")))?;
        c.tax = CustomerTax {
            group: tax.group,
            exempt: tax.exempt,
            exemption: tax.exemption.trim().to_string(),
        };
        Ok(())
    }

    /// Whether to remind the user to check their rates: they charge sales tax,
    /// and a new quarter's rates have been published since they last checked.
    pub fn tax_review_due(&self, today: Date) -> Option<TaxReview> {
        if !self.tax_components.iter().any(|t| t.active) {
            return None;
        }
        let published = rate_publication_date(today);
        if self.tax_rates_checked.is_some_and(|d| d >= published) {
            return None;
        }
        let (year, month, _) = published.ymd();
        let takes_effect = if month == 12 {
            Date::from_ymd(year + 1, 1, 1)
        } else {
            Date::from_ymd(year, month + 1, 1)
        }
        .unwrap_or(published);
        Some(TaxReview {
            published,
            takes_effect,
            last_checked: self.tax_rates_checked,
        })
    }

    /// Records that the rates were checked (or changed) on `on`.
    pub fn mark_tax_rates_checked(&mut self, on: Date) {
        if self.tax_rates_checked.is_none_or(|d| d < on) {
            self.tax_rates_checked = Some(on);
        }
    }

    /// Marks an item taxable or not. Invoices already recorded keep the tax
    /// they charged.
    pub fn set_item_taxable(&mut self, id: ItemId, taxable: bool) -> LedgerResult<()> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Item {id}")))?;
        item.spec.taxable = taxable;
        Ok(())
    }

    /// The tax a group charges on `taxable` on `date`, at the rates in force
    /// that day, ready to store on an invoice.
    pub fn invoice_tax(
        &self,
        group: TaxGroupId,
        date: Date,
        taxable: Money,
    ) -> LedgerResult<InvoiceTax> {
        let g = self
            .tax_group(group)
            .ok_or_else(|| LedgerError::NotFound(format!("Tax group {group}")))?;
        let account = self
            .sales_tax_account()
            .ok_or(LedgerError::Required("A Sales Tax Payable account"))?;
        let mut lines = Vec::new();
        for id in &g.components {
            let t = self
                .tax_component(*id)
                .ok_or_else(|| LedgerError::NotFound(format!("Sales tax {id}")))?;
            let rate = t.rate_on(date).ok_or_else(|| {
                LedgerError::Invalid(format!("{} has no rate in force on {date}", t.name))
            })?;
            let amount = rate.tax_on(taxable).ok_or(LedgerError::Overflow)?;
            lines.push(TaxLine {
                component: t.id,
                name: t.name.clone(),
                rate,
                taxable,
                amount,
            });
        }
        Ok(InvoiceTax {
            group,
            account,
            lines,
        })
    }

    /// Checks the tax stored on an invoice adds up: each line taxes the
    /// invoice's taxable lines at its own rate.
    pub(crate) fn validate_invoice_tax(&self, inv: &Invoice) -> LedgerResult<()> {
        let Some(tax) = &inv.tax else {
            return Ok(());
        };
        if self.tax_group(tax.group).is_none() {
            return Err(LedgerError::NotFound(format!("Tax group {}", tax.group)));
        }
        self.require_kind(
            tax.account,
            |k| {
                matches!(
                    k,
                    AccountType::OtherCurrentLiability | AccountType::LongTermLiability
                )
            },
            "a liability account for sales tax",
        )?;
        let taxable = inv.taxable_amount().ok_or(LedgerError::Overflow)?;
        for l in &tax.lines {
            if self.tax_component(l.component).is_none() {
                return Err(LedgerError::NotFound(format!("Sales tax {}", l.component)));
            }
            if l.taxable != taxable {
                return Err(LedgerError::Invalid(format!(
                    "{}: the taxable amount {} does not match the taxable lines ({taxable})",
                    l.name, l.taxable
                )));
            }
            if l.rate.tax_on(l.taxable) != Some(l.amount) {
                return Err(LedgerError::Invalid(format!(
                    "{}: {} of {} is not {}",
                    l.name, l.rate, l.taxable, l.amount
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Contact, Qty, SalesLine, TxnBody};

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn rate(s: &str) -> TaxRate {
        TaxRate::parse(s).unwrap()
    }

    #[test]
    fn rates_parse_and_print_exactly() {
        assert_eq!(rate("7.125").millionths(), 71_250);
        assert_eq!(rate("7.125%"), rate(" 7.1250 "));
        assert_eq!(rate("0.5").to_string(), "0.5%");
        assert_eq!(rate("6").to_string(), "6%");
        assert_eq!(rate(".25").millionths(), 2_500);
        assert_eq!(TaxRate::from_fraction("0.06875").unwrap(), rate("6.875"));
        assert_eq!(TaxRate::from_fraction("0.0050").unwrap(), rate("0.5"));
        for bad in ["", "abc", "7.12345", "101", "-1", "1.2.3"] {
            assert!(TaxRate::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn rates_round_half_a_cent_away_from_zero() {
        // 7.25% of $10.10 is 73.225 cents: 73. Of $10.30, 74.675: 75.
        assert_eq!(
            rate("7.25").tax_on(Money::from_cents(1010)),
            Some(Money::from_cents(73))
        );
        assert_eq!(
            rate("7.25").tax_on(Money::from_cents(1030)),
            Some(Money::from_cents(75))
        );
        // 5% of $0.10 is half a cent: rounds up; of -$0.10, down.
        assert_eq!(
            rate("5").tax_on(Money::from_cents(10)),
            Some(Money::from_cents(1))
        );
        assert_eq!(
            rate("5").tax_on(Money::from_cents(-10)),
            Some(Money::from_cents(-1))
        );
    }

    #[test]
    fn rates_serialise_as_percentages() {
        let json = serde_json::to_string(&rate("7.125")).unwrap();
        assert_eq!(json, "\"7.125\"");
        let back: TaxRate = serde_json::from_str("7.125").unwrap();
        assert_eq!(back, rate("7.125"));
        let text: TaxRate = serde_json::from_str("\"0.25%\"").unwrap();
        assert_eq!(text, rate("0.25"));
    }

    fn books() -> (Company, TaxGroupId, TaxComponentId) {
        let mut c = Company::with_default_accounts("Tax Co");
        let state = c
            .add_tax_component(
                "Utah State",
                "Utah State Tax Commission",
                "",
                d("2020-01-01"),
                rate("4.85"),
            )
            .unwrap();
        let city = c
            .add_tax_component(
                "Provo City",
                "Utah State Tax Commission",
                "",
                d("2020-01-01"),
                rate("2.4"),
            )
            .unwrap();
        c.set_tax_rate(city, d("2026-10-01"), rate("2.6")).unwrap();
        let group = c
            .add_tax_group("Provo combined", vec![state, city])
            .unwrap();
        (c, group, city)
    }

    #[test]
    fn the_rate_in_force_follows_the_date() {
        let (c, _, city) = books();
        let t = c.tax_component(city).unwrap();
        assert_eq!(t.rate_on(d("2019-12-31")), None);
        assert_eq!(t.rate_on(d("2026-09-30")), Some(rate("2.4")));
        assert_eq!(t.rate_on(d("2026-10-01")), Some(rate("2.6")));
    }

    #[test]
    fn invoices_store_their_tax_and_post_it_to_sales_tax_payable() {
        let (mut c, group, city) = books();
        let customer = c
            .add_customer(Contact {
                name: "Cafe".into(),
                ..Contact::default()
            })
            .unwrap();
        let sales = c.account_by_name("Sales").unwrap().id;
        let line = |amount: i64, taxable: bool| SalesLine {
            item: None,
            description: String::new(),
            qty: Qty::ONE,
            rate: Money::from_dollars(amount),
            account: sales,
            taxable,
        };
        let date = d("2026-09-15");
        let tax = c
            .invoice_tax(group, date, Money::from_dollars(100))
            .unwrap();
        let invoice = Invoice {
            customer,
            due_date: date,
            ar_account: c.first_account_of(AccountType::AccountsReceivable).unwrap(),
            lines: vec![line(100, true), line(50, false)],
            tax: Some(tax),
        };
        // $4.85 + $2.40 on the $100 taxable; the $50 line is not taxed.
        assert_eq!(invoice.total(), Some(Money::from_cents(15_725)));
        let id = c
            .post(date, "1", "", TxnBody::Invoice(invoice.clone()))
            .unwrap();
        let payable = c.sales_tax_account().unwrap();
        assert_eq!(c.account_balance(payable, None), Money::from_cents(725));

        // A later rate change leaves the recorded invoice alone.
        c.set_tax_rate(city, d("2026-09-01"), rate("3")).unwrap();
        assert_eq!(c.account_balance(payable, None), Money::from_cents(725));
        assert_eq!(
            c.txn(id).and_then(|t| t.body.total()),
            Some(Money::from_cents(15_725))
        );

        // Tax that does not add up is refused.
        let mut wrong = invoice;
        if let Some(t) = wrong.tax.as_mut() {
            t.lines[0].amount = Money::from_cents(1);
        }
        assert!(c.post(date, "2", "", TxnBody::Invoice(wrong)).is_err());
    }

    #[test]
    fn groups_need_real_components_and_unique_names() {
        let (mut c, _, city) = books();
        assert!(c.add_tax_group("Provo combined", vec![city]).is_err());
        assert!(c.add_tax_group("Empty", vec![]).is_err());
        assert!(c.add_tax_group("Twice", vec![city, city]).is_err());
        assert!(c
            .add_tax_group("Ghost", vec![TaxComponentId(9_999)])
            .is_err());
        assert!(c.add_tax_group("City only", vec![city]).is_ok());
    }

    #[test]
    fn a_component_keeps_at_least_one_rate() {
        let (mut c, _, city) = books();
        c.remove_tax_rate(city, d("2026-10-01")).unwrap();
        assert!(c.remove_tax_rate(city, d("2020-01-01")).is_err());
    }

    #[test]
    fn rates_are_published_before_each_quarter() {
        assert_eq!(rate_publication_date(d("2026-10-22")), d("2026-09-01"));
        assert_eq!(rate_publication_date(d("2026-12-01")), d("2026-12-01"));
        assert_eq!(rate_publication_date(d("2027-02-28")), d("2026-12-01"));
        assert_eq!(rate_publication_date(d("2026-03-01")), d("2026-03-01"));
    }

    #[test]
    fn a_review_is_due_after_new_rates_are_published() {
        let (mut c, _, _) = books();
        let review = c.tax_review_due(d("2026-10-22")).unwrap();
        assert_eq!(review.published, d("2026-09-01"));
        assert_eq!(review.takes_effect, d("2026-10-01"));
        assert_eq!(review.last_checked, None);
        c.mark_tax_rates_checked(d("2026-09-05"));
        assert_eq!(c.tax_review_due(d("2026-10-22")), None);
        // Due again once December's rates are out, for January 1.
        let next = c.tax_review_due(d("2026-12-02")).unwrap();
        assert_eq!(next.takes_effect, d("2027-01-01"));
        // An older check never replaces a newer one.
        c.mark_tax_rates_checked(d("2026-01-01"));
        assert_eq!(c.tax_rates_checked, Some(d("2026-09-05")));
        // Without sales tax there is nothing to review.
        assert_eq!(Company::new("No Tax").tax_review_due(d("2026-10-22")), None);
    }

    #[test]
    fn the_sales_tax_account_is_made_when_missing() {
        let mut c = Company::new("Bare Co");
        assert_eq!(c.sales_tax_account(), None);
        c.add_tax_component("State", "", "", d("2026-01-01"), rate("5"))
            .unwrap();
        assert!(c.sales_tax_account().is_some());
    }
}
