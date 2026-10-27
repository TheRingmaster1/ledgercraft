//! Recurring transactions: a transaction saved with a schedule,
//! such as the monthly rent check or a retainer invoice. Each one either
//! reminds you when it is due (you open it, check it and save it) or is
//! entered automatically when the company is opened on or after its date.
//!
//! Occurrences are counted from `start`, so a monthly one on the 31st stays
//! on the last day of short months instead of drifting to the 28th.

use ledgercraft_core::{Date, Money};
use serde::{Deserialize, Serialize};

use crate::{Company, LedgerError, LedgerResult, RecurringId, TxnBody, TxnClassId, TxnId};

/// How often a recurring transaction comes round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl Frequency {
    pub const ALL: [Self; 5] = [
        Self::Daily,
        Self::Weekly,
        Self::Monthly,
        Self::Quarterly,
        Self::Yearly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
            Self::Quarterly => "Quarterly",
            Self::Yearly => "Yearly",
        }
    }

    /// The unit for "every N …".
    pub fn unit(self, n: u32) -> &'static str {
        match (self, n == 1) {
            (Self::Daily, true) => "day",
            (Self::Daily, false) => "days",
            (Self::Weekly, true) => "week",
            (Self::Weekly, false) => "weeks",
            (Self::Monthly, true) => "month",
            (Self::Monthly, false) => "months",
            (Self::Quarterly, true) => "quarter",
            (Self::Quarterly, false) => "quarters",
            (Self::Yearly, true) => "year",
            (Self::Yearly, false) => "years",
        }
    }
}

/// What happens when one is due.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecurringMode {
    /// Listed as due; you enter it yourself.
    Remind,
    /// Entered for you when the company is opened on or after its date.
    Automatic,
}

/// The schedule and settings of a recurring transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurringSpec {
    pub name: String,
    pub frequency: Frequency,
    /// Every this many days, weeks, months… (at least 1).
    pub every: u32,
    /// When the next one is due.
    pub next: Date,
    /// None after this date; `None` for no end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Date>,
    pub mode: RecurringMode,
}

/// The transaction a recurring transaction enters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxnTemplate {
    /// The date it was made recurring with: due dates keep their distance from it.
    pub date: Date,
    #[serde(default)]
    pub memo: String,
    pub body: TxnBody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<TxnClassId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recurring {
    pub id: RecurringId,
    pub name: String,
    pub frequency: Frequency,
    pub every: u32,
    /// The first occurrence; the n-th is counted from it.
    pub start: Date,
    /// Occurrences entered or skipped so far.
    #[serde(default)]
    pub done: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Date>,
    pub mode: RecurringMode,
    #[serde(default = "yes")]
    pub active: bool,
    pub template: TxnTemplate,
}

fn yes() -> bool {
    true
}

/// The most occurrences entered automatically for one recurring
/// transaction at a time (a daily one left for years stops here).
pub const MAX_CATCH_UP: u32 = 400;

impl Recurring {
    /// The n-th occurrence (0 is `start`).
    pub fn occurrence(&self, n: u32) -> Date {
        let steps = i64::from(n).saturating_mul(i64::from(self.every.max(1)));
        match self.frequency {
            Frequency::Daily => self.start.add_days(steps),
            Frequency::Weekly => self.start.add_days(steps.saturating_mul(7)),
            Frequency::Monthly => self.start.add_months(steps),
            Frequency::Quarterly => self.start.add_months(steps.saturating_mul(3)),
            Frequency::Yearly => self.start.add_months(steps.saturating_mul(12)),
        }
    }

    /// When the next one is due; `None` once past its end date.
    pub fn next(&self) -> Option<Date> {
        let next = self.occurrence(self.done);
        self.end.is_none_or(|end| next <= end).then_some(next)
    }

    /// Due on or before `today` (and active).
    pub fn is_due(&self, today: Date) -> bool {
        self.active && self.next().is_some_and(|n| n <= today)
    }

    /// "Every 2 weeks", "Monthly".
    pub fn schedule(&self) -> String {
        if self.every <= 1 {
            self.frequency.label().to_string()
        } else {
            format!("Every {} {}", self.every, self.frequency.unit(self.every))
        }
    }

    /// Its settings, for editing.
    pub fn spec(&self) -> RecurringSpec {
        RecurringSpec {
            name: self.name.clone(),
            frequency: self.frequency,
            every: self.every,
            next: self.occurrence(self.done),
            end: self.end,
            mode: self.mode,
        }
    }

    /// The template's amount, for lists.
    pub fn amount(&self) -> Option<Money> {
        self.template.body.total()
    }
}

