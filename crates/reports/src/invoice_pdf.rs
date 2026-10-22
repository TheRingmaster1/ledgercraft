//! An invoice as a PDF, ready to email or print.
//!
//! The layout: the company's name and contact details at the top left, the
//! word INVOICE and the invoice's number, dates and terms at the top right,
//! who it is billed to, the lines, then the totals and balance due, with the
//! memo as a note. Long invoices continue on further pages, each repeating
//! the column headings; every page is numbered.

use ledgercraft_core::Money;
use ledgercraft_ledger::{Company, Invoice, Transaction, TxnBody, TxnId};

use crate::pdf::{finish, wrap, Align, Font, Page, Rgb};

const LEFT: f32 = 50.0;
const RIGHT: f32 = 562.0;
/// Content stops here; the footer sits below.
const BOTTOM: f32 = 730.0;

const DESC_X: f32 = 58.0;
const DESC_WIDTH: f32 = 270.0;
const QTY_RIGHT: f32 = 380.0;
const RATE_RIGHT: f32 = 468.0;
const AMOUNT_RIGHT: f32 = 554.0;
const LINE_HEIGHT: f32 = 13.0;

const ACCENT: Rgb = Rgb(0.122, 0.435, 0.545);
const INK: Rgb = Rgb(0.13, 0.13, 0.13);
const MUTED: Rgb = Rgb(0.42, 0.45, 0.50);
const RULE: Rgb = Rgb(0.82, 0.84, 0.87);
const WHITE: Rgb = Rgb(1.0, 1.0, 1.0);
const RED: Rgb = Rgb(0.75, 0.15, 0.15);
const GREEN: Rgb = Rgb(0.12, 0.50, 0.27);

/// What a printed invoice line shows.
struct PrintLine {
    /// The item's name, in bold above the description; empty without an item.
    head: String,
    description: String,
    qty: String,
    rate: String,
    amount: String,
}

/// One line of the totals block.
struct TotalLine {
    label: String,
    amount: String,
    strong: bool,
}

