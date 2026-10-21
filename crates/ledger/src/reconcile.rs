//! Reconciling a bank or credit card account against its statement.
//!
//! A reconciliation records the statement date and ending balance and the
//! transactions it cleared. It is only accepted when the cleared items take
//! the account from its last reconciled balance to exactly the statement's
//! ending balance. Reconciled transactions are protected: they cannot be
//! voided, and an edit cannot change what they move the account by, until
//! the reconciliation is undone.

use std::collections::BTreeSet;

use ledgercraft_core::{Date, Money};
use serde::{Deserialize, Serialize};

use crate::{AccountId, AccountType, Company, LedgerError, LedgerResult, TxnBody, TxnId};

/// One completed reconciliation, kept in the company file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reconciliation {
    pub account: AccountId,
    pub statement_date: Date,
    /// The previous reconciliation's ending balance (zero for the first).
    pub beginning_balance: Money,
    pub ending_balance: Money,
    /// The transactions this reconciliation cleared.
    pub cleared: Vec<TxnId>,
}

/// A transaction waiting to be reconciled, with what it moves the account by
/// (positive raises the balance as the statement shows it: money in for a
/// bank, a charge for a credit card).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconcileItem {
    pub txn: TxnId,
    pub date: Date,
    pub amount: Money,
}

impl Company {
    /// Whether an account has statements to reconcile: banks and credit cards.
    pub fn can_reconcile(&self, account: AccountId) -> bool {
        self.account(account)
            .is_some_and(|a| matches!(a.kind, AccountType::Bank | AccountType::CreditCard))
    }

    /// An account's reconciliations, oldest first.
    pub fn reconciliations_for(&self, account: AccountId) -> Vec<&Reconciliation> {
        self.reconciliations
            .iter()
            .filter(|r| r.account == account)
            .collect()
    }

    /// The most recent reconciliation of an account.
    pub fn last_reconciliation(&self, account: AccountId) -> Option<&Reconciliation> {
        self.reconciliations
            .iter()
            .rev()
            .find(|r| r.account == account)
    }

    /// The balance the last statement agreed on: where the next one starts.
    pub fn reconciled_balance(&self, account: AccountId) -> Money {
        self.last_reconciliation(account)
            .map_or(Money::ZERO, |r| r.ending_balance)
    }

    /// The accounts a transaction has been reconciled in.
    pub fn reconciled_in(&self, txn: TxnId) -> Vec<AccountId> {
        let accounts: BTreeSet<AccountId> = self
            .reconciliations
            .iter()
            .filter(|r| r.cleared.contains(&txn))
            .map(|r| r.account)
            .collect();
        accounts.into_iter().collect()
    }

    /// Whether a transaction has been reconciled in this account.
    pub fn is_reconciled(&self, txn: TxnId, account: AccountId) -> bool {
        self.reconciliations
            .iter()
            .any(|r| r.account == account && r.cleared.contains(&txn))
    }

    /// What a transaction body moves an account by, in the account's own
    /// sign (see [`ReconcileItem`]); `None` if it does not touch the account.
    pub fn amount_in(&self, body: &TxnBody, account: AccountId) -> Option<Money> {
        let kind = self.account(account)?.kind;
        let postings = body.postings().ok()?;
        let mut touched = false;
        let mut raw = Money::ZERO;
        for p in postings.iter().filter(|p| p.account == account) {
            touched = true;
            raw = raw.checked_add(p.amount)?;
        }
        if !touched {
            return None;
        }
        if kind.is_debit_normal() {
            Some(raw)
        } else {
            raw.checked_neg()
        }
    }

    /// Live transactions in an account not yet reconciled, dated on or
    /// before `through`, oldest first.
    pub fn unreconciled(&self, account: AccountId, through: Date) -> Vec<ReconcileItem> {
        let mut items: Vec<ReconcileItem> = self
            .transactions
            .iter()
            .filter(|t| !t.voided && t.date <= through && !self.is_reconciled(t.id, account))
            .filter_map(|t| {
                Some(ReconcileItem {
                    txn: t.id,
                    date: t.date,
                    amount: self.amount_in(&t.body, account)?,
                })
            })
            .collect();
        items.sort_by_key(|i| (i.date, i.txn));
        items
    }

