//! Transaction entry forms. Each form keeps what the user typed as text and
//! only turns it into a [`Command`] when they press Save, so a half-typed
//! amount never reaches the books.

use std::collections::BTreeMap;

use eframe::egui::{self, RichText, Ui};
use ledgercraft_core::{Date, Money, Qty};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{
    AccountId, AccountLine, AccountType, Applied, Bill, Check, Company, CustomerId, Deposit,
    Invoice, ItemId, JournalEntry, JournalLine, Party, PayBill, ReceivePayment, SalesLine,
    Transaction, TxnBody, TxnId, VendorId,
};

use crate::widgets::{self, field, money_label};
use crate::{currency, theme, Action, FormKind, Reset};

fn today() -> String {
    Date::today().to_string()
}

fn parse_date(label: &str, s: &str) -> Result<Date, String> {
    Date::parse(s).map_err(|e| format!("{label}: {e}"))
}

// ----- state ----------------------------------------------------------------

#[derive(Clone, Default)]
pub(crate) struct SalesLineInput {
    pub item: Option<ItemId>,
    pub description: String,
    pub qty: String,
    pub rate: String,
    pub account: Option<AccountId>,
}

impl SalesLineInput {
    fn is_blank(&self) -> bool {
        self.item.is_none()
            && self.description.trim().is_empty()
            && self.rate.trim().is_empty()
            && self.account.is_none()
    }
}

pub(crate) struct InvoiceForm {
    /// The invoice being edited, if this is not a new one.
    pub editing: Option<TxnId>,
    pub customer: Option<CustomerId>,
    pub date: String,
    pub due: String,
    pub number: String,
    pub memo: String,
    pub lines: Vec<SalesLineInput>,
}

impl Default for InvoiceForm {
    fn default() -> Self {
        Self {
            editing: None,
            customer: None,
            date: today(),
            due: String::new(),
            number: String::new(),
            memo: String::new(),
            lines: vec![SalesLineInput::default()],
        }
    }
}

/// Receive Payment (customer side) or Pay Bills (vendor side).
pub(crate) struct PaymentForm {
    /// The payment being edited, if this is not a new one.
    pub editing: Option<TxnId>,
    pub customer: Option<CustomerId>,
    pub vendor: Option<VendorId>,
    pub date: String,
    pub number: String,
    pub memo: String,
    pub method: String,
    pub account: Option<AccountId>,
    pub amounts: BTreeMap<TxnId, String>,
}

