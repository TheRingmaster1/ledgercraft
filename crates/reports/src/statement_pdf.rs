//! A customer statement as a PDF: what they owed at the start of a period,
//! every charge and payment in it with a running balance, what they owe at
//! the end, and how overdue their open invoices are.
//!
//! It shares the invoice's look: the company at the top left, the word
//! STATEMENT and its dates at the top right, who it is for, then the
//! activity. Long statements continue on further pages, each repeating the
//! column headings; every page is numbered.

use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{AccountType, Company, CustomerId, Party, TxnBody, TxnId};

use crate::invoice_pdf::{
    bill_to, company_lines, page_mut, row_count, BOTTOM, INK, LEFT, LINE_HEIGHT, MUTED, RIGHT, RULE,
};
use crate::look::{letterhead, Look};
use crate::pdf::{finish_with, wrap, Align, Font, Image, Page};

const DATE_X: f32 = 58.0;
const TXN_X: f32 = 140.0;
const TXN_WIDTH: f32 = 250.0;
const AMOUNT_RIGHT: f32 = 468.0;
const BALANCE_RIGHT: f32 = 554.0;

/// The aging columns at the foot of the statement.
const AGING: [&str; 6] = [
    "Current",
    "1 - 30 days",
    "31 - 60 days",
    "61 - 90 days",
    "Over 90 days",
    "Amount due",
];

/// One line of activity.
struct Activity {
    date: String,
    /// "Invoice 1001", "Payment": in bold.
    what: String,
    /// The due date, the payment method or the memo, in quieter text.
    detail: String,
    amount: String,
    balance: String,
}

