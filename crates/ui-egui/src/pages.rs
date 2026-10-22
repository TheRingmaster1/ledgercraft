//! The list and overview pages: Home, Chart of Accounts, Customers, Vendors,
//! Items, Transactions, Reports and account registers.

use eframe::egui::{self, Color32, RichText, Ui};
use ledgercraft_core::{Date, Money};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{
    AccountId, AccountType, Address, Company, Contact, Customer, CustomerId, CustomerTax, ItemKind,
    ItemSpec, TxnBody, TxnId,
};
use ledgercraft_reports::{account_register, ReportKind};

use crate::widgets::{self, field, money_label};
use crate::{theme, Action, FormKind, Page, Reset};

// ----- state ----------------------------------------------------------------

pub(crate) struct NewAccount {
    pub number: String,
    pub name: String,
    pub kind: AccountType,
    pub description: String,
}

impl Default for NewAccount {
    fn default() -> Self {
        Self {
            number: String::new(),
            name: String::new(),
            kind: AccountType::Expense,
            description: String::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct ContactInput {
    pub name: String,
    pub company: String,
    pub email: String,
    pub phone: String,
    pub line1: String,
    pub city: String,
    pub region: String,
    pub postal_code: String,
    pub terms: String,
}

impl ContactInput {
    fn build(&self) -> Result<Contact, String> {
        let terms_days = if self.terms.trim().is_empty() {
            0
        } else {
            self.terms
                .trim()
                .parse::<u32>()
                .map_err(|_| "Terms must be a whole number of days".to_string())?
        };
        Ok(Contact {
            name: self.name.trim().to_string(),
            company: self.company.trim().to_string(),
            email: self.email.trim().to_string(),
            phone: self.phone.trim().to_string(),
            address: Address {
                line1: self.line1.trim().to_string(),
                city: self.city.trim().to_string(),
                region: self.region.trim().to_string(),
                postal_code: self.postal_code.trim().to_string(),
                ..Address::default()
            },
            terms_days,
            notes: String::new(),
        })
    }
}

#[derive(Default)]
pub(crate) struct NewItem {
    pub name: String,
    pub description: String,
    pub kind: ItemKind,
    pub rate: String,
    pub account: Option<AccountId>,
    /// Not charged sales tax (most items are).
    pub not_taxable: bool,
}

#[derive(Default)]
pub(crate) struct ListState {
    pub new_account: NewAccount,
    pub new_customer: ContactInput,
    pub new_vendor: ContactInput,
    pub new_item: NewItem,
    pub show_inactive: bool,
    /// Exemption certificate numbers being typed, by customer.
    pub exemption_notes: std::collections::BTreeMap<CustomerId, String>,
}

impl ListState {
    pub(crate) fn reset(&mut self, what: Reset) {
        match what {
            Reset::NewAccount => self.new_account = NewAccount::default(),
            Reset::NewCustomer => self.new_customer = ContactInput::default(),
            Reset::NewVendor => self.new_vendor = ContactInput::default(),
            Reset::NewItem => self.new_item = NewItem::default(),
            Reset::Nothing
            | Reset::Form(_)
            | Reset::NewTax
            | Reset::NewTaxGroup
            | Reset::NewTaxRate
            | Reset::PayTax => {}
        }
    }
}

pub(crate) struct ReportState {
    pub kind: ReportKind,
    pub from: String,
    pub to: String,
}

impl Default for ReportState {
    fn default() -> Self {
        let today = Date::today();
        let start = Date::from_ymd(today.year(), 1, 1).unwrap_or(today);
        Self {
            kind: ReportKind::ProfitAndLoss,
            from: start.to_string(),
            to: today.to_string(),
        }
    }
}

fn title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).heading().color(theme::ACCENT));
    ui.add_space(6.0);
}

// ----- home -------------------------------------------------------------------

fn flow_button(ui: &mut Ui, label: &str, page: Page, out: &mut Vec<Action>) {
    let button =
        egui::Button::new(RichText::new(label).size(15.0)).min_size(egui::vec2(150.0, 44.0));
    if ui.add(button).on_hover_text(page.hint()).clicked() {
        out.push(Action::Go(page));
    }
}

