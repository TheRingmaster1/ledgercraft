//! Lists > Sales Tax: the sales taxes (each with its dated rates), the tax
//! groups invoices charge, and importing rates from a state's Streamlined
//! Sales Tax files.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use eframe::egui::{self, RichText, Ui};
use ledgercraft_core::Date;
use ledgercraft_engine::Command;
use ledgercraft_file::sst;
use ledgercraft_ledger::{
    tax_payment_memo, AccountId, AccountLine, AccountType, Check, Company, ImportedTax, Money,
    Party, TaxComponentId, TaxGroupId, TaxRate, TaxReview, TxnBody, VendorId,
};

use crate::widgets::{self, field};
use crate::{theme, Action, Reset};

/// Where the SST files are published.
const SST_FILES: &str = "https://www.streamlinedsalestax.org/Shared-Pages/rate-and-boundary-files";

#[derive(Default)]
struct NewTax {
    name: String,
    agency: String,
    rate: String,
    from: String,
}

/// Pay sales tax: a check to the agency, posted to Sales Tax Payable.
#[derive(Default)]
struct PayTax {
    agency: String,
    payee: Option<VendorId>,
    from: Option<AccountId>,
    date: String,
    number: String,
    amount: String,
}

#[derive(Clone, Default)]
struct GroupInput {
    name: String,
    members: Vec<TaxComponentId>,
}

#[derive(Default)]
pub(crate) struct SalesTaxState {
    new_tax: NewTax,
    new_group: GroupInput,
    editing_group: Option<(TaxGroupId, GroupInput)>,
    rates_for: Option<TaxComponentId>,
    /// A tax being renamed: (id, name, agency).
    editing_tax: Option<(TaxComponentId, String, String)>,
    pay: PayTax,
    new_rate_from: String,
    new_rate: String,
    show_inactive: bool,
    /// The import window, while open.
    pub(crate) import: Option<ImportWindow>,
}

impl SalesTaxState {
    pub(crate) fn reset(&mut self, what: Reset) {
        match what {
            Reset::NewTax => self.new_tax = NewTax::default(),
            Reset::NewTaxGroup => self.new_group = GroupInput::default(),
            Reset::NewTaxRate => {
                self.new_rate.clear();
                self.new_rate_from.clear();
            }
            Reset::PayTax => {
                self.pay.amount.clear();
                self.pay.number.clear();
            }
            _ => {}
        }
    }
}

fn run(cmd: Command, reset: Reset) -> Action {
    Action::Run { cmd, reset }
}

fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(theme::MUTED)
}

/// A rate as shown in lists, or a dash when none is in force.
fn shown(rate: Option<TaxRate>) -> String {
    rate.map_or_else(|| "—".to_string(), |r| r.to_string())
}

/// What a group charges on `date`: its taxes' rates added up.
pub(crate) fn group_rate(c: &Company, group: TaxGroupId, date: Date) -> Option<TaxRate> {
    let g = c.tax_group(group)?;
    let total = g
        .components
        .iter()
        .filter_map(|id| c.tax_component(*id)?.rate_on(date))
        .map(TaxRate::millionths)
        .sum();
    Some(TaxRate::from_millionths(total))
}

pub(crate) fn page(ui: &mut Ui, c: &Company, s: &mut SalesTaxState, out: &mut Vec<Action>) {
    ui.label(RichText::new("Sales Tax").heading().color(theme::ACCENT));
    ui.label(muted(
        "A sales tax is one agency's rate (state, county, city or district) with the dates \
         each rate starts. A tax group is the taxes charged together where you sell; invoices \
         and customers choose a group. Invoices keep the tax they charged, so changing a rate \
         never alters one already recorded.",
    ));
    ui.add_space(6.0);
    let today = Date::today();
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Import Rates from State Files…")
            .on_hover_text("Read your state's Streamlined Sales Tax rate and boundary files for your ZIP+4")
            .clicked()
        {
            s.import = Some(ImportWindow::default());
        }
        let checked = c
            .tax_rates_checked
            .map_or_else(|| "never".to_string(), |d| d.to_string());
        ui.label(muted(format!("Rates last checked: {checked}")));
        if !c.tax_components.is_empty()
            && ui
                .button("Mark Checked Today")
                .on_hover_text("You have checked the rates against your state's: stops the quarterly reminder until the next quarter")
                .clicked()
        {
            out.push(run(Command::TaxMarkChecked { on: today }, Reset::Nothing));
        }
    });
    ui.add_space(6.0);
    new_tax(ui, s, out);
    ui.add_space(6.0);
    taxes(ui, c, s, out, today);
    tax_editor(ui, c, s, out);
    rates_editor(ui, c, s, out);
    ui.add_space(12.0);
    groups(ui, c, s, out, today);
    ui.add_space(12.0);
    pay_tax(ui, c, &mut s.pay, out, today);
}