impl Default for PaymentForm {
    fn default() -> Self {
        Self {
            editing: None,
            customer: None,
            vendor: None,
            date: today(),
            number: String::new(),
            memo: String::new(),
            method: String::new(),
            account: None,
            amounts: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct AccountLineInput {
    pub account: Option<AccountId>,
    pub description: String,
    pub amount: String,
}

impl AccountLineInput {
    fn is_blank(&self) -> bool {
        self.account.is_none()
            && self.description.trim().is_empty()
            && self.amount.trim().is_empty()
    }
}

/// Enter Bill, Write Check and Make Deposit share this shape.
pub(crate) struct LinesForm {
    /// The bill, check or deposit being edited, if this is not a new one.
    pub editing: Option<TxnId>,
    pub vendor: Option<VendorId>,
    pub payee: Option<Party>,
    pub account: Option<AccountId>,
    pub date: String,
    pub due: String,
    pub number: String,
    pub memo: String,
    pub lines: Vec<AccountLineInput>,
}

impl Default for LinesForm {
    fn default() -> Self {
        Self {
            editing: None,
            vendor: None,
            payee: None,
            account: None,
            date: today(),
            due: String::new(),
            number: String::new(),
            memo: String::new(),
            lines: vec![AccountLineInput::default()],
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct JournalLineInput {
    pub account: Option<AccountId>,
    pub debit: String,
    pub credit: String,
    pub memo: String,
    pub party: Option<Party>,
}

impl JournalLineInput {
    fn is_blank(&self) -> bool {
        self.account.is_none()
            && self.debit.trim().is_empty()
            && self.credit.trim().is_empty()
            && self.memo.trim().is_empty()
            && self.party.is_none()
    }
}

pub(crate) struct JournalForm {
    /// The journal entry being edited, if this is not a new one.
    pub editing: Option<TxnId>,
    pub date: String,
    pub number: String,
    pub memo: String,
    pub lines: Vec<JournalLineInput>,
}

impl Default for JournalForm {
    fn default() -> Self {
        Self {
            editing: None,
            date: today(),
            number: String::new(),
            memo: String::new(),
            lines: vec![JournalLineInput::default(); 2],
        }
    }
}

#[derive(Default)]
pub(crate) struct Forms {
    pub invoice: InvoiceForm,
    pub receive: PaymentForm,
    pub bill: LinesForm,
    pub pay_bills: PaymentForm,
    pub check: LinesForm,
    pub deposit: LinesForm,
    pub journal: JournalForm,
}

impl Forms {
    pub(crate) fn reset(&mut self, kind: FormKind) {
        match kind {
            FormKind::Invoice => self.invoice = InvoiceForm::default(),
            FormKind::ReceivePayment => self.receive = PaymentForm::default(),
            FormKind::Bill => self.bill = LinesForm::default(),
            FormKind::PayBills => self.pay_bills = PaymentForm::default(),
            FormKind::Check => self.check = LinesForm::default(),
            FormKind::Deposit => self.deposit = LinesForm::default(),
            FormKind::Journal => self.journal = JournalForm::default(),
        }
    }

    /// Fills the matching form with a recorded transaction so it can be
    /// edited, and says which form that is.
    pub(crate) fn load(&mut self, t: &Transaction) -> Result<FormKind, String> {
        if t.voided {
            return Err(format!(
                "{} {} is void. A void transaction cannot be edited.",
                t.body.label(),
                t.number
            ));
        }
        let editing = Some(t.id);
        let date = t.date.to_string();
        let number = t.number.clone();
        let memo = t.memo.clone();
        Ok(match &t.body {
            TxnBody::Invoice(inv) => {
                self.invoice = InvoiceForm {
                    editing,
                    customer: Some(inv.customer),
                    date,
                    due: inv.due_date.to_string(),
                    number,
                    memo,
                    lines: inv
                        .lines
                        .iter()
                        .map(|l| SalesLineInput {
                            item: l.item,
                            description: l.description.clone(),
                            qty: l.qty.to_string(),
                            rate: l.rate.to_plain_string(),
                            account: Some(l.account),
                        })
                        .collect(),
                };
                FormKind::Invoice
            }
            TxnBody::ReceivePayment(p) => {
                self.receive = PaymentForm {
                    editing,
                    customer: Some(p.customer),
                    vendor: None,
                    date,
                    number,
                    memo,
                    method: p.method.clone(),
                    account: Some(p.deposit_to),
                    amounts: applied_amounts(&p.applied),
                };
                FormKind::ReceivePayment
            }
            TxnBody::PayBill(p) => {
                self.pay_bills = PaymentForm {
                    editing,
                    customer: None,
                    vendor: Some(p.vendor),
                    date,
                    number,
                    memo,
                    method: String::new(),
                    account: Some(p.pay_from),
                    amounts: applied_amounts(&p.applied),
                };
                FormKind::PayBills
            }
            TxnBody::Bill(b) => {
                self.bill = LinesForm {
                    editing,
                    vendor: Some(b.vendor),
                    payee: None,
                    account: None,
                    date,
                    due: b.due_date.to_string(),
                    number,
                    memo,
                    lines: account_line_inputs(&b.lines),
                };
                FormKind::Bill
            }
            TxnBody::Check(ch) => {
                self.check = LinesForm {
                    editing,
                    vendor: None,
                    payee: ch.payee,
                    account: Some(ch.pay_from),
                    date,
                    due: String::new(),
                    number,
                    memo,
                    lines: account_line_inputs(&ch.lines),
                };
                FormKind::Check
            }
            TxnBody::Deposit(d) => {
                self.deposit = LinesForm {
                    editing,
                    vendor: None,
                    payee: None,
                    account: Some(d.deposit_to),
                    date,
                    due: String::new(),
                    number,
                    memo,
                    lines: account_line_inputs(&d.lines),
                };
                FormKind::Deposit
            }
            TxnBody::Journal(j) => {
                let blank_if_zero = |m: Money| {
                    if m.is_zero() {
                        String::new()
                    } else {
                        m.to_plain_string()
                    }
                };
                self.journal = JournalForm {
                    editing,
                    date,
                    number,
                    memo,
                    lines: j
                        .lines
                        .iter()
                        .map(|l| JournalLineInput {
                            account: Some(l.account),
                            debit: blank_if_zero(l.debit),
                            credit: blank_if_zero(l.credit),
                            memo: l.memo.clone(),
                            party: l.party,
                        })
                        .collect(),
                };
                FormKind::Journal
            }
        })
    }
}

fn applied_amounts(applied: &[Applied]) -> BTreeMap<TxnId, String> {
    applied
        .iter()
        .map(|a| (a.txn, a.amount.to_plain_string()))
        .collect()
}

fn account_line_inputs(lines: &[AccountLine]) -> Vec<AccountLineInput> {
    lines
        .iter()
        .map(|l| AccountLineInput {
            account: Some(l.account),
            description: l.description.clone(),
            amount: l.amount.to_plain_string(),
        })
        .collect()
}

// ----- shared pieces ----------------------------------------------------------

/// Keeps exactly one empty line at the end of a form's lines (and at least
/// `min` lines), so there is always somewhere to type the next one: typing in
/// the last line adds a new one, and spare empty lines at the end go away.
fn tidy_lines<T: Default>(lines: &mut Vec<T>, min: usize, is_blank: impl Fn(&T) -> bool) {
    while lines.len() >= 2 && lines.len() > min && lines.iter().rev().take(2).all(&is_blank) {
        lines.pop();
    }
    if lines.last().is_none_or(|l| !is_blank(l)) {
        lines.push(T::default());
    }
    while lines.len() < min {
        lines.push(T::default());
    }
}

fn header(ui: &mut Ui, kind: FormKind, editing: Option<TxnId>) {
    if editing.is_some() {
        ui.label(
            RichText::new(format!("Edit {}", kind.title()))
                .heading()
                .color(theme::ACCENT),
        );
        ui.label(
            RichText::new(
                "You are changing a transaction that is already recorded. Save Changes \
                 replaces it; Edit > Undo puts the original back.",
            )
            .color(theme::MUTED),
        );
    } else {
        ui.label(RichText::new(kind.title()).heading().color(theme::ACCENT));
    }
    ui.add_space(6.0);
}

/// The Save / Clear buttons, or Save Changes / Cancel while editing.
/// Returns (save, clear or cancel).
fn footer(ui: &mut Ui, editing: Option<TxnId>) -> (bool, bool) {
    let (save, clear, _) = footer_with(ui, editing, None);
    (save, clear)
}

/// The footer with one more button, `(label, hover)`, before Clear/Cancel;
/// the third value says whether it was clicked.
fn footer_with(
    ui: &mut Ui,
    editing: Option<TxnId>,
    extra: Option<(&str, &str)>,
) -> (bool, bool, bool) {
    ui.add_space(10.0);
    let mut save = false;
    let mut clear = false;
    let mut more = false;
    let (save_label, clear_label) = if editing.is_some() {
        ("Save Changes", "Cancel")
    } else {
        ("Save & New", "Clear")
    };
    ui.horizontal_wrapped(|ui| {
        save = ui.button(RichText::new(save_label).strong()).clicked();
        if let Some((label, hover)) = extra {
            more = ui.button(label).on_hover_text(hover).clicked();
        }
        clear = ui.button(clear_label).clicked();
    });
    (save, clear, more)
}

/// Runs what the form built: a new transaction, or, while editing, the
/// changes to the one being edited.
fn finish(
    out: &mut Vec<Action>,
    kind: FormKind,
    editing: Option<TxnId>,
    built: Result<Command, String>,
) {
    match command_for(editing, built) {
        Ok(cmd) => out.push(Action::Run {
            cmd,
            reset: Reset::Form(kind),
        }),
        Err(e) => out.push(Action::Error(e)),
    }
}

/// What saving the form runs: posting a new transaction, or, while editing,
/// updating the recorded one.
fn command_for(editing: Option<TxnId>, built: Result<Command, String>) -> Result<Command, String> {
    built.map(|cmd| match (editing, cmd) {
        (
            Some(id),
            Command::TxnPost {
                date,
                number,
                memo,
                body,
            },
        ) => Command::TxnUpdate {
            id,
            date,
            number,
            memo,
            body,
        },
        (_, cmd) => cmd,
    })
}

/// The account box on invoice and journal lines (narrower than elsewhere, so
/// the lines fit the smallest window).
const LINE_ACCOUNT_WIDTH: f32 = 170.0;
/// What an invoice line's other columns take: item 130, qty 50, rate 80,
/// account, amount about 85, the remove button about 22, and the gaps.
const INVOICE_FIXED: f32 = 130.0 + 50.0 + 80.0 + LINE_ACCOUNT_WIDTH + 85.0 + 22.0 + 6.0 * 8.0;
/// Bill, check and deposit lines: account 230, amount 90, remove, gaps.
const ACCOUNT_LINES_FIXED: f32 = 230.0 + 90.0 + 22.0 + 3.0 * 8.0;
/// Journal lines: account, debit, credit, name about 170, remove, gaps.
const JOURNAL_FIXED: f32 = LINE_ACCOUNT_WIDTH + 90.0 + 90.0 + 170.0 + 22.0 + 5.0 * 8.0;

/// The width left for a line's stretchy text box once the fixed columns are
/// placed, kept between `min` and `max`: the lines fill a wide window and
/// still fit a small one.
fn flex_width(ui: &Ui, fixed: f32, min: f32, max: f32) -> f32 {
    (ui.available_width() - fixed).clamp(min, max)
}

/// Part of a line grid's id: a grid remembers its column widths, so when the
/// stretchy column changes size (in 10-point steps) the grid starts afresh
/// instead of holding on to the old, wider column.
// Widths are clamped to a few hundred points, far inside i32.
#[allow(clippy::cast_possible_truncation)]
fn width_step(width: f32) -> i32 {
    (width / 10.0).round() as i32
}

/// Narrower than this, a form's heading fields go one to a row.
const HEAD_TWO_PAIRS_WIDTH: f32 = 640.0;

/// Where a heading row ends: after every second label-and-field pair while
/// two fit side by side, after every pair in a narrow window.
#[derive(Clone, Copy)]
struct HeadPairs {
    two_per_row: bool,
}

impl HeadPairs {
    /// Call after each pair's field; `second` says it is the right-hand pair.
    fn end(self, ui: &mut Ui, second: bool) {
        if second || !self.two_per_row {
            ui.end_row();
        }
    }
}

/// A form's heading fields as label-and-field pairs, labels lined up, two
/// pairs to a row or one in a narrow window.
fn head_grid(ui: &mut Ui, salt: &str, add: impl FnOnce(&mut Ui, HeadPairs)) {
    let two_per_row = ui.available_width() >= HEAD_TWO_PAIRS_WIDTH;
    let columns = if two_per_row { 4 } else { 2 };
    // Each layout keeps its own grid, so remembered column widths from one
    // never squeeze the other.
    egui::Grid::new((salt, two_per_row))
        .num_columns(columns)
        .spacing([12.0, 6.0])
        .show(ui, |ui| add(ui, HeadPairs { two_per_row }));
}

/// Clear on a new form; Cancel on a form editing a recorded transaction.
fn cleared(out: &mut Vec<Action>, editing: Option<TxnId>) {
    out.push(if editing.is_some() {
        Action::EditCancelled
    } else {
        Action::ClearStatus
    });
}

fn total_row(ui: &mut Ui, label: &str, total: Money) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).strong());
        ui.label(RichText::new(currency::format(total)).strong().size(16.0));
    });
}

// ----- invoice ------------------------------------------------------------------

pub(crate) fn invoice(ui: &mut Ui, c: &Company, f: &mut InvoiceForm, out: &mut Vec<Action>) {
    header(ui, FormKind::Invoice, f.editing);
    head_grid(ui, "inv_head", |ui, pairs| {
            widgets::hint_label(ui, "Customer");
            if widgets::customer_combo(ui, "inv_customer", c, &mut f.customer) {
                set_due_from_terms(c, f);
            }
            pairs.end(ui, false);
            widgets::hint_label(ui, "Invoice #");
            let hint = widgets::next_number(c, 1001, |b| matches!(b, TxnBody::Invoice(_)));
            field(ui, &mut f.number, 100.0, &hint);
            pairs.end(ui, true);
            widgets::hint_label(ui, "Date");
            if field(ui, &mut f.date, 110.0, "YYYY-MM-DD").changed() {
                set_due_from_terms(c, f);
            }
            pairs.end(ui, false);
            widgets::hint_label(ui, "Due date");
            field(ui, &mut f.due, 110.0, "from terms");
            pairs.end(ui, true);
        });
    if let Some(cust) = f.customer.and_then(|id| c.customer(id)) {
        let addr = cust.contact.address.lines().join(", ");
        if !addr.is_empty() {
            ui.label(RichText::new(format!("Bill to: {addr}")).color(theme::MUTED));
        }
    }
    ui.add_space(8.0);

    let mut remove = None;
    let mut total = Money::ZERO;
    // Description takes the room the other columns leave.
    let description_width = flex_width(ui, INVOICE_FIXED, 100.0, 420.0);
    egui::Grid::new(("inv_lines", width_step(description_width)))
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for h in [
                "Item",
                "Description",
                "Qty",
                "Rate",
                "Income account",
                "Amount",
                "",
            ] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for (i, line) in f.lines.iter_mut().enumerate() {
                if widgets::item_combo(ui, ("inv_item", i), c, &mut line.item) {
                    if let Some(item) = line.item.and_then(|id| c.item(id)) {
                        line.description = item.spec.description.clone();
                        line.rate = item.spec.rate.to_plain_string();
                        line.account = Some(item.spec.income_account);
                        if line.qty.trim().is_empty() {
                            line.qty = "1".into();
                        }
                    }
                }
                field(ui, &mut line.description, description_width, "");
                field(ui, &mut line.qty, 50.0, "1");
                field(ui, &mut line.rate, 80.0, "0.00");
                widgets::account_combo_width(
                    ui,
                    ("inv_acct", i),
                    c,
                    &mut line.account,
                    |k| !k.needs_party(),
                    LINE_ACCOUNT_WIDTH,
                );
                let qty = if line.qty.trim().is_empty() {
                    Ok(Qty::ONE)
                } else {
                    Qty::parse(&line.qty)
                };
                // Qty x Rate, shown from the start: a blank rate counts as 0.
                match (qty, Money::parse_or_zero(&line.rate)) {
                    (Ok(q), Ok(r)) => {
                        let amount = r.times(q).unwrap_or(Money::ZERO);
                        total = total.saturating_add(amount);
                        money_label(ui, amount, false);
                    }
                    _ => {
                        ui.label(
                            RichText::new("check qty / rate")
                                .small()
                                .color(theme::NEGATIVE),
                        );
                    }
                }
                if widgets::remove_button(ui) {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        if i < f.lines.len() {
            f.lines.remove(i);
        }
    }
    tidy_lines(&mut f.lines, 1, SalesLineInput::is_blank);
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Total", total);
    let hover = if f.editing.is_some() {
        "Save your changes, then save the invoice as a PDF to email or print"
    } else {
        "Record the invoice, then save it as a PDF to email or print"
    };
    let (save, clear, pdf) = footer_with(ui, f.editing, Some(("Save as PDF…", hover)));
    if save {
        finish(out, FormKind::Invoice, f.editing, build_invoice(c, f));
    }
    if pdf {
        match command_for(f.editing, build_invoice(c, f)) {
            Ok(cmd) => out.push(Action::SaveInvoicePdf(cmd)),
            Err(e) => out.push(Action::Error(e)),
        }
    }
    if clear {
        let editing = f.editing;
        *f = InvoiceForm::default();
        cleared(out, editing);
    }
}

fn set_due_from_terms(c: &Company, f: &mut InvoiceForm) {
    let terms = f
        .customer
        .and_then(|id| c.customer(id))
        .map(|x| x.contact.terms_days);
    if let (Some(terms), Ok(date)) = (terms, Date::parse(&f.date)) {
        f.due = date.add_days(i64::from(terms)).to_string();
    }
}

fn build_invoice(c: &Company, f: &InvoiceForm) -> Result<Command, String> {
    let customer = f.customer.ok_or("Choose a customer")?;
    let date = parse_date("Date", &f.date)?;
    let due_date = if f.due.trim().is_empty() {
        date
    } else {
        parse_date("Due date", &f.due)?
    };
    let ar_account = c
        .first_account_of(AccountType::AccountsReceivable)
        .ok_or("Add an Accounts Receivable account first")?;
    let mut lines = Vec::new();
    for (i, l) in f.lines.iter().enumerate().filter(|(_, l)| !l.is_blank()) {
        let n = i + 1;
        let account = l
            .account
            .ok_or_else(|| format!("Line {n}: choose an income account"))?;
        let qty = if l.qty.trim().is_empty() {
            Qty::ONE
        } else {
            Qty::parse(&l.qty).map_err(|e| format!("Line {n}: {e}"))?
        };
        let rate = Money::parse(&l.rate).map_err(|e| format!("Line {n}: {e}"))?;
        lines.push(SalesLine {
            item: l.item,
            description: l.description.clone(),
            qty,
            rate,
            account,
        });
    }
    let number = if f.number.trim().is_empty() {
        widgets::next_number(c, 1001, |b| matches!(b, TxnBody::Invoice(_)))
    } else {
        f.number.clone()
    };
    Ok(Command::TxnPost {
        date,
        number,
        memo: f.memo.clone(),
        body: TxnBody::Invoice(Invoice {
            customer,
            due_date,
            ar_account,
            lines,
        }),
    })
}

// ----- receive payment / pay bills --------------------------------------------

pub(crate) fn receive_payment(
    ui: &mut Ui,
    c: &Company,
    f: &mut PaymentForm,
    out: &mut Vec<Action>,
) {
    header(ui, FormKind::ReceivePayment, f.editing);
    if f.account.is_none() {
        f.account = c
            .account_by_name("Undeposited Funds")
            .filter(|a| a.active && a.kind == AccountType::OtherCurrentAsset)
            .map(|a| a.id)
            .or_else(|| c.first_account_of(AccountType::Bank));
    }
    head_grid(ui, "rp_head", |ui, pairs| {
            widgets::hint_label(ui, "Received from");
            if widgets::customer_combo(ui, "rp_customer", c, &mut f.customer) {
                f.amounts.clear();
            }
            pairs.end(ui, false);
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            pairs.end(ui, true);
            widgets::hint_label(ui, "Deposit to");
            widgets::account_combo(ui, "rp_deposit", c, &mut f.account, |k| {
                matches!(k, AccountType::Bank | AccountType::OtherCurrentAsset)
            });
            pairs.end(ui, false);
            widgets::hint_label(ui, "Method / ref");
            field(ui, &mut f.method, 140.0, "Check #, ACH, card…");
            pairs.end(ui, true);
        });
    let docs: Vec<OpenDoc> = match f.customer {
        Some(id) => c
            .open_invoices_for(id, f.editing)
            .into_iter()
            .map(|(t, open)| {
                let (due, total) = match &t.body {
                    TxnBody::Invoice(i) => (i.due_date, i.total().unwrap_or(Money::ZERO)),
                    _ => (t.date, Money::ZERO),
                };
                OpenDoc {
                    id: t.id,
                    number: t.number.clone(),
                    date: t.date,
                    due,
                    total,
                    open,
                }
            })
            .collect(),
        None => Vec::new(),
    };
    let total = open_docs_grid(ui, "rp_docs", &docs, &mut f.amounts);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Amount received", total);
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(
            out,
            FormKind::ReceivePayment,
            f.editing,
            build_receive(c, f, total),
        );
    }
    if clear {
        let editing = f.editing;
        *f = PaymentForm::default();
        cleared(out, editing);
    }
}

pub(crate) fn pay_bills(ui: &mut Ui, c: &Company, f: &mut PaymentForm, out: &mut Vec<Action>) {
    header(ui, FormKind::PayBills, f.editing);
    if f.account.is_none() {
        f.account = c.first_account_of(AccountType::Bank);
    }
    head_grid(ui, "pb_head", |ui, pairs| {
            widgets::hint_label(ui, "Pay to");
            if widgets::vendor_combo(ui, "pb_vendor", c, &mut f.vendor) {
                f.amounts.clear();
            }
            pairs.end(ui, false);
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            pairs.end(ui, true);
            widgets::hint_label(ui, "Pay from");
            widgets::account_combo(ui, "pb_from", c, &mut f.account, |k| {
                matches!(k, AccountType::Bank | AccountType::CreditCard)
            });
            pairs.end(ui, false);
            widgets::hint_label(ui, "Check #");
            let hint = widgets::next_number(c, 1001, |b| {
                matches!(b, TxnBody::Check(_) | TxnBody::PayBill(_))
            });
            field(ui, &mut f.number, 100.0, &hint);
            pairs.end(ui, true);
        });
    let docs: Vec<OpenDoc> = match f.vendor {
        Some(id) => c
            .open_bills_for(id, f.editing)
            .into_iter()
            .map(|(t, open)| {
                let (due, total) = match &t.body {
                    TxnBody::Bill(b) => (b.due_date, b.total().unwrap_or(Money::ZERO)),
                    _ => (t.date, Money::ZERO),
                };
                OpenDoc {
                    id: t.id,
                    number: t.number.clone(),
                    date: t.date,
                    due,
                    total,
                    open,
                }
            })
            .collect(),
        None => Vec::new(),
    };
    let total = open_docs_grid(ui, "pb_docs", &docs, &mut f.amounts);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Payment total", total);
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(
            out,
            FormKind::PayBills,
            f.editing,
            build_pay_bills(c, f, total),
        );
    }
    if clear {
        let editing = f.editing;
        *f = PaymentForm::default();
        cleared(out, editing);
    }
}

