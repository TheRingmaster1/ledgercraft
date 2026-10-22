use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{Company, Party, TxnBody};

use crate::{Report, RowKind};

const BUCKETS: [&str; 6] = ["Current", "1 - 30", "31 - 60", "61 - 90", "> 90", "Total"];

fn bucket(days_past_due: i64) -> usize {
    match days_past_due {
        i64::MIN..=0 => 0,
        1..=30 => 1,
        31..=60 => 2,
        61..=90 => 3,
        _ => 4,
    }
}

fn aging(company: &Company, as_of: Date, receivable: bool) -> Report {
    let title = if receivable {
        "A/R Aging Summary"
    } else {
        "A/P Aging Summary"
    };
    let mut r = Report::new(
        title,
        &company.info.name,
        format!("As of {as_of}"),
        &BUCKETS,
    );
    r.label_header = if receivable { "Customer" } else { "Vendor" }.into();

    // (party, buckets) in first-seen order, sorted by name below.
    let mut rows: Vec<(Party, [Money; 6])> = Vec::new();
    for t in company
        .transactions
        .iter()
        .filter(|t| !t.voided && t.date <= as_of)
    {
        // Credit left on a credit memo counts against what is owed, as current.
        let (party, due, credit) = match (&t.body, receivable) {
            (TxnBody::Invoice(i), true) => (Party::Customer(i.customer), i.due_date, false),
            (TxnBody::CreditMemo(m), true) => (Party::Customer(m.customer), as_of, true),
            (TxnBody::Bill(b), false) => (Party::Vendor(b.vendor), b.due_date, false),
            _ => continue,
        };
        let left = company.open_balance(t.id, Some(as_of));
        if !left.is_positive() {
            continue;
        }
        let open = if credit { left.saturating_neg() } else { left };
        let idx = bucket(due.days_until(as_of));
        let pos = match rows.iter().position(|(p, _)| *p == party) {
            Some(pos) => pos,
            None => {
                rows.push((party, [Money::ZERO; 6]));
                rows.len() - 1
            }
        };
        if let Some((_, values)) = rows.get_mut(pos) {
            for i in [idx, 5] {
                if let Some(v) = values.get_mut(i) {
                    *v = v.saturating_add(open);
                }
            }
        }
    }
    rows.sort_by_key(|(p, _)| company.party_name(*p).to_lowercase());
    let mut totals = [Money::ZERO; 6];
    for (party, values) in &rows {
        for (t, v) in totals.iter_mut().zip(values) {
            *t = t.saturating_add(*v);
        }
        r.money(
            RowKind::Detail,
            &company.party_name(*party),
            0,
            values,
            None,
        );
    }
    r.money(RowKind::Total, "Total", 0, &totals, None);
    r
}

/// What customers owe, by how overdue it is.
pub fn ar_aging(company: &Company, as_of: Date) -> Report {
    aging(company, as_of, true)
}

/// What you owe vendors, by how overdue it is.
pub fn ap_aging(company: &Company, as_of: Date) -> Report {
    aging(company, as_of, false)
}
