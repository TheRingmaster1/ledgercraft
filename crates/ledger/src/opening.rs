//! Opening balances: where the books stood on the day LedgerCraft takes
//! over from paper or another program.
//!
//! Balance sheet accounts get one journal entry. What each customer owes
//! becomes an opening invoice, and what is owed to each vendor an opening
//! bill, so Receive Payments and Pay Bills can settle them like any other.
//! Everything is offset to the Opening Balance Equity account, which nets
//! to zero once every balance is in (or is closed into Owner's Equity).

use ledgercraft_core::{Date, Money, Qty};
use serde::{Deserialize, Serialize};

use crate::{
    AccountId, AccountType, Bill, Class, Company, CustomerId, Invoice, JournalEntry, JournalLine,
    LedgerError, LedgerResult, SalesLine, TxnBody, TxnId, VendorId,
};

/// The equity account opening balances are offset to.
pub const OPENING_BALANCE_EQUITY: &str = "Opening Balance Equity";
/// The number it is given when created, if that number is free.
const OPENING_BALANCE_NUMBER: &str = "3050";
/// What opening transactions are called (their memo and line description).
pub const OPENING_BALANCE_MEMO: &str = "Opening balance";

/// Balances as a person reads them: positive is an account's normal
/// balance (money in the bank, a loan owed), what a customer owes, or what
/// is owed to a vendor.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpeningBalances {
    #[serde(default)]
    pub accounts: Vec<(AccountId, Money)>,
    #[serde(default)]
    pub customers: Vec<(CustomerId, Money)>,
    #[serde(default)]
    pub vendors: Vec<(VendorId, Money)>,
}

impl OpeningBalances {
    pub fn is_empty(&self) -> bool {
        self.accounts.iter().all(|(_, m)| m.is_zero())
            && self.customers.iter().all(|(_, m)| m.is_zero())
            && self.vendors.iter().all(|(_, m)| m.is_zero())
    }
}

/// Accounts that take an opening balance directly: the balance sheet,
/// except receivables and payables (those come from customers and vendors).
pub fn takes_opening_balance(kind: AccountType) -> bool {
    !kind.needs_party()
        && matches!(
            kind.class(),
            Class::Asset | Class::Liability | Class::Equity
        )
}

impl Company {
    /// The Opening Balance Equity account, created if there is none.
    pub fn opening_balance_equity(&mut self) -> LedgerResult<AccountId> {
        if let Some(a) = self.accounts.iter().find(|a| {
            a.kind == AccountType::Equity && a.name.eq_ignore_ascii_case(OPENING_BALANCE_EQUITY)
        }) {
            return Ok(a.id);
        }
        let description = "Offsets opening balances";
        match self.add_account(
            OPENING_BALANCE_NUMBER,
            OPENING_BALANCE_EQUITY,
            AccountType::Equity,
            description,
        ) {
            Ok(id) => Ok(id),
            // The number is taken: add it without one.
            Err(_) => {
                self.add_account("", OPENING_BALANCE_EQUITY, AccountType::Equity, description)
            }
        }
    }

    /// Enters opening balances as of `date` and returns the transactions
    /// made. Zero amounts are left out. Nothing is entered if any balance is
    /// refused.
    pub fn enter_opening_balances(
        &mut self,
        date: Date,
        balances: &OpeningBalances,
    ) -> LedgerResult<Vec<TxnId>> {
        if balances.is_empty() {
            return Err(LedgerError::Required("At least one opening balance"));
        }
        let before = self.clone();
        let result = self.enter_opening_balances_inner(date, balances);
        if result.is_err() {
            *self = before;
        }
        result
    }