/// An open invoice or bill shown on a payment form.
struct OpenDoc {
    id: TxnId,
    number: String,
    date: Date,
    due: Date,
    total: Money,
    open: Money,
}

/// Lists open documents with a payment field each. Returns the payment total.
fn open_docs_grid(
    ui: &mut Ui,
    salt: &str,
    docs: &[OpenDoc],
    amounts: &mut BTreeMap<TxnId, String>,
) -> Money {
    ui.add_space(8.0);
    if docs.is_empty() {
        ui.label(RichText::new("Nothing open.").color(theme::MUTED));
        ui.add_space(8.0);
        return Money::ZERO;
    }
    if ui
        .button("Pay all open")
        .on_hover_text("Fill in the full open amount of every item listed.")
        .clicked()
    {
        for doc in docs {
            amounts.insert(doc.id, doc.open.to_plain_string());
        }
    }
    let mut total = Money::ZERO;
    egui::Grid::new(salt)
        .striped(true)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            for h in ["Date", "Number", "Due", "Original", "Open", "Payment"] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for doc in docs {
                ui.label(doc.date.to_string());
                ui.label(doc.number.as_str());
                ui.label(doc.due.to_string());
                money_label(ui, doc.total, false);
                money_label(ui, doc.open, false);
                let text = amounts.entry(doc.id).or_default();
                field(ui, text, 90.0, "0.00");
                if let Ok(m) = Money::parse(text) {
                    total = total.saturating_add(m);
                }
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    total
}

fn applications(amounts: &BTreeMap<TxnId, String>) -> Result<Vec<Applied>, String> {
    let mut out = Vec::new();
    for (txn, text) in amounts {
        if text.trim().is_empty() {
            continue;
        }
        let amount = Money::parse(text).map_err(|e| e.to_string())?;
        if !amount.is_zero() {
            out.push(Applied { txn: *txn, amount });
        }
    }
    Ok(out)
}

fn build_receive(c: &Company, f: &PaymentForm, amount: Money) -> Result<Command, String> {
    let customer = f.customer.ok_or("Choose a customer")?;
    let deposit_to = f.account.ok_or("Choose where to deposit the payment")?;
    let ar_account = c
        .first_account_of(AccountType::AccountsReceivable)
        .ok_or("Add an Accounts Receivable account first")?;
    Ok(Command::TxnPost {
        date: parse_date("Date", &f.date)?,
        number: f.number.clone(),
        memo: f.memo.clone(),
        body: TxnBody::ReceivePayment(ReceivePayment {
            customer,
            deposit_to,
            ar_account,
            amount,
            method: f.method.clone(),
            applied: applications(&f.amounts)?,
        }),
    })
}

fn build_pay_bills(c: &Company, f: &PaymentForm, amount: Money) -> Result<Command, String> {
    let vendor = f.vendor.ok_or("Choose a vendor")?;
    let pay_from = f.account.ok_or("Choose the account to pay from")?;
    let ap_account = c
        .first_account_of(AccountType::AccountsPayable)
        .ok_or("Add an Accounts Payable account first")?;
    let number = if f.number.trim().is_empty() {
        widgets::next_number(c, 1001, |b| {
            matches!(b, TxnBody::Check(_) | TxnBody::PayBill(_))
        })
    } else {
        f.number.clone()
    };
    Ok(Command::TxnPost {
        date: parse_date("Date", &f.date)?,
        number,
        memo: f.memo.clone(),
        body: TxnBody::PayBill(PayBill {
            vendor,
            pay_from,
            ap_account,
            amount,
            applied: applications(&f.amounts)?,
        }),
    })
}

// ----- bill / check / deposit -----------------------------------------------------

fn account_lines(ui: &mut Ui, salt: &str, c: &Company, lines: &mut Vec<AccountLineInput>) -> Money {
    let mut remove = None;
    let mut total = Money::ZERO;
    let description_width = flex_width(ui, ACCOUNT_LINES_FIXED, 160.0, 480.0);
    egui::Grid::new((salt, width_step(description_width)))
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
        for h in ["Account", "Description", "Amount", ""] {
            widgets::column_header(ui, h);
        }
        ui.end_row();
        for (i, line) in lines.iter_mut().enumerate() {
            widgets::account_combo(ui, (salt, "acct", i), c, &mut line.account, |k| {
                !k.needs_party()
            });
            field(ui, &mut line.description, description_width, "");
            field(ui, &mut line.amount, 90.0, "0.00");
            if let Ok(m) = Money::parse(&line.amount) {
                total = total.saturating_add(m);
            }
            if widgets::remove_button(ui) {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        if i < lines.len() {
            lines.remove(i);
        }
    }
    tidy_lines(lines, 1, AccountLineInput::is_blank);
    total
}

fn build_lines(lines: &[AccountLineInput]) -> Result<Vec<AccountLine>, String> {
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.account.is_none() && l.amount.trim().is_empty() && l.description.trim().is_empty() {
            continue;
        }
        let n = i + 1;
        let account = l
            .account
            .ok_or_else(|| format!("Line {n}: choose an account"))?;
        let amount = Money::parse(&l.amount).map_err(|e| format!("Line {n}: {e}"))?;
        out.push(AccountLine {
            account,
            description: l.description.clone(),
            amount,
        });
    }
    Ok(out)
}

pub(crate) fn bill(ui: &mut Ui, c: &Company, f: &mut LinesForm, out: &mut Vec<Action>) {
    header(ui, FormKind::Bill, f.editing);
    head_grid(ui, "bill_head", |ui, pairs| {
            widgets::hint_label(ui, "Vendor");
            let changed = widgets::vendor_combo(ui, "bill_vendor", c, &mut f.vendor);
            pairs.end(ui, false);
            widgets::hint_label(ui, "Ref no.");
            field(ui, &mut f.number, 120.0, "vendor's invoice #");
            pairs.end(ui, true);
            widgets::hint_label(ui, "Date");
            let date_changed = field(ui, &mut f.date, 110.0, "YYYY-MM-DD").changed();
            pairs.end(ui, false);
            widgets::hint_label(ui, "Due date");
            field(ui, &mut f.due, 110.0, "from terms");
            pairs.end(ui, true);
            if changed || date_changed {
                let terms = f
                    .vendor
                    .and_then(|id| c.vendor(id))
                    .map(|v| v.contact.terms_days);
                if let (Some(t), Ok(d)) = (terms, Date::parse(&f.date)) {
                    f.due = d.add_days(i64::from(t)).to_string();
                }
            }
        });
    ui.add_space(8.0);
    let total = account_lines(ui, "bill_lines", c, &mut f.lines);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Amount due", total);
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(out, FormKind::Bill, f.editing, build_bill(c, f));
    }
    if clear {
        let editing = f.editing;
        *f = LinesForm::default();
        cleared(out, editing);
    }
}