/// A workflow arrow, drawn rather than typed: egui's built-in font has no
/// arrow glyph, so "→" would show as an empty box.
fn arrow(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 44.0), egui::Sense::hover());
    let y = rect.center().y;
    let stroke = egui::Stroke::new(2.0_f32, theme::MUTED);
    let start = egui::pos2(rect.left() + 4.0, y);
    let tip = egui::pos2(rect.right() - 4.0, y);
    ui.painter().line_segment([start, tip], stroke);
    ui.painter()
        .line_segment([tip, egui::pos2(tip.x - 7.0, y - 6.0)], stroke);
    ui.painter()
        .line_segment([tip, egui::pos2(tip.x - 7.0, y + 6.0)], stroke);
}

/// Width of the divider between the Home page columns, in ui points.
const DIVIDER_WIDTH: f32 = 5.0;
/// Space either side of the Home page divider.
const DIVIDER_GAP: f32 = 12.0;
/// Narrowest the At A Glance column gets before it moves below the workflow.
const GLANCE_MIN_WIDTH: f32 = 280.0;

/// One At A Glance line: the label on the left, the amount right-aligned,
/// on a rounded `stripe` background when one is given.
fn glance_row(ui: &mut Ui, stripe: Option<Color32>, label: &str, hint: bool, amount: Money) {
    let background = ui.painter().add(egui::Shape::Noop);
    let row = ui
        .horizontal(|ui| {
            ui.add_space(4.0);
            if hint {
                widgets::hint_label(ui, label);
            } else {
                ui.label(label);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                money_label(ui, amount, true);
            });
        })
        .response
        .rect;
    if let Some(colour) = stripe {
        let shape = egui::Shape::rect_filled(row.expand2(egui::vec2(0.0, 2.0)), 2.0, colour);
        ui.painter().set(background, shape);
    }
}