    /// Records a reconciliation. The cleared transactions must belong to the
    /// account, be live, unreconciled and dated by the statement date, and
    /// take the last reconciled balance to exactly `ending_balance`.
    pub fn reconcile(
        &mut self,
        account: AccountId,
        statement_date: Date,
        ending_balance: Money,
        cleared: Vec<TxnId>,
    ) -> LedgerResult<()> {
        self.require_kind(
            account,
            |k| matches!(k, AccountType::Bank | AccountType::CreditCard),
            "a bank or credit card account",
        )?;
        if let Some(last) = self.last_reconciliation(account) {
            if statement_date < last.statement_date {
                return Err(LedgerError::Invalid(format!(
                    "the statement date is before the last reconciliation ({})",
                    last.statement_date
                )));
            }
        }
        let beginning_balance = self.reconciled_balance(account);
        let mut seen = BTreeSet::new();
        let mut balance = beginning_balance;
        for id in &cleared {
            if !seen.insert(*id) {
                return Err(LedgerError::Invalid(format!("{id} is listed twice")));
            }
            let t = self
                .txn(*id)
                .filter(|t| !t.voided)
                .ok_or_else(|| LedgerError::NotFound(format!("Transaction {id}")))?;
            if t.date > statement_date {
                return Err(LedgerError::Invalid(format!(
                    "{id} is dated after the statement date"
                )));
            }
            if self.is_reconciled(*id, account) {
                return Err(LedgerError::Invalid(format!("{id} is already reconciled")));
            }
            let amount = self
                .amount_in(&t.body, account)
                .ok_or_else(|| LedgerError::Invalid(format!("{id} does not touch this account")))?;
            balance = balance.checked_add(amount).ok_or(LedgerError::Overflow)?;
        }
        if balance != ending_balance {
            let difference = ending_balance
                .checked_sub(balance)
                .ok_or(LedgerError::Overflow)?;
            return Err(LedgerError::Invalid(format!(
                "the cleared items come to {balance}, not the statement's {ending_balance} \
                 (a difference of {difference})"
            )));
        }
        self.reconciliations.push(Reconciliation {
            account,
            statement_date,
            beginning_balance,
            ending_balance,
            cleared,
        });
        Ok(())
    }

    /// Undoes an account's most recent reconciliation: its transactions are
    /// no longer reconciled and can be changed again.
    pub fn undo_last_reconciliation(&mut self, account: AccountId) -> LedgerResult<()> {
        let index = self
            .reconciliations
            .iter()
            .rposition(|r| r.account == account)
            .ok_or_else(|| {
                LedgerError::NotFound(format!(
                    "A reconciliation of {}",
                    self.account_name(account)
                ))
            })?;
        self.reconciliations.remove(index);
        Ok(())
    }