/// Everything the page shows, gathered from the books first.
struct Sheet {
    company: String,
    company_lines: Vec<String>,
    number: String,
    void: bool,
    paid: bool,
    facts: Vec<(&'static str, String)>,
    bill_to: Vec<String>,
    lines: Vec<PrintLine>,
    totals: Vec<TotalLine>,
    memo: String,
}

/// A suggested file name: `Invoice 1001.pdf`.
pub fn invoice_pdf_name(company: &Company, id: TxnId) -> String {
    match company.txn(id) {
        Some(t) if !t.number.trim().is_empty() => format!("Invoice {}.pdf", t.number.trim()),
        _ => "Invoice.pdf".into(),
    }
}

/// The invoice as a PDF file. `currency` is the symbol amounts are shown
/// with, such as `$` or `€`.
pub fn invoice_pdf(company: &Company, id: TxnId, currency: &str) -> Result<Vec<u8>, String> {
    let t = company
        .txn(id)
        .ok_or_else(|| format!("Transaction {id} was not found."))?;
    let TxnBody::Invoice(invoice) = &t.body else {
        return Err(format!("{} is not an invoice.", t.body.label()));
    };
    let sheet = gather(company, t, invoice, currency)?;
    let title = if sheet.number.is_empty() {
        format!(
            "Invoice for {}",
            sheet.bill_to.first().map_or("", String::as_str)
        )
    } else {
        format!("Invoice {}", sheet.number)
    };
    Ok(finish(&layout(&sheet), &title))
}

fn gather(c: &Company, t: &Transaction, inv: &Invoice, currency: &str) -> Result<Sheet, String> {
    let money = |m: Money| m.format_with(currency);
    let total = inv
        .total()
        .ok_or("The invoice total is too large to print.")?;
    let open = if t.voided {
        Money::ZERO
    } else {
        c.open_balance(t.id, None)
    };
    let applied = total.saturating_sub(open);

    let info = &c.info;
    let mut company_lines = Vec::new();
    if !info.legal_name.trim().is_empty() && info.legal_name.trim() != info.name.trim() {
        company_lines.push(info.legal_name.trim().to_string());
    }
    company_lines.extend(info.address.lines());
    for extra in [&info.phone, &info.email] {
        if !extra.trim().is_empty() {
            company_lines.push(extra.trim().to_string());
        }
    }

    let mut bill_to = Vec::new();
    if let Some(customer) = c.customer(inv.customer) {
        let contact = &customer.contact;
        for name in [&contact.company, &contact.name] {
            let name = name.trim();
            if !name.is_empty() && !bill_to.iter().any(|b: &String| b == name) {
                bill_to.push(name.to_string());
            }
        }
        bill_to.extend(contact.address.lines());
        if !contact.email.trim().is_empty() {
            bill_to.push(contact.email.trim().to_string());
        }
        if customer.tax.exempt {
            let cert = customer.tax.exemption.trim();
            bill_to.push(if cert.is_empty() {
                "Sales tax exempt".to_string()
            } else {
                format!("Sales tax exempt: {cert}")
            });
        }
    }
    if bill_to.is_empty() {
        bill_to.push("Customer".into());
    }

    let number = t.number.trim().to_string();
    let mut facts = Vec::new();
    if !number.is_empty() {
        facts.push(("Invoice #", number.clone()));
    }
    facts.push(("Date", t.date.to_string()));
    facts.push(("Due date", inv.due_date.to_string()));
    let days = t.date.days_until(inv.due_date);
    facts.push((
        "Terms",
        if days <= 0 {
            "Due on receipt".to_string()
        } else {
            format!("Net {days}")
        },
    ));

    let lines = inv
        .lines
        .iter()
        .map(|l| {
            let head = l
                .item
                .and_then(|id| c.item(id))
                .map(|i| i.spec.name.trim().to_string())
                .unwrap_or_default();
            let mut description = l.description.trim().to_string();
            if description == head {
                description.clear();
            }
            let amount = l.amount();
            // A line with no price is a note: it shows only its words.
            let note = l.rate.is_zero() && amount.is_some_and(Money::is_zero);
            PrintLine {
                head,
                description,
                qty: if note {
                    String::new()
                } else {
                    l.qty.to_string()
                },
                rate: if note { String::new() } else { money(l.rate) },
                amount: match amount {
                    Some(a) if !note => money(a),
                    _ => String::new(),
                },
            }
        })
        .collect();

    let mut totals = Vec::new();
    if let Some(tax) = &inv.tax {
        totals.push(TotalLine {
            label: "Subtotal".into(),
            amount: money(inv.subtotal().ok_or("The invoice total is too large to print.")?),
            strong: false,
        });
        for l in &tax.lines {
            totals.push(TotalLine {
                label: format!("{} ({})", l.name, l.rate),
                amount: money(l.amount),
                strong: false,
            });
        }
    }
    totals.push(TotalLine {
        label: "Total".into(),
        amount: money(total),
        strong: t.voided,
    });
    if !t.voided {
        if !applied.is_zero() {
            totals.push(TotalLine {
                label: "Payments applied".into(),
                amount: money(applied.saturating_neg()),
                strong: false,
            });
        }
        totals.push(TotalLine {
            label: "Balance due".into(),
            amount: money(open),
            strong: true,
        });
    }

    Ok(Sheet {
        company: info.name.trim().to_string(),
        company_lines,
        number,
        void: t.voided,
        paid: !t.voided && open.is_zero() && total.is_positive(),
        facts,
        bill_to,
        lines,
        totals,
        memo: t.memo.trim().to_string(),
    })
}

fn layout(s: &Sheet) -> Vec<Page> {
    let mut pages = vec![Page::default()];
    let mut y = first_page_top(page_mut(&mut pages), s);

    for line in &s.lines {
        let rows = line_rows(line);
        let height = row_count(rows.len()) * LINE_HEIGHT + 10.0;
        if y + height > BOTTOM {
            pages.push(Page::default());
            y = continued_top(page_mut(&mut pages), s);
        }
        let p = page_mut(&mut pages);
        let baseline = y + 14.5;
        let mut row_y = baseline;
        for (text, font, color) in &rows {
            p.text(DESC_X, row_y, text, *font, *color, Align::Left);
            row_y += LINE_HEIGHT;
        }
        let body = Font::regular(10.0);
        p.text(QTY_RIGHT, baseline, &line.qty, body, INK, Align::Right);
        p.text(RATE_RIGHT, baseline, &line.rate, body, INK, Align::Right);
        p.text(
            AMOUNT_RIGHT,
            baseline,
            &line.amount,
            body,
            INK,
            Align::Right,
        );
        y += height;
        p.rule(LEFT, RIGHT, y, RULE, 0.6);
    }

    // The totals, with the memo beside them as a note.
    let note = if s.memo.is_empty() {
        Vec::new()
    } else {
        wrap(&s.memo, Font::regular(9.5), DESC_WIDTH)
    };
    let totals_height = 24.0 + 18.0 * row_count(s.totals.len()) + 30.0;
    let note_height = 38.0 + LINE_HEIGHT * row_count(note.len());
    if y + totals_height.max(note_height) > BOTTOM {
        pages.push(Page::default());
        y = continued_top(page_mut(&mut pages), s) - 20.0;
    }
    let p = page_mut(&mut pages);
    totals(p, s, y + 22.0);
    if !note.is_empty() {
        let mut ny = y + 22.0;
        p.text(LEFT, ny, "NOTE", Font::bold(8.5), ACCENT, Align::Left);
        ny += 14.0;
        for l in &note {
            p.text(LEFT, ny, l, Font::regular(9.5), INK, Align::Left);
            ny += LINE_HEIGHT;
        }
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

/// The last page, which `layout` always has.
fn page_mut(pages: &mut Vec<Page>) -> &mut Page {
    if pages.is_empty() {
        pages.push(Page::default());
    }
    let last = pages.len() - 1;
    &mut pages[last]
}

#[allow(clippy::cast_precision_loss)]
fn row_count(n: usize) -> f32 {
    n as f32
}

/// What the description column shows for a line: the item name in bold,
/// then the description, wrapped.
fn line_rows(line: &PrintLine) -> Vec<(String, Font, Rgb)> {
    let mut rows = Vec::new();
    if !line.head.is_empty() {
        let font = Font::bold(10.0);
        rows.extend(
            wrap(&line.head, font, DESC_WIDTH)
                .into_iter()
                .map(|t| (t, font, INK)),
        );
    }
    if !line.description.is_empty() {
        // Under an item name the description is the smaller, quieter text.
        let (font, color) = if line.head.is_empty() {
            (Font::regular(10.0), INK)
        } else {
            (Font::regular(9.0), MUTED)
        };
        rows.extend(
            wrap(&line.description, font, DESC_WIDTH)
                .into_iter()
                .map(|t| (t, font, color)),
        );
    }
    if rows.is_empty() {
        rows.push((String::new(), Font::regular(10.0), INK));
    }
    rows
}

/// The first page's heading, Bill To and column headings; returns where
/// the lines start.
fn first_page_top(p: &mut Page, s: &Sheet) -> f32 {
    p.text(LEFT, 64.0, &s.company, Font::bold(18.0), INK, Align::Left);
    let mut y = 82.0;
    for l in &s.company_lines {
        p.text(LEFT, y, l, Font::regular(9.5), MUTED, Align::Left);
        y += LINE_HEIGHT;
    }
    let left_bottom = y;

    p.text(
        RIGHT,
        66.0,
        "INVOICE",
        Font::bold(24.0),
        ACCENT,
        Align::Right,
    );
    let mut ry = 90.0;
    if s.void {
        p.text(RIGHT, ry, "VOID", Font::bold(12.0), RED, Align::Right);
        ry += 16.0;
    }
    for (label, value) in &s.facts {
        p.text(
            RIGHT - 110.0,
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
    p.text(LEFT, y, "BILL TO", Font::bold(8.5), ACCENT, Align::Left);
    y += 15.0;
    for (i, l) in s.bill_to.iter().enumerate() {
        let (font, color) = if i == 0 {
            (Font::bold(10.5), INK)
        } else {
            (Font::regular(9.5), MUTED)
        };
        p.text(LEFT, y, l, font, color, Align::Left);
        y += 13.5;
    }
    column_headings(p, y + 14.0)
}

/// A later page's heading; returns where the lines continue.
fn continued_top(p: &mut Page, s: &Sheet) -> f32 {
    p.text(LEFT, 60.0, &s.company, Font::bold(11.0), INK, Align::Left);
    let label = if s.number.is_empty() {
        "Invoice (continued)".to_string()
    } else {
        format!("Invoice {} (continued)", s.number)
    };
    p.text(RIGHT, 60.0, &label, Font::regular(9.5), MUTED, Align::Right);
    column_headings(p, 76.0)
}

fn column_headings(p: &mut Page, y: f32) -> f32 {
    p.rect(LEFT, y, RIGHT - LEFT, 20.0, ACCENT);
    let baseline = y + 13.5;
    let font = Font::bold(9.0);
    p.text(DESC_X, baseline, "Description", font, WHITE, Align::Left);
    p.text(QTY_RIGHT, baseline, "Qty", font, WHITE, Align::Right);
    p.text(RATE_RIGHT, baseline, "Rate", font, WHITE, Align::Right);
    p.text(AMOUNT_RIGHT, baseline, "Amount", font, WHITE, Align::Right);
    y + 20.0
}

fn totals(p: &mut Page, s: &Sheet, top: f32) {
    let label_right = RATE_RIGHT - 40.0;
    let mut y = top;
    for t in &s.totals {
        if t.strong {
            p.rule(QTY_RIGHT - 60.0, RIGHT, y - 12.0, INK, 0.8);
            y += 4.0;
            let font = Font::bold(11.5);
            p.text(label_right, y, &t.label, font, INK, Align::Right);
            p.text(AMOUNT_RIGHT, y, &t.amount, font, INK, Align::Right);
        } else {
            let font = Font::regular(10.0);
            p.text(label_right, y, &t.label, font, MUTED, Align::Right);
            p.text(AMOUNT_RIGHT, y, &t.amount, font, INK, Align::Right);
        }
        y += 18.0;
    }
    if s.paid {
        let font = Font::bold(9.5);
        p.text(
            AMOUNT_RIGHT,
            y + 2.0,
            "Paid in full. Thank you!",
            font,
            GREEN,
            Align::Right,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_core::{Date, Qty};
    use ledgercraft_ledger::{AccountLine, Address, Check, Contact, ItemKind, ItemSpec, SalesLine};

    /// Books with one invoice (an item line and a plain line) and one check.
    fn books() -> (Company, TxnId, TxnId) {
        let mut c = Company::with_default_accounts("Pine Ridge Woodworks");
        c.info.address = Address {
            line1: "100 Example Way".into(),
            city: "Provo".into(),
            region: "UT".into(),
            ..Address::default()
        };
        let account = |c: &Company, n: &str| c.account_by_name(n).unwrap().id;
        let sales = account(&c, "Service Income");
        let customer = c
            .add_customer(Contact {
                name: "Dana Ortiz".into(),
                company: "Maple Street Cafe".into(),
                ..Contact::default()
            })
            .unwrap();
        let table = c
            .add_item(ItemSpec {
                name: "Dining Table".into(),
                description: "Solid hardwood".into(),
                kind: ItemKind::NonInventory,
                rate: Money::from_dollars(2400),
                income_account: sales,
                taxable: true,
            })
            .unwrap();
        let date = Date::parse("2026-01-15").unwrap();
        let invoice = TxnBody::Invoice(Invoice {
            customer,
            due_date: Date::parse("2026-02-14").unwrap(),
            ar_account: account(&c, "Accounts Receivable"),
            tax: None,
            lines: vec![
                SalesLine {
                    item: Some(table),
                    description: "Solid hardwood, made to order".into(),
                    qty: Qty::from_whole(2),
                    rate: Money::from_dollars(2400),
                    taxable: true,
                    account: sales,
                },
                SalesLine {
                    item: None,
                    description: "Delivery (München) — “white glove”".into(),
                    qty: Qty::ONE,
                    rate: Money::from_dollars(150),
                    taxable: true,
                    account: sales,
                },
            ],
        });
        let inv = c
            .post(date, "1001", "Thanks for the order!", invoice)
            .unwrap();
        let check = TxnBody::Check(Check {
            payee: None,
            pay_from: account(&c, "Checking"),
            lines: vec![AccountLine {
                account: account(&c, "Rent"),
                description: String::new(),
                amount: Money::from_dollars(300),
            }],
        });
        let chk = c.post(date, "101", "", check).unwrap();
        (c, inv, chk)
    }

    fn text_of(pdf: &[u8]) -> String {
        String::from_utf8_lossy(pdf).into_owned()
    }

    #[test]
    fn an_invoice_prints_with_its_details() {
        let (c, inv, _) = books();
        let text = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(text.starts_with("%PDF-1.4"));
        for shown in [
            "(INVOICE) Tj",
            "(Pine Ridge Woodworks) Tj",
            "(Maple Street Cafe) Tj",
            "(Dana Ortiz) Tj",
            "(Dining Table) Tj",
            "(Net 30) Tj",
            "($4,950.00) Tj",
            "(Balance due) Tj",
            "(Thanks for the order!) Tj",
            "(Page 1 of 1) Tj",
        ] {
            assert!(text.contains(shown), "missing {shown}");
        }
        // ü, the dash and the curly quotes are WinAnsi bytes, not "?".
        assert!(text.contains("M\\374nchen"));
        assert!(!text.contains('?'));
        assert_eq!(invoice_pdf_name(&c, inv), "Invoice 1001.pdf");
    }

    #[test]
    fn only_invoices_print() {
        let (c, _, check) = books();
        assert!(invoice_pdf(&c, check, "$").is_err());
        assert!(invoice_pdf(&c, TxnId(999_999), "$").is_err());
    }

    #[test]
    fn a_long_invoice_runs_onto_more_pages() {
        let (mut c, inv, _) = books();
        let t = c.transactions.iter_mut().find(|t| t.id == inv).unwrap();
        if let TxnBody::Invoice(invoice) = &mut t.body {
            let line = invoice.lines[0].clone();
            invoice.lines = vec![line; 60];
        }
        let text = text_of(&invoice_pdf(&c, inv, "€").unwrap());
        assert!(!text.contains("(Page 1 of 1) Tj"));
        assert!(text.contains("(Page 2 of "));
        assert!(text.contains("(Invoice 1001 \\(continued\\)) Tj"));
        // € is byte 0x80 in WinAnsi, written as an octal escape.
        assert!(text.contains("\\200"));
    }

    #[test]
    fn sales_tax_prints_under_the_subtotal() {
        let (mut c, inv, _) = books();
        let from = Date::parse("2020-01-01").unwrap();
        let rate = ledgercraft_ledger::TaxRate::parse("4.85").unwrap();
        let state = c.add_tax_component("Utah State", "", "", from, rate).unwrap();
        let group = c.add_tax_group("Statewide", vec![state]).unwrap();
        let date = c.txn(inv).unwrap().date;
        // $4,950.00 taxable at 4.85% is $240.08 (240.075, rounded up).
        let tax = c.invoice_tax(group, date, Money::from_dollars(4950)).unwrap();
        if let Some(t) = c.transactions.iter_mut().find(|t| t.id == inv) {
            if let TxnBody::Invoice(invoice) = &mut t.body {
                invoice.tax = Some(tax);
            }
        }
        let text = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(text.contains("(Subtotal) Tj"));
        assert!(text.contains("(Utah State \\(4.85%\\)) Tj"));
        assert!(text.contains("($240.08) Tj"));
        assert!(text.contains("($5,190.08) Tj"));
    }

    #[test]
    fn a_void_invoice_says_so_and_owes_nothing() {
        let (mut c, inv, _) = books();
        if let Some(t) = c.transactions.iter_mut().find(|t| t.id == inv) {
            t.voided = true;
        }
        let text = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(text.contains("(VOID) Tj"));
        assert!(!text.contains("(Balance due) Tj"));
    }
}