/// Only these can recur: payments, credits and refunds apply to
/// particular open invoices or bills, which a schedule cannot know.
pub fn can_recur(body: &TxnBody) -> bool {
    matches!(
        body,
        TxnBody::Invoice(_)
            | TxnBody::SalesReceipt(_)
            | TxnBody::Bill(_)
            | TxnBody::Check(_)
            | TxnBody::Deposit(_)
            | TxnBody::Journal(_)
    )
}

impl Company {
    pub fn recurring(&self, id: RecurringId) -> Option<&Recurring> {
        self.recurring.iter().find(|r| r.id == id)
    }

    fn recurring_mut(&mut self, id: RecurringId) -> LedgerResult<&mut Recurring> {
        self.recurring
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Recurring transaction {id}")))
    }

    fn check_spec(&self, spec: &RecurringSpec, me: Option<RecurringId>) -> LedgerResult<String> {
        let name = spec.name.trim();
        if name.is_empty() {
            return Err(LedgerError::Required("A name for the recurring transaction"));
        }
        if self
            .recurring
            .iter()
            .any(|r| Some(r.id) != me && r.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!(
                "A recurring transaction named {name}"
            )));
        }
        if spec.every == 0 {
            return Err(LedgerError::NotPositive("How often it recurs"));
        }
        if spec.end.is_some_and(|end| end < spec.next) {
            return Err(LedgerError::Invalid(
                "The end date is before the next date.".into(),
            ));
        }
        Ok(name.to_string())
    }

    /// Makes a transaction recurring, with a schedule. The transaction is checked
    /// as if it were being entered.
    pub fn add_recurring(
        &mut self,
        spec: RecurringSpec,
        template: TxnTemplate,
    ) -> LedgerResult<RecurringId> {
        let name = self.check_spec(&spec, None)?;
        if !can_recur(&template.body) {
            return Err(LedgerError::Invalid(format!(
                "A {} cannot recur: it applies to particular open transactions.",
                template.body.label().to_lowercase()
            )));
        }
        self.validate(&template.body)?;
        if let Some(class) = template.class {
            self.class(class)
                .ok_or_else(|| LedgerError::NotFound(format!("Class {class}")))?;
        }
        let id = RecurringId(self.alloc());
        self.recurring.push(Recurring {
            id,
            name,
            frequency: spec.frequency,
            every: spec.every,
            start: spec.next,
            done: 0,
            end: spec.end,
            mode: spec.mode,
            active: true,
            template,
        });
        Ok(id)
    }

    /// Changes the name and schedule; the next date given becomes the start
    /// of the schedule.
    pub fn update_recurring(&mut self, id: RecurringId, spec: RecurringSpec) -> LedgerResult<()> {
        let name = self.check_spec(&spec, Some(id))?;
        let r = self.recurring_mut(id)?;
        r.name = name;
        r.frequency = spec.frequency;
        r.every = spec.every;
        r.start = spec.next;
        r.done = 0;
        r.end = spec.end;
        r.mode = spec.mode;
        Ok(())
    }

    pub fn delete_recurring(&mut self, id: RecurringId) -> LedgerResult<()> {
        self.recurring_mut(id)?;
        self.recurring.retain(|r| r.id != id);
        Ok(())
    }

    /// An inactive one is never due and never entered.
    pub fn set_recurring_active(&mut self, id: RecurringId, active: bool) -> LedgerResult<()> {
        self.recurring_mut(id)?.active = active;
        Ok(())
    }

    /// Moves to the next date without entering this one.
    pub fn skip_recurring(&mut self, id: RecurringId) -> LedgerResult<()> {
        let r = self.recurring_mut(id)?;
        if r.next().is_none() {
            return Err(LedgerError::Invalid(format!("{} has ended.", r.name)));
        }
        r.done = r.done.saturating_add(1);
        Ok(())
    }

    /// The ones due on or before `today`, soonest first.
    pub fn recurring_due(&self, today: Date) -> Vec<&Recurring> {
        let mut due: Vec<&Recurring> = self.recurring.iter().filter(|r| r.is_due(today)).collect();
        due.sort_by_key(|r| (r.next(), r.id));
        due
    }

    /// The automatic ones due on or before `today`.
    pub fn automatic_due(&self, today: Date) -> bool {
        self.recurring
            .iter()
            .any(|r| r.mode == RecurringMode::Automatic && r.is_due(today))
    }

    /// The template made ready for `date`: due dates keep their distance
    /// from the transaction date, and sales tax is worked out again at the
    /// rates in force that day.
    pub fn recurring_body(&self, id: RecurringId, date: Date) -> LedgerResult<TxnBody> {
        let r = self
            .recurring(id)
            .ok_or_else(|| LedgerError::NotFound(format!("Recurring transaction {id}")))?;
        let shift = |due: Date| date.add_days(r.template.date.days_until(due));
        let mut body = r.template.body.clone();
        match &mut body {
            TxnBody::Invoice(inv) => {
                inv.due_date = shift(inv.due_date);
                if let Some(group) = inv.tax.as_ref().map(|t| t.group) {
                    inv.tax = self.retax(group, date, &inv.lines)?;
                }
            }
            TxnBody::SalesReceipt(sr) => {
                if let Some(group) = sr.tax.as_ref().map(|t| t.group) {
                    sr.tax = self.retax(group, date, &sr.lines)?;
                }
            }
            TxnBody::Bill(bill) => bill.due_date = shift(bill.due_date),
            _ => {}
        }
        Ok(body)
    }

    fn retax(
        &self,
        group: crate::TaxGroupId,
        date: Date,
        lines: &[crate::SalesLine],
    ) -> LedgerResult<Option<crate::InvoiceTax>> {
        let taxable = lines
            .iter()
            .filter(|l| l.taxable)
            .try_fold(Money::ZERO, |acc, l| acc.checked_add(l.amount()?))
            .ok_or(LedgerError::Overflow)?;
        if taxable.is_zero() {
            return Ok(None);
        }
        self.invoice_tax(group, date, taxable).map(Some)
    }

    /// Enters the next occurrence as given (the form may have changed it)
    /// and moves the schedule on.
    pub fn enter_recurring(
        &mut self,
        id: RecurringId,
        date: Date,
        number: &str,
        memo: &str,
        body: TxnBody,
        class: Option<TxnClassId>,
    ) -> LedgerResult<TxnId> {
        let r = self
            .recurring(id)
            .ok_or_else(|| LedgerError::NotFound(format!("Recurring transaction {id}")))?;
        if r.next().is_none() {
            return Err(LedgerError::Invalid(format!("{} has ended.", r.name)));
        }
        let txn = self.post(date, number, memo, body)?;
        self.set_txn_class(txn, class)?;
        let r = self.recurring_mut(id)?;
        r.done = r.done.saturating_add(1);
        Ok(txn)
    }

    /// Enters every automatic occurrence due on or before `today`, oldest
    /// first, and returns the transactions made.
    pub fn enter_due_recurring(&mut self, today: Date) -> LedgerResult<Vec<TxnId>> {
        let mut made = Vec::new();
        let ids: Vec<RecurringId> = self
            .recurring
            .iter()
            .filter(|r| r.mode == RecurringMode::Automatic)
            .map(|r| r.id)
            .collect();
        for id in ids {
            for _ in 0..MAX_CATCH_UP {
                let Some(r) = self.recurring(id).filter(|r| r.is_due(today)) else {
                    break;
                };
                let Some(date) = r.next() else {
                    break;
                };
                let (memo, class) = (r.template.memo.clone(), r.template.class);
                let body = self.recurring_body(id, date)?;
                let number = self.next_number_for(&body);
                made.push(self.enter_recurring(id, date, &number, &memo, body, class)?);
            }
        }
        Ok(made)
    }

    /// The number an automatically entered transaction gets: the next
    /// invoice or sales receipt number; others are left blank.
    fn next_number_for(&self, body: &TxnBody) -> String {
        if !matches!(body, TxnBody::Invoice(_) | TxnBody::SalesReceipt(_)) {
            return String::new();
        }
        let same = |t: &TxnBody| std::mem::discriminant(t) == std::mem::discriminant(body);
        self.transactions
            .iter()
            .filter(|t| same(&t.body))
            .filter_map(|t| t.number.trim().parse::<u64>().ok())
            .max()
            .map_or_else(|| "1001".to_string(), |n| n.saturating_add(1).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountLine, Check, Contact};

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn rent(c: &Company) -> TxnTemplate {
        let id = |n: &str| c.account_by_name(n).unwrap().id;
        TxnTemplate {
            date: d("2026-01-31"),
            memo: "Rent".into(),
            body: TxnBody::Check(Check {
                payee: None,
                pay_from: id("Checking"),
                lines: vec![AccountLine {
                    account: id("Rent"),
                    description: String::new(),
                    amount: Money::from_dollars(1200),
                }],
            }),
            class: None,
        }
    }

    fn monthly(name: &str, next: &str, mode: RecurringMode) -> RecurringSpec {
        RecurringSpec {
            name: name.into(),
            frequency: Frequency::Monthly,
            every: 1,
            next: d(next),
            end: None,
            mode,
        }
    }

    #[test]
    fn month_ends_do_not_drift() {
        let mut c = Company::with_default_accounts("R");
        let t = rent(&c);
        let id = c
            .add_recurring(monthly("Rent", "2026-01-31", RecurringMode::Remind), t)
            .unwrap();
        let r = c.recurring(id).unwrap();
        let dates: Vec<Date> = (0..4).map(|n| r.occurrence(n)).collect();
        assert_eq!(
            dates,
            [d("2026-01-31"), d("2026-02-28"), d("2026-03-31"), d("2026-04-30")]
        );
    }

    #[test]
    fn automatic_ones_catch_up_and_reminders_wait() {
        let mut c = Company::with_default_accounts("R");
        let t = rent(&c);
        let auto = c
            .add_recurring(monthly("Rent", "2026-01-31", RecurringMode::Automatic), t.clone())
            .unwrap();
        let remind = c
            .add_recurring(monthly("Rent reminder", "2026-01-31", RecurringMode::Remind), t)
            .unwrap();
        assert!(c.automatic_due(d("2026-03-31")));
        let made = c.enter_due_recurring(d("2026-03-31")).unwrap();
        assert_eq!(made.len(), 3);
        let dates: Vec<Date> = made.iter().map(|id| c.txn(*id).unwrap().date).collect();
        assert_eq!(dates, [d("2026-01-31"), d("2026-02-28"), d("2026-03-31")]);
        assert_eq!(c.recurring(auto).unwrap().next(), Some(d("2026-04-30")));
        // The reminder is due, not entered.
        let due: Vec<RecurringId> = c
            .recurring_due(d("2026-03-31"))
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(due, [remind]);
        // Entering again the same day does nothing more.
        assert!(!c.automatic_due(d("2026-03-31")));
        assert!(c.enter_due_recurring(d("2026-03-31")).unwrap().is_empty());
    }

    #[test]
    fn skipping_ending_and_inactive() {
        let mut c = Company::with_default_accounts("R");
        let t = rent(&c);
        let mut spec = monthly("Rent", "2026-01-31", RecurringMode::Automatic);
        spec.end = Some(d("2026-02-28"));
        let id = c.add_recurring(spec, t).unwrap();
        c.skip_recurring(id).unwrap();
        assert_eq!(c.recurring(id).unwrap().next(), Some(d("2026-02-28")));
        c.set_recurring_active(id, false).unwrap();
        assert!(c.enter_due_recurring(d("2026-12-31")).unwrap().is_empty());
        c.set_recurring_active(id, true).unwrap();
        assert_eq!(c.enter_due_recurring(d("2026-12-31")).unwrap().len(), 1);
        // Past its end: nothing more is due, and it cannot be skipped.
        assert_eq!(c.recurring(id).unwrap().next(), None);
        assert!(c.skip_recurring(id).is_err());
    }

    #[test]
    fn bills_keep_their_terms_and_bad_settings_are_refused() {
        let mut c = Company::with_default_accounts("R");
        let vendor = c
            .add_vendor(Contact {
                name: "Landlord".into(),
                ..Contact::default()
            })
            .unwrap();
        let id = |c: &Company, n: &str| c.account_by_name(n).unwrap().id;
        let bill = TxnBody::Bill(crate::Bill {
            vendor,
            due_date: d("2026-02-15"),
            ap_account: id(&c, "Accounts Payable"),
            lines: vec![AccountLine {
                account: id(&c, "Rent"),
                description: String::new(),
                amount: Money::from_dollars(900),
            }],
        });
        let template = TxnTemplate {
            date: d("2026-02-01"),
            memo: String::new(),
            body: bill,
            class: None,
        };
        let r = c
            .add_recurring(monthly("Lease", "2026-03-01", RecurringMode::Remind), template)
            .unwrap();
        let body = c.recurring_body(r, d("2026-03-01")).unwrap();
        assert!(
            matches!(&body, TxnBody::Bill(b) if b.due_date == d("2026-03-15")),
            "{body:?}"
        );
        // A duplicate name, a zero interval and an end before the start are refused.
        let t = rent(&c);
        assert!(c
            .add_recurring(monthly("lease", "2026-03-01", RecurringMode::Remind), t.clone())
            .is_err());
        let mut zero = monthly("Zero", "2026-03-01", RecurringMode::Remind);
        zero.every = 0;
        assert!(c.add_recurring(zero, t.clone()).is_err());
        let mut ended = monthly("Ended", "2026-03-01", RecurringMode::Remind);
        ended.end = Some(d("2026-02-01"));
        assert!(c.add_recurring(ended, t).is_err());
    }
}