/// Everything the page shows, gathered from the books first.
struct Sheet {
    look: Look,
    /// The company's logo, drawn as image 0.
    logo: Option<Image>,
    company: String,
    company_lines: Vec<String>,
    customer: String,
    facts: Vec<(&'static str, String)>,
    to: Vec<String>,
    from: String,
    opening: String,
    rows: Vec<Activity>,
    closing: String,
    aging: [String; 6],
    /// Payments not yet applied to an invoice, which the aging leaves out.
    credit_note: Option<String>,
}

/// A suggested file name: `Statement Maple Street Cafe 2026-03-31.pdf`.
pub fn statement_pdf_name(company: &Company, customer: CustomerId, to: Date) -> String {
    let name: String = company
        .customer(customer)
        .map(|c| c.contact.name.trim().to_string())
        .unwrap_or_default()
        .chars()
        .filter(|ch| !matches!(*ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    if name.is_empty() {
        format!("Statement {to}.pdf")
    } else {
        format!("Statement {name} {to}.pdf")
    }
}

/// A customer's statement for `from` to `to` (both included) as a PDF file.
/// `currency` is the symbol amounts are shown with, such as `$` or `€`.
pub fn statement_pdf(
    company: &Company,
    customer: CustomerId,
    from: Date,
    to: Date,
    currency: &str,
) -> Result<Vec<u8>, String> {
    let sheet = gather(company, customer, from, to, currency)?;
    let title = format!("Statement for {}", sheet.customer);
    Ok(finish_with(&layout(&sheet), &title, sheet.logo.as_slice()))
}

/// What a transaction is called on the statement, and its quieter detail.
fn describe(c: &Company, id: TxnId) -> (String, String) {
    let Some(t) = c.txn(id) else {
        return (format!("Transaction {id}"), String::new());
    };
    let number = t.number.trim();
    let what = if number.is_empty() {
        t.body.label().to_string()
    } else {
        format!("{} {number}", t.body.label())
    };
    let mut detail = match &t.body {
        TxnBody::Invoice(i) => format!("Due {}", i.due_date),
        TxnBody::ReceivePayment(p) => p.method.trim().to_string(),
        TxnBody::Refund(r) => r.method.trim().to_string(),
        _ => String::new(),
    };
    let memo = t.memo.trim();
    if !memo.is_empty() {
        if !detail.is_empty() {
            detail.push_str(" · ");
        }
        detail.push_str(memo);
    }
    (what, detail)
}

fn gather(
    c: &Company,
    customer: CustomerId,
    from: Date,
    to: Date,
    currency: &str,
) -> Result<Sheet, String> {
    let who = c
        .customer(customer)
        .ok_or_else(|| format!("Customer {customer} was not found."))?;
    if from > to {
        return Err("The statement's start date is after its end date.".into());
    }
    let money = |m: Money| m.format_with(currency);
    let party = Party::Customer(customer);

    // Their receivable postings, in date order; one total per transaction.
    let mut opening = Money::ZERO;
    let mut activity: Vec<(TxnId, Date, Money)> = Vec::new();
    for e in c.ledger_entries() {
        if e.posting.party != Some(party)
            || e.date > to
            || !c
                .account(e.posting.account)
                .is_some_and(|a| a.kind == AccountType::AccountsReceivable)
        {
            continue;
        }
        if e.date < from {
            opening = opening.saturating_add(e.posting.amount);
        } else if let Some(last) = activity.last_mut().filter(|(id, _, _)| *id == e.txn) {
            last.2 = last.2.saturating_add(e.posting.amount);
        } else {
            activity.push((e.txn, e.date, e.posting.amount));
        }
    }

    let mut balance = opening;
    let rows = activity
        .into_iter()
        .filter(|(_, _, amount)| !amount.is_zero())
        .map(|(id, date, amount)| {
            balance = balance.saturating_add(amount);
            let (what, detail) = describe(c, id);
            Activity {
                date: date.to_string(),
                what,
                detail,
                amount: money(amount),
                balance: money(balance),
            }
        })
        .collect();

    // How overdue their open invoices are on the statement date.
    let mut aging = [Money::ZERO; 6];
    for t in c.transactions.iter().filter(|t| !t.voided && t.date <= to) {
        let TxnBody::Invoice(i) = &t.body else {
            continue;
        };
        if i.customer != customer {
            continue;
        }
        let open = c.open_balance(t.id, Some(to));
        if !open.is_positive() {
            continue;
        }
        let bucket = match i.due_date.days_until(to) {
            i64::MIN..=0 => 0,
            1..=30 => 1,
            31..=60 => 2,
            61..=90 => 3,
            _ => 4,
        };
        if let Some(v) = aging.get_mut(bucket) {
            *v = v.saturating_add(open);
        }
    }
    let invoiced = aging
        .iter()
        .take(5)
        .fold(Money::ZERO, |a, b| a.saturating_add(*b));
    if let Some(due) = aging.get_mut(5) {
        *due = balance;
    }
    let credit = invoiced.saturating_sub(balance);
    let credit_note = credit.is_positive().then(|| {
        format!(
            "Amount due is after {} in payments or credits not yet applied to an invoice.",
            money(credit)
        )
    });

    let name = who.contact.name.trim().to_string();
    Ok(Sheet {
        look: Look::of(c.info.print.template),
        logo: crate::look::logo(c),
        company: c.info.name.trim().to_string(),
        company_lines: company_lines(c),
        customer: if name.is_empty() {
            "customer".into()
        } else {
            name
        },
        facts: vec![
            ("Statement date", to.to_string()),
            ("Period", format!("{from} to {to}")),
            ("Amount due", money(balance)),
        ],
        to: bill_to(c, customer),
        from: from.to_string(),
        opening: money(opening),
        rows,
        closing: money(balance),
        aging: aging.map(money),
        credit_note,
    })
}

fn layout(s: &Sheet) -> Vec<Page> {
    let mut pages = vec![Page::default()];
    let mut y = first_page_top(page_mut(&mut pages), s);

    // The balance carried in from before the period.
    y = simple_row(
        page_mut(&mut pages),
        y,
        &s.from,
        "Balance forward",
        &s.opening,
    );

    for row in &s.rows {
        let mut lines = vec![(row.what.clone(), Font::bold(10.0), INK)];
        if !row.detail.is_empty() {
            let font = Font::regular(9.0);
            lines.extend(
                wrap(&row.detail, font, TXN_WIDTH)
                    .into_iter()
                    .map(|t| (t, font, MUTED)),
            );
        }
        let height = row_count(lines.len()) * LINE_HEIGHT + 10.0;
        if y + height > BOTTOM {
            pages.push(Page::default());
            y = continued_top(page_mut(&mut pages), s);
        }
        let p = page_mut(&mut pages);
        let baseline = y + 14.5;
        let body = Font::regular(10.0);
        p.text(DATE_X, baseline, &row.date, body, INK, Align::Left);
        let mut row_y = baseline;
        for (text, font, color) in &lines {
            p.text(TXN_X, row_y, text, *font, *color, Align::Left);
            row_y += LINE_HEIGHT;
        }
        p.text(AMOUNT_RIGHT, baseline, &row.amount, body, INK, Align::Right);
        p.text(
            BALANCE_RIGHT,
            baseline,
            &row.balance,
            body,
            INK,
            Align::Right,
        );
        y += height;
        p.rule(LEFT, RIGHT, y, RULE, 0.6);
    }

    // Amount due, then the aging under it, kept together.
    let note_height = if s.credit_note.is_some() { 16.0 } else { 0.0 };
    if y + 120.0 + note_height > BOTTOM {
        pages.push(Page::default());
        y = continued_top(page_mut(&mut pages), s) - 20.0;
    }
    let p = page_mut(&mut pages);
    let mut y = y + 26.0;
    p.rule(AMOUNT_RIGHT - 120.0, RIGHT, y - 13.0, INK, 0.8);
    let strong = Font::bold(11.5);
    p.text(
        AMOUNT_RIGHT,
        y + 2.0,
        "Amount due",
        strong,
        INK,
        Align::Right,
    );
    p.text(
        BALANCE_RIGHT,
        y + 2.0,
        &s.closing,
        strong,
        INK,
        Align::Right,
    );
    y += 26.0;
    aging_table(p, s, y);
    if let Some(note) = &s.credit_note {
        p.text(LEFT, y + 56.0, note, Font::regular(9.0), MUTED, Align::Left);
    }

    let count = pages.len();
    for (i, page) in pages.iter_mut().enumerate() {
        page.rule(LEFT, RIGHT, 752.0, RULE, 0.6);
        let small = Font::regular(8.5);
        page.text(
            LEFT,
            766.0,
            "Thank you for your business.",
            small,
            MUTED,
            Align::Left,
        );
        let n = format!("Page {} of {count}", i + 1);
        page.text(RIGHT, 766.0, &n, small, MUTED, Align::Right);
    }
    pages
}

/// A one-line row with no amount, only a balance; returns the next row's top.
fn simple_row(p: &mut Page, y: f32, date: &str, what: &str, balance: &str) -> f32 {
    let baseline = y + 14.5;
    let body = Font::regular(10.0);
    p.text(DATE_X, baseline, date, body, INK, Align::Left);
    p.text(TXN_X, baseline, what, Font::bold(10.0), INK, Align::Left);
    p.text(BALANCE_RIGHT, baseline, balance, body, INK, Align::Right);
    let y = y + LINE_HEIGHT + 10.0;
    p.rule(LEFT, RIGHT, y, RULE, 0.6);
    y
}

/// The aging band: six columns, headings over amounts.
fn aging_table(p: &mut Page, s: &Sheet, y: f32) {
    let width = (RIGHT - LEFT) / 6.0;
    let headings: Vec<(f32, &str, Align)> = AGING
        .iter()
        .enumerate()
        .map(|(i, label)| (LEFT + width * row_count(i + 1) - 8.0, *label, Align::Right))
        .collect();
    s.look.headings(p, y, 8.5, &headings);
    let mut right = LEFT + width - 8.0;
    for (i, value) in s.aging.iter().enumerate() {
        let font = if i == 5 {
            Font::bold(10.5)
        } else {
            Font::regular(10.0)
        };
        p.text(right, y + 36.0, value, font, INK, Align::Right);
        right += width;
    }
    p.rule(LEFT, RIGHT, y + 44.0, RULE, 0.6);
}

/// The first page's heading, the customer and the column headings; returns
/// where the activity starts.
fn first_page_top(p: &mut Page, s: &Sheet) -> f32 {
    let left_bottom = letterhead(p, s.logo.as_ref(), &s.company, &s.company_lines);

    p.text(
        RIGHT,
        66.0,
        "STATEMENT",
        Font::bold(24.0),
        s.look.accent,
        Align::Right,
    );
    let mut ry = 90.0;
    for (label, value) in &s.facts {
        p.text(
            RIGHT - 140.0,
            ry,
            label,
            Font::regular(9.0),
            MUTED,
            Align::Right,
        );
        p.text(RIGHT, ry, value, Font::bold(9.5), INK, Align::Right);
        ry += 14.0;
    }

    let mut y = left_bottom.max(ry) + 18.0;
    p.text(LEFT, y, "TO", Font::bold(8.5), s.look.accent, Align::Left);
    y += 15.0;
    for (i, l) in s.to.iter().enumerate() {
        let (font, color) = if i == 0 {
            (Font::bold(10.5), INK)
        } else {
            (Font::regular(9.5), MUTED)
        };
        p.text(LEFT, y, l, font, color, Align::Left);
        y += 13.5;
    }
    column_headings(p, s, y + 14.0)
}

/// A later page's heading; returns where the activity continues.
fn continued_top(p: &mut Page, s: &Sheet) -> f32 {
    p.text(LEFT, 60.0, &s.company, Font::bold(11.0), INK, Align::Left);
    let label = format!("Statement for {} (continued)", s.customer);
    p.text(RIGHT, 60.0, &label, Font::regular(9.5), MUTED, Align::Right);
    column_headings(p, s, 76.0)
}

fn column_headings(p: &mut Page, s: &Sheet, y: f32) -> f32 {
    s.look.headings(
        p,
        y,
        9.0,
        &[
            (DATE_X, "Date", Align::Left),
            (TXN_X, "Transaction", Align::Left),
            (AMOUNT_RIGHT, "Amount", Align::Right),
            (BALANCE_RIGHT, "Balance", Align::Right),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_core::Qty;
    use ledgercraft_ledger::{Applied, Contact, Invoice, ReceivePayment, SalesLine};

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn invoice(c: &Company, customer: CustomerId, due: &str, dollars: i64) -> TxnBody {
        let sales = c.account_by_name("Service Income").unwrap().id;
        TxnBody::Invoice(Invoice {
            customer,
            due_date: d(due),
            ar_account: c.account_by_name("Accounts Receivable").unwrap().id,
            ship_to: None,
            tax: None,
            lines: vec![SalesLine {
                item: None,
                description: "Shelving".into(),
                qty: Qty::ONE,
                rate: Money::from_dollars(dollars),
                taxable: false,
                account: sales,
            }],
        })
    }

    fn payment(c: &Company, customer: CustomerId, dollars: i64, to: Option<TxnId>) -> TxnBody {
        TxnBody::ReceivePayment(ReceivePayment {
            customer,
            deposit_to: c.account_by_name("Checking").unwrap().id,
            ar_account: c.account_by_name("Accounts Receivable").unwrap().id,
            amount: Money::from_dollars(dollars),
            method: "Check".into(),
            applied: to
                .map(|txn| Applied {
                    txn,
                    amount: Money::from_dollars(dollars),
                })
                .into_iter()
                .collect(),
        })
    }

    /// Maple Street Cafe: $500 invoiced in January, $1,000 in February, $200
    /// paid on the first in March. Another customer's invoice stays off.
    fn books() -> (Company, CustomerId) {
        let mut c = Company::with_default_accounts("Pine Ridge Woodworks");
        let cafe = c
            .add_customer(Contact {
                name: "Maple Street Cafe".into(),
                ..Contact::default()
            })
            .unwrap();
        let other = c
            .add_customer(Contact {
                name: "Someone Else".into(),
                ..Contact::default()
            })
            .unwrap();
        let jan = invoice(&c, cafe, "2026-02-09", 500);
        let jan = c.post(d("2026-01-10"), "1001", "", jan).unwrap();
        let feb = invoice(&c, cafe, "2026-03-14", 1000);
        c.post(d("2026-02-12"), "1002", "Spring order", feb)
            .unwrap();
        let pay = payment(&c, cafe, 200, Some(jan));
        c.post(d("2026-03-01"), "", "", pay).unwrap();
        let theirs = invoice(&c, other, "2026-02-20", 777);
        c.post(d("2026-02-01"), "1003", "", theirs).unwrap();
        (c, cafe)
    }

    fn text_of(pdf: &[u8]) -> String {
        String::from_utf8_lossy(pdf).into_owned()
    }

    #[test]
    fn a_statement_shows_the_period_with_a_running_balance() {
        let (c, cafe) = books();
        let pdf = statement_pdf(&c, cafe, d("2026-02-01"), d("2026-03-31"), "$").unwrap();
        let text = text_of(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        for shown in [
            "(STATEMENT) Tj",
            "(Maple Street Cafe) Tj",
            "(Balance forward) Tj",
            // January's invoice is carried in, not listed.
            "($500.00) Tj",
            "(Invoice 1002) Tj",
            "(Due 2026-03-14 \\267 Spring order) Tj",
            "($1,500.00) Tj",
            "(Payment) Tj",
            "(Check) Tj",
            "(-$200.00) Tj",
            "($1,300.00) Tj",
            "(Amount due) Tj",
            "(2026-02-01 to 2026-03-31) Tj",
            "(Page 1 of 1) Tj",
        ] {
            assert!(text.contains(shown), "missing {shown}");
        }
        assert!(!text.contains("(Invoice 1001) Tj"));
        assert!(!text.contains("Someone Else"));
        assert!(!text.contains("$777.00"));
    }

    #[test]
    fn the_aging_sorts_open_invoices_by_how_late_they_are() {
        let (c, cafe) = books();
        let s = gather(&c, cafe, d("2026-02-01"), d("2026-03-31"), "$").unwrap();
        // 1001 ($300 left) was due 2026-02-09: 50 days late on 2026-03-31.
        // 1002 ($1,000) was due 2026-03-14: 17 days late.
        assert_eq!(
            s.aging,
            [
                "$0.00",
                "$1,000.00",
                "$300.00",
                "$0.00",
                "$0.00",
                "$1,300.00"
            ]
            .map(String::from)
        );
        assert!(s.credit_note.is_none());
    }

    #[test]
    fn an_unapplied_payment_is_explained() {
        let (mut c, cafe) = books();
        let pay = payment(&c, cafe, 100, None);
        c.post(d("2026-03-20"), "", "", pay).unwrap();
        let s = gather(&c, cafe, d("2026-03-01"), d("2026-03-31"), "$").unwrap();
        assert_eq!(s.closing, "$1,200.00");
        assert_eq!(s.aging[5], "$1,200.00");
        assert!(s.credit_note.unwrap_or_default().contains("$100.00"));
    }

    #[test]
    fn a_long_statement_runs_onto_more_pages() {
        let (mut c, cafe) = books();
        for day in 1..=28 {
            for n in 0..2 {
                let body = invoice(&c, cafe, "2026-04-30", 10 + n);
                let number = format!("2{day:02}{n}");
                c.post(d(&format!("2026-04-{day:02}")), &number, "", body)
                    .unwrap();
            }
        }
        let text =
            text_of(&statement_pdf(&c, cafe, d("2026-04-01"), d("2026-04-30"), "€").unwrap());
        assert!(text.contains("(Page 2 of "));
        assert!(text.contains("(Statement for Maple Street Cafe \\(continued\\)) Tj"));
    }

    #[test]
    fn bad_requests_are_refused() {
        let (c, cafe) = books();
        assert!(statement_pdf(
            &c,
            CustomerId(999_999),
            d("2026-01-01"),
            d("2026-01-31"),
            "$"
        )
        .is_err());
        assert!(statement_pdf(&c, cafe, d("2026-02-01"), d("2026-01-01"), "$").is_err());
        assert_eq!(
            statement_pdf_name(&c, cafe, d("2026-03-31")),
            "Statement Maple Street Cafe 2026-03-31.pdf"
        );
    }
}
