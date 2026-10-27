use std::collections::BTreeMap;

use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{
    Account, AccountId, AccountType, Class, Company, TxnClass, TxnClassId, TxnId,
};

use crate::{Cell, Report, RowKind};

/// Net debit-positive activity per account for dates in `[from, to]`.
fn activity(company: &Company, from: Option<Date>, to: Option<Date>) -> BTreeMap<AccountId, Money> {
    let mut map: BTreeMap<AccountId, Money> = BTreeMap::new();
    for e in company.ledger_entries() {
        if from.is_some_and(|f| e.date < f) || to.is_some_and(|t| e.date > t) {
            continue;
        }
        let slot = map.entry(e.posting.account).or_insert(Money::ZERO);
        *slot = slot.saturating_add(e.posting.amount);
    }
    map
}

/// Converts a debit-positive amount to the account's natural sign.
fn natural(kind: AccountType, raw: Money) -> Money {
    if kind.is_debit_normal() {
        raw
    } else {
        raw.saturating_neg()
    }
}

fn sum(values: impl IntoIterator<Item = Money>) -> Money {
    values.into_iter().fold(Money::ZERO, Money::saturating_add)
}

/// Debit-positive activity per account, one amount per report column.
type Columns = BTreeMap<AccountId, Vec<Money>>;

fn add_into(total: &mut [Money], values: &[Money]) {
    for (t, v) in total.iter_mut().zip(values) {
        *t = t.saturating_add(*v);
    }
}

fn all_zero(values: &[Money]) -> bool {
    values.iter().all(|m| m.is_zero())
}

/// Adds a row per account of the given types with activity, sub-accounts
/// indented under their parents with a total for each parent, and returns
/// their natural-sign total.
fn account_rows(
    report: &mut Report,
    company: &Company,
    amounts: &BTreeMap<AccountId, Money>,
    kinds: &[AccountType],
    indent: u8,
) -> Money {
    let columns: Columns = amounts.iter().map(|(id, m)| (*id, vec![*m])).collect();
    account_rows_in(report, company, &columns, 1, kinds, indent)
        .first()
        .copied()
        .unwrap_or(Money::ZERO)
}

/// [`account_rows`] for `n` columns of amounts.
fn account_rows_in(
    report: &mut Report,
    company: &Company,
    amounts: &Columns,
    n: usize,
    kinds: &[AccountType],
    indent: u8,
) -> Vec<Money> {
    let mut total = vec![Money::ZERO; n];
    // Top level here: no parent, or a parent of another type (which a
    // damaged file could have).
    let top = company.sorted_accounts().into_iter().filter(|a| {
        kinds.contains(&a.kind)
            && a.parent
                .and_then(|p| company.account(p))
                .is_none_or(|p| !kinds.contains(&p.kind))
    });
    for a in top {
        let value = tree_rows(report, company, amounts, n, a, indent, 0);
        add_into(&mut total, &value);
    }
    total
}

/// The raw activity of an account and everything under it.
fn subtree_raw(
    company: &Company,
    amounts: &Columns,
    n: usize,
    a: &Account,
    depth: usize,
) -> Vec<Money> {
    let mut total = amounts
        .get(&a.id)
        .cloned()
        .unwrap_or_else(|| vec![Money::ZERO; n]);
    // Deeper than the chart is long means a loop in a damaged file.
    if depth > company.accounts.len() {
        return total;
    }
    for child in company.sub_accounts(a.id) {
        add_into(&mut total, &subtree_raw(company, amounts, n, child, depth + 1));
    }
    total
}

