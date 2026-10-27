//! Merging duplicate list entries, and sub-accounts.
//!
//! Merging moves everything that names one customer, vendor, item or account
//! onto another, then removes the first: "Acme Inc" and "ACME, Inc." become
//! one customer with both histories. Amounts never change.

use crate::{
    Account, AccountId, Company, CustomerId, InvoiceTax, ItemId, LedgerError, LedgerResult, Party,
    SalesLine, TxnBody, VendorId,
};

/// The accounts in a sale's lines and tax.
fn sale_accounts<'a>(
    out: &mut Vec<&'a mut AccountId>,
    lines: &'a mut [SalesLine],
    tax: &'a mut Option<InvoiceTax>,
) {
    out.extend(lines.iter_mut().map(|l| &mut l.account));
    if let Some(t) = tax {
        out.push(&mut t.account);
    }
}

/// Puts a party's id with the customers or the vendors.
fn split_party<'a>(
    p: &'a mut Party,
    customers: &mut Vec<&'a mut CustomerId>,
    vendors: &mut Vec<&'a mut VendorId>,
) {
    match p {
        Party::Customer(id) => customers.push(id),
        Party::Vendor(id) => vendors.push(id),
    }
}

impl TxnBody {
    /// Every account the transaction names.
    pub(crate) fn accounts_mut(&mut self) -> Vec<&mut AccountId> {
        let mut out: Vec<&mut AccountId> = Vec::new();
        match self {
            Self::Journal(j) => out.extend(j.lines.iter_mut().map(|l| &mut l.account)),
            Self::Invoice(i) => {
                out.push(&mut i.ar_account);
                sale_accounts(&mut out, &mut i.lines, &mut i.tax);
            }
            Self::SalesReceipt(r) => {
                out.push(&mut r.deposit_to);
                sale_accounts(&mut out, &mut r.lines, &mut r.tax);
            }
            Self::CreditMemo(m) => {
                out.push(&mut m.ar_account);
                sale_accounts(&mut out, &mut m.lines, &mut m.tax);
            }
            Self::ReceivePayment(p) => {
                out.push(&mut p.deposit_to);
                out.push(&mut p.ar_account);
            }
            Self::Refund(r) => {
                out.push(&mut r.pay_from);
                out.push(&mut r.ar_account);
            }
            Self::Bill(b) => {
                out.push(&mut b.ap_account);
                out.extend(b.lines.iter_mut().map(|l| &mut l.account));
            }
            Self::PayBill(p) => {
                out.push(&mut p.pay_from);
                out.push(&mut p.ap_account);
            }
            Self::Check(c) => {
                out.push(&mut c.pay_from);
                out.extend(c.lines.iter_mut().map(|l| &mut l.account));
            }
            Self::Deposit(d) => {
                out.push(&mut d.deposit_to);
                out.extend(d.lines.iter_mut().map(|l| &mut l.account));
            }
        }
        out
    }

    /// Every customer or vendor the transaction names.
    pub(crate) fn parties_mut(&mut self) -> (Vec<&mut CustomerId>, Vec<&mut VendorId>) {
        let mut customers: Vec<&mut CustomerId> = Vec::new();
        let mut vendors: Vec<&mut VendorId> = Vec::new();
        match self {
            Self::Journal(j) => {
                for l in &mut j.lines {
                    if let Some(p) = &mut l.party {
                        split_party(p, &mut customers, &mut vendors);
                    }
                }
            }
            Self::Check(c) => {
                if let Some(p) = &mut c.payee {
                    split_party(p, &mut customers, &mut vendors);
                }
            }
            Self::Invoice(i) => customers.push(&mut i.customer),
            Self::SalesReceipt(r) => {
                if let Some(id) = &mut r.customer {
                    customers.push(id);
                }
            }
            Self::CreditMemo(m) => customers.push(&mut m.customer),
            Self::ReceivePayment(p) => customers.push(&mut p.customer),
            Self::Refund(r) => customers.push(&mut r.customer),
            Self::Bill(b) => vendors.push(&mut b.vendor),
            Self::PayBill(p) => vendors.push(&mut p.vendor),
            Self::Deposit(_) => {}
        }
        (customers, vendors)
    }

