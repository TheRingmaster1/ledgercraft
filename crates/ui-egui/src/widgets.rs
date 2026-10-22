//! Small reusable pieces: pickers, amount labels and the report table.

use std::collections::HashMap;
use std::hash::Hash;

use eframe::egui::{self, Align, Layout, RichText, Ui};
use ledgercraft_core::Money;
use ledgercraft_ledger::{
    Account, AccountId, AccountType, Company, CustomerId, ItemId, Party, TxnBody, TxnId, VendorId,
};
use ledgercraft_reports::{Cell, Report, RowKind};

use crate::theme;
use crate::{Action, Page};

/// Every account's balance in its natural sign, computed in one pass.
pub(crate) fn balances(c: &Company) -> HashMap<AccountId, Money> {
    let mut raw: HashMap<AccountId, Money> = HashMap::new();
    for e in c.ledger_entries() {
        let slot = raw.entry(e.posting.account).or_insert(Money::ZERO);
        *slot = slot.saturating_add(e.posting.amount);
    }
    for a in &c.accounts {
        if !a.kind.is_debit_normal() {
            if let Some(v) = raw.get_mut(&a.id) {
                *v = v.saturating_neg();
            }
        }
    }
    raw
}

pub(crate) fn money_label(ui: &mut Ui, m: Money, strong: bool) {
    let mut text = RichText::new(crate::currency::format(m));
    if strong {
        text = text.strong();
    }
    if m.is_negative() {
        text = text.color(theme::NEGATIVE);
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(text);
    });
}

pub(crate) fn account_combo(
    ui: &mut Ui,
    salt: impl Hash + std::fmt::Debug,
    c: &Company,
    value: &mut Option<AccountId>,
    allow: impl Fn(AccountType) -> bool,
) {
    account_combo_width(ui, salt, c, value, allow, 230.0);
}

/// [`account_combo`] at a chosen width, for tables short of room.
pub(crate) fn account_combo_width(
    ui: &mut Ui,
    salt: impl Hash + std::fmt::Debug,
    c: &Company,
    value: &mut Option<AccountId>,
    allow: impl Fn(AccountType) -> bool,
    width: f32,
) {
    let text = (*value)
        .and_then(|id| c.account(id))
        .map(Account::display_name)
        .unwrap_or_else(|| "Choose account…".into());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(width)
        .show_ui(ui, |ui| {
            for a in c.sorted_accounts() {
                if a.active && allow(a.kind) {
                    let label = format!("{}   ({})", a.display_name(), a.kind.label());
                    ui.selectable_value(value, Some(a.id), label);
                }
            }
        });
}

/// Returns true when the choice changed.
pub(crate) fn customer_combo(
    ui: &mut Ui,
    salt: impl Hash + std::fmt::Debug,
    c: &Company,
    value: &mut Option<CustomerId>,
) -> bool {
    let before = *value;
    let text = (*value)
        .and_then(|id| c.customer(id))
        .map(|x| x.contact.name.clone())
        .unwrap_or_else(|| "Choose customer…".into());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(230.0)
        .show_ui(ui, |ui| {
            for x in c.customers.iter().filter(|x| x.active) {
                ui.selectable_value(value, Some(x.id), x.contact.name.as_str());
            }
        });
    before != *value
}

pub(crate) fn vendor_combo(
    ui: &mut Ui,
    salt: impl Hash + std::fmt::Debug,
    c: &Company,
    value: &mut Option<VendorId>,
) -> bool {
    let before = *value;
    let text = (*value)
        .and_then(|id| c.vendor(id))
        .map(|x| x.contact.name.clone())
        .unwrap_or_else(|| "Choose vendor…".into());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(230.0)
        .show_ui(ui, |ui| {
            for x in c.vendors.iter().filter(|x| x.active) {
                ui.selectable_value(value, Some(x.id), x.contact.name.as_str());
            }
        });
    before != *value
}

/// A customer-or-vendor picker with an empty choice.
pub(crate) fn party_combo(
    ui: &mut Ui,
    salt: impl Hash + std::fmt::Debug,
    c: &Company,
    value: &mut Option<Party>,
) {
    let text = (*value).map(|p| c.party_name(p)).unwrap_or_default();
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(170.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "(none)");
            for x in c.customers.iter().filter(|x| x.active) {
                let label = format!("{}  · customer", x.contact.name);
                ui.selectable_value(value, Some(Party::Customer(x.id)), label);
            }
            for x in c.vendors.iter().filter(|x| x.active) {
                let label = format!("{}  · vendor", x.contact.name);
                ui.selectable_value(value, Some(Party::Vendor(x.id)), label);
            }
        });
}

/// Returns true when the choice changed.
pub(crate) fn item_combo(
    ui: &mut Ui,
    salt: impl Hash + std::fmt::Debug,
    c: &Company,
    value: &mut Option<ItemId>,
) -> bool {
    let before = *value;
    let text = (*value)
        .and_then(|id| c.item(id))
        .map(|x| x.spec.name.clone())
        .unwrap_or_else(|| "Choose item…".into());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(130.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "(no item)");
            for x in c.items.iter().filter(|x| x.active) {
                ui.selectable_value(value, Some(x.id), x.spec.name.as_str());
            }
        });
    before != *value
}