/// Renaming a tax, or changing its agency.
fn tax_editor(ui: &mut Ui, c: &Company, s: &mut SalesTaxState, out: &mut Vec<Action>) {
    let mut close = false;
    if let Some((id, name, agency)) = s.editing_tax.as_mut() {
        let code = c.tax_component(*id).map(|t| t.code.clone()).unwrap_or_default();
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::Grid::new("edit_tax").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                widgets::hint_label(ui, "Name");
                field(ui, name, 220.0, "");
                ui.end_row();
                widgets::hint_label(ui, "Agency");
                field(ui, agency, 220.0, "");
                ui.end_row();
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button(RichText::new("Save").strong()).clicked() {
                    out.push(run(
                        Command::TaxUpdate {
                            id: *id,
                            name: name.clone(),
                            agency: agency.clone(),
                            code: code.clone(),
                        },
                        Reset::Nothing,
                    ));
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    }
    if close {
        s.editing_tax = None;
    }
}

/// What is owed each agency, and recording a payment to one.
fn pay_tax(ui: &mut Ui, c: &Company, p: &mut PayTax, out: &mut Vec<Action>, today: Date) {
    let owed = c.sales_tax_owed(today);
    if owed.is_empty() {
        return;
    }
    egui::CollapsingHeader::new("Pay sales tax").show(ui, |ui| {
        ui.label(muted(
            "What each agency is owed today: tax charged on invoices, less payments recorded \
             here. For a filing period, see Reports > Sales Tax Liability.",
        ));
        egui::Grid::new("tax_owed").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
            for h in ["Agency", "Charged", "Paid", "Owed"] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for o in &owed {
                ui.label(o.agency.as_str());
                widgets::money_label(ui, o.collected, false);
                widgets::money_label(ui, o.paid, false);
                widgets::money_label(ui, o.owed, true);
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        if p.agency.is_empty() || !owed.iter().any(|o| o.agency == p.agency) {
            p.agency = owed.first().map(|o| o.agency.clone()).unwrap_or_default();
        }
        if p.date.is_empty() {
            p.date = today.to_string();
        }
        egui::Grid::new("pay_tax").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            widgets::hint_label(ui, "Agency");
            egui::ComboBox::from_id_salt("pay_tax_agency")
                .selected_text(p.agency.as_str())
                .width(260.0)
                .show_ui(ui, |ui| {
                    for o in &owed {
                        ui.selectable_value(&mut p.agency, o.agency.clone(), o.agency.as_str());
                    }
                });
            ui.end_row();
            widgets::hint_label(ui, "Pay to");
            widgets::vendor_combo(ui, "pay_tax_vendor", c, &mut p.payee);
            ui.end_row();
            widgets::hint_label(ui, "Pay from");
            widgets::account_combo(ui, "pay_tax_from", c, &mut p.from, |k| {
                matches!(k, AccountType::Bank | AccountType::CreditCard)
            });
            ui.end_row();
            widgets::hint_label(ui, "Date");
            field(ui, &mut p.date, 110.0, "YYYY-MM-DD");
            ui.end_row();
            widgets::hint_label(ui, "Check #");
            field(ui, &mut p.number, 100.0, "");
            ui.end_row();
            widgets::hint_label(ui, "Amount");
            ui.horizontal(|ui| {
                field(ui, &mut p.amount, 100.0, "0.00");
                let due = owed.iter().find(|o| o.agency == p.agency).map(|o| o.owed);
                if let Some(due) = due.filter(|d| d.is_positive()) {
                    if ui.small_button("Amount owed").clicked() {
                        p.amount = due.to_plain_string();
                    }
                }
            });
            ui.end_row();
        });
        if ui.button(RichText::new("Record Payment").strong()).clicked() {
            match build_payment(c, p) {
                Ok(cmd) => out.push(run(cmd, Reset::PayTax)),
                Err(e) => out.push(Action::Error(e)),
            }
        }
    });
}