    /// Every item on the transaction's lines.
    pub(crate) fn items_mut(&mut self) -> Vec<&mut ItemId> {
        let lines: &mut [SalesLine] = match self {
            Self::Invoice(i) => &mut i.lines,
            Self::SalesReceipt(r) => &mut r.lines,
            Self::CreditMemo(m) => &mut m.lines,
            _ => &mut [],
        };
        lines.iter_mut().filter_map(|l| l.item.as_mut()).collect()
    }
}

impl Company {
    /// Every transaction body on file: the transactions, and the ones
    /// recurring transactions enter.
    fn bodies_mut(&mut self) -> impl Iterator<Item = &mut TxnBody> {
        self.transactions
            .iter_mut()
            .map(|t| &mut t.body)
            .chain(self.recurring.iter_mut().map(|r| &mut r.template.body))
    }

    /// Moves everything of customer `from` onto customer `into`, then
    /// removes `from`. `into` keeps its own details.
    pub fn merge_customers(&mut self, from: CustomerId, into: CustomerId) -> LedgerResult<()> {
        if from == into {
            return Err(LedgerError::Invalid("Choose a different customer to merge into".into()));
        }
        for id in [from, into] {
            self.customer(id)
                .ok_or_else(|| LedgerError::NotFound(format!("Customer {id}")))?;
        }
        for body in self.bodies_mut() {
            for id in body.parties_mut().0 {
                if *id == from {
                    *id = into;
                }
            }
        }
        self.customers.retain(|x| x.id != from);
        Ok(())
    }

    /// Moves everything of vendor `from` onto vendor `into`, then removes `from`.
    pub fn merge_vendors(&mut self, from: VendorId, into: VendorId) -> LedgerResult<()> {
        if from == into {
            return Err(LedgerError::Invalid("Choose a different vendor to merge into".into()));
        }
        for id in [from, into] {
            self.vendor(id)
                .ok_or_else(|| LedgerError::NotFound(format!("Vendor {id}")))?;
        }
        for body in self.bodies_mut() {
            for id in body.parties_mut().1 {
                if *id == from {
                    *id = into;
                }
            }
        }
        self.vendors.retain(|x| x.id != from);
        Ok(())
    }

    /// Moves every line with item `from` onto item `into`, then removes
    /// `from`. The lines keep their own descriptions, prices and accounts.
    pub fn merge_items(&mut self, from: ItemId, into: ItemId) -> LedgerResult<()> {
        if from == into {
            return Err(LedgerError::Invalid("Choose a different item to merge into".into()));
        }
        for id in [from, into] {
            self.item(id)
                .ok_or_else(|| LedgerError::NotFound(format!("Item {id}")))?;
        }
        for body in self.bodies_mut() {
            for id in body.items_mut() {
                if *id == from {
                    *id = into;
                }
            }
        }
        self.items.retain(|x| x.id != from);
        Ok(())
    }

    /// Moves everything posted to account `from` onto account `into` (of the
    /// same type), with its items, bank rules, downloads and sub-accounts,
    /// then removes `from`. An account that has been reconciled cannot be
    /// merged away: its reconciliation reports would no longer add up.
    pub fn merge_accounts(&mut self, from: AccountId, into: AccountId) -> LedgerResult<()> {
        if from == into {
            return Err(LedgerError::Invalid("Choose a different account to merge into".into()));
        }
        let kind_of = |c: &Company, id: AccountId| {
            c.account(id)
                .map(|a| a.kind)
                .ok_or_else(|| LedgerError::NotFound(format!("Account {id}")))
        };
        if kind_of(self, from)? != kind_of(self, into)? {
            return Err(LedgerError::Invalid(
                "Accounts can only be merged into an account of the same type".into(),
            ));
        }
        if !self.reconciliations_for(from).is_empty() {
            return Err(LedgerError::InUse(
                "This account has been reconciled, so it cannot be merged away; merge the \
                 other account into it instead"
                    .into(),
            ));
        }
        if self.descends_from(into, from) {
            return Err(LedgerError::Invalid(
                "An account cannot be merged into one of its own sub-accounts".into(),
            ));
        }
        let swap = |id: &mut AccountId| {
            if *id == from {
                *id = into;
            }
        };
        for body in self.bodies_mut() {
            body.accounts_mut().into_iter().for_each(swap);
        }
        for item in &mut self.items {
            swap(&mut item.spec.income_account);
        }
        for rule in &mut self.bank_rules {
            swap(&mut rule.category);
        }
        for link in &mut self.feed_links {
            swap(&mut link.account);
        }
        for a in &mut self.accounts {
            if a.parent == Some(from) {
                a.parent = Some(into);
            }
        }
        self.accounts.retain(|a| a.id != from);
        Ok(())
    }

