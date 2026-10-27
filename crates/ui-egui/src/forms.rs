//! Transaction entry forms. Each form keeps what the user typed as text and
//! only turns it into a [`Command`] when they press Save, so a half-typed
//! amount never reaches the books.

use std::collections::BTreeMap;

use eframe::egui::{self, RichText, Ui};
use ledgercraft_core::{Date, Money, Qty};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{
    AccountId, Address, AccountLine, AccountType, Applied, Bill, Check, Company, CreditMemo, CustomerId,
    Deposit, Invoice, InvoiceTax, ItemId, JournalEntry, JournalLine, Party, PayBill,
    ReceivePayment, Refund, SalesLine, SalesReceipt, TaxGroupId, Transaction, TxnBody,
    TxnClassId, TxnId, VendorId,
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
    /// Sales tax is not charged on this line.
    pub no_tax: bool,
}

impl SalesLineInput {
    fn is_blank(&self) -> bool {
        self.item.is_none()
            && self.description.trim().is_empty()
            && self.rate.trim().is_empty()
            && self.account.is_none()
    }
}

/// An invoice, sales receipt or credit memo: lines, sales tax and a
/// customer, with the few fields that differ between them.
pub(crate) struct InvoiceForm {
    /// The invoice being edited, if this is not a new one.
    pub editing: Option<TxnId>,
    pub customer: Option<CustomerId>,
    pub date: String,
    pub due: String,
    pub number: String,
    pub memo: String,
    pub lines: Vec<SalesLineInput>,
    /// The tax group charged; `None` for no sales tax.
    pub tax_group: Option<TaxGroupId>,
    /// While editing: the tax the invoice charged when recorded.
    pub recorded_tax: Option<Money>,
    /// Sales receipt: where the money goes.
    pub deposit_to: Option<AccountId>,
    /// Sales receipt: how they paid.
    pub method: String,
    /// Credit memo: what it pays on each open invoice.
    pub amounts: BTreeMap<TxnId, String>,
    /// Invoice: where the goods go, when not to the customer's address.
    pub ship_to: Option<Address>,
    /// The class (department, location…) it belongs to.
    pub class: Option<TxnClassId>,
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
            tax_group: None,
            recorded_tax: None,
            deposit_to: None,
            method: String::new(),
            amounts: BTreeMap::new(),
            ship_to: None,
            class: None,
        }
    }
}