fn build_payment(c: &Company, p: &PayTax) -> Result<Command, String> {
    let pay_from = p.from.ok_or("Choose the account to pay from")?;
    let date = Date::parse(&p.date).map_err(|e| e.to_string())?;
    let amount = Money::parse(&p.amount).map_err(|e| e.to_string())?;
    if !amount.is_positive() {
        return Err("Enter the amount paid".into());
    }
    let account = c
        .sales_tax_account()
        .ok_or("There is no Sales Tax Payable account")?;
    Ok(Command::TxnPost {
        date,
        number: p.number.trim().to_string(),
        memo: "Sales tax payment".into(),
        body: TxnBody::Check(Check {
            payee: p.payee.map(Party::Vendor),
            pay_from,
            lines: vec![AccountLine {
                account,
                description: tax_payment_memo(&p.agency),
                amount,
            }],
        }),
    })
}

fn new_tax(ui: &mut Ui, s: &mut SalesTaxState, out: &mut Vec<Action>) {
    egui::CollapsingHeader::new("New sales tax").show(ui, |ui| {
        let n = &mut s.new_tax;
        egui::Grid::new("new_tax").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            widgets::hint_label(ui, "Name");
            field(ui, &mut n.name, 220.0, "e.g. Utah State");
            ui.end_row();
            widgets::hint_label(ui, "Agency");
            field(ui, &mut n.agency, 220.0, "who it is paid to");
            ui.end_row();
            widgets::hint_label(ui, "Rate %");
            field(ui, &mut n.rate, 90.0, "e.g. 4.85");
            ui.end_row();
            widgets::hint_label(ui, "Starts");
            field(ui, &mut n.from, 110.0, "YYYY-MM-DD");
            ui.end_row();
        });
        if ui.button("Add sales tax").clicked() {
            let built = TaxRate::parse(&n.rate).and_then(|rate| {
                let from = if n.from.trim().is_empty() {
                    Date::from_ymd(2000, 1, 1).map_err(|e| e.to_string())?
                } else {
                    Date::parse(&n.from).map_err(|e| e.to_string())?
                };
                Ok(Command::TaxCreate {
                    name: n.name.clone(),
                    agency: n.agency.clone(),
                    code: String::new(),
                    from,
                    rate,
                })
            });
            match built {
                Ok(cmd) => out.push(run(cmd, Reset::NewTax)),
                Err(e) => out.push(Action::Error(e)),
            }
        }
    });
}