fn build_bill(c: &Company, f: &LinesForm) -> Result<Command, String> {
    let vendor = f.vendor.ok_or("Choose a vendor")?;
    let date = parse_date("Date", &f.date)?;
    let due_date = if f.due.trim().is_empty() {
        date
    } else {
        parse_date("Due date", &f.due)?
    };
    let ap_account = c
        .first_account_of(AccountType::AccountsPayable)
        .ok_or("Add an Accounts Payable account first")?;
    Ok(Command::TxnPost {
        date,
        number: f.number.clone(),
        memo: f.memo.clone(),
        body: TxnBody::Bill(Bill {
            vendor,
            due_date,
            ap_account,
            lines: build_lines(&f.lines)?,
        }),
    })
}

pub(crate) fn check(ui: &mut Ui, c: &Company, f: &mut LinesForm, out: &mut Vec<Action>) {
    header(ui, FormKind::Check, f.editing);
    if f.account.is_none() {
        f.account = c.first_account_of(AccountType::Bank);
    }
    head_grid(ui, "chk_head", |ui, pairs| {
            widgets::hint_label(ui, "Pay from");
            widgets::account_combo(ui, "chk_from", c, &mut f.account, |k| {
                matches!(k, AccountType::Bank | AccountType::CreditCard)
            });
            pairs.end(ui, false);
            widgets::hint_label(ui, "Check #");
            let hint = widgets::next_number(c, 1001, |b| {
                matches!(b, TxnBody::Check(_) | TxnBody::PayBill(_))
            });
            field(ui, &mut f.number, 100.0, &hint);
            pairs.end(ui, true);
            widgets::hint_label(ui, "Payee");
            widgets::party_combo(ui, "chk_payee", c, &mut f.payee);
            pairs.end(ui, false);
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            pairs.end(ui, true);
        });
    ui.add_space(8.0);
    let total = account_lines(ui, "chk_lines", c, &mut f.lines);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Amount", total);
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(out, FormKind::Check, f.editing, build_check(c, f));
    }
    if clear {
        let editing = f.editing;
        *f = LinesForm::default();
        cleared(out, editing);
    }
}