/// A single-line text field of a fixed width.
///
/// `add_sized` forces the width: inside a `Grid`, a plain `desired_width` is
/// capped by the column's width from the previous frame, so fields that start
/// narrow would stay narrow forever.
pub(crate) fn field(ui: &mut Ui, text: &mut String, width: f32, hint: &str) -> egui::Response {
    let height = ui.spacing().interact_size.y;
    ui.add_sized(
        [width, height],
        egui::TextEdit::singleline(text).hint_text(hint),
    )
}

/// The small "remove this line" button. Plain ASCII, so it renders with any font.
pub(crate) fn remove_button(ui: &mut Ui) -> bool {
    ui.small_button("x").on_hover_text("Remove line").clicked()
}

/// The next unused number for a kind of transaction: one more than the
/// largest numeric reference already used, starting at `start`.
pub(crate) fn next_number(c: &Company, start: u64, pick: impl Fn(&TxnBody) -> bool) -> String {
    let max = c
        .transactions
        .iter()
        .filter(|t| pick(&t.body))
        .filter_map(|t| t.number.trim().parse::<u64>().ok())
        .max();
    match max {
        Some(n) => n.saturating_add(1).to_string(),
        None => start.to_string(),
    }
}

/// A report: its heading, then its table centered beneath. `fill_page`
/// makes the scroll area take the whole page (on a page; not in a window,
/// which would grow to the screen's size).
pub(crate) fn report_view(ui: &mut Ui, r: &Report, out: &mut Vec<Action>, fill_page: bool) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(r.company.as_str()).strong());
        ui.label(RichText::new(r.title.as_str()).heading());
        ui.label(RichText::new(r.period.as_str()).color(theme::MUTED));
    });
    ui.add_space(8.0);
    // The table sits centered under the heading, and the scroll area fills
    // the page so its bars run along the page's edges. The table's width is
    // measured each frame and used to center it on the next.
    let width_id = ui.id().with(("report_width", r.title.as_str()));
    let last_width = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or(0.0);
    egui::ScrollArea::both()
        .id_salt("report_scroll")
        .auto_shrink([!fill_page, !fill_page])
        .show(ui, |ui| {
            let pad = ((ui.available_width() - last_width) / 2.0).max(0.0);
            let width = ui
                .horizontal_top(|ui| {
                    ui.add_space(pad);
                    report_grid(ui, r, out)
                })
                .inner;
            if (width - last_width).abs() > 0.5 {
                ui.data_mut(|d| d.insert_temp(width_id, width));
                ui.ctx().request_repaint();
            }
        });
}

/// The report's rows; returns the table's width.
fn report_grid(ui: &mut Ui, r: &Report, out: &mut Vec<Action>) -> f32 {
            egui::Grid::new("report_grid")
                .striped(true)
                .min_col_width(90.0)
                .spacing([18.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new(r.label_header.as_str()).strong());
                    for col in &r.columns {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(col.as_str()).strong());
                        });
                    }
                    ui.end_row();
                    for row in &r.rows {
                        let strong = matches!(
                            row.kind,
                            RowKind::Section | RowKind::Subtotal | RowKind::Total
                        );
                        let label =
                            format!("{}{}", "    ".repeat(usize::from(row.indent)), row.label);
                        let mut text = RichText::new(label);
                        if strong {
                            text = text.strong();
                        }
                        match (row.account, row.txn) {
                            (Some(acct), _) => {
                                if ui.link(text).clicked() {
                                    out.push(Action::Go(Page::Register(acct)));
                                }
                            }
                            (None, Some(txn)) => {
                                if ui
                                    .link(text)
                                    .on_hover_text("Open this transaction to see or change it")
                                    .clicked()
                                {
                                    out.push(Action::Open(txn));
                                }
                            }
                            (None, None) => {
                                ui.label(text);
                            }
                        }
                        for i in 0..r.columns.len() {
                            match row.cells.get(i) {
                                Some(Cell::Money(m)) => money_label(ui, *m, strong),
                                Some(Cell::Text(t)) => match row.txn {
                                    Some(txn) => {
                                        txn_cell(ui, RichText::new(t.as_str()), txn, out);
                                    }
                                    None => {
                                        ui.label(t.as_str());
                                    }
                                },
                                _ => {
                                    ui.label("");
                                }
                            }
                        }
                        ui.end_row();
                    }
                })
                .response
                .rect
                .width()
}

/// A cell in a row that stands for one transaction: double-click opens it.
pub(crate) fn txn_cell(ui: &mut Ui, text: RichText, txn: TxnId, out: &mut Vec<Action>) {
    let cell = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
    if cell.double_clicked() {
        out.push(Action::Open(txn));
    }
}

/// A label with hover help when `hints::field` has some for its text.
pub(crate) fn hint_label(ui: &mut Ui, text: &str) -> egui::Response {
    let response = ui.label(text);
    match crate::hints::field(text) {
        Some(hint) => response.on_hover_text(hint),
        None => response,
    }
}

/// A bold table column header with hover help when `hints::column` has some.
pub(crate) fn column_header(ui: &mut Ui, text: &str) -> egui::Response {
    let response = ui.label(RichText::new(text).strong());
    match crate::hints::column(text) {
        Some(hint) => response.on_hover_text(hint),
        None => response,
    }
}
