use ledgercraft_core::{Date, Money, Qty};
use serde::{Deserialize, Serialize};

use crate::{AccountId, CustomerId, ItemId, LedgerError, LedgerResult, Party, TxnId, VendorId};

/// One side of a double-entry: a positive amount is a debit, a negative one a credit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Posting {
    pub account: AccountId,
    pub amount: Money,
    pub party: Option<Party>,
    pub memo: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TxnId,
    pub date: Date,
    /// Reference number: invoice no., check no., bill ref.
    #[serde(default)]
    pub number: String,
    #[serde(default)]
    pub memo: String,
    pub body: TxnBody,
    /// Voided transactions stay in the file for the audit trail but post nothing.
    #[serde(default)]
    pub voided: bool,
}

/// The forms a transaction can be entered on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TxnBody {
    Journal(JournalEntry),
    Invoice(Invoice),
    ReceivePayment(ReceivePayment),
    Bill(Bill),
    PayBill(PayBill),
    Check(Check),
    Deposit(Deposit),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalLine {
    pub account: AccountId,
    #[serde(default)]
    pub debit: Money,
    #[serde(default)]
    pub credit: Money,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub party: Option<Party>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub lines: Vec<JournalLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesLine {
    #[serde(default)]
    pub item: Option<ItemId>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub qty: Qty,
    pub rate: Money,
    /// The income account this line credits.
    pub account: AccountId,
}

impl SalesLine {
    pub fn amount(&self) -> Option<Money> {
        self.rate.times(self.qty)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invoice {
    pub customer: CustomerId,
    pub due_date: Date,
    pub ar_account: AccountId,
    pub lines: Vec<SalesLine>,
}

impl Invoice {
    pub fn total(&self) -> Option<Money> {
        self.lines
            .iter()
            .try_fold(Money::ZERO, |acc, l| acc.checked_add(l.amount()?))
    }
}

/// A payment amount applied to an open invoice or bill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    pub txn: TxnId,
    pub amount: Money,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceivePayment {
    pub customer: CustomerId,
    /// A bank account, or Undeposited Funds to group payments into one deposit.
    pub deposit_to: AccountId,
    pub ar_account: AccountId,
    pub amount: Money,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub applied: Vec<Applied>,
}

/// An amount charged to (or deposited from) one account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountLine {
    pub account: AccountId,
    #[serde(default)]
    pub description: String,
    pub amount: Money,
}

fn lines_total(lines: &[AccountLine]) -> Option<Money> {
    Money::checked_sum(lines.iter().map(|l| l.amount))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bill {
    pub vendor: VendorId,
    pub due_date: Date,
    pub ap_account: AccountId,
    pub lines: Vec<AccountLine>,
}

impl Bill {
    pub fn total(&self) -> Option<Money> {
        lines_total(&self.lines)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayBill {
    pub vendor: VendorId,
    pub pay_from: AccountId,
    pub ap_account: AccountId,
    pub amount: Money,
    #[serde(default)]
    pub applied: Vec<Applied>,
}

/// A check, debit-card purchase or other direct expense.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    #[serde(default)]
    pub payee: Option<Party>,
    pub pay_from: AccountId,
    pub lines: Vec<AccountLine>,
}

impl Check {
    pub fn total(&self) -> Option<Money> {
        lines_total(&self.lines)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deposit {
    pub deposit_to: AccountId,
    pub lines: Vec<AccountLine>,
}

impl Deposit {
    pub fn total(&self) -> Option<Money> {
        lines_total(&self.lines)
    }
}

fn neg(m: Money) -> LedgerResult<Money> {
    m.checked_neg().ok_or(LedgerError::Overflow)
}

fn post(account: AccountId, amount: Money, party: Option<Party>, memo: &str) -> Posting {
    Posting {
        account,
        amount,
        party,
        memo: memo.to_string(),
    }
}

impl TxnBody {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Journal(_) => "Journal Entry",
            Self::Invoice(_) => "Invoice",
            Self::ReceivePayment(_) => "Payment",
            Self::Bill(_) => "Bill",
            Self::PayBill(_) => "Bill Payment",
            Self::Check(_) => "Check",
            Self::Deposit(_) => "Deposit",
        }
    }

    /// The customer or vendor the transaction is with, if any.
    pub fn party(&self) -> Option<Party> {
        match self {
            Self::Journal(j) => j.lines.iter().find_map(|l| l.party),
            Self::Invoice(i) => Some(Party::Customer(i.customer)),
            Self::ReceivePayment(p) => Some(Party::Customer(p.customer)),
            Self::Bill(b) => Some(Party::Vendor(b.vendor)),
            Self::PayBill(p) => Some(Party::Vendor(p.vendor)),
            Self::Check(c) => c.payee,
            Self::Deposit(_) => None,
        }
    }

    /// The headline amount shown in transaction lists.
    pub fn total(&self) -> Option<Money> {
        match self {
            Self::Journal(j) => Money::checked_sum(j.lines.iter().map(|l| l.debit)),
            Self::Invoice(i) => i.total(),
            Self::ReceivePayment(p) => Some(p.amount),
            Self::Bill(b) => b.total(),
            Self::PayBill(p) => Some(p.amount),
            Self::Check(c) => c.total(),
            Self::Deposit(d) => d.total(),
        }
    }

    /// The double-entry postings this transaction makes. They always sum to zero.
    pub fn postings(&self) -> LedgerResult<Vec<Posting>> {
        let mut out = Vec::new();
        match self {
            Self::Journal(j) => {
                for l in &j.lines {
                    let amount = l.debit.checked_sub(l.credit).ok_or(LedgerError::Overflow)?;
                    out.push(post(l.account, amount, l.party, &l.memo));
                }
            }
            Self::Invoice(inv) => {
                let party = Some(Party::Customer(inv.customer));
                let total = inv.total().ok_or(LedgerError::Overflow)?;
                out.push(post(inv.ar_account, total, party, ""));
                for l in &inv.lines {
                    let amount = l.amount().ok_or(LedgerError::Overflow)?;
                    out.push(post(l.account, neg(amount)?, party, &l.description));
                }
            }
            Self::ReceivePayment(p) => {
                let party = Some(Party::Customer(p.customer));
                out.push(post(p.deposit_to, p.amount, party, &p.method));
                out.push(post(p.ar_account, neg(p.amount)?, party, &p.method));
            }
            Self::Bill(b) => {
                let party = Some(Party::Vendor(b.vendor));
                let total = b.total().ok_or(LedgerError::Overflow)?;
                out.push(post(b.ap_account, neg(total)?, party, ""));
                for l in &b.lines {
                    out.push(post(l.account, l.amount, party, &l.description));
                }
            }
            Self::PayBill(p) => {
                let party = Some(Party::Vendor(p.vendor));
                out.push(post(p.ap_account, p.amount, party, ""));
                out.push(post(p.pay_from, neg(p.amount)?, party, ""));
            }
            Self::Check(c) => {
                let total = c.total().ok_or(LedgerError::Overflow)?;
                out.push(post(c.pay_from, neg(total)?, c.payee, ""));
                for l in &c.lines {
                    out.push(post(l.account, l.amount, c.payee, &l.description));
                }
            }
            Self::Deposit(d) => {
                let total = d.total().ok_or(LedgerError::Overflow)?;
                out.push(post(d.deposit_to, total, None, ""));
                for l in &d.lines {
                    out.push(post(l.account, neg(l.amount)?, None, &l.description));
                }
            }
        }
        Ok(out)
    }

    /// Documents this transaction pays down, with amounts.
    pub fn applications(&self) -> &[Applied] {
        match self {
            Self::ReceivePayment(p) => &p.applied,
            Self::PayBill(p) => &p.applied,
            _ => &[],
        }
    }
}