/// `stripe` shades every other At A Glance row (Help > Preferences).
pub(crate) fn home(ui: &mut Ui, c: &Company, stripe: Color32, out: &mut Vec<Action>) {
    title(ui, &c.info.name);
    let balances = widgets::balances(c);
    let total_of = |kind: AccountType| {
        c.accounts
            .iter()
            .filter(|a| a.kind == kind)
            .fold(Money::ZERO, |acc, a| {
                acc.saturating_add(balances.get(&a.id).copied().unwrap_or(Money::ZERO))
            })
    };
    let today = Date::today();
    let fy_start = today.fiscal_year_start(c.info.fiscal_year_start_month);
    let pl = ledgercraft_reports::profit_and_loss(c, fy_start, today);
    let net_income = pl
        .rows
        .iter()
        .rev()
        .find(|r| r.label == "Net Income")
        .and_then(|r| match r.cells.first() {
            Some(ledgercraft_reports::Cell::Money(m)) => Some(*m),
            _ => None,
        })
        .unwrap_or(Money::ZERO);

    let mut workflow = |ui: &mut Ui| {
        ui.label(RichText::new("Workflow").strong().underline());
        ui.add_space(4.0);
        ui.label(RichText::new("Vendors").color(theme::MUTED));
        ui.horizontal_wrapped(|ui| {
            flow_button(ui, "Enter Bills", Page::Form(FormKind::Bill), out);
            arrow(ui);
            flow_button(ui, "Pay Bills", Page::Form(FormKind::PayBills), out);
        });
        ui.add_space(10.0);
        ui.label(RichText::new("Customers").color(theme::MUTED));
        ui.horizontal_wrapped(|ui| {
            flow_button(ui, "Create Invoices", Page::Form(FormKind::Invoice), out);
            arrow(ui);
            flow_button(
                ui,
                "Receive Payments",
                Page::Form(FormKind::ReceivePayment),
                out,
            );
            arrow(ui);
            flow_button(ui, "Record Deposits", Page::Form(FormKind::Deposit), out);
        });
        ui.add_space(10.0);
        ui.label(RichText::new("Banking and company").color(theme::MUTED));
        ui.horizontal_wrapped(|ui| {
            flow_button(ui, "Write Checks", Page::Form(FormKind::Check), out);
            flow_button(ui, "Journal Entry", Page::Form(FormKind::Journal), out);
            flow_button(ui, "Chart of Accounts", Page::Accounts, out);
        });
    };
    let glance = |ui: &mut Ui| {
        ui.label(RichText::new("At A Glance").strong().underline());
        ui.add_space(4.0);
        let mut shade = false;
        let mut row = |ui: &mut Ui, label: &str, hint: bool, amount: Money| {
            glance_row(ui, shade.then_some(stripe), label, hint, amount);
            shade = !shade;
        };
        for a in c
            .sorted_accounts()
            .into_iter()
            .filter(|a| a.kind == AccountType::Bank && a.active)
        {
            let balance = balances.get(&a.id).copied().unwrap_or(Money::ZERO);
            row(ui, a.name.as_str(), false, balance);
        }
        let receivable = total_of(AccountType::AccountsReceivable);
        let payable = total_of(AccountType::AccountsPayable);
        let cards = total_of(AccountType::CreditCard);
        row(ui, "Customers owe you", true, receivable);
        row(ui, "You owe vendors", true, payable);
        row(ui, "Credit cards", true, cards);
        let since = format!("Net income since {fy_start}");
        row(ui, &since, false, net_income);
    };

    // Side by side, split down the middle, while At A Glance still fits next to
    // the workflow buttons; otherwise At A Glance moves below them. The
    // workflow's natural width is measured each frame and used on the next.
    let width_id = ui.scope_id().with("home_workflow_width");
    let workflow_width = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or(0.0);
    let full = ui.available_width();
    let left_width = workflow_width.max((full - DIVIDER_WIDTH) / 2.0 - DIVIDER_GAP);
    let measured = if full - left_width - DIVIDER_WIDTH - 2.0 * DIVIDER_GAP >= GLANCE_MIN_WIDTH {
        ui.horizontal_top(|ui| {
            let size = egui::vec2(left_width, ui.available_height());
            let left = ui.allocate_ui_with_layout(
                size,
                egui::Layout::top_down(egui::Align::Min),
                &mut workflow,
            );
            let left = left.response.rect;
            ui.add_space((left_width - left.width()).max(0.0) + DIVIDER_GAP);
            let x = ui.cursor().left();
            ui.add_space(DIVIDER_WIDTH + DIVIDER_GAP);
            let right = ui.vertical(glance).response.rect;
            let divider = egui::Rect::from_x_y_ranges(
                x..=x + DIVIDER_WIDTH,
                left.top()..=left.bottom().max(right.bottom()),
            );
            ui.painter().rect_filled(divider, 0.0, theme::NAV_BG);
            left.width()
        })
        .inner
    } else {
        let left = ui.vertical(&mut workflow).response.rect;
        ui.add_space(DIVIDER_GAP);
        let (divider, _) =
            ui.allocate_exact_size(egui::vec2(full, DIVIDER_WIDTH), egui::Sense::hover());
        ui.painter().rect_filled(divider, 0.0, theme::NAV_BG);
        ui.add_space(DIVIDER_GAP);
        ui.vertical(|ui| {
            ui.set_max_width(full.min(2.0 * GLANCE_MIN_WIDTH));
            glance(ui);
        });
        left.width()
    };
    if measured != workflow_width {
        ui.data_mut(|d| d.insert_temp(width_id, measured));
        ui.ctx().request_repaint();
    }
}

// ----- chart of accounts -----------------------------------------------------------