    fn enter_opening_balances_inner(
        &mut self,
        date: Date,
        balances: &OpeningBalances,
    ) -> LedgerResult<Vec<TxnId>> {
        let equity = self.opening_balance_equity()?;
        let mut made = Vec::new();

        // Balance sheet accounts: one journal entry.
        let mut lines = Vec::new();
        let mut net = Money::ZERO; // debits less credits so far
        for (id, amount) in balances.accounts.iter().filter(|(_, m)| !m.is_zero()) {
            let account = self
                .account(*id)
                .ok_or_else(|| LedgerError::NotFound(format!("Account {id}")))?;
            if !takes_opening_balance(account.kind) {
                return Err(LedgerError::Invalid(format!(
                    "{} takes no opening balance of its own: enter what customers owe, or \
                     what is owed to vendors, instead",
                    account.name
                )));
            }
            if *id == equity {
                return Err(LedgerError::Invalid(format!(
                    "{OPENING_BALANCE_EQUITY} is what opening balances are offset to"
                )));
            }
            // A positive amount is the account's normal side.
            let debit_side = account.kind.is_debit_normal() != amount.is_negative();
            let size = abs(*amount)?;
            let (debit, credit) = if debit_side {
                net = net.checked_add(size).ok_or(LedgerError::Overflow)?;
                (size, Money::ZERO)
            } else {
                net = net.checked_sub(size).ok_or(LedgerError::Overflow)?;
                (Money::ZERO, size)
            };
            lines.push(JournalLine {
                account: *id,
                debit,
                credit,
                memo: OPENING_BALANCE_MEMO.into(),
                party: None,
            });
        }
        if !lines.is_empty() {
            if !net.is_zero() {
                let size = abs(net)?;
                let (debit, credit) = if net.is_negative() {
                    (size, Money::ZERO)
                } else {
                    (Money::ZERO, size)
                };
                lines.push(JournalLine {
                    account: equity,
                    debit,
                    credit,
                    memo: OPENING_BALANCE_MEMO.into(),
                    party: None,
                });
            }
            if lines.len() >= 2 {
                made.push(self.post(
                    date,
                    "",
                    "Opening balances",
                    TxnBody::Journal(JournalEntry { lines }),
                )?);
            }
        }

        // Customers: an opening invoice each.
        let ar = self.first_account_of(AccountType::AccountsReceivable);
        for (customer, amount) in balances.customers.iter().filter(|(_, m)| !m.is_zero()) {
            let c = self
                .customer(*customer)
                .ok_or_else(|| LedgerError::NotFound(format!("Customer {customer}")))?;
            if amount.is_negative() {
                return Err(LedgerError::Invalid(format!(
                    "{}: an opening balance is what they owe you; enter a credit you owe \
                     them as a credit memo",
                    c.contact.name
                )));
            }
            let ar = ar.ok_or(LedgerError::Required("An Accounts Receivable account"))?;
            let due = date.add_days(i64::from(c.contact.terms_days));
            let body = TxnBody::Invoice(Invoice {
                customer: *customer,
                due_date: due,
                ar_account: ar,
                lines: vec![opening_line(*amount, equity)],
                tax: None,
                ship_to: None,
            });
            made.push(self.post(date, "", OPENING_BALANCE_MEMO, body)?);
        }

        // Vendors: an opening bill each.
        let ap = self.first_account_of(AccountType::AccountsPayable);
        for (vendor, amount) in balances.vendors.iter().filter(|(_, m)| !m.is_zero()) {
            let v = self
                .vendor(*vendor)
                .ok_or_else(|| LedgerError::NotFound(format!("Vendor {vendor}")))?;
            if amount.is_negative() {
                return Err(LedgerError::Invalid(format!(
                    "{}: an opening balance is what you owe them",
                    v.contact.name
                )));
            }
            let ap = ap.ok_or(LedgerError::Required("An Accounts Payable account"))?;
            let due = date.add_days(i64::from(v.contact.terms_days));
            let body = TxnBody::Bill(Bill {
                vendor: *vendor,
                due_date: due,
                ap_account: ap,
                lines: vec![crate::AccountLine {
                    account: equity,
                    description: OPENING_BALANCE_MEMO.into(),
                    amount: *amount,
                }],
            });
            made.push(self.post(date, "", OPENING_BALANCE_MEMO, body)?);
        }
        Ok(made)
    }
}

fn abs(m: Money) -> LedgerResult<Money> {
    if m.is_negative() {
        m.checked_neg().ok_or(LedgerError::Overflow)
    } else {
        Ok(m)
    }
}

