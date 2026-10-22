use ledgercraft_core::{Date, Money, Qty};
use serde::{Deserialize, Serialize};

use crate::{
    AccountId, CustomerId, InvoiceTax, ItemId, LedgerError, LedgerResult, Party, TxnId, VendorId,
};

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
    SalesReceipt(SalesReceipt),
    CreditMemo(CreditMemo),
    Refund(Refund),
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
    /// Whether sales tax is charged on the line.
    #[serde(default = "taxable_by_default")]
    pub taxable: bool,
}

/// Lines and items are taxable unless marked otherwise.
pub(crate) const fn taxable_by_default() -> bool {
    true
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
    /// The sales tax charged, as worked out when the invoice was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax: Option<InvoiceTax>,
}

impl Invoice {
    fn sale(&self) -> Sale<'_> {
        Sale {
            customer: Some(self.customer),
            lines: &self.lines,
            tax: self.tax.as_ref(),
        }
    }

    /// The lines added up, before tax.
    pub fn subtotal(&self) -> Option<Money> {
        self.sale().subtotal()
    }

    /// The lines sales tax is charged on, added up.
    pub fn taxable_amount(&self) -> Option<Money> {
        self.sale().taxable_amount()
    }

    /// The sales tax charged; zero without tax.
    pub fn tax_total(&self) -> Option<Money> {
        self.sale().tax_total()
    }

    /// What the customer owes: the lines plus tax.
    pub fn total(&self) -> Option<Money> {
        self.sale().total()
    }
}

/// A sale paid for on the spot (cash, card, check): nothing is owed, and
/// the money goes straight to a bank account or Undeposited Funds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesReceipt {
    /// Who bought; walk-in sales need not say.
    #[serde(default)]
    pub customer: Option<CustomerId>,
    /// A bank account, or Undeposited Funds to group with other takings.
    pub deposit_to: AccountId,
    /// How they paid: cash, a check number, a card.
    #[serde(default)]
    pub method: String,
    pub lines: Vec<SalesLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax: Option<InvoiceTax>,
}

impl SalesReceipt {
    fn sale(&self) -> Sale<'_> {
        Sale {
            customer: self.customer,
            lines: &self.lines,
            tax: self.tax.as_ref(),
        }
    }

    pub fn total(&self) -> Option<Money> {
        self.sale().total()
    }
}

/// Money a customer is credited (a return, an allowance, a mistake): it
/// lowers what they owe, can pay down their open invoices, and what is left
/// can be refunded. Sales tax charged on the credited lines is given back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditMemo {
    pub customer: CustomerId,
    pub ar_account: AccountId,
    pub lines: Vec<SalesLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax: Option<InvoiceTax>,
    /// Open invoices the credit pays down.
    #[serde(default)]
    pub applied: Vec<Applied>,
}

impl CreditMemo {
    fn sale(&self) -> Sale<'_> {
        Sale {
            customer: Some(self.customer),
            lines: &self.lines,
            tax: self.tax.as_ref(),
        }
    }

    pub fn total(&self) -> Option<Money> {
        self.sale().total()
    }
}

/// Money paid back to a customer out of their credit memos.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refund {
    pub customer: CustomerId,
    /// The bank or credit card account the money goes out of.
    pub pay_from: AccountId,
    pub ar_account: AccountId,
    pub amount: Money,
    #[serde(default)]
    pub method: String,
    /// The credit memos refunded.
    #[serde(default)]
    pub applied: Vec<Applied>,
}

/// The lines and tax of an invoice, sales receipt or credit memo, which are
/// added up and taxed the same way.
#[derive(Clone, Copy, Debug)]
pub struct Sale<'a> {
    pub customer: Option<CustomerId>,
    pub lines: &'a [SalesLine],
    pub tax: Option<&'a InvoiceTax>,
}