pub(crate) fn accounts(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Chart of Accounts");
    egui::CollapsingHeader::new("New account")
        .default_open(false)
        .show(ui, |ui| {
            let n = &mut s.new_account;
            ui.horizontal_wrapped(|ui| {
                field(ui, &mut n.number, 60.0, "Number");
                field(ui, &mut n.name, 200.0, "Name");
                egui::ComboBox::from_id_salt("new_acct_kind")
                    .selected_text(n.kind.label())
                    .show_ui(ui, |ui| {
                        for k in AccountType::ALL {
                            ui.selectable_value(&mut n.kind, k, k.label());
                        }
                    });
                field(ui, &mut n.description, 200.0, "Description");
                if ui.button("Add").clicked() {
                    out.push(Action::Run {
                        cmd: Command::AccountCreate {
                            number: n.number.clone(),
                            name: n.name.clone(),
                            kind: n.kind,
                            description: n.description.clone(),
                        },
                        reset: Reset::NewAccount,
                    });
                }
            });
        });
    ui.checkbox(&mut s.show_inactive, "Show inactive accounts");
    ui.add_space(6.0);
    let balances = widgets::balances(c);
    egui::ScrollArea::both().id_salt("accounts_scroll")
        .auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("accounts").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
            for h in ["Number", "Name", "Type", "Balance", ""] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for a in c.sorted_accounts() {
                if !a.active && !s.show_inactive {
                    continue;
                }
                ui.label(a.number.as_str());
                let mut name = RichText::new(a.name.as_str());
                if !a.active {
                    name = name.color(theme::MUTED).italics();
                }
                if ui.link(name).on_hover_text("Open register").clicked() {
                    out.push(Action::Go(Page::Register(a.id)));
                }
                ui.label(a.kind.label());
                money_label(ui, balances.get(&a.id).copied().unwrap_or(Money::ZERO), false);
                let toggle = if a.active { "Make inactive" } else { "Make active" };
                                let toggle_hint = "Inactive accounts are hidden from lists but keep their history. Only an account with a zero balance can be made inactive.";
                if ui.small_button(toggle).on_hover_text(toggle_hint).clicked() {

                    out.push(Action::Run {
                        cmd: Command::AccountSetActive { id: a.id, active: !a.active },
                        reset: Reset::Nothing,
                    });
                }
                ui.end_row();
            }
        });
    });
}

pub(crate) fn register(ui: &mut Ui, c: &Company, account: AccountId, out: &mut Vec<Action>) {
    ui.horizontal_wrapped(|ui| {
        if ui.link("< Back to Chart of Accounts").clicked() {
            out.push(Action::Go(Page::Accounts));
        }
        if c.can_reconcile(account) {
            ui.add_space(16.0);
            if ui
                .link("Reconcile")
                .on_hover_text("Check this account against its statement")
                .clicked()
            {
                out.push(Action::Reconcile(account));
            }
        }
    });
    let report = account_register(c, account);
    widgets::report_view(ui, &report, out);
}

// ----- customers / vendors -----------------------------------------------------------

fn contact_editor(ui: &mut Ui, salt: &str, n: &mut ContactInput) {
    egui::Grid::new(salt)
        .num_columns(4)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Name");
            field(ui, &mut n.name, 200.0, "");
            widgets::hint_label(ui, "Company");
            field(ui, &mut n.company, 200.0, "");
            ui.end_row();
            widgets::hint_label(ui, "Email");
            field(ui, &mut n.email, 200.0, "");
            widgets::hint_label(ui, "Phone");
            field(ui, &mut n.phone, 200.0, "");
            ui.end_row();
            widgets::hint_label(ui, "Address");
            field(ui, &mut n.line1, 200.0, "Street");
            widgets::hint_label(ui, "City");
            field(ui, &mut n.city, 200.0, "");
            ui.end_row();
            widgets::hint_label(ui, "State / region");
            field(ui, &mut n.region, 80.0, "");
            widgets::hint_label(ui, "Postal code");
            field(ui, &mut n.postal_code, 100.0, "");
            ui.end_row();
            widgets::hint_label(ui, "Terms (days)");
            field(ui, &mut n.terms, 60.0, "30");
            ui.end_row();
        });
}

pub(crate) fn customers(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Customers");
    egui::CollapsingHeader::new("New customer")
        .default_open(c.customers.is_empty())
        .show(ui, |ui| {
            contact_editor(ui, "new_customer", &mut s.new_customer);
            if ui.button("Add customer").clicked() {
                match s.new_customer.build() {
                    Ok(contact) => out.push(Action::Run {
                        cmd: Command::CustomerCreate { contact },
                        reset: Reset::NewCustomer,
                    }),
                    Err(e) => out.push(Action::Error(e)),
                }
            }
        });
    ui.add_space(6.0);
    egui::ScrollArea::both()
        .id_salt("customers_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("customers")
                .striped(true)
                .spacing([18.0, 4.0])
                .show(ui, |ui| {
                    // The Sales tax column appears once there is sales tax.
                    let taxes = !c.tax_groups.is_empty()
                        || c.customers.iter().any(|x| x.tax != CustomerTax::default());
                    for h in ["Name", "Email", "Phone", "Terms", "Balance"] {
                        widgets::column_header(ui, h);
                    }
                    if taxes {
                        widgets::column_header(ui, "Sales tax");
                    }
                    widgets::column_header(ui, "");
                    widgets::column_header(ui, "");
                    ui.end_row();
                    for x in c.customers.iter().filter(|x| x.active || s.show_inactive) {
                        ui.label(x.contact.name.as_str());
                        ui.label(x.contact.email.as_str());
                        ui.label(x.contact.phone.as_str());
                        ui.label(terms_label(x.contact.terms_days));
                        money_label(ui, c.customer_balance(x.id), false);
                        if taxes {
                            customer_tax(ui, c, x, &mut s.exemption_notes, out);
                        }
                        if ui.small_button("Invoice").clicked() {
                            out.push(Action::InvoiceFor(x.id));
                        }
                        if ui.small_button("Receive payment").clicked() {
                            out.push(Action::PaymentFrom(x.id));
                        }
                        ui.end_row();
                    }
                });
        });
}