/// The tax group a new invoice for `customer` starts with: none for an
/// exempt customer; otherwise their group, or the company's only one.
pub(crate) fn default_tax_group(c: &Company, customer: Option<CustomerId>) -> Option<TaxGroupId> {
    let tax = customer.and_then(|id| c.customer(id)).map(|x| &x.tax);
    if tax.is_some_and(|t| t.exempt) {
        return None;
    }
    let usable = |g: TaxGroupId| c.tax_group(g).is_some_and(|g| g.active);
    if let Some(g) = tax.and_then(|t| t.group).filter(|g| usable(*g)) {
        return Some(g);
    }
    let mut active = c.tax_groups.iter().filter(|g| g.active);
    match (active.next(), active.next()) {
        (Some(only), None) => Some(only.id),
        _ => None,
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
    pub class: Option<TxnClassId>,
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
            class: None,
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
    pub class: Option<TxnClassId>,
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
            class: None,
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
    pub class: Option<TxnClassId>,
}

impl Default for JournalForm {
    fn default() -> Self {
        Self {
            editing: None,
            date: today(),
            number: String::new(),
            memo: String::new(),
            lines: vec![JournalLineInput::default(); 2],
            class: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct Forms {
    pub invoice: InvoiceForm,
    pub sales_receipt: InvoiceForm,
    pub credit_memo: InvoiceForm,
    pub refund: PaymentForm,
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
            FormKind::SalesReceipt => self.sales_receipt = InvoiceForm::default(),
            FormKind::CreditMemo => self.credit_memo = InvoiceForm::default(),
            FormKind::Refund => self.refund = PaymentForm::default(),
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
        // Invoices, sales receipts and credit memos share their lines and tax.
        let sale_form = |customer, lines: &[SalesLine], tax: &Option<InvoiceTax>| InvoiceForm {
            editing,
            class: t.class,
            customer,
            date: date.clone(),
            number: number.clone(),
            memo: memo.clone(),
            lines: lines
                .iter()
                .map(|l| SalesLineInput {
                    item: l.item,
                    description: l.description.clone(),
                    qty: l.qty.to_string(),
                    rate: l.rate.to_plain_string(),
                    account: Some(l.account),
                    no_tax: !l.taxable,
                })
                .collect(),
            tax_group: tax.as_ref().map(|t| t.group),
            recorded_tax: tax.as_ref().and_then(InvoiceTax::total),
            ..InvoiceForm::default()
        };
        Ok(match &t.body {
            TxnBody::Invoice(inv) => {
                self.invoice = InvoiceForm {
                    due: inv.due_date.to_string(),
                    ship_to: inv.ship_to.clone(),
                    ..sale_form(Some(inv.customer), &inv.lines, &inv.tax)
                };
                FormKind::Invoice
            }
            TxnBody::SalesReceipt(r) => {
                self.sales_receipt = InvoiceForm {
                    deposit_to: Some(r.deposit_to),
                    method: r.method.clone(),
                    ..sale_form(r.customer, &r.lines, &r.tax)
                };
                FormKind::SalesReceipt
            }
            TxnBody::CreditMemo(m) => {
                self.credit_memo = InvoiceForm {
                    amounts: applied_amounts(&m.applied),
                    ..sale_form(Some(m.customer), &m.lines, &m.tax)
                };
                FormKind::CreditMemo
            }
            TxnBody::Refund(r) => {
                self.refund = PaymentForm {
                    editing,
                    class: t.class,
                    customer: Some(r.customer),
                    vendor: None,
                    date,
                    number,
                    memo,
                    method: r.method.clone(),
                    account: Some(r.pay_from),
                    amounts: applied_amounts(&r.applied),
                };
                FormKind::Refund
            }
            TxnBody::ReceivePayment(p) => {
                self.receive = PaymentForm {
                    editing,
                    class: t.class,
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
                    class: t.class,
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
                    class: t.class,
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
                    class: t.class,
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
                    class: t.class,
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
                    class: t.class,
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
                class,
            },
        ) => Command::TxnUpdate {
            id,
            date,
            number,
            memo,
            body,
            class,
        },
        (_, cmd) => cmd,
    })
}

/// The account box on invoice and journal lines (narrower than elsewhere, so
/// the lines fit the smallest window).
const LINE_ACCOUNT_WIDTH: f32 = 170.0;
/// The Tax checkbox column on invoice lines, with its gap.
const TAX_BOX_WIDTH: f32 = 24.0 + 8.0;
/// The x (remove line) button, padding included.
const REMOVE_WIDTH: f32 = 28.0;
/// What an invoice line's other columns take: item 130, qty 50, rate 80,
/// account, amount about 85, the remove button, and the gaps.
const INVOICE_FIXED: f32 =
    130.0 + 50.0 + 80.0 + LINE_ACCOUNT_WIDTH + 85.0 + REMOVE_WIDTH + 6.0 * 8.0;
/// Bill, check and deposit lines: account 230, amount 90, remove, gaps.
const ACCOUNT_LINES_FIXED: f32 = 230.0 + 90.0 + REMOVE_WIDTH + 3.0 * 8.0;
/// Journal lines: account, debit, credit, name about 170, remove, gaps.
const JOURNAL_FIXED: f32 = LINE_ACCOUNT_WIDTH + 90.0 + 90.0 + 170.0 + REMOVE_WIDTH + 5.0 * 8.0;
/// Room kept spare at the right, for combo box arrows and rounding, so a
/// line never ends a few points past the edge.
const LINE_SLACK: f32 = 16.0;

/// The width left for a line's stretchy text box once the fixed columns are
/// placed, kept between `min` and `max`: the lines fill a wide window and
/// still fit a small one.
fn flex_width(ui: &Ui, fixed: f32, min: f32, max: f32) -> f32 {
    (ui.available_width() - fixed - LINE_SLACK).clamp(min, max)
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

/// The Class picker, once the company has classes (or the transaction
/// already has one).
fn class_choice(ui: &mut Ui, c: &Company, class: &mut Option<TxnClassId>) {
    if c.classes.iter().all(|k| !k.active) && class.is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Class");
        let shown = class.map_or_else(|| "(none)".to_string(), |k| c.class_name(k));
        egui::ComboBox::from_id_salt("txn_class")
            .selected_text(shown)
            .width(200.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(class, None, "(none)");
                // The chosen class stays listed even if made inactive since.
                let chosen = *class;
                for k in c.classes.iter().filter(|k| k.active || chosen == Some(k.id)) {
                    ui.selectable_value(class, Some(k.id), k.name.as_str());
                }
            });
    })
    .response
    .on_hover_text("The department, location or line of business it belongs to (Lists > Classes)");
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
    sales_form(ui, c, f, FormKind::Invoice, out);
}

/// A sale paid on the spot.
pub(crate) fn sales_receipt(
    ui: &mut Ui,
    c: &Company,
    f: &mut InvoiceForm,
    out: &mut Vec<Action>,
) {
    sales_form(ui, c, f, FormKind::SalesReceipt, out);
}

/// A credit to a customer, which can pay down their open invoices.
pub(crate) fn credit_memo(ui: &mut Ui, c: &Company, f: &mut InvoiceForm, out: &mut Vec<Action>) {
    sales_form(ui, c, f, FormKind::CreditMemo, out);
}

/// Whether a recorded transaction is the kind a sales form makes, for
/// numbering each kind in its own run.
fn same_sale_kind(kind: FormKind) -> impl Fn(&TxnBody) -> bool {
    move |b| match kind {
        FormKind::SalesReceipt => matches!(b, TxnBody::SalesReceipt(_)),
        FormKind::CreditMemo => matches!(b, TxnBody::CreditMemo(_)),
        _ => matches!(b, TxnBody::Invoice(_)),
    }
}

/// The invoice, sales receipt or credit memo form.
fn sales_form(
    ui: &mut Ui,
    c: &Company,
    f: &mut InvoiceForm,
    kind: FormKind,
    out: &mut Vec<Action>,
) {
    header(ui, kind, f.editing);
    if kind == FormKind::SalesReceipt && f.deposit_to.is_none() {
        f.deposit_to = undeposited_or_bank(c);
    }
    let number_label = match kind {
        FormKind::SalesReceipt => "Receipt #",
        FormKind::CreditMemo => "Credit #",
        _ => "Invoice #",
    };
    head_grid(ui, "inv_head", |ui, pairs| {
        widgets::hint_label(ui, "Customer");
        if widgets::customer_combo(ui, "inv_customer", c, &mut f.customer) {
            set_due_from_terms(c, f);
            f.tax_group = default_tax_group(c, f.customer);
            f.amounts.clear();
            // A new customer's own ship-to address, if they have one.
            f.ship_to = f
                .customer
                .and_then(|id| c.customer(id))
                .map(|x| x.contact.ship_to.clone())
                .filter(|a| !a.is_empty());
        }
        pairs.end(ui, false);
        widgets::hint_label(ui, number_label);
        let hint = widgets::next_number(c, 1001, same_sale_kind(kind));
        field(ui, &mut f.number, 100.0, &hint);
        pairs.end(ui, true);
        widgets::hint_label(ui, "Date");
        if field(ui, &mut f.date, 110.0, "YYYY-MM-DD").changed() {
            set_due_from_terms(c, f);
        }
        pairs.end(ui, false);
        match kind {
            FormKind::Invoice => {
                widgets::hint_label(ui, "Due date");
                field(ui, &mut f.due, 110.0, "from terms");
                pairs.end(ui, true);
            }
            FormKind::SalesReceipt => {
                widgets::hint_label(ui, "Paid by");
                field(ui, &mut f.method, 140.0, "Cash, check #, card…");
                pairs.end(ui, true);
                widgets::hint_label(ui, "Deposit to");
                widgets::account_combo(ui, "sr_deposit", c, &mut f.deposit_to, |k| {
                    matches!(k, AccountType::Bank | AccountType::OtherCurrentAsset)
                });
                pairs.end(ui, false);
            }
            _ => {}
        }
    });
    class_choice(ui, c, &mut f.class);
    if kind == FormKind::SalesReceipt && f.customer.is_none() {
        ui.label(
            RichText::new("A walk-in sale needs no customer.").color(theme::MUTED),
        );
    }
    if let Some(cust) = f.customer.and_then(|id| c.customer(id)) {
        let addr = cust.contact.address.lines().join(", ");
        if !addr.is_empty() {
            ui.label(RichText::new(format!("Bill to: {addr}")).color(theme::MUTED));
        }
    }
    if kind == FormKind::Invoice {
        if let Some(ship) = &f.ship_to {
            let text = format!("Ship to: {}", ship.lines().join(", "));
            let mut clear = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(text).color(theme::MUTED));
                clear = ui
                    .small_button("Bill-to address")
                    .on_hover_text("Ship to the customer's own address instead")
                    .clicked();
            });
            if clear {
                f.ship_to = None;
            }
        }
    }
    // Sales tax: once the company has any.
    let taxes_on = !c.tax_groups.is_empty() || f.tax_group.is_some();
    if taxes_on {
        tax_choice(ui, c, f);
    }
    ui.add_space(8.0);

    let mut remove = None;
    let mut total = Money::ZERO;
    let mut taxable = Money::ZERO;
    // Description takes the room the other columns leave.
    let fixed = INVOICE_FIXED + if taxes_on { TAX_BOX_WIDTH } else { 0.0 };
    let description_width = flex_width(ui, fixed, 80.0, 420.0);
    egui::Grid::new(("inv_lines", taxes_on, width_step(description_width)))
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for h in ["Item", "Description", "Qty", "Rate", "Income account"] {
                widgets::column_header(ui, h);
            }
            if taxes_on {
                widgets::column_header(ui, "Tax");
            }
            widgets::column_header(ui, "Amount");
            widgets::column_header(ui, "");
            ui.end_row();
            for (i, line) in f.lines.iter_mut().enumerate() {
                if widgets::item_combo(ui, ("inv_item", i), c, &mut line.item) {
                    if let Some(item) = line.item.and_then(|id| c.item(id)) {
                        line.description = item.spec.description.clone();
                        line.rate = item.spec.rate.to_plain_string();
                        line.account = Some(item.spec.income_account);
                        line.no_tax = !item.spec.taxable;
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
                if taxes_on {
                    // The empty line waiting for the next item has no box.
                    if line.is_blank() {
                        ui.label("");
                    } else {
                        let mut taxed = !line.no_tax;
                        if ui
                            .checkbox(&mut taxed, "")
                            .on_hover_text("Charge sales tax on this line")
                            .changed()
                        {
                            line.no_tax = !taxed;
                        }
                    }
                }
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
                        if !line.no_tax {
                            taxable = taxable.saturating_add(amount);
                        }
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
    let grand_total = invoice_totals(ui, c, f, total, taxable);
    if kind == FormKind::CreditMemo {
        credit_applications(ui, c, f, grand_total);
    }
    let hover = if f.editing.is_some() {
        "Save your changes, then save it as a PDF to email or print"
    } else {
        "Record it, then save it as a PDF to email or print"
    };
    let (save, clear, pdf) = footer_with(ui, f.editing, Some(("Save as PDF…", hover)));
    if save {
        finish(out, kind, f.editing, build_sale(c, f, kind));
    }
    if pdf {
        match command_for(f.editing, build_sale(c, f, kind)) {
            Ok(cmd) => out.push(Action::SaveInvoicePdf(kind, cmd)),
            Err(e) => out.push(Action::Error(e)),
        }
    }
    if clear {
        let editing = f.editing;
        *f = InvoiceForm::default();
        cleared(out, editing);
    }
}

/// The invoice's Sales tax choice: a group, or none; with a note for an
/// exempt customer.
fn tax_choice(ui: &mut Ui, c: &Company, f: &mut InvoiceForm) {
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Sales tax");
        let shown = f
            .tax_group
            .and_then(|g| c.tax_group(g))
            .map_or_else(|| "No sales tax".to_string(), |g| g.name.clone());
        egui::ComboBox::from_id_salt("inv_tax")
            .selected_text(shown)
            .width(200.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut f.tax_group, None, "No sales tax");
                // The chosen group stays listed even if made inactive since.
                let chosen = f.tax_group;
                for g in c
                    .tax_groups
                    .iter()
                    .filter(|g| g.active || chosen == Some(g.id))
                {
                    ui.selectable_value(&mut f.tax_group, Some(g.id), g.name.as_str());
                }
            });
        if let Some(x) = f
            .customer
            .and_then(|id| c.customer(id))
            .filter(|x| x.tax.exempt)
        {
            let note = if x.tax.exemption.is_empty() {
                "This customer is tax-exempt.".to_string()
            } else {
                format!("This customer is tax-exempt ({}).", x.tax.exemption)
            };
            ui.label(RichText::new(note).color(theme::MUTED));
        }
    });
}

