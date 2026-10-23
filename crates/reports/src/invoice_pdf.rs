//! An invoice, sales receipt or credit memo as a PDF, ready to email or
//! print.
//!
//! The layout: the company's name and contact details at the top left, the
//! word INVOICE and the invoice's number, dates and terms at the top right,
//! who it is billed to, the lines, then the totals and balance due, with the
//! memo as a note. Long invoices continue on further pages, each repeating
//! the column headings; every page is numbered.

use ledgercraft_core::Money;
use ledgercraft_ledger::{Company, CustomerId, Sale, Transaction, TxnBody, TxnId};

use crate::look::{letterhead, Look};
use crate::pdf::{finish_with, wrap, Align, Font, Image, Page, Rgb};

pub(crate) const LEFT: f32 = 50.0;
pub(crate) const RIGHT: f32 = 562.0;
/// Content stops here; the footer sits below.
pub(crate) const BOTTOM: f32 = 730.0;

const DESC_X: f32 = 58.0;
const DESC_WIDTH: f32 = 270.0;
const QTY_RIGHT: f32 = 380.0;
const RATE_RIGHT: f32 = 468.0;
const AMOUNT_RIGHT: f32 = 554.0;
pub(crate) const LINE_HEIGHT: f32 = 13.0;

pub(crate) const ACCENT: Rgb = Rgb(0.122, 0.435, 0.545);
pub(crate) const INK: Rgb = Rgb(0.13, 0.13, 0.13);
pub(crate) const MUTED: Rgb = Rgb(0.42, 0.45, 0.50);
pub(crate) const RULE: Rgb = Rgb(0.82, 0.84, 0.87);
pub(crate) const WHITE: Rgb = Rgb(1.0, 1.0, 1.0);
pub(crate) const RED: Rgb = Rgb(0.75, 0.15, 0.15);
pub(crate) const GREEN: Rgb = Rgb(0.12, 0.50, 0.27);

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
    /// The template's colours and style.
    look: Look,
    /// The company's logo, drawn as image 0.
    logo: Option<Image>,
    /// INVOICE, SALES RECEIPT or CREDIT MEMO.
    heading: &'static str,
    /// Invoice, Sales Receipt or Credit Memo, for "continued" lines.
    kind: &'static str,
    /// BILL TO, SOLD TO or CREDIT TO.
    to_label: &'static str,
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

/// Whether a transaction prints as a PDF: invoices, sales receipts and
/// credit memos do.
pub fn has_pdf(body: &TxnBody) -> bool {
    body.sale().is_some()
}

/// A suggested file name: `Invoice 1001.pdf`, `Credit Memo 12.pdf`.
pub fn invoice_pdf_name(company: &Company, id: TxnId) -> String {
    match company.txn(id) {
        Some(t) if !t.number.trim().is_empty() => {
            format!("{} {}.pdf", t.body.label(), t.number.trim())
        }
        Some(t) => format!("{}.pdf", t.body.label()),
        None => "Invoice.pdf".into(),
    }
}

/// An invoice, sales receipt or credit memo as a PDF file. `currency` is the
/// symbol amounts are shown with, such as `$` or `€`.
pub fn invoice_pdf(company: &Company, id: TxnId, currency: &str) -> Result<Vec<u8>, String> {
    let t = company
        .txn(id)
        .ok_or_else(|| format!("Transaction {id} was not found."))?;
    let Some(sale) = t.body.sale() else {
        return Err(format!(
            "A {} has no PDF; invoices, sales receipts and credit memos do.",
            t.body.label()
        ));
    };
    let sheet = gather(company, t, &sale, currency)?;
    let title = if sheet.number.is_empty() {
        format!(
            "{} for {}",
            sheet.kind,
            sheet.bill_to.first().map_or("", String::as_str)
        )
    } else {
        format!("{} {}", sheet.kind, sheet.number)
    };
    Ok(finish_with(&layout(&sheet), &title, sheet.logo.as_slice()))
}

/// The company's contact lines under its name: legal name (if different),
/// address, phone and email.
pub(crate) fn company_lines(c: &Company) -> Vec<String> {
    let info = &c.info;
    let mut lines = Vec::new();
    if !info.legal_name.trim().is_empty() && info.legal_name.trim() != info.name.trim() {
        lines.push(info.legal_name.trim().to_string());
    }
    lines.extend(info.address.lines());
    for extra in [&info.phone, &info.email] {
        if !extra.trim().is_empty() {
            lines.push(extra.trim().to_string());
        }
    }
    lines
}

