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
            lines: vec![SalesLineInput::default(); 3],
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
            lines: vec![AccountLineInput::default(); 3],
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
            lines: vec![JournalLineInput::default(); 4],
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
    ui.add_space(10.0);
    let mut save = false;
    let mut clear = false;
    let (save_label, clear_label) = if editing.is_some() {
        ("Save Changes", "Cancel")
    } else {
        ("Save & New", "Clear")
    };
    ui.horizontal(|ui| {
        save = ui.button(RichText::new(save_label).strong()).clicked();
        clear = ui.button(clear_label).clicked();
    });
    (save, clear)
}

/// Runs what the form built: a new transaction, or, while editing, the
/// changes to the one being edited.
fn finish(
    out: &mut Vec<Action>,
    kind: FormKind,
    editing: Option<TxnId>,
    built: Result<Command, String>,
) {
    let built = built.map(|cmd| match (editing, cmd) {
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
    });
    match built {
        Ok(cmd) => out.push(Action::Run {
            cmd,
            reset: Reset::Form(kind),
        }),
        Err(e) => out.push(Action::Error(e)),
    }
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
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        ui.label(RichText::new(currency::format(total)).strong().size(16.0));
    });
}

// ----- invoice ------------------------------------------------------------------

pub(crate) fn invoice(ui: &mut Ui, c: &Company, f: &mut InvoiceForm, out: &mut Vec<Action>) {
    header(ui, FormKind::Invoice, f.editing);
    egui::Grid::new("inv_head")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Customer");
            if widgets::customer_combo(ui, "inv_customer", c, &mut f.customer) {
                set_due_from_terms(c, f);
            }
            widgets::hint_label(ui, "Invoice #");
            let hint = widgets::next_number(c, 1001, |b| matches!(b, TxnBody::Invoice(_)));
            field(ui, &mut f.number, 100.0, &hint);
            ui.end_row();
            widgets::hint_label(ui, "Date");
            if field(ui, &mut f.date, 110.0, "YYYY-MM-DD").changed() {
                set_due_from_terms(c, f);
            }
            widgets::hint_label(ui, "Due date");
            field(ui, &mut f.due, 110.0, "from terms");
            ui.end_row();
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
    egui::Grid::new("inv_lines")
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
                field(ui, &mut line.description, 220.0, "");
                field(ui, &mut line.qty, 50.0, "1");
                field(ui, &mut line.rate, 80.0, "0.00");
                widgets::account_combo(ui, ("inv_acct", i), c, &mut line.account, |k| {
                    !k.needs_party()
                });
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
    if ui.button("+ Add line").clicked() {
        f.lines.push(SalesLineInput::default());
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    total_row(ui, "Total", total);
    let (save, clear) = footer(ui, f.editing);
    if save {
        finish(out, FormKind::Invoice, f.editing, build_invoice(c, f));
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
    egui::Grid::new("rp_head")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Received from");
            if widgets::customer_combo(ui, "rp_customer", c, &mut f.customer) {
                f.amounts.clear();
            }
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            ui.end_row();
            widgets::hint_label(ui, "Deposit to");
            widgets::account_combo(ui, "rp_deposit", c, &mut f.account, |k| {
                matches!(k, AccountType::Bank | AccountType::OtherCurrentAsset)
            });
            widgets::hint_label(ui, "Method / ref");
            field(ui, &mut f.method, 140.0, "Check #, ACH, card…");
            ui.end_row();
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
    ui.horizontal(|ui| {
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
    egui::Grid::new("pb_head")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Pay to");
            if widgets::vendor_combo(ui, "pb_vendor", c, &mut f.vendor) {
                f.amounts.clear();
            }
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            ui.end_row();
            widgets::hint_label(ui, "Pay from");
            widgets::account_combo(ui, "pb_from", c, &mut f.account, |k| {
                matches!(k, AccountType::Bank | AccountType::CreditCard)
            });
            widgets::hint_label(ui, "Check #");
            let hint = widgets::next_number(c, 1001, |b| {
                matches!(b, TxnBody::Check(_) | TxnBody::PayBill(_))
            });
            field(ui, &mut f.number, 100.0, &hint);
            ui.end_row();
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
    ui.horizontal(|ui| {
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
    egui::Grid::new(salt).spacing([8.0, 4.0]).show(ui, |ui| {
        for h in ["Account", "Description", "Amount", ""] {
            widgets::column_header(ui, h);
        }
        ui.end_row();
        for (i, line) in lines.iter_mut().enumerate() {
            widgets::account_combo(ui, (salt, "acct", i), c, &mut line.account, |k| {
                !k.needs_party()
            });
            field(ui, &mut line.description, 260.0, "");
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
    if ui.button("+ Add line").clicked() {
        lines.push(AccountLineInput::default());
    }
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
    egui::Grid::new("bill_head")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Vendor");
            let changed = widgets::vendor_combo(ui, "bill_vendor", c, &mut f.vendor);
            widgets::hint_label(ui, "Ref no.");
            field(ui, &mut f.number, 120.0, "vendor's invoice #");
            ui.end_row();
            widgets::hint_label(ui, "Date");
            let date_changed = field(ui, &mut f.date, 110.0, "YYYY-MM-DD").changed();
            widgets::hint_label(ui, "Due date");
            field(ui, &mut f.due, 110.0, "from terms");
            ui.end_row();
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
    ui.horizontal(|ui| {
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
    egui::Grid::new("chk_head")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Pay from");
            widgets::account_combo(ui, "chk_from", c, &mut f.account, |k| {
                matches!(k, AccountType::Bank | AccountType::CreditCard)
            });
            widgets::hint_label(ui, "Check #");
            let hint = widgets::next_number(c, 1001, |b| {
                matches!(b, TxnBody::Check(_) | TxnBody::PayBill(_))
            });
            field(ui, &mut f.number, 100.0, &hint);
            ui.end_row();
            widgets::hint_label(ui, "Payee");
            widgets::party_combo(ui, "chk_payee", c, &mut f.payee);
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            ui.end_row();
        });
    ui.add_space(8.0);
    let total = account_lines(ui, "chk_lines", c, &mut f.lines);
    ui.horizontal(|ui| {
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
    egui::Grid::new("dep_head")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Deposit to");
            widgets::account_combo(ui, "dep_to", c, &mut f.account, |k| k == AccountType::Bank);
            widgets::hint_label(ui, "Date");
            field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
            ui.end_row();
        });
    // Offer payments waiting in Undeposited Funds.
    if let Some(uf) = c.account_by_name("Undeposited Funds") {
        let waiting = c.account_balance(uf.id, None);
        if waiting.is_positive() {
            ui.horizontal(|ui| {
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
    ui.horizontal(|ui| {
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
    ui.horizontal(|ui| {
        widgets::hint_label(ui, "Date");
        field(ui, &mut f.date, 110.0, "YYYY-MM-DD");
        widgets::hint_label(ui, "Entry #");
        field(ui, &mut f.number, 80.0, "");
    });
    ui.add_space(8.0);
    let mut remove = None;
    let mut debits = Money::ZERO;
    let mut credits = Money::ZERO;
    egui::Grid::new("je_lines")
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for h in ["Account", "Debit", "Credit", "Memo", "Name", ""] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for (i, line) in f.lines.iter_mut().enumerate() {
                widgets::account_combo(ui, ("je_acct", i), c, &mut line.account, |_| true);
                field(ui, &mut line.debit, 90.0, "");
                field(ui, &mut line.credit, 90.0, "");
                field(ui, &mut line.memo, 180.0, "");
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
    if ui.button("+ Add line").clicked() {
        f.lines.push(JournalLineInput::default());
    }
    ui.horizontal(|ui| {
        widgets::hint_label(ui, "Memo");
        field(ui, &mut f.memo, 360.0, "");
    });
    ui.horizontal(|ui| {
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