fn taxes(ui: &mut Ui, c: &Company, s: &mut SalesTaxState, out: &mut Vec<Action>, today: Date) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Sales taxes").strong());
        ui.checkbox(&mut s.show_inactive, "Show inactive");
    });
    if c.tax_components.is_empty() {
        ui.label(muted("None yet: add one above, or import your state's rates."));
        return;
    }
    egui::Grid::new("tax_components")
        .striped(true)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            for h in ["Name", "Agency", "Rate today", "Since", "", "", ""] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for t in c.tax_components.iter().filter(|t| t.active || s.show_inactive) {
                let name = if t.active {
                    RichText::new(t.name.as_str())
                } else {
                    muted(format!("{} (inactive)", t.name))
                };
                ui.label(name);
                ui.label(t.agency.as_str());
                ui.label(shown(t.rate_on(today)));
                let since = t
                    .rates
                    .iter()
                    .filter(|r| r.from <= today)
                    .max_by_key(|r| r.from)
                    .map_or_else(String::new, |r| r.from.to_string());
                ui.label(muted(since));
                if ui
                    .small_button("Edit")
                    .on_hover_text("Rename it, or change its agency")
                    .clicked()
                {
                    s.editing_tax = Some((t.id, t.name.clone(), t.agency.clone()));
                }
                let open = s.rates_for == Some(t.id);
                if ui
                    .small_button(if open { "Hide rates" } else { "Rates…" })
                    .on_hover_text("See and change this tax's rates and when each starts")
                    .clicked()
                {
                    s.rates_for = if open { None } else { Some(t.id) };
                }
                let (label, active) = if t.active {
                    ("Make inactive", false)
                } else {
                    ("Make active", true)
                };
                if ui.small_button(label).clicked() {
                    out.push(run(Command::TaxSetActive { id: t.id, active }, Reset::Nothing));
                }
                ui.end_row();
            }
        });
}

fn rates_editor(ui: &mut Ui, c: &Company, s: &mut SalesTaxState, out: &mut Vec<Action>) {
    let Some(t) = s.rates_for.and_then(|id| c.tax_component(id)) else {
        return;
    };
    ui.add_space(8.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(format!("Rates for {}", t.name)).strong());
        ui.label(muted(
            "Each rate applies to invoices dated on or after its start, until the next one. \
             Invoices already recorded keep the rate they were charged.",
        ));
        egui::Grid::new(("tax_rates", t.id)).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
            for h in ["Starts", "Rate", ""] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for r in t.rates.iter().rev() {
                ui.label(r.from.to_string());
                ui.label(r.rate.to_string());
                let remove = ui
                    .add_enabled(t.rates.len() > 1, egui::Button::new("Remove").small())
                    .on_disabled_hover_text("A tax keeps at least one rate: change it instead");
                if remove.clicked() {
                    out.push(run(
                        Command::TaxRemoveRate {
                            id: t.id,
                            from: r.from,
                        },
                        Reset::Nothing,
                    ));
                }
                ui.end_row();
            }
        });
        ui.horizontal_wrapped(|ui| {
            widgets::hint_label(ui, "New rate %");
            field(ui, &mut s.new_rate, 80.0, "e.g. 4.85");
            widgets::hint_label(ui, "Starts");
            field(ui, &mut s.new_rate_from, 110.0, "YYYY-MM-DD");
            if ui.button("Set Rate").clicked() {
                let built = TaxRate::parse(&s.new_rate).and_then(|rate| {
                    let from = Date::parse(&s.new_rate_from).map_err(|e| e.to_string())?;
                    Ok(Command::TaxSetRate { id: t.id, from, rate })
                });
                match built {
                    Ok(cmd) => out.push(run(cmd, Reset::NewTaxRate)),
                    Err(e) => out.push(Action::Error(e)),
                }
            }
        });
    });
}

/// Checkboxes choosing a group's taxes, in the order they are ticked.
fn member_boxes(ui: &mut Ui, c: &Company, g: &mut GroupInput) {
    ui.horizontal_wrapped(|ui| {
        // Taxes already in the group stay listed even if made inactive since.
        let before = g.members.clone();
        for t in c
            .tax_components
            .iter()
            .filter(|t| t.active || before.contains(&t.id))
        {
            let mut on = g.members.contains(&t.id);
            if ui.checkbox(&mut on, t.name.as_str()).changed() {
                if on {
                    g.members.push(t.id);
                } else {
                    g.members.retain(|m| *m != t.id);
                }
            }
        }
    });
}