pub(crate) fn vendors(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Vendors");
    egui::CollapsingHeader::new("New vendor")
        .default_open(c.vendors.is_empty())
        .show(ui, |ui| {
            contact_editor(ui, "new_vendor", &mut s.new_vendor);
            if ui.button("Add vendor").clicked() {
                match s.new_vendor.build() {
                    Ok(contact) => out.push(Action::Run {
                        cmd: Command::VendorCreate { contact },
                        reset: Reset::NewVendor,
                    }),
                    Err(e) => out.push(Action::Error(e)),
                }
            }
        });
    ui.add_space(6.0);
    egui::ScrollArea::both()
        .id_salt("vendors_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("vendors")
                .striped(true)
                .spacing([18.0, 4.0])
                .show(ui, |ui| {
                    for h in ["Name", "Email", "Phone", "Terms", "Balance", "", ""] {
                        widgets::column_header(ui, h);
                    }
                    ui.end_row();
                    for x in c.vendors.iter().filter(|x| x.active || s.show_inactive) {
                        ui.label(x.contact.name.as_str());
                        ui.label(x.contact.email.as_str());
                        ui.label(x.contact.phone.as_str());
                        ui.label(terms_label(x.contact.terms_days));
                        money_label(ui, c.vendor_balance(x.id), false);
                        if ui.small_button("Enter bill").clicked() {
                            out.push(Action::BillFrom(x.id));
                        }
                        if ui.small_button("Pay").clicked() {
                            out.push(Action::PayVendor(x.id));
                        }
                        ui.end_row();
                    }
                });
        });
}

/// A customer's sales tax: the group their invoices start with, or Exempt
/// with their certificate number.
fn customer_tax(
    ui: &mut Ui,
    c: &Company,
    x: &Customer,
    notes: &mut std::collections::BTreeMap<CustomerId, String>,
    out: &mut Vec<Action>,
) {
    let current = &x.tax;
    let shown = if current.exempt {
        "Exempt".to_string()
    } else {
        current
            .group
            .and_then(|g| c.tax_group(g))
            .map_or_else(|| "—".to_string(), |g| g.name.clone())
    };
    let set = |tax: CustomerTax| Action::Run {
        cmd: Command::CustomerSetTax { id: x.id, tax },
        reset: Reset::Nothing,
    };
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(("customer_tax", x.id))
            .selected_text(shown)
            .width(150.0)
            .show_ui(ui, |ui| {
                if ui.add(egui::Button::selectable(
                        !current.exempt && current.group.is_none(),
                        "— (choose on each invoice)",
                    ))
                    .clicked()
                {
                    out.push(set(CustomerTax::default()));
                }
                for g in c.tax_groups.iter().filter(|g| g.active || current.group == Some(g.id)) {
                    let on = !current.exempt && current.group == Some(g.id);
                    if ui.add(egui::Button::selectable(on, g.name.as_str())).clicked() {
                        out.push(set(CustomerTax {
                            group: Some(g.id),
                            exempt: false,
                            exemption: current.exemption.clone(),
                        }));
                    }
                }
                if ui.add(egui::Button::selectable(current.exempt, "Exempt"))
                    .on_hover_text("Not charged sales tax, e.g. a reseller or nonprofit")
                    .clicked()
                {
                    out.push(set(CustomerTax {
                        group: current.group,
                        exempt: true,
                        exemption: current.exemption.clone(),
                    }));
                }
            });
        if current.exempt {
            let note = notes.entry(x.id).or_insert_with(|| current.exemption.clone());
            let typed = field(ui, note, 110.0, "certificate #");
            if typed.lost_focus() && note.trim() != current.exemption {
                out.push(set(CustomerTax {
                    group: current.group,
                    exempt: true,
                    exemption: note.trim().to_string(),
                }));
            }
        }
    });
}