    // ----- sub-accounts ------------------------------------------------------

    /// Whether `id` is `ancestor` or sits somewhere under it.
    pub fn descends_from(&self, id: AccountId, ancestor: AccountId) -> bool {
        let mut at = Some(id);
        // The chart cannot be deeper than it has accounts.
        for _ in 0..=self.accounts.len() {
            match at {
                Some(a) if a == ancestor => return true,
                Some(a) => at = self.account(a).and_then(|x| x.parent),
                None => return false,
            }
        }
        false
    }

    /// Makes `id` a sub-account of `parent` (of the same type), or a
    /// top-level account with `None`.
    pub fn set_account_parent(
        &mut self,
        id: AccountId,
        parent: Option<AccountId>,
    ) -> LedgerResult<()> {
        let kind = self
            .account(id)
            .map(|a| a.kind)
            .ok_or_else(|| LedgerError::NotFound(format!("Account {id}")))?;
        if let Some(p) = parent {
            let parent_kind = self
                .account(p)
                .map(|a| a.kind)
                .ok_or_else(|| LedgerError::NotFound(format!("Account {p}")))?;
            if parent_kind != kind {
                return Err(LedgerError::Invalid(
                    "A sub-account must be of the same type as its parent".into(),
                ));
            }
            if self.descends_from(p, id) {
                return Err(LedgerError::Invalid(
                    "An account cannot be a sub-account of itself or of its own sub-accounts"
                        .into(),
                ));
            }
        }
        if let Some(a) = self.accounts.iter_mut().find(|a| a.id == id) {
            a.parent = parent;
        }
        Ok(())
    }

    /// An account's direct sub-accounts, in chart order.
    pub fn sub_accounts(&self, id: AccountId) -> Vec<&Account> {
        self.sorted_accounts()
            .into_iter()
            .filter(|a| a.parent == Some(id))
            .collect()
    }

    /// Every account in chart order with sub-accounts under their parents,
    /// and how deep each is (0 for a top-level account).
    pub fn accounts_in_tree_order(&self) -> Vec<(&Account, usize)> {
        let mut out = Vec::new();
        let top = self
            .sorted_accounts()
            .into_iter()
            .filter(|a| a.parent.is_none_or(|p| self.account(p).is_none()));
        for a in top {
            self.push_subtree(a, 0, &mut out);
        }
        out
    }

