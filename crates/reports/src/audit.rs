//! The Audit Trail and Sales by State reports.

use std::collections::BTreeMap;

use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{Company, TxnBody};

use crate::report::{Cell, Report, RowKind};

/// Every change made to the books from `from` to `to`, oldest first: what
/// was done (added, edited, voided, undone…) and to what.
pub fn audit_trail(company: &Company, from: Date, to: Date) -> Report {
    let mut r = Report::new(
        "Audit Trail",
        &company.info.name,
        format!("{from} through {to}"),
        &["Change", "Details"],
    );
    r.label_header = "When".into();
    let mut shown = 0;
    for e in company
        .audit
        .iter()
        .filter(|e| e.date().is_some_and(|d| d >= from && d <= to))
    {
        r.line(
            RowKind::Detail,
            &e.at,
            0,
            vec![Cell::Text(e.action.clone()), Cell::Text(e.detail.clone())],
            None,
        );
        shown += 1;
    }
    if shown == 0 {
        r.line(
            RowKind::Detail,
            "No changes recorded in this period.",
            0,
            vec![Cell::Empty, Cell::Empty],
            None,
        );
    }
    r
}

/// The state (or region) a sale counts in: where it was shipped, else the
/// customer's address, else (a walk-in sale) the company's own.
fn sale_state(company: &Company, body: &TxnBody) -> String {
    let customer = body.sale().and_then(|s| s.customer).and_then(|id| company.customer(id));
    let shipped = match body {
        TxnBody::Invoice(i) => i.ship_to.as_ref().map(|a| a.region.clone()),
        _ => None,
    };
    let region = shipped
        .filter(|r| !r.trim().is_empty())
        .or_else(|| customer.map(|c| c.contact.address.region.clone()))
        .filter(|r| !r.trim().is_empty())
        .unwrap_or_else(|| {
            if customer.is_none() {
                company.info.address.region.clone()
            } else {
                String::new()
            }
        });
    let region = region.trim().to_uppercase();
    if region.is_empty() {
        "No state on file".into()
    } else {
        region
    }
}

/// Sales from `from` to `to` by the state they were sold into: invoices and
/// sales receipts, less credit memos, with the taxable part and the tax.
pub fn sales_by_state(company: &Company, from: Date, to: Date) -> Report {
    let mut r = Report::new(
        "Sales by State",
        &company.info.name,
        format!("{from} through {to}"),
        &["Sales", "Taxable sales", "Sales tax"],
    );
    r.label_header = "State".into();
    let mut by_state: BTreeMap<String, [Money; 3]> = BTreeMap::new();
    for t in company
        .transactions
        .iter()
        .filter(|t| !t.voided && t.date >= from && t.date <= to)
    {
        let Some(sale) = t.body.sale() else {
            continue;
        };
        let credit = t.body.sales_sign() < 0;
        let signed = |m: Option<Money>| {
            let m = m.unwrap_or(Money::ZERO);
            if credit {
                m.saturating_neg()
            } else {
                m
            }
        };
        let taxable = if sale.tax.is_some() {
            sale.taxable_amount()
        } else {
            Some(Money::ZERO)
        };
        let values = [
            signed(sale.subtotal()),
            signed(taxable),
            signed(sale.tax_total()),
        ];
        let entry = by_state
            .entry(sale_state(company, &t.body))
            .or_insert([Money::ZERO; 3]);
        for (total, value) in entry.iter_mut().zip(values) {
            *total = total.saturating_add(value);
        }
    }
    let mut totals = [Money::ZERO; 3];
    for (state, values) in &by_state {
        r.money(RowKind::Detail, state, 0, values, None);
        for (total, value) in totals.iter_mut().zip(values) {
            *total = total.saturating_add(*value);
        }
    }
    r.money(RowKind::Total, "Total", 0, &totals, None);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_core::Qty;
    use ledgercraft_ledger::{Address, AuditEntry, Contact, Invoice, SalesLine, SalesReceipt};

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn money_in(r: &Report, label: &str, column: usize) -> Money {
        match r.rows.iter().find(|row| row.label == label).and_then(|row| row.cells.get(column)) {
            Some(Cell::Money(m)) => *m,
            _ => Money::from_cents(i64::MIN),
        }
    }

    #[test]
    fn the_audit_trail_lists_changes_in_the_period() {
        let mut c = Company::with_default_accounts("Audit Co");
        for (at, action) in [
            ("2026-01-05 09:00 UTC", "Invoice"),
            ("2026-02-10 10:30 UTC", "Void T7"),
            ("2026-03-01 08:15 UTC", "Undo Void T7"),
        ] {
            c.audit.push(AuditEntry {
                at: at.into(),
                action: action.into(),
                detail: String::new(),
            });
        }
        let r = audit_trail(&c, d("2026-02-01"), d("2026-03-31"));
        let when: Vec<&str> = r.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(when, vec!["2026-02-10 10:30 UTC", "2026-03-01 08:15 UTC"]);
        assert_eq!(r.rows[0].cells[0], Cell::Text("Void T7".into()));
        let empty = audit_trail(&c, d("2025-01-01"), d("2025-12-31"));
        assert_eq!(empty.rows.len(), 1);
    }

    #[test]
    fn sales_are_grouped_by_where_they_went() {
        let mut c = Company::with_default_accounts("State Co");
        c.info.address.region = "UT".into();
        let utah = c
            .add_customer(Contact {
                name: "Utah Cafe".into(),
                address: Address {
                    region: "ut".into(),
                    ..Address::default()
                },
                ..Contact::default()
            })
            .unwrap();
        let sales = c.account_by_name("Sales").unwrap().id;
        let ar = c.account_by_name("Accounts Receivable").unwrap().id;
        let line = |dollars: i64| SalesLine {
            item: None,
            description: String::new(),
            qty: Qty::ONE,
            rate: Money::from_dollars(dollars),
            account: sales,
            taxable: true,
        };
        let date = d("2026-05-05");
        // $100 to the Utah customer's address; $300 shipped to Idaho; a $50
        // walk-in sale at the shop (Utah).
        let invoice = |dollars, ship_to| {
            TxnBody::Invoice(Invoice {
                customer: utah,
                due_date: date,
                ar_account: ar,
                lines: vec![line(dollars)],
                tax: None,
                ship_to,
            })
        };
        c.post(date, "1", "", invoice(100, None)).unwrap();
        let idaho = Address {
            city: "Boise".into(),
            region: "ID".into(),
            ..Address::default()
        };
        c.post(date, "2", "", invoice(300, Some(idaho))).unwrap();
        let receipt = SalesReceipt {
            customer: None,
            deposit_to: c.account_by_name("Checking").unwrap().id,
            method: "Cash".into(),
            lines: vec![line(50)],
            tax: None,
        };
        c.post(date, "1", "", TxnBody::SalesReceipt(receipt))
            .unwrap();

        let r = sales_by_state(&c, d("2026-05-01"), d("2026-05-31"));
        assert_eq!(money_in(&r, "UT", 0), Money::from_dollars(150));
        assert_eq!(money_in(&r, "ID", 0), Money::from_dollars(300));
        assert_eq!(money_in(&r, "Total", 0), Money::from_dollars(450));
        // No tax was charged, so nothing counts as taxable.
        assert_eq!(money_in(&r, "Total", 1), Money::ZERO);
    }
}
