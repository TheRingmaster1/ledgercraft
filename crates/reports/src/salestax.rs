//! The Sales Tax Liability report: what was sold, the tax charged on it by
//! location and by agency, and what is owed.

use std::collections::{BTreeMap, BTreeSet};

use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{Company, TaxComponentId, TaxGroupId, TaxRate};

use crate::report::{Cell, Report, RowKind};

/// Taxable sales, the rates seen, and the tax charged.
#[derive(Default)]
struct Tally {
    taxable: Money,
    rates: BTreeSet<TaxRate>,
    tax: Money,
}

impl Tally {
    fn add(&mut self, taxable: Money, rate: TaxRate, tax: Money) {
        self.taxable = self.taxable.saturating_add(taxable);
        self.rates.insert(rate);
        self.tax = self.tax.saturating_add(tax);
    }

    /// `7.45%`, or each rate when it changed during the period.
    fn rates_text(&self) -> String {
        let shown: Vec<String> = self.rates.iter().map(ToString::to_string).collect();
        shown.join(" / ")
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Money(self.taxable),
            Cell::Text(self.rates_text()),
            Cell::Money(self.tax),
        ]
    }
}

/// Sales tax charged on invoices and sales receipts dated `from` to `to`, less
/// tax credited back on credit memos (voided ones left out),
/// and the Sales Tax Payable account over the period.
pub fn sales_tax_liability(company: &Company, from: Date, to: Date) -> Report {
    let mut r = Report::new(
        "Sales Tax Liability",
        &company.info.name,
        format!("{from} through {to}"),
        &["Taxable sales", "Rate", "Tax"],
    );
    let mut sales = Money::ZERO;
    let mut taxable = Money::ZERO;
    let mut by_group: BTreeMap<TaxGroupId, Tally> = BTreeMap::new();
    let mut by_tax: BTreeMap<TaxComponentId, (String, Tally)> = BTreeMap::new();
    let invoices = company
        .transactions
        .iter()
        .filter(|t| !t.voided && t.date >= from && t.date <= to);
    for t in invoices {
        // Invoices and sales receipts are sales; credit memos take them back.
        let Some(sale) = t.body.sale() else {
            continue;
        };
        let credit = t.body.sales_sign() < 0;
        let signed = |m: Money| if credit { m.saturating_neg() } else { m };
        sales = sales.saturating_add(signed(sale.subtotal().unwrap_or(Money::ZERO)));
        let Some(tax) = sale.tax else {
            continue;
        };
        let charged = signed(sale.taxable_amount().unwrap_or(Money::ZERO));
        taxable = taxable.saturating_add(charged);
        let total = signed(tax.total().unwrap_or(Money::ZERO));
        let combined =
            TaxRate::from_millionths(tax.lines.iter().map(|l| l.rate.millionths()).sum());
        by_group
            .entry(tax.group)
            .or_default()
            .add(charged, combined, total);
        for l in &tax.lines {
            let (_, tally) = by_tax
                .entry(l.component)
                .or_insert_with(|| (company.tax_agency(l), Tally::default()));
            tally.add(signed(l.taxable), l.rate, signed(l.amount));
        }
    }

    r.section("Sales", 0);
    r.money(RowKind::Detail, "Total sales", 1, &[sales], None);
    r.money(
        RowKind::Detail,
        "Non-taxable sales",
        1,
        &[sales.saturating_sub(taxable)],
        None,
    );
    r.money(RowKind::Subtotal, "Taxable sales", 1, &[taxable], None);

    r.section("Tax by location (tax group)", 0);
    let mut collected = Money::ZERO;
    for (id, tally) in &by_group {
        let name = company
            .tax_group(*id)
            .map_or_else(|| format!("Tax group {id}"), |g| g.name.clone());
        r.line(RowKind::Detail, &name, 1, tally.cells(), None);
        collected = collected.saturating_add(tally.tax);
    }
    r.line(
        RowKind::Subtotal,
        "Tax charged",
        1,
        vec![Cell::Money(taxable), Cell::Empty, Cell::Money(collected)],
        None,
    );

    r.section("Tax by agency", 0);
    let mut agencies: BTreeMap<&str, Vec<(TaxComponentId, &Tally)>> = BTreeMap::new();
    for (id, (agency, tally)) in &by_tax {
        agencies
            .entry(agency.as_str())
            .or_default()
            .push((*id, tally));
    }
    for (agency, taxes) in &agencies {
        r.section(agency, 1);
        let mut subtotal = Money::ZERO;
        for (id, tally) in taxes {
            let name = company
                .tax_component(*id)
                .map_or_else(|| format!("Sales tax {id}"), |t| t.name.clone());
            r.line(RowKind::Detail, &name, 2, tally.cells(), None);
            subtotal = subtotal.saturating_add(tally.tax);
        }
        r.line(
            RowKind::Subtotal,
            &format!("Total {agency}"),
            2,
            vec![Cell::Empty, Cell::Empty, Cell::Money(subtotal)],
            None,
        );
    }

    if let Some(account) = company.sales_tax_account() {
        // Payments and adjustments: everything posted to the account in the
        // period except the tax invoices charged.
        let mut paid = Money::ZERO;
        for e in company.ledger_entries() {
            if e.posting.account != account || e.date < from || e.date > to {
                continue;
            }
            let from_sale = company.txn(e.txn).is_some_and(|t| t.body.sale().is_some());
            if !from_sale {
                paid = paid.saturating_add(e.posting.amount);
            }
        }
        let owed = company.account_balance(account, Some(to));
        r.section("Sales Tax Payable", 0);
        let money_row = |r: &mut Report, kind, label: &str, m: Money| {
            r.line(
                kind,
                label,
                1,
                vec![Cell::Empty, Cell::Empty, Cell::Money(m)],
                Some(account),
            );
        };
        money_row(
            &mut r,
            RowKind::Detail,
            "Tax charged this period",
            collected,
        );
        money_row(
            &mut r,
            RowKind::Detail,
            "Paid or adjusted this period",
            paid,
        );
        money_row(&mut r, RowKind::Total, &format!("Owed at {to}"), owed);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_core::Qty;
    use ledgercraft_ledger::{
        tax_payment_memo, AccountLine, AccountType, Check, Contact, Invoice, SalesLine, TxnBody,
    };

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn money_in(r: &Report, label: &str, column: usize) -> Money {
        let row = r.rows.iter().find(|row| row.label == label).unwrap();
        match row.cells.get(column) {
            Some(Cell::Money(m)) => *m,
            // Not an amount: a value no assertion expects.
            _ => Money::from_cents(i64::MIN),
        }
    }

    #[test]
    fn tax_is_reported_by_location_and_agency_with_what_is_owed() {
        let mut c = Company::with_default_accounts("Tax Co");
        let rate = |s: &str| TaxRate::parse(s).unwrap();
        let agency = "Utah State Tax Commission";
        let state = c
            .add_tax_component("Utah State", agency, "", d("2020-01-01"), rate("4.85"))
            .unwrap();
        let city = c
            .add_tax_component("Provo City", agency, "", d("2020-01-01"), rate("0.1"))
            .unwrap();
        let group = c.add_tax_group("Provo", vec![state, city]).unwrap();
        let customer = c
            .add_customer(Contact {
                name: "Cafe".into(),
                ..Contact::default()
            })
            .unwrap();
        let sales = c.account_by_name("Sales").unwrap().id;
        let ar = c.first_account_of(AccountType::AccountsReceivable).unwrap();
        let line = |dollars: i64, taxable: bool| SalesLine {
            item: None,
            description: String::new(),
            qty: Qty::ONE,
            rate: Money::from_dollars(dollars),
            account: sales,
            taxable,
        };
        // $1,000 taxed (+ $200 not taxed), and a $300 invoice with no tax.
        let date = d("2026-10-05");
        let tax = c
            .invoice_tax(group, date, Money::from_dollars(1000))
            .unwrap();
        let taxed = Invoice {
            customer,
            due_date: date,
            ar_account: ar,
            lines: vec![line(1000, true), line(200, false)],
            tax: Some(tax),
        };
        c.post(date, "1", "", TxnBody::Invoice(taxed)).unwrap();
        let untaxed = Invoice {
            customer,
            due_date: date,
            ar_account: ar,
            lines: vec![line(300, true)],
            tax: None,
        };
        c.post(date, "2", "", TxnBody::Invoice(untaxed)).unwrap();
        // $20 paid to the agency.
        let payable = c.sales_tax_account().unwrap();
        let check = TxnBody::Check(Check {
            payee: None,
            pay_from: c.account_by_name("Checking").unwrap().id,
            lines: vec![AccountLine {
                account: payable,
                description: tax_payment_memo(agency),
                amount: Money::from_dollars(20),
            }],
        });
        c.post(d("2026-10-20"), "", "", check).unwrap();

        let r = sales_tax_liability(&c, d("2026-10-01"), d("2026-10-31"));
        assert_eq!(money_in(&r, "Total sales", 0), Money::from_dollars(1500));
        assert_eq!(
            money_in(&r, "Non-taxable sales", 0),
            Money::from_dollars(500)
        );
        assert_eq!(money_in(&r, "Taxable sales", 0), Money::from_dollars(1000));
        // 4.85% + 0.1% of $1,000.
        assert_eq!(money_in(&r, "Provo", 2), Money::from_cents(4950));
        let provo = r.rows.iter().find(|row| row.label == "Provo").unwrap();
        assert_eq!(provo.cells[1], Cell::Text("4.95%".into()));
        assert_eq!(money_in(&r, "Utah State", 2), Money::from_cents(4850));
        assert_eq!(
            money_in(&r, &format!("Total {agency}"), 2),
            Money::from_cents(4950)
        );
        assert_eq!(
            money_in(&r, "Paid or adjusted this period", 2),
            Money::from_dollars(20)
        );
        assert_eq!(
            money_in(&r, "Owed at 2026-10-31", 2),
            Money::from_cents(2950)
        );
        // Before the invoices: nothing charged, nothing owed.
        let empty = sales_tax_liability(&c, d("2026-09-01"), d("2026-09-30"));
        assert_eq!(money_in(&empty, "Tax charged", 2), Money::ZERO);
    }

    #[test]
    fn sales_receipts_add_and_credit_memos_take_back() {
        let mut c = Company::with_default_accounts("Tax Co");
        let state = c
            .add_tax_component(
                "Utah State",
                "",
                "",
                d("2020-01-01"),
                TaxRate::parse("5").unwrap(),
            )
            .unwrap();
        let group = c.add_tax_group("Utah", vec![state]).unwrap();
        let customer = c
            .add_customer(Contact {
                name: "Cafe".into(),
                ..Contact::default()
            })
            .unwrap();
        let sales = c.account_by_name("Sales").unwrap().id;
        let lines = |dollars: i64| {
            vec![SalesLine {
                item: None,
                description: String::new(),
                qty: Qty::ONE,
                rate: Money::from_dollars(dollars),
                account: sales,
                taxable: true,
            }]
        };
        let date = d("2026-10-05");
        // $400 sold over the counter, $100 of it returned.
        let receipt = ledgercraft_ledger::SalesReceipt {
            customer: None,
            deposit_to: c.account_by_name("Undeposited Funds").unwrap().id,
            method: "Cash".into(),
            lines: lines(400),
            tax: Some(
                c.invoice_tax(group, date, Money::from_dollars(400))
                    .unwrap(),
            ),
        };
        c.post(date, "1", "", TxnBody::SalesReceipt(receipt))
            .unwrap();
        let memo = ledgercraft_ledger::CreditMemo {
            customer,
            ar_account: c.first_account_of(AccountType::AccountsReceivable).unwrap(),
            lines: lines(100),
            tax: Some(
                c.invoice_tax(group, date, Money::from_dollars(100))
                    .unwrap(),
            ),
            applied: Vec::new(),
        };
        c.post(date, "CM-1", "", TxnBody::CreditMemo(memo)).unwrap();

        let r = sales_tax_liability(&c, d("2026-10-01"), d("2026-10-31"));
        assert_eq!(money_in(&r, "Total sales", 0), Money::from_dollars(300));
        assert_eq!(money_in(&r, "Taxable sales", 0), Money::from_dollars(300));
        assert_eq!(money_in(&r, "Utah", 2), Money::from_dollars(15));
        assert_eq!(
            money_in(&r, "Owed at 2026-10-31", 2),
            Money::from_dollars(15)
        );
        let owed = c.sales_tax_owed(d("2026-10-31"));
        assert_eq!(owed.first().map(|o| o.owed), Some(Money::from_dollars(15)));
    }
}