/// One account's rows: a plain line, or, when its sub-accounts have
/// activity, a heading, its own line, its sub-accounts and "Total …".
/// Returns the natural-sign total of the account and its sub-accounts.
fn tree_rows(
    report: &mut Report,
    company: &Company,
    amounts: &Columns,
    n: usize,
    a: &Account,
    indent: u8,
    depth: usize,
) -> Vec<Money> {
    let subtree = subtree_raw(company, amounts, n, a, depth);
    if all_zero(&subtree) {
        return vec![Money::ZERO; n];
    }
    let natural_all =
        |raw: &[Money]| -> Vec<Money> { raw.iter().map(|m| natural(a.kind, *m)).collect() };
    let own = amounts
        .get(&a.id)
        .cloned()
        .unwrap_or_else(|| vec![Money::ZERO; n]);
    let children: Vec<&Account> = if depth > company.accounts.len() {
        Vec::new()
    } else {
        company
            .sub_accounts(a.id)
            .into_iter()
            .filter(|c| !all_zero(&subtree_raw(company, amounts, n, c, depth + 1)))
            .collect()
    };
    let name = a.display_name();
    if children.is_empty() {
        let value = natural_all(&own);
        report.money(RowKind::Detail, &name, indent, &value, Some(a.id));
        return value;
    }
    report.section(&name, indent);
    let inner = indent.saturating_add(1);
    if !all_zero(&own) {
        report.money(RowKind::Detail, &name, inner, &natural_all(&own), Some(a.id));
    }
    for child in children {
        tree_rows(report, company, amounts, n, child, inner, depth + 1);
    }
    let value = natural_all(&subtree);
    report.money(
        RowKind::Subtotal,
        &format!("Total {name}"),
        indent,
        &value,
        None,
    );
    value
}

pub fn trial_balance(company: &Company, as_of: Date) -> Report {
    let mut r = Report::new(
        "Trial Balance",
        &company.info.name,
        format!("As of {as_of}"),
        &["Debit", "Credit"],
    );
    r.label_header = "Account".into();
    let amounts = activity(company, None, Some(as_of));
    let mut debits = Money::ZERO;
    let mut credits = Money::ZERO;
    for a in company.sorted_accounts() {
        let raw = amounts.get(&a.id).copied().unwrap_or(Money::ZERO);
        if raw.is_zero() {
            continue;
        }
        let (dr, cr) = if raw.is_positive() {
            debits = debits.saturating_add(raw);
            (Cell::Money(raw), Cell::Empty)
        } else {
            let c = raw.saturating_neg();
            credits = credits.saturating_add(c);
            (Cell::Empty, Cell::Money(c))
        };
        r.line(
            RowKind::Detail,
            &company.account_full_name(a.id),
            0,
            vec![dr, cr],
            Some(a.id),
        );
    }
    r.money(RowKind::Total, "Total", 0, &[debits, credits], None);
    r
}

pub fn profit_and_loss(company: &Company, from: Date, to: Date) -> Report {
    let mut r = Report::new(
        "Profit & Loss",
        &company.info.name,
        format!("{from} through {to}"),
        &["Amount"],
    );
    let amounts: Columns = activity(company, Some(from), Some(to))
        .into_iter()
        .map(|(id, m)| (id, vec![m]))
        .collect();
    profit_and_loss_rows(&mut r, company, &amounts, 1);
    r
}

/// Profit & Loss with a column per class (those used in the period), then
/// Unclassified and Total.
pub fn profit_and_loss_by_class(company: &Company, from: Date, to: Date) -> Report {
    let class_of: BTreeMap<TxnId, TxnClassId> = company
        .transactions
        .iter()
        .filter_map(|t| Some((t.id, t.class?)))
        .collect();
    let entries: Vec<_> = company
        .ledger_entries()
        .into_iter()
        .filter(|e| e.date >= from && e.date <= to)
        .collect();
    // Classes in the period, in list order; then Unclassified, then Total.
    let used: Vec<&TxnClass> = company
        .classes
        .iter()
        .filter(|k| entries.iter().any(|e| class_of.get(&e.txn) == Some(&k.id)))
        .collect();
    let mut headers: Vec<&str> = used.iter().map(|k| k.name.as_str()).collect();
    headers.push("Unclassified");
    headers.push("Total");
    let n = headers.len();
    let unclassified = n - 2;
    let mut amounts = Columns::new();
    for e in &entries {
        let col = class_of
            .get(&e.txn)
            .and_then(|k| used.iter().position(|u| u.id == *k))
            .unwrap_or(unclassified);
        let slot = amounts
            .entry(e.posting.account)
            .or_insert_with(|| vec![Money::ZERO; n]);
        for i in [col, n - 1] {
            if let Some(m) = slot.get_mut(i) {
                *m = m.saturating_add(e.posting.amount);
            }
        }
    }
    let mut r = Report::new(
        "Profit & Loss by Class",
        &company.info.name,
        format!("{from} through {to}"),
        &headers,
    );
    profit_and_loss_rows(&mut r, company, &amounts, n);
    r
}