/// Subtotal, each tax and the total, under the lines.
/// Shows the totals; returns the total, tax included.
fn invoice_totals(
    ui: &mut Ui,
    c: &Company,
    f: &InvoiceForm,
    subtotal: Money,
    taxable: Money,
) -> Money {
    let Some(group) = f.tax_group else {
        total_row(ui, "Total", subtotal);
        return subtotal;
    };
    total_row(ui, "Subtotal", subtotal);
    let mut tax_total = Money::ZERO;
    match Date::parse(&f.date) {
        Ok(date) => match c.invoice_tax(group, date, taxable) {
            Ok(tax) => {
                for l in &tax.lines {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(format!("{} {}", l.name, l.rate)).color(theme::MUTED),
                        );
                        ui.label(currency::format(l.amount));
                    });
                }
                tax_total = tax.total().unwrap_or(Money::ZERO);
                // Editing an invoice whose date's rates have changed since.
                if let Some(recorded) = f.recorded_tax.filter(|r| *r != tax_total) {
                    ui.label(
                        RichText::new(format!(
                            "The tax was {} when this invoice was recorded; with today's rates \
                             for its date and its lines it is {}. Saving charges {}.",
                            currency::format(recorded),
                            currency::format(tax_total),
                            currency::format(tax_total),
                        ))
                        .color(theme::NEGATIVE),
                    );
                }
            }
            Err(e) => {
                ui.label(RichText::new(e.to_string()).color(theme::NEGATIVE));
            }
        },
        Err(_) => {
            ui.label(RichText::new("Enter the date to work out the tax.").color(theme::MUTED));
        }
    }
    let total = subtotal.saturating_add(tax_total);
    total_row(ui, "Total", total);
    total
}