fn groups(ui: &mut Ui, c: &Company, s: &mut SalesTaxState, out: &mut Vec<Action>, today: Date) {
    ui.label(RichText::new("Tax groups").strong());
    if !c.tax_components.is_empty() {
        egui::CollapsingHeader::new("New tax group")
            .default_open(c.tax_groups.is_empty())
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    widgets::hint_label(ui, "Name");
                    field(ui, &mut s.new_group.name, 220.0, "e.g. Provo combined");
                });
                member_boxes(ui, c, &mut s.new_group);
                if ui.button("Add tax group").clicked() {
                    out.push(run(
                        Command::TaxGroupCreate {
                            name: s.new_group.name.clone(),
                            components: s.new_group.members.clone(),
                        },
                        Reset::NewTaxGroup,
                    ));
                }
            });
    }
    if c.tax_groups.is_empty() {
        ui.label(muted("No groups yet. Invoices charge tax through a group."));
        return;
    }
    egui::Grid::new("tax_groups").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        for h in ["Name", "Taxes", "Rate today", "", ""] {
            widgets::column_header(ui, h);
        }
        ui.end_row();
        for g in c.tax_groups.iter().filter(|g| g.active || s.show_inactive) {
            let name = if g.active {
                RichText::new(g.name.as_str())
            } else {
                muted(format!("{} (inactive)", g.name))
            };
            ui.label(name);
            let names: Vec<&str> = g
                .components
                .iter()
                .filter_map(|id| c.tax_component(*id).map(|t| t.name.as_str()))
                .collect();
            ui.label(muted(names.join(" + ")));
            ui.label(RichText::new(shown(group_rate(c, g.id, today))).strong());
            if ui.small_button("Edit").clicked() {
                s.editing_group = Some((
                    g.id,
                    GroupInput {
                        name: g.name.clone(),
                        members: g.components.clone(),
                    },
                ));
            }
            let (label, active) = if g.active {
                ("Make inactive", false)
            } else {
                ("Make active", true)
            };
            if ui.small_button(label).clicked() {
                out.push(run(Command::TaxGroupSetActive { id: g.id, active }, Reset::Nothing));
            }
            ui.end_row();
        }
    });
    let mut close = false;
    if let Some((id, g)) = s.editing_group.as_mut() {
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                widgets::hint_label(ui, "Name");
                field(ui, &mut g.name, 220.0, "");
            });
            member_boxes(ui, c, g);
            ui.horizontal_wrapped(|ui| {
                if ui.button(RichText::new("Save").strong()).clicked() {
                    out.push(run(
                        Command::TaxGroupUpdate {
                            id: *id,
                            name: g.name.clone(),
                            components: g.members.clone(),
                        },
                        Reset::Nothing,
                    ));
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    }
    if close {
        s.editing_group = None;
    }
}

// ----- the quarterly reminder --------------------------------------------------

/// What the user chose in the reminder.
pub(crate) enum Reminder {
    /// Open Lists > Sales Tax to look.
    Review,
    /// The rates are current: record today as checked.
    UpToDate,
    /// Ask again when the company is next opened.
    Later,
}

/// `October 1, 2026`
fn long_date(d: Date) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August",
        "September", "October", "November", "December",
    ];
    let (year, month, day) = d.ymd();
    let name = usize::try_from(month.saturating_sub(1))
        .ok()
        .and_then(|i| MONTHS.get(i))
        .copied()
        .unwrap_or("");
    format!("{name} {day}, {year}")
}

/// The reminder text: when new rates may start, and when the rates were
/// last checked.
pub(crate) fn reminder_text(review: &TaxReview) -> String {
    let checked = review.last_checked.map_or_else(
        || "Your rates have not been checked yet.".to_string(),
        |d| format!("Your rates were last checked on {}.", long_date(d)),
    );
    format!(
        "New sales tax rates may take effect on {}. States change rates at the start of a \
         quarter and publish the changes by {}. {checked}",
        long_date(review.takes_effect),
        long_date(review.published),
    )
}

pub(crate) fn reminder_window(ctx: &egui::Context, review: &TaxReview) -> Option<Reminder> {
    let mut choice = None;
    egui::Window::new("Check Your Sales Tax Rates")
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(reminder_text(review));
            ui.add_space(4.0);
            ui.label(muted(
                "Compare them with your state's rates, or import its new rate files, so new \
                 invoices charge the right tax.",
            ));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button(RichText::new("Review Rates").strong()).clicked() {
                    choice = Some(Reminder::Review);
                }
                if ui
                    .button("Rates Are Up to Date")
                    .on_hover_text("Record today as checked: no reminder until next quarter")
                    .clicked()
                {
                    choice = Some(Reminder::UpToDate);
                }
                if ui
                    .button("Remind Me Later")
                    .on_hover_text("Ask again the next time this company is opened")
                    .clicked()
                {
                    choice = Some(Reminder::Later);
                }
            });
        });
    choice
}