impl Sale<'_> {
    /// The lines added up, before tax.
    pub fn subtotal(&self) -> Option<Money> {
        self.lines
            .iter()
            .try_fold(Money::ZERO, |acc, l| acc.checked_add(l.amount()?))
    }

    /// The lines sales tax is charged on, added up.
    pub fn taxable_amount(&self) -> Option<Money> {
        self.lines
            .iter()
            .filter(|l| l.taxable)
            .try_fold(Money::ZERO, |acc, l| acc.checked_add(l.amount()?))
    }

    /// The sales tax; zero without tax.
    pub fn tax_total(&self) -> Option<Money> {
        self.tax.map_or(Some(Money::ZERO), InvoiceTax::total)
    }

    /// The lines plus tax.
    pub fn total(&self) -> Option<Money> {
        self.subtotal()?.checked_add(self.tax_total()?)
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

/// The income and tax side of a sale: credits to income and sales tax, or,
/// for a credit memo (`reverse`), debits taking them back.
fn sale_postings(
    out: &mut Vec<Posting>,
    lines: &[SalesLine],
    tax: Option<&InvoiceTax>,
    party: Option<Party>,
    reverse: bool,
) -> LedgerResult<()> {
    let signed = |m: Money| if reverse { Ok(m) } else { neg(m) };
    for l in lines {
        let amount = l.amount().ok_or(LedgerError::Overflow)?;
        out.push(post(l.account, signed(amount)?, party, &l.description));
    }
    if let Some(tax) = tax {
        for l in &tax.lines {
            out.push(post(tax.account, signed(l.amount)?, party, &l.name));
        }
    }
    Ok(())
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
            Self::SalesReceipt(_) => "Sales Receipt",
            Self::CreditMemo(_) => "Credit Memo",
            Self::Refund(_) => "Refund",
        }
    }

    /// The lines and tax of an invoice, sales receipt or credit memo.
    pub fn sale(&self) -> Option<Sale<'_>> {
        match self {
            Self::Invoice(i) => Some(i.sale()),
            Self::SalesReceipt(r) => Some(r.sale()),
            Self::CreditMemo(m) => Some(m.sale()),
            _ => None,
        }
    }

    /// Whether the transaction's sales count against sales (a credit memo
    /// gives sales, and their tax, back): 1, -1, or 0 for no sales.
    pub fn sales_sign(&self) -> i64 {
        match self {
            Self::Invoice(_) | Self::SalesReceipt(_) => 1,
            Self::CreditMemo(_) => -1,
            _ => 0,
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
            Self::SalesReceipt(r) => r.customer.map(Party::Customer),
            Self::CreditMemo(m) => Some(Party::Customer(m.customer)),
            Self::Refund(r) => Some(Party::Customer(r.customer)),
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
            Self::SalesReceipt(r) => r.total(),
            Self::CreditMemo(m) => m.total(),
            Self::Refund(r) => Some(r.amount),
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
                sale_postings(&mut out, &inv.lines, inv.tax.as_ref(), party, false)?;
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
            Self::SalesReceipt(r) => {
                let party = r.customer.map(Party::Customer);
                let total = r.total().ok_or(LedgerError::Overflow)?;
                out.push(post(r.deposit_to, total, party, &r.method));
                sale_postings(&mut out, &r.lines, r.tax.as_ref(), party, false)?;
            }
            Self::CreditMemo(m) => {
                let party = Some(Party::Customer(m.customer));
                let total = m.total().ok_or(LedgerError::Overflow)?;
                out.push(post(m.ar_account, neg(total)?, party, ""));
                sale_postings(&mut out, &m.lines, m.tax.as_ref(), party, true)?;
            }
            Self::Refund(r) => {
                let party = Some(Party::Customer(r.customer));
                out.push(post(r.ar_account, r.amount, party, &r.method));
                out.push(post(r.pay_from, neg(r.amount)?, party, &r.method));
            }
        }
        Ok(out)
    }

    /// Documents this transaction pays down, with amounts.
    pub fn applications(&self) -> &[Applied] {
        match self {
            Self::ReceivePayment(p) => &p.applied,
            Self::PayBill(p) => &p.applied,
            Self::CreditMemo(m) => &m.applied,
            Self::Refund(r) => &r.applied,
            _ => &[],
        }
    }
}