/// The account money received on the spot goes to by default: Undeposited
/// Funds, or the first bank account.
fn undeposited_or_bank(c: &Company) -> Option<AccountId> {
    c.account_by_name("Undeposited Funds")
        .filter(|a| a.active && a.kind == AccountType::OtherCurrentAsset)
        .map(|a| a.id)
        .or_else(|| c.first_account_of(AccountType::Bank))
}

/// A credit memo's customer's open invoices, with what the credit pays on
/// each, and the credit left over.
fn credit_applications(ui: &mut Ui, c: &Company, f: &mut InvoiceForm, credit: Money) {
    let Some(customer) = f.customer else {
        return;
    };
    ui.add_space(6.0);
    ui.label(RichText::new("Apply the credit to open invoices").strong());
    let docs = open_invoice_docs(c, customer, f.editing);
    let applied = open_docs_grid(
        ui,
        "cm_docs",
        &docs,
        &mut f.amounts,
        ("Apply to all open", "Fill in each open amount, oldest first."),
    );
    let left = credit.saturating_sub(applied);
    let text = if left.is_negative() {
        RichText::new(format!(
            "That applies {} more than the credit.",
            currency::format(left.saturating_neg())
        ))
        .color(theme::NEGATIVE)
    } else {
        RichText::new(format!(
            "Credit left for later invoices or a refund: {}",
            currency::format(left)
        ))
        .color(theme::MUTED)
    };
    ui.label(text);
}