fn build_check(c: &Company, f: &LinesForm) -> Result<Command, String> {
    let pay_from = f.account.ok_or("Choose the account to pay from")?;
    let is_bank = c
        .account(pay_from)
        .is_some_and(|a| a.kind == AccountType::Bank);
    let number = if f.number.trim().is_empty() && is_bank {
        widgets::next_number(c, 1001, |b| {
            matches!(b, TxnBody::Check(_) | TxnBody::PayBill(_))
        })
    } else {
        f.number.clone()
    };
    Ok(Command::TxnPost {
        date: parse_date("Date", &f.date)?,
        number,
        memo: f.memo.clone(),
        body: TxnBody::Check(Check {
            payee: f.payee,
            pay_from,
            lines: build_lines(&f.lines)?,
        }),
    })
}

pub(crate) fn deposit(ui: &mut Ui, c: &Company, f: &mut LinesForm, out: &mut Vec<Action>) {
    header(ui, FormKind::Deposit, f.editing);
    if f.account.is_none() {
        f.account = c.first_account_of(AccountType::Bank);
    }
    head_grid(ui, "dep_head", |ui, pairs| {
            widgets::hint_label(ui, "Deposit to");
            widgets::account_combo(ui, "dep_to", c, &mut f.account, |k| k == AccountType::Bank);
            pairs.end(ui, false);
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            pairs.end(ui, true);
        });
    // Offer payments waiting in Undeposited Funds.
    if let Some(uf) = c.account_by_name("Undeposited Funds") {
        let waiting = c.account_balance(uf.id, None);
        if waiting.is_positive() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} is waiting in Undeposited Funds.",
                    currency::format(waiting)
                ));
                if ui.button("Add it to this deposit").clicked() {
                    f.lines
                        .retain(|l| l.account.is_some() || !l.amount.trim().is_empty());
                    f.lines.push(AccountLineInput {
                        account: Some(uf.id),
                        description: "Payments received".into(),
                        amount: waiting.to_plain_string(),
                    });
                }
            });
        }
    }
    ui.add_space(8.0);
    let total = account_lines(ui, "dep_lines", c, &mut f.lines);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Deposit total", total);
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(out, FormKind::Deposit, f.editing, build_deposit(f));
    }
    if clear {
        let editing = f.editing;
        *f = LinesForm::default();
        cleared(out, editing);
    }
}