    /// Refuses a change that would alter a reconciled transaction's effect on
    /// the account it was reconciled in. `new_body` is `None` for a void.
    pub(crate) fn check_reconciled_change(
        &self,
        id: TxnId,
        new_body: Option<&TxnBody>,
    ) -> LedgerResult<()> {
        let Some(old) = self.txn(id) else {
            return Ok(());
        };
        for account in self.reconciled_in(id) {
            let unchanged = new_body.is_some_and(|body| {
                self.amount_in(body, account) == self.amount_in(&old.body, account)
            });
            if !unchanged {
                return Err(LedgerError::InUse(format!(
                    "{id} is reconciled in {}; undo that reconciliation first \
                     (Banking > Reconcile > Undo Last Reconciliation) to change its amount \
                     or void it",
                    self.account_name(account)
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountLine, Check, Contact, Deposit};

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn dollars(n: i64) -> Money {
        Money::from_cents(n * 100)
    }

    /// Checking with a 1,000 deposit, a 200 check and a 50 check.
    fn books() -> (Company, AccountId, [TxnId; 3]) {
        let mut c = Company::with_default_accounts("Bank Co");
        let checking = c.account_by_name("Checking").unwrap().id;
        let id = |c: &Company, n: &str| c.account_by_name(n).unwrap().id;
        c.add_vendor(Contact {
            name: "Landlord".into(),
            ..Contact::default()
        })
        .unwrap();
        let deposit = TxnBody::Deposit(Deposit {
            deposit_to: checking,
            lines: vec![AccountLine {
                account: id(&c, "Owner's Equity"),
                description: String::new(),
                amount: dollars(1_000),
            }],
        });
        let check = |c: &Company, amount| {
            TxnBody::Check(Check {
                payee: None,
                pay_from: checking,
                lines: vec![AccountLine {
                    account: id(c, "Rent"),
                    description: String::new(),
                    amount: dollars(amount),
                }],
            })
        };
        let a = c.post(d("2026-01-02"), "", "", deposit).unwrap();
        let b = c.post(d("2026-01-05"), "101", "", check(&c, 200)).unwrap();
        let e = c.post(d("2026-02-03"), "102", "", check(&c, 50)).unwrap();
        (c, checking, [a, b, e])
    }

    #[test]
    fn a_statement_that_matches_reconciles() {
        let (mut c, checking, [dep, chk, later]) = books();
        let open = c.unreconciled(checking, d("2026-01-31"));
        assert_eq!(open.len(), 2, "the February check is after the statement");
        assert_eq!(open[0].amount, dollars(1_000));
        assert_eq!(open[1].amount, dollars(-200));
        c.reconcile(checking, d("2026-01-31"), dollars(800), vec![dep, chk])
            .unwrap();
        assert_eq!(c.reconciled_balance(checking), dollars(800));
        assert!(c.is_reconciled(dep, checking));
        assert_eq!(c.unreconciled(checking, d("2026-02-28")).len(), 1);
        // The next statement starts from 800.
        c.reconcile(checking, d("2026-02-28"), dollars(750), vec![later])
            .unwrap();
        assert_eq!(c.reconciliations_for(checking).len(), 2);
        assert_eq!(
            c.last_reconciliation(checking).unwrap().beginning_balance,
            dollars(800)
        );
    }

    #[test]
    fn a_difference_or_a_bad_item_is_refused() {
        let (mut c, checking, [dep, chk, later]) = books();
        // Off by the 200 check.
        assert!(c
            .reconcile(checking, d("2026-01-31"), dollars(800), vec![dep])
            .is_err());
        // Dated after the statement.
        assert!(c
            .reconcile(checking, d("2026-01-31"), dollars(950), vec![dep, later])
            .is_err());
        // Twice.
        assert!(c
            .reconcile(checking, d("2026-01-31"), dollars(1_000), vec![dep, dep])
            .is_err());
        // Not a bank or credit card account.
        let rent = c.account_by_name("Rent").unwrap().id;
        assert!(c
            .reconcile(rent, d("2026-01-31"), dollars(200), vec![chk])
            .is_err());
        assert!(c.reconciliations.is_empty());
    }

    #[test]
    fn reconciled_transactions_are_protected_until_undone() {
        let (mut c, checking, [dep, chk, _]) = books();
        c.reconcile(checking, d("2026-01-31"), dollars(800), vec![dep, chk])
            .unwrap();
        // Cannot be voided, or have its amount changed...
        assert!(matches!(c.void(chk), Err(LedgerError::InUse(_))));
        let body = c.txn(chk).unwrap().body.clone();
        let mut bigger = body.clone();
        if let TxnBody::Check(ch) = &mut bigger {
            ch.lines[0].amount = dollars(250);
        }
        assert!(c
            .update(chk, d("2026-01-05"), "101", "", bigger.clone())
            .is_err());
        // ...but the memo can change.
        c.update(chk, d("2026-01-05"), "101", "January rent", body)
            .unwrap();
        // Undo frees it.
        c.undo_last_reconciliation(checking).unwrap();
        assert!(!c.is_reconciled(chk, checking));
        c.update(chk, d("2026-01-05"), "101", "", bigger).unwrap();
        c.void(chk).unwrap();
        assert!(c.undo_last_reconciliation(checking).is_err());
    }

    #[test]
    fn older_files_without_reconciliations_still_load() {
        let (c, _, _) = books();
        let mut json: serde_json::Value = serde_json::to_value(&c).unwrap();
        json.as_object_mut().unwrap().remove("reconciliations");
        let back: Company = serde_json::from_value(json).unwrap();
        assert!(back.reconciliations.is_empty());
    }
}