// ----- import ---------------------------------------------------------------

/// One location found in the boundary file, with what importing it means.
struct Found {
    location: sst::Location,
    plan: Result<sst::Plan, String>,
}

struct Preview {
    group: String,
    taxes: Vec<sst::PlannedTax>,
    total: TaxRate,
}

enum Step {
    Ask,
    Reading(Receiver<Result<Vec<Found>, String>>),
    Choose(Vec<Found>),
    Preview(Preview),
}

/// Lists > Sales Tax > Import Rates from State Files…
pub(crate) struct ImportWindow {
    rates_path: Option<PathBuf>,
    boundary_path: Option<PathBuf>,
    zip: String,
    date: String,
    step: Step,
    error: Option<String>,
}

impl Default for ImportWindow {
    fn default() -> Self {
        Self {
            rates_path: None,
            boundary_path: None,
            zip: String::new(),
            date: Date::today().to_string(),
            step: Step::Ask,
            error: None,
        }
    }
}

/// Reads both files and works out each location's taxes. Runs on its own
/// thread: a boundary file can be a hundred megabytes.
fn find(rates: &Path, boundary: &Path, zip: &str, date: Date) -> Result<Vec<Found>, String> {
    let bytes = std::fs::read(rates).map_err(|e| format!("{}: {e}", rates.display()))?;
    let rates = sst::read_rates(&String::from_utf8_lossy(&bytes))?;
    let file = std::fs::File::open(boundary).map_err(|e| format!("{}: {e}", boundary.display()))?;
    let locations = sst::locations(std::io::BufReader::new(file), zip, date)?;
    Ok(locations
        .into_iter()
        .map(|location| {
            let plan = sst::plan(&rates, &location, date);
            Found { location, plan }
        })
        .collect())
}

fn file_name(path: Option<&PathBuf>) -> String {
    path.and_then(|p| p.file_name())
        .map_or_else(|| "none chosen".to_string(), |n| n.to_string_lossy().into_owned())
}

fn pick_csv(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Rate or boundary file", &["csv", "txt"])
        .pick_file()
}

fn preview_of(plan: &sst::Plan) -> Preview {
    Preview {
        group: plan.group_name.clone(),
        taxes: plan.taxes.clone(),
        total: plan.total_rate(),
    }
}

/// Draws the import window; false once it should close.
pub(crate) fn import_window(
    ctx: &egui::Context,
    w: &mut ImportWindow,
    c: &Company,
    out: &mut Vec<Action>,
) -> bool {
    // A finished read moves the window on.
    let next = match &w.step {
        Step::Reading(rx) => match rx.try_recv() {
            Ok(Ok(found)) => {
                let only = (found.len() == 1).then(|| found[0].plan.as_ref().ok().map(preview_of)).flatten();
                Some(match only {
                    Some(p) => Step::Preview(p),
                    None => Step::Choose(found),
                })
            }
            Ok(Err(e)) => {
                w.error = Some(e);
                Some(Step::Ask)
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
                None
            }
            Err(TryRecvError::Disconnected) => {
                w.error = Some("Reading the files stopped unexpectedly.".into());
                Some(Step::Ask)
            }
        },
        _ => None,
    };
    if let Some(step) = next {
        w.step = step;
    }

    let mut open = true;
    let mut done = false;
    egui::Window::new("Import Sales Tax Rates")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                "States in the Streamlined Sales Tax agreement publish a rate file and a \
                 boundary file every quarter. Download your state's two files (unzip them if \
                 they come as .zip), choose them here, and give your business's ZIP+4.",
            );
            ui.hyperlink_to("Rate and Boundary Files at streamlinedsalestax.org", SST_FILES);
            ui.add_space(6.0);
            match &mut w.step {
                Step::Ask => {
                    let found = ask(
                        ui,
                        [&mut w.rates_path, &mut w.boundary_path],
                        &mut w.zip,
                        &mut w.date,
                        &mut w.error,
                    );
                    if let Some(start) = found {
                        w.step = start;
                    }
                }
                Step::Reading(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading the boundary file… a large one takes a few seconds.");
                    });
                }
                Step::Choose(found) => {
                    if let Some(step) = choose(ui, found, &w.zip) {
                        w.step = step;
                    }
                }
                Step::Preview(p) => {
                    let (import, back) = preview(ui, c, p);
                    if import {
                        let taxes = p
                            .taxes
                            .iter()
                            .map(|t| ImportedTax {
                                code: t.code.clone(),
                                name: t.name.clone(),
                                agency: t.agency.clone(),
                                rates: t.rates.clone(),
                            })
                            .collect();
                        out.push(Action::Run {
                            cmd: Command::TaxImport {
                                taxes,
                                group: p.group.clone(),
                                on: Date::today(),
                            },
                            reset: Reset::Nothing,
                        });
                        done = true;
                    }
                    if back {
                        w.step = Step::Ask;
                    }
                }
            }
        });
    open && !done
}