fn minus(a: &[Money], b: &[Money]) -> Vec<Money> {
    a.iter().zip(b).map(|(x, y)| x.saturating_sub(*y)).collect()
}

/// The sections of a Profit & Loss, over `n` columns.
fn profit_and_loss_rows(r: &mut Report, company: &Company, amounts: &Columns, n: usize) {
    r.section("Income", 0);
    let income = account_rows_in(r, company, amounts, n, &[AccountType::Income], 1);
    r.money(RowKind::Subtotal, "Total Income", 0, &income, None);

    r.section("Cost of Goods Sold", 0);
    let cogs = account_rows_in(r, company, amounts, n, &[AccountType::CostOfGoodsSold], 1);
    r.money(RowKind::Subtotal, "Total COGS", 0, &cogs, None);
    let gross = minus(&income, &cogs);
    r.money(RowKind::Subtotal, "Gross Profit", 0, &gross, None);

    r.section("Expenses", 0);
    let expenses = account_rows_in(r, company, amounts, n, &[AccountType::Expense], 1);
    r.money(RowKind::Subtotal, "Total Expenses", 0, &expenses, None);
    let operating = minus(&gross, &expenses);
    r.money(RowKind::Subtotal, "Net Operating Income", 0, &operating, None);

    r.section("Other Income and Expense", 0);
    let other_income = account_rows_in(r, company, amounts, n, &[AccountType::OtherIncome], 1);
    let other_expense = account_rows_in(r, company, amounts, n, &[AccountType::OtherExpense], 1);
    let net_other = minus(&other_income, &other_expense);
    r.money(RowKind::Subtotal, "Net Other Income", 0, &net_other, None);

    let net: Vec<Money> = operating
        .iter()
        .zip(&net_other)
        .map(|(a, b)| a.saturating_add(*b))
        .collect();
    r.money(RowKind::Total, "Net Income", 0, &net, None);
}

/// Net income (income less expenses) from the given activity.
fn net_income(company: &Company, amounts: &BTreeMap<AccountId, Money>) -> Money {
    sum(company.accounts.iter().filter_map(|a| {
        let raw = amounts.get(&a.id).copied()?;
        match a.kind.class() {
            // Income is credit-normal, so its raw (debit-positive) value is negative.
            Class::Income | Class::Expense => Some(raw.saturating_neg()),
            _ => None,
        }
    }))
}