    fn push_subtree<'a>(&'a self, a: &'a Account, depth: usize, out: &mut Vec<(&'a Account, usize)>) {
        // A damaged file with a loop is shown once, not forever.
        if out.iter().any(|(seen, _)| seen.id == a.id) {
            return;
        }
        out.push((a, depth));
        for child in self.sub_accounts(a.id) {
            self.push_subtree(child, depth + 1, out);
        }
    }

    /// An account's name with its parents': `6500 · Utilities:Electric`.
    pub fn account_full_name(&self, id: AccountId) -> String {
        let Some(account) = self.account(id) else {
            return id.to_string();
        };
        let mut names = vec![account.name.clone()];
        let mut at = account.parent;
        for _ in 0..self.accounts.len() {
            let Some(parent) = at.and_then(|p| self.account(p)) else {
                break;
            };
            names.push(parent.name.clone());
            at = parent.parent;
        }
        names.reverse();
        let path = names.join(":");
        if account.number.is_empty() {
            path
        } else {
            format!("{} · {path}", account.number)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountLine, AccountType, Check, Contact, Date, Money};

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn acct(c: &Company, name: &str) -> AccountId {
        c.account_by_name(name).unwrap().id
    }

    #[test]
    fn merging_vendors_moves_their_history() {
        let mut c = Company::with_default_accounts("Merge Co");
        let contact = |name: &str| Contact {
            name: name.into(),
            ..Contact::default()
        };
        let a = c.add_vendor(contact("Canyon Lumber")).unwrap();
        let b = c.add_vendor(contact("Canyon Lumber Supply")).unwrap();
        let check = |vendor| {
            TxnBody::Check(Check {
                payee: Some(Party::Vendor(vendor)),
                pay_from: acct(&c, "Checking"),
                lines: vec![AccountLine {
                    account: acct(&c, "Office Supplies"),
                    description: String::new(),
                    amount: Money::from_dollars(10),
                }],
            })
        };
        let first = check(a);
        let second = check(b);
        let scheduled = check(a);
        c.post(d("2026-01-02"), "1", "", first).unwrap();
        c.post(d("2026-01-03"), "2", "", second).unwrap();
        let recurring = c
            .add_recurring(
                crate::RecurringSpec {
                    name: "Lumber".into(),
                    frequency: crate::Frequency::Monthly,
                    every: 1,
                    next: d("2026-02-02"),
                    end: None,
                    mode: crate::RecurringMode::Remind,
                },
                crate::TxnTemplate {
                    date: d("2026-01-02"),
                    memo: String::new(),
                    body: scheduled,
                    class: None,
                },
            )
            .unwrap();
        c.merge_vendors(a, b).unwrap();
        assert!(c.vendor(a).is_none());
        // Recurring transactions move too.
        assert_eq!(
            c.recurring(recurring).unwrap().template.body.party(),
            Some(Party::Vendor(b))
        );
        assert!(c
            .transactions
            .iter()
            .all(|t| t.body.party() == Some(Party::Vendor(b))));
        assert!(c.merge_vendors(b, b).is_err());
        assert!(c.merge_vendors(a, b).is_err(), "already gone");
    }

    #[test]
    fn merging_accounts_keeps_the_balance() {
        let mut c = Company::with_default_accounts("Merge Co");
        let supplies = acct(&c, "Office Supplies");
        let postage = c
            .add_account("6210", "Postage", AccountType::Expense, "")
            .unwrap();
        let line = |account| AccountLine {
            account,
            description: String::new(),
            amount: Money::from_dollars(25),
        };
        let check = TxnBody::Check(Check {
            payee: None,
            pay_from: acct(&c, "Checking"),
            lines: vec![line(supplies), line(postage)],
        });
        c.post(d("2026-01-02"), "1", "", check).unwrap();
        // Different types cannot be merged.
        assert!(c.merge_accounts(postage, acct(&c, "Checking")).is_err());
        c.merge_accounts(postage, supplies).unwrap();
        assert!(c.account(postage).is_none());
        assert_eq!(c.account_balance(supplies, None), Money::from_dollars(50));
        // A reconciled account cannot be merged away.
        let checking = acct(&c, "Checking");
        let savings = acct(&c, "Savings");
        let items = c.unreconciled(checking, d("2026-12-31"));
        let ending = items.iter().fold(Money::ZERO, |t, i| t.saturating_add(i.amount));
        c.reconcile(checking, d("2026-01-31"), ending, items.iter().map(|i| i.txn).collect())
            .unwrap();
        assert!(c.merge_accounts(checking, savings).is_err());
        assert!(c.merge_accounts(savings, checking).is_ok());
    }

    #[test]
    fn sub_accounts_nest_without_loops() {
        let mut c = Company::with_default_accounts("Tree Co");
        let utilities = acct(&c, "Utilities");
        let electric = c
            .add_account("6510", "Electric", AccountType::Expense, "")
            .unwrap();
        let meter = c
            .add_account("6511", "Shop meter", AccountType::Expense, "")
            .unwrap();
        c.set_account_parent(electric, Some(utilities)).unwrap();
        c.set_account_parent(meter, Some(electric)).unwrap();
        assert_eq!(c.account_full_name(meter), "6511 · Utilities:Electric:Shop meter");
        // No loops, and no mixing of types.
        assert!(c.set_account_parent(utilities, Some(meter)).is_err());
        assert!(c.set_account_parent(electric, Some(electric)).is_err());
        assert!(c.set_account_parent(electric, Some(acct(&c, "Checking"))).is_err());
        let order: Vec<(String, usize)> = c
            .accounts_in_tree_order()
            .into_iter()
            .filter(|(a, _)| [utilities, electric, meter].contains(&a.id))
            .map(|(a, depth)| (a.name.clone(), depth))
            .collect();
        assert_eq!(
            order,
            vec![
                ("Utilities".to_string(), 0),
                ("Electric".to_string(), 1),
                ("Shop meter".to_string(), 2)
            ]
        );
        // Merging a parent away moves its sub-accounts up to the survivor.
        let rent = acct(&c, "Rent");
        c.merge_accounts(utilities, rent).unwrap();
        assert_eq!(c.account(electric).unwrap().parent, Some(rent));
    }
}