fn terms_label(days: u32) -> String {
    if days == 0 {
        "Due on receipt".into()
    } else {
        format!("Net {days}")
    }
}

// ----- items -------------------------------------------------------------------------

pub(crate) fn items(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Products and Services");
    egui::CollapsingHeader::new("New item")
        .default_open(c.items.is_empty())
        .show(ui, |ui| {
            let n = &mut s.new_item;
            egui::Grid::new("new_item")
                .num_columns(4)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    widgets::hint_label(ui, "Name");
                    field(ui, &mut n.name, 200.0, "");
                    widgets::hint_label(ui, "Type");
                    egui::ComboBox::from_id_salt("new_item_kind")
                        .selected_text(n.kind.label())
                        .show_ui(ui, |ui| {
                            for k in ItemKind::ALL {
                                ui.selectable_value(&mut n.kind, k, k.label());
                            }
                        });
                    ui.end_row();
                    widgets::hint_label(ui, "Description");
                    field(ui, &mut n.description, 200.0, "");
                    widgets::hint_label(ui, "Rate");
                    field(ui, &mut n.rate, 90.0, "0.00");
                    ui.end_row();
                    widgets::hint_label(ui, "Income account");
                    widgets::account_combo(ui, "new_item_acct", c, &mut n.account, |k| {
                        matches!(k, AccountType::Income | AccountType::OtherIncome)
                    });
                    widgets::hint_label(ui, "Taxable");
                    let mut taxable = !n.not_taxable;
                    if ui
                        .checkbox(&mut taxable, "")
                        .on_hover_text("Charge sales tax on it (most goods are taxed; many services are not)")
                        .changed()
                    {
                        n.not_taxable = !taxable;
                    }
                    ui.end_row();
                });
            if ui.button("Add item").clicked() {
                let built = match (n.account, Money::parse_or_zero(&n.rate)) {
                    (None, _) => Err("Choose an income account".to_string()),
                    (_, Err(e)) => Err(e.to_string()),
                    (Some(income_account), Ok(rate)) => Ok(ItemSpec {
                        name: n.name.trim().to_string(),
                        description: n.description.clone(),
                        kind: n.kind,
                        rate,
                        income_account,
                        taxable: !n.not_taxable,
                    }),
                };
                match built {
                    Ok(spec) => out.push(Action::Run {
                        cmd: Command::ItemCreate { spec },
                        reset: Reset::NewItem,
                    }),
                    Err(e) => out.push(Action::Error(e)),
                }
            }
        });
    ui.add_space(6.0);
    egui::Grid::new("items")
        .striped(true)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            for h in ["Name", "Type", "Description", "Rate", "Income account", "Taxable"] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for x in c.items.iter().filter(|x| x.active) {
                ui.label(x.spec.name.as_str());
                ui.label(x.spec.kind.label());
                ui.label(x.spec.description.as_str());
                money_label(ui, x.spec.rate, false);
                ui.label(c.account_name(x.spec.income_account));
                let mut taxable = x.spec.taxable;
                if ui
                    .checkbox(&mut taxable, "")
                    .on_hover_text("Charge sales tax on it. Invoices already recorded keep their tax.")
                    .changed()
                {
                    out.push(Action::Run {
                        cmd: Command::ItemSetTaxable { id: x.id, taxable },
                        reset: Reset::Nothing,
                    });
                }
                ui.end_row();
            }
        });
}

// ----- transactions -------------------------------------------------------------------