/// The first step: the files, ZIP and date. Returns the reading step once
/// Find Rates is clicked.
fn ask(
    ui: &mut Ui,
    [rates, boundary]: [&mut Option<PathBuf>; 2],
    zip: &mut String,
    date: &mut String,
    error: &mut Option<String>,
) -> Option<Step> {
    let mut start = None;
    egui::Grid::new("sst_files").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Rate file");
        if ui.button("Choose…").clicked() {
            if let Some(p) = pick_csv("Choose the rate file (e.g. UTR2026Q4AUG11.csv)") {
                *rates = Some(p);
            }
        }
        ui.label(muted(file_name(rates.as_ref())));
        ui.end_row();
        ui.label("Boundary file");
        if ui.button("Choose…").clicked() {
            if let Some(p) = pick_csv("Choose the boundary file (e.g. UTB2026Q4AUG11.csv)") {
                *boundary = Some(p);
            }
        }
        ui.label(muted(file_name(boundary.as_ref())));
        ui.end_row();
        ui.label("ZIP+4");
        field(ui, zip, 120.0, "84601-1234");
        ui.label(muted("the full ZIP+4 finds the exact location"));
        ui.end_row();
        ui.label("Rates as of");
        field(ui, date, 110.0, "YYYY-MM-DD");
        ui.label(muted("today, or the start of next quarter"));
        ui.end_row();
    });
    if let Some(e) = error.as_ref() {
        ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
    }
    ui.add_space(6.0);
    let ready = rates.is_some() && boundary.is_some() && !zip.trim().is_empty();
    let find_button = ui
        .add_enabled(ready, egui::Button::new(RichText::new("Find Rates").strong()))
        .on_disabled_hover_text("Choose both files and enter a ZIP+4");
    if find_button.clicked() {
        let checked = sst::parse_zip(zip).and_then(|_| Date::parse(date).map_err(|e| e.to_string()));
        match (checked, rates.clone(), boundary.clone()) {
            (Ok(on), Some(r), Some(b)) => {
                *error = None;
                let (tx, rx) = mpsc::channel();
                let zip = zip.clone();
                std::thread::spawn(move || {
                    // The window may be closed before this finishes.
                    let _ = tx.send(find(&r, &b, &zip, on));
                });
                start = Some(Step::Reading(rx));
            }
            (Err(e), _, _) => *error = Some(e),
            _ => {}
        }
    }
    start
}