fn build_deposit(f: &LinesForm) -> Result<Command, String> {
    let deposit_to = f.account.ok_or("Choose the bank account")?;
    Ok(Command::TxnPost {
        date: parse_date("Date", &f.date)?,
        number: f.number.clone(),
        memo: f.memo.clone(),
        body: TxnBody::Deposit(Deposit {
            deposit_to,
            lines: build_lines(&f.lines)?,
        }),
    })
}

// ----- journal entry ------------------------------------------------------------------

pub(crate) fn journal(ui: &mut Ui, c: &Company, f: &mut JournalForm, out: &mut Vec<Action>) {
    header(ui, FormKind::Journal, f.editing);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Date");
        field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
        widgets::hint_label(ui, "Entry #");
        field(ui, &mut f.number, 80.0, "");
    });
    ui.add_space(8.0);
    let mut remove = None;
    let mut debits = Money::ZERO;
    let mut credits = Money::ZERO;
    // The memo takes the room the other columns leave.
    let memo_width = flex_width(ui, JOURNAL_FIXED, 90.0, 360.0);
    egui::Grid::new(("je_lines", width_step(memo_width)))
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for h in ["Account", "Debit", "Credit", "Memo", "Name", ""] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for (i, line) in f.lines.iter_mut().enumerate() {
                widgets::account_combo_width(
                    ui,
                    ("je_acct", i),
                    c,
                    &mut line.account,
                    |_| true,
                    LINE_ACCOUNT_WIDTH,
                );
                field(ui, &mut line.debit, 90.0, "");
                field(ui, &mut line.credit, 90.0, "");
                field(ui, &mut line.memo, memo_width, "");
                widgets::party_combo(ui, ("je_party", i), c, &mut line.party);
                debits =
                    debits.saturating_add(Money::parse_or_zero(&line.debit).unwrap_or(Money::ZERO));
                credits = credits
                    .saturating_add(Money::parse_or_zero(&line.credit).unwrap_or(Money::ZERO));
                if widgets::remove_button(ui) {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        if i < f.lines.len() {
            f.lines.remove(i);
        }
    }
    tidy_lines(&mut f.lines, 2, JournalLineInput::is_blank);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    ui.horizontal_wrapped(|ui| {
        let totals = format!(
            "Debits {}   Credits {}",
            currency::format(debits),
            currency::format(credits)
        );
        ui.label(RichText::new(totals).strong());
        let diff = debits.saturating_sub(credits);
        if debits.is_zero() && credits.is_zero() {
            // Nothing entered yet: say nothing rather than "Balanced".
        } else if diff.is_zero() {
            ui.label(RichText::new("Balanced").color(theme::POSITIVE));
        } else {
            let off = format!("Out of balance by {}", currency::format(diff.abs()));
            ui.label(RichText::new(off).color(theme::NEGATIVE));
        }
    });
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(out, FormKind::Journal, f.editing, build_journal(f));
    }
    if clear {
        let editing = f.editing;
        *f = JournalForm::default();
        cleared(out, editing);
    }
}