pub(crate) fn transactions(ui: &mut Ui, c: &Company, out: &mut Vec<Action>) {
    title(ui, "Transactions");
    let mut txns: Vec<_> = c.transactions.iter().collect();
    txns.sort_by_key(|t| std::cmp::Reverse((t.date, t.id)));
    egui::ScrollArea::both()
        .id_salt("txn_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("txns")
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for h in ["Date", "Type", "Num", "Name", "Memo", "Amount", ""] {
                        widgets::column_header(ui, h);
                    }
                    ui.end_row();
                    for t in txns {
                        let muted = |s: String| {
                            if t.voided {
                                RichText::new(s).color(theme::MUTED).strikethrough()
                            } else {
                                RichText::new(s)
                            }
                        };
                        // Double-click any part of a row to open it.
                        let open = t.id;
                        widgets::txn_cell(ui, muted(t.date.to_string()), open, out);
                        widgets::txn_cell(ui, muted(c.txn_type_label(t).to_string()), open, out);
                        widgets::txn_cell(ui, muted(t.number.clone()), open, out);
                        widgets::txn_cell(
                            ui,
                            muted(t.body.party().map(|p| c.party_name(p)).unwrap_or_default()),
                            open,
                            out,
                        );
                        widgets::txn_cell(ui, muted(t.memo.clone()), open, out);
                        money_label(ui, t.body.total().unwrap_or(Money::ZERO), false);
                        if t.voided {
                            ui.label(RichText::new("VOID").color(theme::NEGATIVE));
                        } else {
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button("Edit")
                                    .on_hover_text("Open this transaction to change it")
                                    .clicked()
                                {
                                    out.push(Action::Open(t.id));
                                }
                                if ui
                                    .small_button("Void")
                                    .on_hover_text("Void keeps the record but removes its effect")
                                    .clicked()
                                {
                                    out.push(void(t.id));
                                }
                                // Last, so Edit and Void line up on every row.
                                if matches!(t.body, TxnBody::Invoice(_))
                                    && ui
                                        .small_button("PDF")
                                        .on_hover_text("Save this invoice as a PDF to email or print")
                                        .clicked()
                                {
                                    out.push(Action::InvoicePdf(t.id));
                                }
                            });
                        }
                        ui.end_row();
                    }
                });
        });
}

fn void(id: TxnId) -> Action {
    Action::Run {
        cmd: Command::TxnVoid { id },
        reset: Reset::Nothing,
    }
}

// ----- reports ----------------------------------------------------------------------------

pub(crate) fn reports(ui: &mut Ui, c: &Company, s: &mut ReportState, out: &mut Vec<Action>) {
    ui.horizontal_wrapped(|ui| {
        for k in ReportKind::ALL {
            ui.selectable_value(&mut s.kind, k, k.label());
        }
    });
    ui.horizontal_wrapped(|ui| {
        if s.kind.uses_period() {
            widgets::hint_label(ui, "From");
            field(ui, &mut s.from, 110.0, "YYYY-MM-DD");
            widgets::hint_label(ui, "To");
        } else {
            widgets::hint_label(ui, "As of");
        }
        field(ui, &mut s.to, 110.0, "YYYY-MM-DD");
        if ui.button("This year").clicked() {
            let today = Date::today();
            s.from = today
                .fiscal_year_start(c.info.fiscal_year_start_month)
                .to_string();
            s.to = today.to_string();
        }
        if ui.button("This month").clicked() {
            let today = Date::today();
            s.from = today.first_of_month().to_string();
            s.to = today.to_string();
        }
    });
    let (from, to) = match (Date::parse(&s.from), Date::parse(&s.to)) {
        (Ok(f), Ok(t)) => (f, t),
        _ => {
            ui.label(RichText::new("Enter dates as YYYY-MM-DD.").color(theme::NEGATIVE));
            return;
        }
    };
    let report = s.kind.build(c, from, to);
    ui.horizontal_wrapped(|ui| {
        let excel = ui.button("Export to Excel…").on_hover_text(
            "A formatted Excel workbook (.xlsx): money formats, bold totals and sized columns.",
        );
        if excel.clicked() {
            out.push(Action::ExportXlsx {
                name: format!("{}.xlsx", s.kind.cli_id()),
                data: report.to_xlsx(crate::currency::current().symbol()),
            });
        }
        let csv = ui
            .button("Export CSV…")
            .on_hover_text("Plain values only, for importing into other programs.");
        if csv.clicked() {
            out.push(Action::ExportCsv {
                name: format!("{}.csv", s.kind.cli_id()),
                csv: report.to_csv(),
            });
        }
    });

    ui.separator();
    widgets::report_view(ui, &report, out);
}