pub fn balance_sheet(company: &Company, as_of: Date) -> Report {
    let mut r = Report::new(
        "Balance Sheet",
        &company.info.name,
        format!("As of {as_of}"),
        &["Amount"],
    );
    let amounts = activity(company, None, Some(as_of));
    let fy_start = as_of.fiscal_year_start(company.info.fiscal_year_start_month);
    let prior = activity(company, None, Some(fy_start.add_days(-1)));
    let current = activity(company, Some(fy_start), Some(as_of));
    let retained = net_income(company, &prior);
    let this_year = net_income(company, &current);

    use AccountType::*;
    r.section("Assets", 0);
    r.section("Current Assets", 1);
    let current_assets = account_rows(
        &mut r,
        company,
        &amounts,
        &[Bank, AccountsReceivable, OtherCurrentAsset],
        2,
    );
    r.money(
        RowKind::Subtotal,
        "Total Current Assets",
        1,
        &[current_assets],
        None,
    );
    let fixed = account_rows(&mut r, company, &amounts, &[FixedAsset], 1);
    let other_assets = account_rows(&mut r, company, &amounts, &[OtherAsset], 1);
    let assets = sum([current_assets, fixed, other_assets]);
    r.money(RowKind::Total, "Total Assets", 0, &[assets], None);

    r.section("Liabilities and Equity", 0);
    r.section("Liabilities", 1);
    let current_liab = account_rows(
        &mut r,
        company,
        &amounts,
        &[AccountsPayable, CreditCard, OtherCurrentLiability],
        2,
    );
    let long_term = account_rows(&mut r, company, &amounts, &[LongTermLiability], 2);
    let liabilities = current_liab.saturating_add(long_term);
    r.money(
        RowKind::Subtotal,
        "Total Liabilities",
        1,
        &[liabilities],
        None,
    );

    r.section("Equity", 1);
    // Earlier years' profit folds into Retained Earnings, as on paper books.
    let re_account: Option<&Account> = company
        .accounts
        .iter()
        .find(|a| a.kind == Equity && a.name.eq_ignore_ascii_case("Retained Earnings"));
    let mut equity = Money::ZERO;
    for a in company
        .sorted_accounts()
        .into_iter()
        .filter(|a| a.kind == Equity)
    {
        let mut value = natural(a.kind, amounts.get(&a.id).copied().unwrap_or(Money::ZERO));
        if Some(a.id) == re_account.map(|x| x.id) {
            value = value.saturating_add(retained);
        }
        if value.is_zero() {
            continue;
        }
        equity = equity.saturating_add(value);
        r.money(RowKind::Detail, &a.display_name(), 2, &[value], Some(a.id));
    }
    if re_account.is_none() && !retained.is_zero() {
        equity = equity.saturating_add(retained);
        r.money(RowKind::Detail, "Retained Earnings", 2, &[retained], None);
    }
    equity = equity.saturating_add(this_year);
    r.money(RowKind::Detail, "Net Income", 2, &[this_year], None);
    r.money(RowKind::Subtotal, "Total Equity", 1, &[equity], None);
    r.money(
        RowKind::Total,
        "Total Liabilities and Equity",
        0,
        &[liabilities.saturating_add(equity)],
        None,
    );
    r
}

/// Every posting to one account with a running balance (natural sign).
pub fn account_register(company: &Company, account: AccountId) -> Report {
    let name = company.account_name(account);
    let mut r = Report::new(
        &format!("Register: {name}"),
        &company.info.name,
        "All dates".into(),
        &["Type", "Num", "Name", "Memo", "Amount", "Balance", "Rec"],
    );
    r.label_header = "Date".into();
    let Some(acct) = company.account(account) else {
        return r;
    };
    let mut balance = Money::ZERO;
    for e in company
        .ledger_entries()
        .into_iter()
        .filter(|e| e.posting.account == account)
    {
        let Some(t) = company.txn(e.txn) else {
            continue;
        };
        let amount = natural(acct.kind, e.posting.amount);
        balance = balance.saturating_add(amount);
        let party = e
            .posting
            .party
            .map(|p| company.party_name(p))
            .unwrap_or_default();
        let memo = if e.posting.memo.is_empty() {
            t.memo.clone()
        } else {
            e.posting.memo.clone()
        };
        r.txn_line(
            &e.date.to_string(),
            vec![
                Cell::Text(company.txn_type_label(t).into()),
                Cell::Text(t.number.clone()),
                Cell::Text(party),
                Cell::Text(memo),
                Cell::Money(amount),
                Cell::Money(balance),
                // R: reconciled against a statement.
                Cell::Text(if company.is_reconciled(t.id, account) {
                    "R".into()
                } else {
                    String::new()
                }),
            ],
            t.id,
        );
    }
    r
}

/// All live transactions in a date range.
pub fn transaction_list(company: &Company, from: Date, to: Date) -> Report {
    let mut r = Report::new(
        "Transaction List by Date",
        &company.info.name,
        format!("{from} through {to}"),
        &["Type", "Num", "Name", "Memo", "Amount"],
    );
    r.label_header = "Date".into();
    let mut txns: Vec<_> = company
        .transactions
        .iter()
        .filter(|t| !t.voided && t.date >= from && t.date <= to)
        .collect();
    txns.sort_by_key(|t| (t.date, t.id));
    for t in txns {
        let party = t
            .body
            .party()
            .map(|p| company.party_name(p))
            .unwrap_or_default();
        r.txn_line(
            &t.date.to_string(),
            vec![
                Cell::Text(company.txn_type_label(t).into()),
                Cell::Text(t.number.clone()),
                Cell::Text(party),
                Cell::Text(t.memo.clone()),
                t.body.total().map_or(Cell::Empty, Cell::Money),
            ],
            t.id,
        );
    }
    r
}