fn build_journal(f: &JournalForm) -> Result<Command, String> {
    let mut lines = Vec::new();
    for (i, l) in f.lines.iter().enumerate() {
        if l.account.is_none() && l.debit.trim().is_empty() && l.credit.trim().is_empty() {
            continue;
        }
        let n = i + 1;
        let account = l
            .account
            .ok_or_else(|| format!("Line {n}: choose an account"))?;
        let debit = Money::parse_or_zero(&l.debit).map_err(|e| format!("Line {n}: {e}"))?;
        let credit = Money::parse_or_zero(&l.credit).map_err(|e| format!("Line {n}: {e}"))?;
        lines.push(JournalLine {
            account,
            debit,
            credit,
            memo: l.memo.clone(),
            party: l.party,
        });
    }
    Ok(Command::TxnPost {
        date: parse_date("Date", &f.date)?,
        number: f.number.clone(),
        memo: f.memo.clone(),
        body: TxnBody::Journal(JournalEntry { lines }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws every form at `width`; returns what they asked for.
    fn draw_all(c: &Company, width: f32) -> Vec<Action> {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut f = Forms::default();
        let mut out = Vec::new();
        crate::theme::test_frame_sized(&ctx, egui::vec2(width, 900.0), |ui| {
            invoice(ui, c, &mut f.invoice, &mut out);
            receive_payment(ui, c, &mut f.receive, &mut out);
            bill(ui, c, &mut f.bill, &mut out);
            pay_bills(ui, c, &mut f.pay_bills, &mut out);
            check(ui, c, &mut f.check, &mut out);
            deposit(ui, c, &mut f.deposit, &mut out);
            journal(ui, c, &mut f.journal, &mut out);
        });
        out
    }

    #[test]
    fn every_form_draws_in_a_narrow_and_a_wide_window() {
        let c = ledgercraft_engine::sample_company().unwrap();
        assert!(draw_all(&c, 420.0).is_empty());
        assert!(draw_all(&c, 1400.0).is_empty());
    }

    #[test]
    fn line_text_boxes_stretch_to_the_room_left() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut widths = Vec::new();
        for width in [680.0, 1400.0] {
            crate::theme::test_frame_sized(&ctx, egui::vec2(width, 600.0), |ui| {
                widths.push(flex_width(ui, INVOICE_FIXED, 100.0, 420.0));
            });
        }
        // The smallest window's page still leaves the description a usable box.
        assert!(widths[0] >= 100.0 && widths[0] < 420.0, "{widths:?}");
        assert!((widths[1] - 420.0).abs() < f32::EPSILON);
        assert_eq!(width_step(104.0), 10);
    }

    #[test]
    fn heading_pairs_go_one_to_a_row_when_narrow() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        for (width, two) in [(420.0, false), (1400.0, true)] {
            let mut seen = None;
            crate::theme::test_frame_sized(&ctx, egui::vec2(width, 600.0), |ui| {
                head_grid(ui, "test_head", |_, pairs| seen = Some(pairs.two_per_row));
            });
            assert_eq!(seen, Some(two), "at {width}");
        }
    }

    /// What Save Changes builds from the form `kind` as it stands.
    fn rebuild(c: &Company, f: &Forms, kind: FormKind) -> Result<Command, String> {
        let paid = |amounts: &BTreeMap<TxnId, String>| {
            amounts
                .values()
                .filter_map(|a| Money::parse(a).ok())
                .fold(Money::ZERO, Money::saturating_add)
        };
        let (editing, built) = match kind {
            FormKind::Invoice => (f.invoice.editing, build_invoice(c, &f.invoice)),
            FormKind::ReceivePayment => (
                f.receive.editing,
                build_receive(c, &f.receive, paid(&f.receive.amounts)),
            ),
            FormKind::Bill => (f.bill.editing, build_bill(c, &f.bill)),
            FormKind::PayBills => (
                f.pay_bills.editing,
                build_pay_bills(c, &f.pay_bills, paid(&f.pay_bills.amounts)),
            ),
            FormKind::Check => (f.check.editing, build_check(c, &f.check)),
            FormKind::Deposit => (f.deposit.editing, build_deposit(&f.deposit)),
            FormKind::Journal => (f.journal.editing, build_journal(&f.journal)),
        };
        let mut out = Vec::new();
        finish(&mut out, kind, editing, built);
        match out.pop() {
            Some(Action::Run { cmd, .. }) => Ok(cmd),
            Some(Action::Error(e)) => Err(e),
            _ => Err("nothing was built".into()),
        }
    }

    // Every kind of transaction in the sample company loads into its form and
    // comes back out unchanged, as an edit of the same transaction.
    #[test]
    fn every_transaction_round_trips_through_its_form() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let mut kinds = std::collections::BTreeSet::new();
        for t in c.transactions.iter().filter(|t| !t.voided) {
            let mut forms = Forms::default();
            let kind = forms.load(t).unwrap();
            kinds.insert(format!("{kind:?}"));
            let built = rebuild(&c, &forms, kind).unwrap();
            assert_eq!(built.id(), "txn.update", "{}", t.id);
            let Command::TxnUpdate {
                id,
                date,
                number,
                memo,
                body,
            } = built
            else {
                continue;
            };
            assert_eq!(id, t.id);
            assert_eq!(date, t.date, "{}", t.id);
            assert_eq!(memo, t.memo, "{}", t.id);
            if !t.number.is_empty() {
                assert_eq!(number, t.number, "{}", t.id);
            }
            // A payment's amount is what the form applies; compare the whole
            // body when the original applied all of it.
            let fully_applied = |amount: Money, applied: &[Applied]| {
                Money::checked_sum(applied.iter().map(|a| a.amount)) == Some(amount)
            };
            let comparable = match &t.body {
                TxnBody::ReceivePayment(p) => fully_applied(p.amount, &p.applied),
                TxnBody::PayBill(p) => fully_applied(p.amount, &p.applied),
                _ => true,
            };
            if comparable {
                assert_eq!(body, t.body, "{}", t.id);
            } else {
                assert_eq!(body.label(), t.body.label());
            }
        }
        // The sample has every kind of form in it.
        assert!(kinds.len() >= 6, "only {kinds:?}");
    }

    #[test]
    fn there_is_always_exactly_one_empty_line_to_type_in() {
        let filled = |amount: &str| AccountLineInput {
            amount: amount.into(),
            ..AccountLineInput::default()
        };
        // Starts with one empty line.
        assert_eq!(LinesForm::default().lines.len(), 1);
        // Typing in the last line adds a new empty one.
        let mut lines = vec![filled("10")];
        tidy_lines(&mut lines, 1, AccountLineInput::is_blank);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].is_blank());
        // Spare empty lines at the end go, leaving one.
        let mut lines = vec![filled("10"), filled(""), filled(""), filled("")];
        tidy_lines(&mut lines, 1, AccountLineInput::is_blank);
        assert_eq!(lines.len(), 2);
        // A blank line in the middle stays where it is.
        let mut lines = vec![filled("10"), filled(""), filled("20")];
        tidy_lines(&mut lines, 1, AccountLineInput::is_blank);
        assert_eq!(lines.len(), 4);
        // Journal entries keep at least two lines.
        let mut lines = vec![JournalLineInput::default()];
        tidy_lines(&mut lines, 2, JournalLineInput::is_blank);
        assert_eq!(lines.len(), 2);
        assert_eq!(JournalForm::default().lines.len(), 2);
    }

    #[test]
    fn void_transactions_do_not_open() {
        let mut c = ledgercraft_engine::sample_company().unwrap();
        let id = c
            .transactions
            .iter()
            .find(|t| matches!(t.body, TxnBody::Check(_)))
            .map(|t| t.id)
            .unwrap();
        c.void(id).unwrap();
        let t = c.txn(id).unwrap();
        assert!(Forms::default().load(t).is_err());
    }

    #[test]
    fn cancel_while_editing_says_so_and_clear_does_not() {
        let mut out = Vec::new();
        cleared(&mut out, Some(TxnId(1)));
        cleared(&mut out, None);
        assert!(matches!(out[0], Action::EditCancelled));
        assert!(matches!(out[1], Action::ClearStatus));
    }
}