/// A bare ZIP with several locations: pick one, or go back for a ZIP+4.
fn choose(ui: &mut Ui, found: &[Found], zip: &str) -> Option<Step> {
    let mut next = None;
    ui.label(format!(
        "{zip} covers more than one tax location. Go back and enter the full ZIP+4 to find \
         yours exactly, or pick it here (the most common first)."
    ));
    egui::Grid::new("sst_locations").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        for h in ["Location", "Rate", "Address ranges", ""] {
            widgets::column_header(ui, h);
        }
        ui.end_row();
        for f in found {
            let code = &f.location.location_code;
            ui.label(if code.is_empty() { "—".to_string() } else { code.clone() });
            match &f.plan {
                Ok(p) => {
                    ui.label(RichText::new(p.total_rate().to_string()).strong());
                    ui.label(muted(f.location.rows.to_string()));
                    if ui.small_button("Use This").clicked() {
                        next = Some(Step::Preview(preview_of(p)));
                    }
                }
                Err(e) => {
                    ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
                    ui.label("");
                    ui.label("");
                }
            }
            ui.end_row();
        }
    });
    if ui.button("Back").clicked() {
        next = Some(Step::Ask);
    }
    next
}

/// What will be imported, with names to change. Returns (import, back).
fn preview(ui: &mut Ui, c: &Company, p: &mut Preview) -> (bool, bool) {
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Group name");
        field(ui, &mut p.group, 240.0, "");
        ui.label(RichText::new(format!("{} in all", p.total)).strong());
    });
    ui.label(muted(
        "Rate files carry codes, not place names: rename the taxes if you like. Taxes already \
         in your books (same code) keep their names and get the file's rates.",
    ));
    egui::Grid::new("sst_preview").striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
        for h in ["Name", "Rate", "Code", ""] {
            widgets::column_header(ui, h);
        }
        ui.end_row();
        for t in &mut p.taxes {
            let existing = c.tax_components.iter().find(|x| x.code == t.code);
            match existing {
                Some(x) => {
                    ui.label(x.name.as_str());
                }
                None => {
                    field(ui, &mut t.name, 200.0, "");
                }
            }
            ui.label(t.rate.to_string());
            ui.label(muted(t.code.as_str()));
            ui.label(muted(if existing.is_some() { "updates your tax" } else { "new" }));
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    let mut import = false;
    let mut back = false;
    ui.horizontal_wrapped(|ui| {
        import = ui.button(RichText::new("Import").strong()).clicked();
        back = ui.button("Back").clicked();
    });
    (import, back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_draws_with_taxes_and_groups() {
        let mut c = Company::with_default_accounts("Tax Co");
        let from = Date::parse("2020-01-01").unwrap();
        let state = c
            .add_tax_component("Utah State", "Utah State Tax Commission", "", from, TaxRate::parse("4.85").unwrap())
            .unwrap();
        let group = c.add_tax_group("Statewide", vec![state]).unwrap();
        assert_eq!(group_rate(&c, group, Date::today()), Some(TaxRate::parse("4.85").unwrap()));
        let mut s = SalesTaxState {
            rates_for: Some(state),
            ..SalesTaxState::default()
        };
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = Vec::new();
        crate::theme::test_frame(&ctx, |ui| page(ui, &c, &mut s, &mut out));
        assert!(out.is_empty());
    }

    #[test]
    fn the_reminder_says_when_rates_change() {
        let review = TaxReview {
            published: Date::parse("2026-09-01").unwrap(),
            takes_effect: Date::parse("2026-10-01").unwrap(),
            last_checked: Some(Date::parse("2026-06-12").unwrap()),
        };
        let text = reminder_text(&review);
        assert!(text.contains("October 1, 2026"), "{text}");
        assert!(text.contains("September 1, 2026"));
        assert!(text.contains("June 12, 2026"));
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut choice = Some(Reminder::Later);
        crate::theme::test_frame(&ctx, |ui| choice = reminder_window(ui.ctx(), &review));
        assert!(choice.is_none());
    }

    #[test]
    fn the_import_window_draws() {
        let c = Company::with_default_accounts("Tax Co");
        let mut w = ImportWindow::default();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = Vec::new();
        let mut open = false;
        crate::theme::test_frame(&ctx, |ui| open = import_window(ui.ctx(), &mut w, &c, &mut out));
        assert!(open && out.is_empty());
    }
}