/// A customer's open invoices for a payment or credit grid.
fn open_invoice_docs(c: &Company, customer: CustomerId, editing: Option<TxnId>) -> Vec<OpenDoc> {
    c.open_invoices_for(customer, editing)
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
        .collect()
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

/// What an invoice, sales receipt or credit memo form records.
fn build_sale(c: &Company, f: &InvoiceForm, kind: FormKind) -> Result<Command, String> {
    let customer = match (kind, f.customer) {
        (FormKind::SalesReceipt, customer) => customer,
        (_, Some(customer)) => Some(customer),
        (_, None) => return Err("Choose a customer".into()),
    };
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
            taxable: !l.no_tax,
            account,
        });
    }
    // The tax at the rates in force on the invoice's date, stored on it.
    let tax = match f.tax_group {
        Some(group) => {
            let taxable = lines
                .iter()
                .filter(|l| l.taxable)
                .try_fold(Money::ZERO, |acc, l| acc.checked_add(l.amount()?))
                .ok_or("The taxable total is too large")?;
            if taxable.is_zero() {
                None
            } else {
                Some(
                    c.invoice_tax(group, date, taxable)
                        .map_err(|e| e.to_string())?,
                )
            }
        }
        None => None,
    };
    let number = if f.number.trim().is_empty() {
        widgets::next_number(c, 1001, same_sale_kind(kind))
    } else {
        f.number.clone()
    };
    let body = match (kind, customer) {
        (FormKind::SalesReceipt, customer) => TxnBody::SalesReceipt(SalesReceipt {
            customer,
            deposit_to: f
                .deposit_to
                .ok_or("Choose where the money goes (Deposit to)")?,
            method: f.method.clone(),
            lines,
            tax,
        }),
        (FormKind::CreditMemo, Some(customer)) => TxnBody::CreditMemo(CreditMemo {
            customer,
            ar_account,
            lines,
            tax,
            applied: applications(&f.amounts)?,
        }),
        (_, Some(customer)) => TxnBody::Invoice(Invoice {
            customer,
            due_date,
            ar_account,
            ship_to: f.ship_to.clone(),
            tax,
            lines,
        }),
        (_, None) => return Err("Choose a customer".into()),
    };
    Ok(Command::TxnPost {
        class: f.class,
        date,
        number,
        memo: f.memo.clone(),
        body,
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
    class_choice(ui, c, &mut f.class);
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
    let total = open_docs_grid(ui, "rp_docs", &docs, &mut f.amounts, PAY_ALL);
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
    class_choice(ui, c, &mut f.class);
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
    let total = open_docs_grid(ui, "pb_docs", &docs, &mut f.amounts, PAY_ALL);
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

/// Refund a Credit: pay a customer back what is left on their credit memos.
pub(crate) fn refund(ui: &mut Ui, c: &Company, f: &mut PaymentForm, out: &mut Vec<Action>) {
    header(ui, FormKind::Refund, f.editing);
    ui.label(
        RichText::new(
            "Pays a customer back from their credit memos' credit, by check, card or \
             transfer. To give credit instead, apply the credit memo to an invoice.",
        )
        .color(theme::MUTED),
    );
    if f.account.is_none() {
        f.account = c.first_account_of(AccountType::Bank);
    }
    head_grid(ui, "rf_head", |ui, pairs| {
        widgets::hint_label(ui, "Refund to");
        if widgets::customer_combo(ui, "rf_customer", c, &mut f.customer) {
            f.amounts.clear();
        }
        pairs.end(ui, false);
        widgets::hint_label(ui, "Date");
        field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
        pairs.end(ui, true);
        widgets::hint_label(ui, "Pay from");
        widgets::account_combo(ui, "rf_from", c, &mut f.account, |k| {
            matches!(k, AccountType::Bank | AccountType::CreditCard)
        });
        pairs.end(ui, false);
        widgets::hint_label(ui, "Check #");
        field(ui, &mut f.number, 100.0, "");
        pairs.end(ui, true);
        widgets::hint_label(ui, "Method / ref");
        field(ui, &mut f.method, 140.0, "Check, card refund…");
        pairs.end(ui, false);
    });
    class_choice(ui, c, &mut f.class);
    let docs: Vec<OpenDoc> = match f.customer {
        Some(id) => c
            .open_credits_for(id, f.editing)
            .into_iter()
            .map(|(t, open)| OpenDoc {
                id: t.id,
                number: t.number.clone(),
                date: t.date,
                due: t.date,
                total: t.body.total().unwrap_or(Money::ZERO),
                open,
            })
            .collect(),
        None => Vec::new(),
    };
    if f.customer.is_some() && docs.is_empty() {
        ui.label(
            RichText::new("This customer has no credit left to refund.").color(theme::MUTED),
        );
    }
    let total = open_docs_grid(
        ui,
        "rf_docs",
        &docs,
        &mut f.amounts,
        ("Refund all credit", "Fill in the credit left on each credit memo."),
    );
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Refund amount", total);
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(out, FormKind::Refund, f.editing, build_refund(c, f, total));
    }
    if clear {
        let editing = f.editing;
        *f = PaymentForm::default();
        cleared(out, editing);
    }
}

fn build_refund(c: &Company, f: &PaymentForm, amount: Money) -> Result<Command, String> {
    let customer = f.customer.ok_or("Choose a customer")?;
    let pay_from = f.account.ok_or("Choose the account to pay from")?;
    let ar_account = c
        .first_account_of(AccountType::AccountsReceivable)
        .ok_or("Add an Accounts Receivable account first")?;
    if !amount.is_positive() {
        return Err("Enter what to refund from each credit memo".into());
    }
    Ok(Command::TxnPost {
        class: f.class,
        date: parse_date("Date", &f.date)?,
        number: f.number.clone(),
        memo: f.memo.clone(),
        body: TxnBody::Refund(Refund {
            customer,
            pay_from,
            ar_account,
            amount,
            method: f.method.clone(),
            applied: applications(&f.amounts)?,
        }),
    })
}

/// The fill-in button on payment forms.
const PAY_ALL: (&str, &str) = (
    "Pay all open",
    "Fill in the full open amount of every item listed.",
);

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
    (fill, fill_hover): (&str, &str),
) -> Money {
    ui.add_space(8.0);
    if docs.is_empty() {
        ui.label(RichText::new("Nothing open.").color(theme::MUTED));
        ui.add_space(8.0);
        return Money::ZERO;
    }
    if ui.button(fill).on_hover_text(fill_hover).clicked()
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
        class: f.class,
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
        class: f.class,
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
    class_choice(ui, c, &mut f.class);
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
        class: f.class,
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
    class_choice(ui, c, &mut f.class);
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
        class: f.class,
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
    class_choice(ui, c, &mut f.class);
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
        class: f.class,
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
    class_choice(ui, c, &mut f.class);
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
        class: f.class,
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
            sales_receipt(ui, c, &mut f.sales_receipt, &mut out);
            credit_memo(ui, c, &mut f.credit_memo, &mut out);
            refund(ui, c, &mut f.refund, &mut out);
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
                widths.push(flex_width(ui, INVOICE_FIXED, 80.0, 420.0));
            });
        }
        // The smallest window's page still leaves the description a usable box.
        assert!(widths[0] >= 80.0 && widths[0] < 420.0, "{widths:?}");
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
            FormKind::Invoice => (
                f.invoice.editing,
                build_sale(c, &f.invoice, FormKind::Invoice),
            ),
            FormKind::SalesReceipt => (
                f.sales_receipt.editing,
                build_sale(c, &f.sales_receipt, FormKind::SalesReceipt),
            ),
            FormKind::CreditMemo => (
                f.credit_memo.editing,
                build_sale(c, &f.credit_memo, FormKind::CreditMemo),
            ),
            FormKind::Refund => (
                f.refund.editing,
                build_refund(c, &f.refund, paid(&f.refund.amounts)),
            ),
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
                class,
            } = built
            else {
                continue;
            };
            assert_eq!(id, t.id);
            assert_eq!(class, t.class, "{}", t.id);
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