fn opening_line(amount: Money, equity: AccountId) -> SalesLine {
    SalesLine {
        item: None,
        description: OPENING_BALANCE_MEMO.into(),
        qty: Qty::ONE,
        rate: amount,
        taxable: false,
        account: equity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Contact;

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    #[test]
    fn opening_balances_balance_and_can_be_paid() {
        let mut c = Company::with_default_accounts("Opening Co");
        let id = |c: &Company, n: &str| c.account_by_name(n).unwrap().id;
        let customer = c
            .add_customer(Contact {
                name: "Dana".into(),
                terms_days: 30,
                ..Contact::default()
            })
            .unwrap();
        let vendor = c
            .add_vendor(Contact {
                name: "Mill".into(),
                ..Contact::default()
            })
            .unwrap();
        let balances = OpeningBalances {
            accounts: vec![
                (id(&c, "Checking"), Money::from_dollars(5_000)),
                (id(&c, "Loan Payable"), Money::from_dollars(2_000)),
                (id(&c, "Savings"), Money::ZERO),
            ],
            customers: vec![(customer, Money::from_dollars(300))],
            vendors: vec![(vendor, Money::from_dollars(120))],
        };
        let made = c
            .enter_opening_balances(d("2026-01-01"), &balances)
            .unwrap();
        assert_eq!(made.len(), 3, "a journal entry, an invoice and a bill");
        let as_of = Some(d("2026-01-01"));
        assert_eq!(
            c.account_balance(id(&c, "Checking"), as_of),
            Money::from_dollars(5_000)
        );
        assert_eq!(
            c.account_balance(id(&c, "Loan Payable"), as_of),
            Money::from_dollars(2_000)
        );
        assert_eq!(
            c.account_balance(id(&c, "Accounts Receivable"), as_of),
            Money::from_dollars(300)
        );
        // 5,000 - 2,000 + 300 - 120 of equity.
        let equity = id(&c, OPENING_BALANCE_EQUITY);
        assert_eq!(c.account_balance(equity, as_of), Money::from_dollars(3_180));
        // The invoice is open, due on the customer's terms.
        let invoice = c.txn(made[1]).unwrap();
        assert!(matches!(&invoice.body, TxnBody::Invoice(i) if i.due_date == d("2026-01-31")));
        assert_eq!(c.open_invoices(customer).len(), 1);
        assert_eq!(c.open_bills(vendor).len(), 1);
    }

    #[test]
    fn refused_balances_leave_nothing_behind() {
        let mut c = Company::with_default_accounts("Refused Co");
        let id = |c: &Company, n: &str| c.account_by_name(n).unwrap().id;
        let customer = c
            .add_customer(Contact {
                name: "Owed".into(),
                ..Contact::default()
            })
            .unwrap();
        let before = c.clone();
        let balances = OpeningBalances {
            accounts: vec![(id(&c, "Checking"), Money::from_dollars(10))],
            customers: vec![(customer, Money::from_dollars(-5))],
            vendors: Vec::new(),
        };
        assert!(c
            .enter_opening_balances(d("2026-01-01"), &balances)
            .is_err());
        assert_eq!(c, before);
        // Receivables take theirs from customers.
        let ar = OpeningBalances {
            accounts: vec![(id(&c, "Accounts Receivable"), Money::from_dollars(10))],
            ..OpeningBalances::default()
        };
        assert!(c.enter_opening_balances(d("2026-01-01"), &ar).is_err());
        assert!(c
            .enter_opening_balances(d("2026-01-01"), &OpeningBalances::default())
            .is_err());
        // A negative bank balance (overdrawn) is a credit.
        let overdrawn = OpeningBalances {
            accounts: vec![(id(&c, "Checking"), Money::from_dollars(-40))],
            ..OpeningBalances::default()
        };
        c.enter_opening_balances(d("2026-01-01"), &overdrawn)
            .unwrap();
        assert_eq!(
            c.account_balance(id(&c, "Checking"), None),
            Money::from_dollars(-40)
        );
    }
}
