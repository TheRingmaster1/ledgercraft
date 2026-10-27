#![cfg(test)]

use ledgercraft_core::{Date, Money, Qty};
use ledgercraft_ledger::{AccountLine, Bill, Check, Company, Contact, Invoice, SalesLine, TxnBody};
use ledgercraft_reports::{
    ap_aging, ar_aging, balance_sheet, profit_and_loss, trial_balance, Cell, RowKind,
};

fn d(s: &str) -> Date {
    Date::parse(s).unwrap()
}

fn books() -> Company {
    let mut c = Company::with_default_accounts("Report Co");
    let id = |c: &Company, n: &str| c.account_by_name(n).unwrap().id;
    let cust = c
        .add_customer(Contact {
            name: "Zed".into(),
            ..Contact::default()
        })
        .unwrap();
    let vend = c
        .add_vendor(Contact {
            name: "Landlord".into(),
            ..Contact::default()
        })
        .unwrap();
    // Last year: 1,000 of income.
    let inv = TxnBody::Invoice(Invoice {
        customer: cust,
        due_date: d("2025-07-01"),
        ar_account: id(&c, "Accounts Receivable"),
        ship_to: None,
        tax: None,
        lines: vec![SalesLine {
            item: None,
            description: String::new(),
            qty: Qty::ONE,
            rate: Money::from_dollars(1000),
            taxable: true,
            account: id(&c, "Service Income"),
        }],
    });
    c.post(d("2025-06-01"), "1", "", inv).unwrap();
    // This year: 300 rent by check, 200 bill unpaid.
    let check = TxnBody::Check(Check {
        payee: None,
        pay_from: id(&c, "Checking"),
        lines: vec![AccountLine {
            account: id(&c, "Rent"),
            description: String::new(),
            amount: Money::from_dollars(300),
        }],
    });
    c.post(d("2026-02-01"), "101", "", check).unwrap();
    let bill = TxnBody::Bill(Bill {
        vendor: vend,
        due_date: d("2026-03-01"),
        ap_account: id(&c, "Accounts Payable"),
        lines: vec![AccountLine {
            account: id(&c, "Utilities"),
            description: String::new(),
            amount: Money::from_dollars(200),
        }],
    });
    c.post(d("2026-02-15"), "", "", bill).unwrap();
    c
}

fn last_money(r: &ledgercraft_reports::Report, label: &str) -> Money {
    let row = r.rows.iter().rev().find(|row| row.label == label).unwrap();
    match row.cells.last() {
        Some(Cell::Money(m)) => *m,
        _ => Money::ZERO,
    }
}

#[test]
fn trial_balance_balances() {
    let r = trial_balance(&books(), d("2026-12-31"));
    let total = r.rows.last().unwrap();
    assert_eq!(total.kind, RowKind::Total);
    assert_eq!(total.cells[0], total.cells[1]);
    assert_eq!(total.cells[0], Cell::Money(Money::from_dollars(1500)));
}

#[test]
fn profit_and_loss_for_the_year() {
    let r = profit_and_loss(&books(), d("2026-01-01"), d("2026-12-31"));
    assert_eq!(last_money(&r, "Total Income"), Money::ZERO);
    assert_eq!(last_money(&r, "Total Expenses"), Money::from_dollars(500));
    assert_eq!(last_money(&r, "Net Income"), Money::from_dollars(-500));
}

#[test]
fn balance_sheet_balances_with_retained_earnings() {
    let r = balance_sheet(&books(), d("2026-12-31"));
    // Assets: AR 1000 − checking 300 = 700. Liabilities: AP 200.
    assert_eq!(last_money(&r, "Total Assets"), Money::from_dollars(700));
    assert_eq!(
        last_money(&r, "Total Liabilities"),
        Money::from_dollars(200)
    );
    assert_eq!(
        last_money(&r, "3900 · Retained Earnings"),
        Money::from_dollars(1000)
    );
    assert_eq!(last_money(&r, "Net Income"), Money::from_dollars(-500));
    assert_eq!(
        last_money(&r, "Total Liabilities and Equity"),
        Money::from_dollars(700)
    );
}

#[test]
fn aging_buckets() {
    let c = books();
    let ar = ar_aging(&c, d("2025-09-15"));
    assert_eq!(ar.rows[0].label, "Zed");
    assert_eq!(ar.rows[0].cells[3], Cell::Money(Money::from_dollars(1000))); // 76 days → 61-90
    let ap = ap_aging(&c, d("2026-02-20"));
    assert_eq!(ap.rows[0].cells[0], Cell::Money(Money::from_dollars(200))); // not yet due
    let csv = ap.to_csv();
    assert!(csv.starts_with("Vendor,Current,"));
    assert!(csv.contains("Landlord,200.00,"));
}

#[test]
fn register_and_transaction_list_rows_point_at_their_transactions() {
    let c = books();
    let checking = c.account_by_name("Checking").unwrap().id;
    let register = ledgercraft_reports::account_register(&c, checking);
    let detail: Vec<_> = register
        .rows
        .iter()
        .filter(|r| r.kind == RowKind::Detail)
        .collect();
    assert!(!detail.is_empty());
    assert!(detail.iter().all(|r| r.txn.is_some()));
    let list = ledgercraft_reports::transaction_list(&c, d("2025-01-01"), d("2026-12-31"));
    let ids: Vec<_> = list.rows.iter().filter_map(|r| r.txn).collect();
    assert_eq!(ids.len(), c.transactions.len());
    assert!(ids.iter().all(|id| c.txn(*id).is_some()));
}

/// Sub-accounts sit under their parent, which gets a total line.
#[test]
fn sub_accounts_are_totalled_under_their_parent() {
    let mut c = books();
    let rent = c.account_by_name("Rent").unwrap().id;
    let storage = c
        .add_account(
            "6310",
            "Storage unit",
            ledgercraft_ledger::AccountType::Expense,
            "",
        )
        .unwrap();
    c.set_account_parent(storage, Some(rent)).unwrap();
    let check = TxnBody::Check(Check {
        payee: None,
        pay_from: c.account_by_name("Checking").unwrap().id,
        lines: vec![AccountLine {
            account: storage,
            description: String::new(),
            amount: Money::from_dollars(50),
        }],
    });
    c.post(d("2026-02-02"), "102", "", check).unwrap();
    let r = profit_and_loss(&c, d("2026-01-01"), d("2026-12-31"));
    let labels: Vec<&str> = r.rows.iter().map(|row| row.label.as_str()).collect();
    let at = labels.iter().position(|l| *l == "6300 · Rent").unwrap();
    // The heading, Rent's own 300, the 50 under it, then the total of 350.
    assert_eq!(r.rows[at].kind, RowKind::Section);
    assert_eq!(labels[at + 1], "6300 · Rent");
    assert_eq!(labels[at + 2], "6310 · Storage unit");
    assert_eq!(labels[at + 3], "Total 6300 · Rent");
    assert_eq!(
        last_money(&r, "Total 6300 · Rent"),
        Money::from_dollars(350)
    );
    assert!(r.rows[at + 2].indent > r.rows[at].indent);
    // The Trial Balance lists it with its parent's name.
    let tb = trial_balance(&c, d("2026-12-31"));
    assert!(tb
        .rows
        .iter()
        .any(|row| row.label == "6310 · Rent:Storage unit"));
}