/// Who a document is addressed to: the customer's company and name, address,
/// email, and any sales tax exemption. Never empty.
pub(crate) fn bill_to(c: &Company, customer: CustomerId) -> Vec<String> {
    let mut bill_to = Vec::new();
    if let Some(customer) = c.customer(customer) {
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
    bill_to
}

fn gather(c: &Company, t: &Transaction, sale: &Sale<'_>, currency: &str) -> Result<Sheet, String> {
    let money = |m: Money| m.format_with(currency);
    let too_large = || format!("The {} total is too large to print.", t.body.label());
    let total = sale.total().ok_or_else(too_large)?;
    // What is still owed on an invoice, or left of a credit; nothing on a
    // sales receipt, which is paid when it is made.
    let open = match &t.body {
        _ if t.voided => Money::ZERO,
        TxnBody::SalesReceipt(_) => Money::ZERO,
        _ => c.open_balance(t.id, None),
    };
    let applied = total.saturating_sub(open);

    let info = &c.info;
    let company_lines = company_lines(c);
    let bill_to = match sale.customer {
        Some(customer) => bill_to(c, customer),
        None => Vec::new(),
    };

    let number = t.number.trim().to_string();
    let mut facts = Vec::new();
    let (heading, to_label, number_label) = match &t.body {
        TxnBody::SalesReceipt(_) => ("SALES RECEIPT", "SOLD TO", "Receipt #"),
        TxnBody::CreditMemo(_) => ("CREDIT MEMO", "CREDIT TO", "Credit memo #"),
        _ => ("INVOICE", "BILL TO", "Invoice #"),
    };
    if !number.is_empty() {
        facts.push((number_label, number.clone()));
    }
    facts.push(("Date", t.date.to_string()));
    match &t.body {
        TxnBody::Invoice(inv) => {
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
        }
        TxnBody::SalesReceipt(r) if !r.method.trim().is_empty() => {
            facts.push(("Paid by", r.method.trim().to_string()));
        }
        _ => {}
    }

    let lines = sale
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
    if let Some(tax) = sale.tax {
        totals.push(TotalLine {
            label: "Subtotal".into(),
            amount: money(sale.subtotal().ok_or_else(too_large)?),
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
    let credit = matches!(t.body, TxnBody::CreditMemo(_));
    let receipt = matches!(t.body, TxnBody::SalesReceipt(_));
    totals.push(TotalLine {
        label: if credit { "Total credit" } else { "Total" }.into(),
        amount: money(total),
        strong: t.voided || receipt,
    });
    if !t.voided && !receipt {
        if !applied.is_zero() {
            totals.push(TotalLine {
                label: if credit {
                    "Applied or refunded"
                } else {
                    "Payments applied"
                }
                .into(),
                amount: money(applied.saturating_neg()),
                strong: false,
            });
        }
        totals.push(TotalLine {
            label: if credit {
                "Credit remaining"
            } else {
                "Balance due"
            }
            .into(),
            amount: money(open),
            strong: true,
        });
    }

    Ok(Sheet {
        look: Look::of(c.info.print.template),
        logo: crate::look::logo(c),
        heading,
        kind: t.body.label(),
        to_label,
        company: info.name.trim().to_string(),
        company_lines,
        number,
        void: t.voided,
        // A sales receipt is paid; a used-up credit memo is not "paid".
        paid: !t.voided && !credit && open.is_zero() && total.is_positive(),
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
        p.text(LEFT, ny, "NOTE", Font::bold(8.5), s.look.accent, Align::Left);
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
pub(crate) fn page_mut(pages: &mut Vec<Page>) -> &mut Page {
    if pages.is_empty() {
        pages.push(Page::default());
    }
    let last = pages.len() - 1;
    &mut pages[last]
}

#[allow(clippy::cast_precision_loss)]
pub(crate) fn row_count(n: usize) -> f32 {
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
    let left_bottom = letterhead(p, s.logo.as_ref(), &s.company, &s.company_lines);

    p.text(
        RIGHT,
        66.0,
        s.heading,
        Font::bold(24.0),
        s.look.accent,
        Align::Right,
    );
    let mut ry = 90.0;
    if s.void {
        p.text(RIGHT, ry, "VOID", Font::bold(12.0), s.look.warning, Align::Right);
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
    if !s.bill_to.is_empty() {
        p.text(LEFT, y, s.to_label, Font::bold(8.5), s.look.accent, Align::Left);
        y += 15.0;
    }
    for (i, l) in s.bill_to.iter().enumerate() {
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

/// A later page's heading; returns where the lines continue.
fn continued_top(p: &mut Page, s: &Sheet) -> f32 {
    p.text(LEFT, 60.0, &s.company, Font::bold(11.0), INK, Align::Left);
    let label = if s.number.is_empty() {
        format!("{} (continued)", s.kind)
    } else {
        format!("{} {} (continued)", s.kind, s.number)
    };
    p.text(RIGHT, 60.0, &label, Font::regular(9.5), MUTED, Align::Right);
    column_headings(p, s, 76.0)
}

fn column_headings(p: &mut Page, s: &Sheet, y: f32) -> f32 {
    s.look.headings(
        p,
        y,
        9.0,
        &[
            (DESC_X, "Description", Align::Left),
            (QTY_RIGHT, "Qty", Align::Right),
            (RATE_RIGHT, "Rate", Align::Right),
            (AMOUNT_RIGHT, "Amount", Align::Right),
        ],
    )
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
            s.look.good,
            Align::Right,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_core::{Date, Qty};
    use ledgercraft_ledger::{
        AccountLine, Address, Check, Contact, Invoice, ItemKind, ItemSpec, SalesLine,
    };

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
        let state = c
            .add_tax_component("Utah State", "", "", from, rate)
            .unwrap();
        let group = c.add_tax_group("Statewide", vec![state]).unwrap();
        let date = c.txn(inv).unwrap().date;
        // $4,950.00 taxable at 4.85% is $240.08 (240.075, rounded up).
        let tax = c
            .invoice_tax(group, date, Money::from_dollars(4950))
            .unwrap();
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

    #[test]
    fn credit_memos_and_sales_receipts_print_with_their_own_headings() {
        let (mut c, inv, _) = books();
        let sales = c.account_by_name("Sales").unwrap().id;
        let line = SalesLine {
            item: None,
            description: "Returned chair".into(),
            qty: Qty::ONE,
            rate: Money::from_dollars(150),
            taxable: false,
            account: sales,
        };
        let customer = match &c.txn(inv).unwrap().body {
            TxnBody::Invoice(i) => i.customer,
            _ => CustomerId(0),
        };
        // $150 credited, $100 of it against invoice 1001.
        let memo = ledgercraft_ledger::CreditMemo {
            customer,
            ar_account: c.account_by_name("Accounts Receivable").unwrap().id,
            lines: vec![line.clone()],
            tax: None,
            applied: vec![ledgercraft_ledger::Applied {
                txn: inv,
                amount: Money::from_dollars(100),
            }],
        };
        let date = Date::parse("2026-01-20").unwrap();
        let memo = c.post(date, "CM-7", "", TxnBody::CreditMemo(memo)).unwrap();
        let text = text_of(&invoice_pdf(&c, memo, "$").unwrap());
        for shown in [
            "(CREDIT MEMO) Tj",
            "(CREDIT TO) Tj",
            "(Total credit) Tj",
            "(Credit remaining) Tj",
            "($50.00) Tj",
        ] {
            assert!(text.contains(shown), "missing {shown}");
        }
        assert_eq!(invoice_pdf_name(&c, memo), "Credit Memo CM-7.pdf");
        // The invoice shows the credit among what was applied to it.
        let text = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(text.contains("($4,850.00) Tj"));

        // A walk-in sale: no customer block, paid in full.
        let receipt = ledgercraft_ledger::SalesReceipt {
            customer: None,
            deposit_to: c.account_by_name("Checking").unwrap().id,
            method: "Cash".into(),
            lines: vec![line],
            tax: None,
        };
        let receipt = c
            .post(date, "SR-1", "", TxnBody::SalesReceipt(receipt))
            .unwrap();
        let text = text_of(&invoice_pdf(&c, receipt, "$").unwrap());
        assert!(text.contains("(SALES RECEIPT) Tj"));
        assert!(text.contains("(Cash) Tj"));
        assert!(text.contains("(Paid in full. Thank you!) Tj"));
        assert!(!text.contains("(SOLD TO) Tj"));
        assert!(!text.contains("(Balance due) Tj"));
    }

    #[test]
    fn templates_change_the_look() {
        let (mut c, inv, _) = books();
        let accent_fill = "0.122 0.435 0.545 rg";
        let classic = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        // Classic: column headings on a filled band.
        assert!(classic.contains(accent_fill) && classic.contains(" re f"));
        c.info.print.template = ledgercraft_ledger::PdfTemplate::Modern;
        let modern = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(modern.contains(accent_fill) && !modern.contains(" re f"));
        c.info.print.template = ledgercraft_ledger::PdfTemplate::Simple;
        let simple = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(!simple.contains(accent_fill) && !simple.contains(" re f"));
        assert!(simple.contains("(INVOICE) Tj"));
    }

    #[test]
    fn the_logo_prints_at_the_top() {
        let (mut c, inv, _) = books();
        // A 2 x 1 PNG, one pixel see-through.
        let png = crate::logo::tests::png_file(2, 1, 8, 6, &[0, 255, 0, 0, 0, 0, 0, 255, 255]);
        c.info.print.logo = Some(ledgercraft_ledger::Logo(png));
        let text = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(text.contains("/XObject << /Im1 "));
        assert!(text.contains("/Subtype /Image /Width 2 /Height 1"));
        assert!(text.contains("/SMask "), "its transparency comes along");
        assert!(text.contains("/Im1 Do"));
        // A logo that cannot be read is left off; the invoice still prints.
        c.info.print.logo = Some(ledgercraft_ledger::Logo(b"not an image".to_vec()));
        let text = text_of(&invoice_pdf(&c, inv, "$").unwrap());
        assert!(!text.contains("/Im1 Do"));
        assert!(text.contains("(INVOICE) Tj"));
    }
}
