use std::collections::BTreeMap;

use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{Account, AccountId, AccountType, Class, Company};

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

/// Adds one detail row per account of the given types with activity, and
/// returns their natural-sign total.
fn account_rows(
    report: &mut Report,
    company: &Company,
    amounts: &BTreeMap<AccountId, Money>,
    kinds: &[AccountType],
    indent: u8,
) -> Money {
    let mut total = Money::ZERO;
    for a in company.sorted_accounts() {
        if !kinds.contains(&a.kind) {
            continue;
        }
        let raw = amounts.get(&a.id).copied().unwrap_or(Money::ZERO);
        if raw.is_zero() {
            continue;
        }
        let value = natural(a.kind, raw);
        total = total.saturating_add(value);
        report.money(
            RowKind::Detail,
            &a.display_name(),
            indent,
            &[value],
            Some(a.id),
        );
    }
    total
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
            &a.display_name(),
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
    let amounts = activity(company, Some(from), Some(to));

    r.section("Income", 0);
    let income = account_rows(&mut r, company, &amounts, &[AccountType::Income], 1);
    r.money(RowKind::Subtotal, "Total Income", 0, &[income], None);

    r.section("Cost of Goods Sold", 0);
    let cogs = account_rows(
        &mut r,
        company,
        &amounts,
        &[AccountType::CostOfGoodsSold],
        1,
    );
    r.money(RowKind::Subtotal, "Total COGS", 0, &[cogs], None);
    let gross = income.saturating_sub(cogs);
    r.money(RowKind::Subtotal, "Gross Profit", 0, &[gross], None);

    r.section("Expenses", 0);
    let expenses = account_rows(&mut r, company, &amounts, &[AccountType::Expense], 1);
    r.money(RowKind::Subtotal, "Total Expenses", 0, &[expenses], None);
    let operating = gross.saturating_sub(expenses);
    r.money(
        RowKind::Subtotal,
        "Net Operating Income",
        0,
        &[operating],
        None,
    );

    r.section("Other Income and Expense", 0);
    let other_income = account_rows(&mut r, company, &amounts, &[AccountType::OtherIncome], 1);
    let other_expense = account_rows(&mut r, company, &amounts, &[AccountType::OtherExpense], 1);
    let net_other = other_income.saturating_sub(other_expense);
    r.money(RowKind::Subtotal, "Net Other Income", 0, &[net_other], None);

    r.money(
        RowKind::Total,
        "Net Income",
        0,
        &[operating.saturating_add(net_other)],
        None,
    );
    r
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
    let amount_of = |id: ledgercraft_ledger::TxnId| {
        company
            .txn(id)
            .and_then(|t| company.amount_in(&t.body, account))
            .unwrap_or(Money::ZERO)
    };
    r.money(RowKind::Detail, "Beginning balance", 0, &[rec.beginning_balance], None);
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
        let total = sum(items.iter().map(|(_, a)| *a));
        r.money(RowKind::Subtotal, &format!("Total {}", title.to_lowercase()), 0, &[total], None);
    }
    r.money(RowKind::Total, "Cleared balance", 0, &[rec.ending_balance], None);
    r.money(RowKind::Detail, "Statement ending balance", 0, &[rec.ending_balance], None);
    // What had not cleared by the statement date (and is still not reconciled).
    let uncleared = company.unreconciled(account, rec.statement_date);
    if !uncleared.is_empty() {
        r.section(&format!("Uncleared as of {}", rec.statement_date), 0);
        for item in &uncleared {
            row(&mut r, item.txn, item.amount);
        }
        let total = sum(uncleared.iter().map(|i| i.amount));
        r.money(RowKind::Subtotal, "Total uncleared", 0, &[total], None);
    }
    r
}