/// One reconciliation of a bank or credit card account (`index`, oldest
/// first): the beginning balance, the items it cleared, the cleared balance
/// that matched the statement, and what was still uncleared on that date.
pub fn reconciliation_report(company: &Company, account: AccountId, index: usize) -> Report {
    let name = company.account_name(account);
    let recs = company.reconciliations_for(account);
    let Some(rec) = recs.get(index) else {
        return Report::new(
            &format!("Reconciliation: {name}"),
            &company.info.name,
            "Not found".into(),
            &["Type", "Num", "Name", "Amount"],
        );
    };
    let mut r = Report::new(
        &format!("Reconciliation: {name}"),
        &company.info.name,
        format!("Statement ending {}", rec.statement_date),
        &["Type", "Num", "Name", "Amount"],
    );
    r.label_header = "Date".into();
    let row = |r: &mut Report, id: ledgercraft_ledger::TxnId, amount: Money| {
        let Some(t) = company.txn(id) else {
            return;
        };
        let party = t
            .body
            .party()
            .map(|p| company.party_name(p))
            .unwrap_or_default();
        r.txn_line(
            &t.date.to_string(),
            vec![
                Cell::Text(company.txn_type_label(t).into()),
                Cell::Text(t.number.clone()),
                Cell::Text(party),
                Cell::Money(amount),
            ],
            id,
        );
    };
    // Totals go in the Amount column (the fourth), like the items above them.
    let total = |r: &mut Report, kind: RowKind, label: &str, m: Money| {
        r.line(
            kind,
            label,
            0,
            vec![Cell::Empty, Cell::Empty, Cell::Empty, Cell::Money(m)],
            None,
        );
    };
    let amount_of = |id: ledgercraft_ledger::TxnId| {
        company
            .txn(id)
            .and_then(|t| company.amount_in(&t.body, account))
            .unwrap_or(Money::ZERO)
    };
    total(
        &mut r,
        RowKind::Detail,
        "Beginning balance",
        rec.beginning_balance,
    );
    let mut cleared: Vec<_> = rec.cleared.iter().map(|id| (*id, amount_of(*id))).collect();
    cleared.sort_by_key(|(id, _)| (company.txn(*id).map(|t| t.date), *id));
    for (title, increases) in [("Cleared increases", true), ("Cleared decreases", false)] {
        let items: Vec<_> = cleared
            .iter()
            .filter(|(_, a)| a.is_positive() == increases && !a.is_zero())
            .collect();
        if items.is_empty() {
            continue;
        }
        r.section(title, 0);
        for (id, amount) in &items {
            row(&mut r, *id, *amount);
        }
        let subtotal = sum(items.iter().map(|(_, a)| *a));
        total(
            &mut r,
            RowKind::Subtotal,
            &format!("Total {}", title.to_lowercase()),
            subtotal,
        );
    }
    total(
        &mut r,
        RowKind::Total,
        "Cleared balance",
        rec.ending_balance,
    );
    total(
        &mut r,
        RowKind::Detail,
        "Statement ending balance",
        rec.ending_balance,
    );
    // What had not cleared by the statement date (and is still not reconciled).
    let uncleared = company.unreconciled(account, rec.statement_date);
    if !uncleared.is_empty() {
        r.section(&format!("Uncleared as of {}", rec.statement_date), 0);
        for item in &uncleared {
            row(&mut r, item.txn, item.amount);
        }
        let uncleared_total = sum(uncleared.iter().map(|i| i.amount));
        total(
            &mut r,
            RowKind::Subtotal,
            "Total uncleared",
            uncleared_total,
        );
    }
    r
}
