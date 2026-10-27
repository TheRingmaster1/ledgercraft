//! The list and overview pages: Home, Chart of Accounts, Customers, Vendors,
//! Items, Transactions, Reports and account registers.

use eframe::egui::{self, Color32, RichText, Ui};
use ledgercraft_core::{Date, Money};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{
    AccountId, AccountType, Address, Company, Contact, Customer, CustomerId, CustomerTax, Item,
    ItemId, ItemKind, ItemSpec, TxnClassId, TxnId, VendorId,
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
    /// The account it is a sub-account of.
    pub parent: Option<AccountId>,
}

impl Default for NewAccount {
    fn default() -> Self {
        Self {
            number: String::new(),
            name: String::new(),
            kind: AccountType::Expense,
            description: String::new(),
            parent: None,
        }
    }
}

impl NewAccount {
    /// An account's fields, to edit.
    fn from_account(a: &ledgercraft_ledger::Account) -> Self {
        Self {
            number: a.number.clone(),
            name: a.name.clone(),
            kind: a.kind,
            description: a.description.clone(),
            parent: a.parent,
        }
    }
}

/// "Subaccount of": the accounts of `kind` that `me` (when editing) could
/// sit under, that is not itself or one of its own sub-accounts.
fn parent_combo(
    ui: &mut Ui,
    salt: &str,
    c: &Company,
    kind: AccountType,
    me: Option<AccountId>,
    parent: &mut Option<AccountId>,
) {
    if parent.is_some_and(|p| c.account(p).is_none_or(|a| a.kind != kind)) {
        *parent = None;
    }
    let shown = parent.map_or_else(|| "(none: a top-level account)".to_string(), |p| {
        c.account_full_name(p)
    });
    egui::ComboBox::from_id_salt(("parent_account", salt))
        .selected_text(shown)
        .width(240.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(parent, None, "(none: a top-level account)");
            for (a, _) in c.accounts_in_tree_order() {
                let mine = me.is_some_and(|m| c.descends_from(a.id, m));
                if a.kind == kind && a.active && !mine {
                    ui.selectable_value(parent, Some(a.id), c.account_full_name(a.id));
                }
            }
        });
}

/// "Merge into": a picker of the other entries and a Merge button. Returns
/// the one chosen when Merge is clicked.
/// `warn` looks at the one picked: a warning there must be acknowledged
/// before Merge is enabled.
fn merge_row<T>(
    ui: &mut Ui,
    salt: &str,
    noun: &str,
    name: &str,
    others: &[(T, String)],
    warn: impl Fn(T) -> Option<String>,
) -> Option<T>
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    if others.is_empty() {
        return None;
    }
    let id = egui::Id::NULL.with(("merge_into", salt));
    let mut picked: Option<T> = ui.data(|d| d.get_temp::<Option<T>>(id)).flatten();
    if picked.is_some_and(|p| !others.iter().any(|(o, _)| *o == p)) {
        picked = None;
    }
    let mut merge = None;
    let ack_id = egui::Id::NULL.with(("merge_ack", salt));
    let mut acknowledged: bool = ui.data(|d| d.get_temp(ack_id)).unwrap_or(false);
    let warning = picked.and_then(&warn);
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Merge into");
        let shown = picked
            .and_then(|p| others.iter().find(|(o, _)| *o == p))
            .map_or("Choose…", |(_, label)| label.as_str());
        egui::ComboBox::from_id_salt(("merge_combo", salt))
            .selected_text(shown)
            .width(240.0)
            .show_ui(ui, |ui| {
                for (value, label) in others {
                    if ui
                        .selectable_value(&mut picked, Some(*value), label.as_str())
                        .clicked()
                    {
                        // A new choice needs its own acknowledgement.
                        acknowledged = false;
                    }
                }
            });
        let hover = format!(
            "Moves everything of {name} to the {noun} chosen, then removes {name}: for \
             duplicates. Edit > Undo puts it back."
        );
        let allowed = picked.is_some() && (warning.is_none() || acknowledged);
        if ui
            .add_enabled(allowed, egui::Button::new("Merge"))
            .on_hover_text(hover)
            .clicked()
        {
            merge = picked;
        }
    });
    if let Some(text) = &warning {
        ui.label(RichText::new(format!("⚠ {text}")).color(theme::WARNING));
        ui.checkbox(&mut acknowledged, "I understand, merge anyway");
    }
    if merge.is_some() {
        acknowledged = false;
    }
    ui.data_mut(|d| {
        d.insert_temp(id, picked);
        d.insert_temp(ack_id, acknowledged);
    });
    merge
}

/// Two accounts whose balances run opposite ways are usually an account
/// and its contra account (Equipment and Accumulated Depreciation), not
/// duplicates: merging them nets the two into one balance.
fn contra_warning(c: &Company, from: AccountId, into: AccountId) -> Option<String> {
    let a = c.account_balance(from, None);
    let b = c.account_balance(into, None);
    let opposite = (a.is_positive() && b.is_negative()) || (a.is_negative() && b.is_positive());
    opposite.then(|| {
        format!(
            "{} ({a}) and {} ({b}) have balances running opposite ways. That is usually \
             an account and its contra account, not a duplicate: merging nets them into one \
             balance and loses the split on reports.",
            c.account_name(from),
            c.account_name(into),
        )
    })
}

#[derive(Default)]
pub(crate) struct ContactInput {
    pub name: String,
    pub company: String,
    pub email: String,
    pub phone: String,
    pub line1: String,
    pub line2: String,
    pub city: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
    pub terms: String,
    pub notes: String,
    /// Ship to, when not to the address (customers only).
    pub ship_line1: String,
    pub ship_line2: String,
    pub ship_city: String,
    pub ship_region: String,
    pub ship_postal_code: String,
    pub ship_country: String,
}

impl ContactInput {
    /// The fields filled in from a customer or vendor, to edit.
    pub(crate) fn from_contact(c: &Contact) -> Self {
        let a = &c.address;
        Self {
            name: c.name.clone(),
            company: c.company.clone(),
            email: c.email.clone(),
            phone: c.phone.clone(),
            line1: a.line1.clone(),
            line2: a.line2.clone(),
            city: a.city.clone(),
            region: a.region.clone(),
            postal_code: a.postal_code.clone(),
            country: a.country.clone(),
            terms: if c.terms_days == 0 {
                String::new()
            } else {
                c.terms_days.to_string()
            },
            notes: c.notes.clone(),
            ship_line1: c.ship_to.line1.clone(),
            ship_line2: c.ship_to.line2.clone(),
            ship_city: c.ship_to.city.clone(),
            ship_region: c.ship_to.region.clone(),
            ship_postal_code: c.ship_to.postal_code.clone(),
            ship_country: c.ship_to.country.clone(),
        }
    }

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
                line2: self.line2.trim().to_string(),
                city: self.city.trim().to_string(),
                region: self.region.trim().to_string(),
                postal_code: self.postal_code.trim().to_string(),
                country: self.country.trim().to_string(),
            },
            terms_days,
            notes: self.notes.trim().to_string(),
            ship_to: Address {
                line1: self.ship_line1.trim().to_string(),
                line2: self.ship_line2.trim().to_string(),
                city: self.ship_city.trim().to_string(),
                region: self.ship_region.trim().to_string(),
                postal_code: self.ship_postal_code.trim().to_string(),
                country: self.ship_country.trim().to_string(),
            },
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

impl NewItem {
    /// The fields filled in from an item, to edit.
    pub(crate) fn from_item(item: &Item) -> Self {
        let s = &item.spec;
        Self {
            name: s.name.clone(),
            description: s.description.clone(),
            kind: s.kind,
            rate: s.rate.to_plain_string(),
            account: Some(s.income_account),
            not_taxable: !s.taxable,
        }
    }

    fn build(&self) -> Result<ItemSpec, String> {
        let income_account = self.account.ok_or("Choose an income account")?;
        let rate = Money::parse_or_zero(&self.rate).map_err(|e| e.to_string())?;
        Ok(ItemSpec {
            name: self.name.trim().to_string(),
            description: self.description.clone(),
            kind: self.kind,
            rate,
            income_account,
            taxable: !self.not_taxable,
        })
    }
}

#[derive(Default)]
pub(crate) struct ListState {
    pub new_account: NewAccount,
    pub new_customer: ContactInput,
    pub new_vendor: ContactInput,
    pub new_item: NewItem,
    pub show_inactive: bool,
    /// The customer, vendor or item being edited, with its fields.
    pub editing_customer: Option<(CustomerId, ContactInput)>,
    pub editing_vendor: Option<(VendorId, ContactInput)>,
    pub editing_item: Option<(ItemId, NewItem)>,
    pub editing_account: Option<(AccountId, NewAccount)>,
    /// Exemption certificate numbers being typed, by customer.
    pub exemption_notes: std::collections::BTreeMap<CustomerId, String>,
    /// Lists > Classes: the name of a new class, and the class being renamed.
    pub new_class: String,
    pub editing_class: Option<(TxnClassId, String)>,
}

impl ListState {
    pub(crate) fn reset(&mut self, what: Reset) {
        match what {
            Reset::NewAccount => self.new_account = NewAccount::default(),
            Reset::NewCustomer => self.new_customer = ContactInput::default(),
            Reset::NewVendor => self.new_vendor = ContactInput::default(),
            Reset::NewItem => self.new_item = NewItem::default(),
            Reset::EditedCustomer => self.editing_customer = None,
            Reset::EditedVendor => self.editing_vendor = None,
            Reset::EditedItem => self.editing_item = None,
            Reset::EditedAccount => self.editing_account = None,
            Reset::NewClass => self.new_class.clear(),
            Reset::EditedClass => self.editing_class = None,
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

/// Where a box on a Home flowchart leads.
#[derive(Clone, Copy)]
enum Go {
    Page(Page),
}

impl Go {
    fn hint(self) -> &'static str {
        match self {
            Self::Page(page) => page.hint(),
        }
    }

    fn action(self) -> Action {
        match self {
            Self::Page(page) => Action::Go(page),
        }
    }
}

/// A box on a Home flowchart, placed by column and row.
struct Step {
    label: &'static str,
    col: u8,
    row: u8,
    go: Go,
    /// A less everyday step: a shorter box with smaller text.
    minor: bool,
}

const fn step(label: &'static str, col: u8, row: u8, go: Go) -> Step {
    Step {
        label,
        col,
        row,
        go,
        minor: false,
    }
}

const fn minor(label: &'static str, col: u8, row: u8, go: Go) -> Step {
    Step {
        label,
        col,
        row,
        go,
        minor: true,
    }
}

const fn form(kind: FormKind) -> Go {
    Go::Page(Page::Form(kind))
}

/// A flowcard: one area of work on Home, as a card with a title, a line on
/// what it is for, a small flowchart whose arrows show what leads to what,
/// and links.
struct FlowCard {
    title: &'static str,
    /// Where clicking the title goes (the card's list), if anywhere.
    title_go: Option<Go>,
    blurb: &'static str,
    steps: &'static [Step],
    /// Arrows, as (from, to) indices into `steps`.
    arrows: &'static [(usize, usize)],
    links: &'static [(&'static str, Go)],
}

const CUSTOMERS: FlowCard = FlowCard {
    title: "Customers",
    title_go: Some(Go::Page(Page::Customers)),
    blurb: "Sell, get paid, and take the money to the bank.",
    steps: &[
        step("Create Invoices", 0, 0, form(FormKind::Invoice)),
        step("Receive Payments", 1, 0, form(FormKind::ReceivePayment)),
        step("Record Deposits", 2, 0, form(FormKind::Deposit)),
        step("Sales Receipts", 0, 1, form(FormKind::SalesReceipt)),
        minor("Credit Memos", 0, 2, form(FormKind::CreditMemo)),
        minor("Refunds", 1, 2, form(FormKind::Refund)),
    ],
    // Invoices are paid, then deposited; sales receipts are paid on the
    // spot and go straight to the deposit; credit left can be refunded.
    arrows: &[(0, 1), (1, 2), (3, 2), (4, 5)],
    links: &[],
};

const VENDORS: FlowCard = FlowCard {
    title: "Vendors",
    title_go: Some(Go::Page(Page::Vendors)),
    blurb: "Bills to pay later, or pay on the spot by check or card.",
    steps: &[
        step("Enter Bills", 0, 0, form(FormKind::Bill)),
        step("Pay Bills", 1, 0, form(FormKind::PayBills)),
        step("Write Checks", 0, 1, form(FormKind::Check)),
    ],
    arrows: &[(0, 1)],
    links: &[],
};

const BANKING: FlowCard = FlowCard {
    title: "Banking",
    title_go: None,
    blurb: "Bring in your bank's transactions, then check them off against its statement.",
    steps: &[
        step("Import Transactions", 0, 0, Go::Page(Page::BankImport)),
        step("Reconcile", 1, 0, Go::Page(Page::Reconcile)),
        minor("Journal Entry", 0, 1, form(FormKind::Journal)),
    ],
    arrows: &[(0, 1)],
    links: &[("Transactions", Go::Page(Page::Transactions))],
};

/// Flowchart boxes: widest and narrowest, heights, and the room between
/// columns (where the arrows go) and from one row to the next.
const STEP_MAX_WIDTH: f32 = 170.0;
const STEP_MIN_WIDTH: f32 = 96.0;
const STEP_HEIGHT: f32 = 40.0;
const MINOR_HEIGHT: f32 = 30.0;
const COLUMN_GAP: f32 = 36.0;
const ROW_PITCH: f32 = 52.0;
/// Space inside a card's border, and between cards.
const CARD_MARGIN: f32 = 14.0;
const CARD_GAP: f32 = 14.0;
/// The flowchart cards' widest (three full columns) and narrowest widths
/// before At A Glance moves below them.
const CARDS_MAX_WIDTH: f32 = 3.0 * STEP_MAX_WIDTH + 2.0 * COLUMN_GAP + 2.0 * CARD_MARGIN + 2.0;
const CARDS_MIN_WIDTH: f32 = 3.0 * 120.0 + 2.0 * COLUMN_GAP + 2.0 * CARD_MARGIN + 2.0;
/// At A Glance's narrowest and widest.
const GLANCE_MIN_WIDTH: f32 = 280.0;
const GLANCE_MAX_WIDTH: f32 = 460.0;

/// A Home card: raised from the page, with a thin border.
fn card_frame(ui: &Ui) -> egui::Frame {
    egui::Frame::NONE
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(8.0)
        .inner_margin(CARD_MARGIN)
}

/// A card's title, and the line under it.
fn card_heading(ui: &mut Ui, title: &str, blurb: &str) {
    ui.label(RichText::new(title).strong().size(16.0).color(theme::ACCENT));
    if !blurb.is_empty() {
        ui.label(RichText::new(blurb).small().color(theme::MUTED));
    }
    ui.add_space(8.0);
}

/// Draws an arrow from box `a` to box `b`: straight across when they share
/// a row, otherwise across and then up or down into `b`.
fn flow_arrow(painter: &egui::Painter, a: egui::Rect, b: egui::Rect, stroke: egui::Stroke) {
    let start = egui::pos2(a.right() + 3.0, a.center().y);
    let points = if (a.center().y - b.center().y).abs() < 1.0 {
        vec![start, egui::pos2(b.left() - 3.0, b.center().y)]
    } else {
        let end_y = if b.center().y < a.center().y {
            b.bottom() + 3.0
        } else {
            b.top() - 3.0
        };
        vec![
            start,
            egui::pos2(b.center().x, a.center().y),
            egui::pos2(b.center().x, end_y),
        ]
    };
    for pair in points.windows(2) {
        if let [from, to] = pair {
            painter.line_segment([*from, *to], stroke);
        }
    }
    if let [.., from, tip] = points.as_slice() {
        let dir = (*tip - *from).normalized();
        let side = dir.rot90();
        painter.line_segment([*tip, *tip - dir * 7.0 + side * 5.0], stroke);
        painter.line_segment([*tip, *tip - dir * 7.0 - side * 5.0], stroke);
    }
}

/// "customer list" for the Customers card: the list its title opens.
fn card_list_name(card: &FlowCard) -> String {
    let lower = card.title.to_lowercase();
    format!("{} list", lower.strip_suffix('s').unwrap_or(&lower))
}

/// Draws a flowcard, `width` wide.
fn flow_card(ui: &mut Ui, card: &FlowCard, width: f32, out: &mut Vec<Action>) {
    card_frame(ui).show(ui, |ui| {
        let inner = width - 2.0 * CARD_MARGIN - 2.0;
        ui.set_width(inner);
        match card.title_go {
            Some(go) => {
                // The title is a link to the card's list.
                // The asterisk points to the note at the bottom of the card.
                let title = RichText::new(format!("{}*", card.title))
                    .strong()
                    .size(16.0)
                    .color(theme::ACCENT);
                let link = egui::Button::new(title).frame(false);
                let hover = format!("Open the {}", card_list_name(card));
                if ui
                    .add(link)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(hover)
                    .clicked()
                {
                    out.push(go.action());
                }
                ui.label(RichText::new(card.blurb).small().color(theme::MUTED));
                ui.add_space(8.0);
            }
            None => card_heading(ui, card.title, card.blurb),
        }
        let rows = card.steps.iter().map(|s| s.row).max().map_or(1, |r| r + 1);
        let step_width =
            ((inner - 2.0 * COLUMN_GAP) / 3.0).clamp(STEP_MIN_WIDTH, STEP_MAX_WIDTH);
        let height = f32::from(rows) * ROW_PITCH - (ROW_PITCH - STEP_HEIGHT);
        let (area, _) = ui.allocate_exact_size(egui::vec2(inner, height), egui::Sense::hover());
        let rect_of = |s: &Step| {
            let x = area.left() + f32::from(s.col) * (step_width + COLUMN_GAP);
            let y = area.top() + f32::from(s.row) * ROW_PITCH;
            let h = if s.minor { MINOR_HEIGHT } else { STEP_HEIGHT };
            egui::Rect::from_min_size(
                egui::pos2(x, y + (STEP_HEIGHT - h) / 2.0),
                egui::vec2(step_width, h),
            )
        };
        // Arrows first, so the boxes sit on top of their ends.
        let stroke = egui::Stroke::new(2.0, theme::MUTED);
        for &(from, to) in card.arrows {
            if let (Some(a), Some(b)) = (card.steps.get(from), card.steps.get(to)) {
                flow_arrow(ui.painter(), rect_of(a), rect_of(b), stroke);
            }
        }
        for s in card.steps {
            let size = if s.minor { 13.0 } else { 15.0 };
            let button = egui::Button::new(RichText::new(s.label).size(size));
            if ui
                .put(rect_of(s), button)
                .on_hover_text(s.go.hint())
                .clicked()
            {
                out.push(s.go.action());
            }
        }
        if !card.links.is_empty() {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (i, (label, go)) in card.links.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new("·").color(theme::MUTED));
                    }
                    let link =
                        egui::Button::new(RichText::new(*label).color(theme::ACCENT)).frame(false);
                    if ui
                        .add(link)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(go.hint())
                        .clicked()
                    {
                        out.push(go.action());
                    }
                }
            });
        }
        if card.title_go.is_some() {
            ui.add_space(4.0);
            let note = format!("* Click {} to open the {}", card.title, card_list_name(card));
            // One row high: a right-to-left layout on its own would take the
            // rest of the page.
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(note).small().color(theme::MUTED));
                });
            });
        }
    });
}

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

    let glance = |ui: &mut Ui, width: f32| {
        card_frame(ui).show(ui, |ui| {
            ui.set_width(width - 2.0 * CARD_MARGIN - 2.0);
            card_heading(ui, "At A Glance", "");
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
            row(
                ui,
                "Customers owe you",
                true,
                total_of(AccountType::AccountsReceivable),
            );
            row(
                ui,
                "You owe vendors",
                true,
                total_of(AccountType::AccountsPayable),
            );
            row(ui, "Credit cards", true, total_of(AccountType::CreditCard));
            let since = format!("Net income since {fy_start}");
            row(ui, &since, false, net_income);
        });
    };
    let mut cards = |ui: &mut Ui, width: f32| {
        for card in [&CUSTOMERS, &VENDORS, &BANKING] {
            flow_card(ui, card, width, out);
            ui.add_space(CARD_GAP);
        }
    };
    // The cards and At A Glance side by side when both fit; otherwise At A
    // Glance goes below them.
    let full = ui.available_width();
    let cards_width = (full - CARD_GAP - GLANCE_MIN_WIDTH).min(CARDS_MAX_WIDTH);
    if cards_width >= CARDS_MIN_WIDTH {
        ui.horizontal_top(|ui| {
            let size = egui::vec2(cards_width, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                cards(ui, cards_width);
            });
            ui.add_space(CARD_GAP);
            let width = ui.available_width().min(GLANCE_MAX_WIDTH);
            ui.vertical(|ui| glance(ui, width));
        });
    } else {
        cards(ui, full.min(CARDS_MAX_WIDTH));
        glance(ui, full.min(GLANCE_MAX_WIDTH));
    }
}

// ----- chart of accounts -----------------------------------------------------------

pub(crate) fn accounts(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Chart of Accounts");
    edit_account(ui, c, s, out);
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
            });
            ui.horizontal_wrapped(|ui| {
                widgets::hint_label(ui, "Subaccount of");
                parent_combo(ui, "new_account", c, n.kind, None, &mut n.parent);
                if ui.button("Add").clicked() {
                    out.push(Action::Run {
                        cmd: Command::AccountCreate {
                            number: n.number.clone(),
                            name: n.name.clone(),
                            kind: n.kind,
                            description: n.description.clone(),
                            parent: n.parent,
                        },
                        reset: Reset::NewAccount,
                    });
                }
            });
        });
    ui.checkbox(&mut s.show_inactive, "Show inactive accounts");
    ui.add_space(6.0);
    let balances = widgets::balances(c);
    egui::ScrollArea::both()
        .id_salt("accounts_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("accounts")
                .striped(true)
                .spacing([18.0, 4.0])
                .show(ui, |ui| {
                    for h in ["Number", "Name", "Type", "Balance", "", ""] {
                        widgets::column_header(ui, h);
                    }
                    ui.end_row();
                    // Sub-accounts sit under their parents, indented.
                    for (a, depth) in c.accounts_in_tree_order() {
                        if !a.active && !s.show_inactive {
                            continue;
                        }
                        ui.label(a.number.as_str());
                        let mut name = RichText::new(a.name.as_str());
                        if !a.active {
                            name = name.color(theme::MUTED).italics();
                        }
                        ui.horizontal(|ui| {
                            let depth = u16::try_from(depth).unwrap_or(u16::MAX);
                            ui.add_space(18.0 * f32::from(depth));
                            if ui.link(name).on_hover_text("Open register").clicked() {
                                out.push(Action::Go(Page::Register(a.id)));
                            }
                        });
                        ui.label(a.kind.label());
                        money_label(ui, balances.get(&a.id).copied().unwrap_or(Money::ZERO), false);
                        if ui
                            .small_button("Edit")
                            .on_hover_text(
                                "Rename or renumber it, make it a sub-account, or merge it \
                                 into another",
                            )
                            .clicked()
                        {
                            s.editing_account = Some((a.id, NewAccount::from_account(a)));
                        }
                        let toggle = if a.active { "Make inactive" } else { "Make active" };
                        let toggle_hint = "Inactive accounts are hidden from lists but keep \
                                           their history. Only an account with a zero balance \
                                           can be made inactive.";
                        if ui.small_button(toggle).on_hover_text(toggle_hint).clicked() {
                            out.push(Action::Run {
                                cmd: Command::AccountSetActive {
                                    id: a.id,
                                    active: !a.active,
                                },
                                reset: Reset::Nothing,
                            });
                        }
                        ui.end_row();
                    }
                });
        });
}

/// The account being edited: number, name, description and parent, and
/// merging it into another of the same type.
fn edit_account(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    let Some((id, input)) = s.editing_account.as_mut() else {
        return;
    };
    let id = *id;
    let Some(a) = c.account(id) else {
        s.editing_account = None;
        return;
    };
    let mut close = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(format!("Edit account: {}", a.display_name())).strong());
        egui::Grid::new("edit_account")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                widgets::hint_label(ui, "Number");
                field(ui, &mut input.number, 80.0, "");
                ui.end_row();
                widgets::hint_label(ui, "Name");
                field(ui, &mut input.name, 240.0, "");
                ui.end_row();
                widgets::hint_label(ui, "Type");
                ui.label(RichText::new(a.kind.label()).color(theme::MUTED))
                    .on_hover_text("An account's type cannot change once it is created.");
                ui.end_row();
                widgets::hint_label(ui, "Description");
                field(ui, &mut input.description, 240.0, "");
                ui.end_row();
                widgets::hint_label(ui, "Subaccount of");
                parent_combo(ui, "edit_account", c, a.kind, Some(id), &mut input.parent);
                ui.end_row();
            });
        ui.horizontal_wrapped(|ui| {
            if ui.button(RichText::new("Save Changes").strong()).clicked() {
                out.push(Action::Run {
                    cmd: Command::AccountUpdate {
                        id,
                        number: input.number.clone(),
                        name: input.name.clone(),
                        description: input.description.clone(),
                        parent: input.parent,
                    },
                    reset: Reset::EditedAccount,
                });
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
        // Only into an account of the same type, and not one of its own
        // sub-accounts.
        let others: Vec<(AccountId, String)> = c
            .accounts_in_tree_order()
            .into_iter()
            .filter(|(o, _)| o.kind == a.kind && !c.descends_from(o.id, id))
            .map(|(o, _)| (o.id, c.account_full_name(o.id)))
            .collect();
        if let Some(into) = merge_row(ui, "account", "account", &a.name, &others, |into| {
            contra_warning(c, id, into)
        }) {
            out.push(Action::Run {
                cmd: Command::AccountMerge { from: id, into },
                reset: Reset::EditedAccount,
            });
        }
    });
    if close {
        s.editing_account = None;
    }
    ui.add_space(6.0);
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
    widgets::report_view(ui, &report, out, true);
}

// ----- customers / vendors -----------------------------------------------------------

/// The contact fields; with `ship_to`, a customer's ship-to address too.
fn contact_editor(ui: &mut Ui, salt: &str, n: &mut ContactInput, ship_to: bool) {
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
            widgets::hint_label(ui, "Address line 2");
            field(ui, &mut n.line2, 200.0, "Suite, unit or PO box");
            widgets::hint_label(ui, "State / region");
            field(ui, &mut n.region, 80.0, "");
            ui.end_row();
            widgets::hint_label(ui, "Postal code");
            field(ui, &mut n.postal_code, 100.0, "ZIP+4, e.g. 84601-1234");
            widgets::hint_label(ui, "Country");
            field(ui, &mut n.country, 160.0, "if not your own");
            ui.end_row();
            widgets::hint_label(ui, "Terms (days)");
            field(ui, &mut n.terms, 60.0, "30");
            widgets::hint_label(ui, "Notes");
            field(ui, &mut n.notes, 200.0, "");
            ui.end_row();
        });
    if ship_to {
        let filled = !n.ship_line1.trim().is_empty() || !n.ship_city.trim().is_empty();
        egui::CollapsingHeader::new("Ship to (if not the address above)")
            .id_salt((salt, "ship_to"))
            .default_open(filled)
            .show(ui, |ui| {
                egui::Grid::new((salt, "ship_grid"))
                    .num_columns(4)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        widgets::hint_label(ui, "Street");
                        field(ui, &mut n.ship_line1, 200.0, "");
                        widgets::hint_label(ui, "City");
                        field(ui, &mut n.ship_city, 200.0, "");
                        ui.end_row();
                        widgets::hint_label(ui, "Line 2");
                        field(ui, &mut n.ship_line2, 200.0, "");
                        widgets::hint_label(ui, "State / region");
                        field(ui, &mut n.ship_region, 80.0, "");
                        ui.end_row();
                        widgets::hint_label(ui, "Postal code");
                        field(ui, &mut n.ship_postal_code, 100.0, "");
                        widgets::hint_label(ui, "Country");
                        field(ui, &mut n.ship_country, 160.0, "if not your own");
                        ui.end_row();
                    });
            });
    }
}

/// What was clicked under an edit panel.
enum EditChoice {
    Nothing,
    Save,
    Cancel,
    SetActive(bool),
}

/// Save Changes, Cancel, and Make inactive (or active), under an edit panel.
fn edit_buttons(ui: &mut Ui, active: bool) -> EditChoice {
    let mut choice = EditChoice::Nothing;
    ui.horizontal_wrapped(|ui| {
        if ui.button(RichText::new("Save Changes").strong()).clicked() {
            choice = EditChoice::Save;
        }
        if ui.button("Cancel").clicked() {
            choice = EditChoice::Cancel;
        }
        let (label, hover) = if active {
            (
                "Make inactive",
                "Leave it off the lists on forms. Its transactions and balance stay.",
            )
        } else {
            ("Make active", "List it on forms again.")
        };
        if ui.button(label).on_hover_text(hover).clicked() {
            choice = EditChoice::SetActive(!active);
        }
    });
    choice
}

/// A name in a list, greyed out with "(inactive)" when it is.
fn list_name(ui: &mut Ui, name: &str, active: bool) {
    if active {
        ui.label(name);
    } else {
        ui.label(RichText::new(format!("{name} (inactive)")).color(theme::MUTED));
    }
}

/// The panel editing a customer, above the list.
fn edit_customer(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    let Some((id, input)) = s.editing_customer.as_mut() else {
        return;
    };
    let id = *id;
    let Some(x) = c.customer(id) else {
        s.editing_customer = None;
        return;
    };
    let mut close = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(format!("Edit customer: {}", x.contact.name)).strong());
        contact_editor(ui, "edit_customer", input, true);
        match edit_buttons(ui, x.active) {
            EditChoice::Save => match input.build() {
                Ok(contact) => out.push(Action::Run {
                    cmd: Command::CustomerUpdate { id, contact },
                    reset: Reset::EditedCustomer,
                }),
                Err(e) => out.push(Action::Error(e)),
            },
            EditChoice::Cancel => close = true,
            EditChoice::SetActive(active) => out.push(Action::Run {
                cmd: Command::CustomerSetActive { id, active },
                reset: Reset::Nothing,
            }),
            EditChoice::Nothing => {}
        }
        let others: Vec<(CustomerId, String)> = c
            .customers
            .iter()
            .filter(|o| o.id != id)
            .map(|o| (o.id, o.contact.name.clone()))
            .collect();
        if let Some(into) = merge_row(ui, "customer", "customer", &x.contact.name, &others, |_| None) {
            out.push(Action::Run {
                cmd: Command::CustomerMerge { from: id, into },
                reset: Reset::EditedCustomer,
            });
        }
    });
    if close {
        s.editing_customer = None;
    }
    ui.add_space(6.0);
}

/// The panel editing a vendor, above the list.
fn edit_vendor(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    let Some((id, input)) = s.editing_vendor.as_mut() else {
        return;
    };
    let id = *id;
    let Some(x) = c.vendors.iter().find(|v| v.id == id) else {
        s.editing_vendor = None;
        return;
    };
    let mut close = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(format!("Edit vendor: {}", x.contact.name)).strong());
        contact_editor(ui, "edit_vendor", input, false);
        match edit_buttons(ui, x.active) {
            EditChoice::Save => match input.build() {
                Ok(contact) => out.push(Action::Run {
                    cmd: Command::VendorUpdate { id, contact },
                    reset: Reset::EditedVendor,
                }),
                Err(e) => out.push(Action::Error(e)),
            },
            EditChoice::Cancel => close = true,
            EditChoice::SetActive(active) => out.push(Action::Run {
                cmd: Command::VendorSetActive { id, active },
                reset: Reset::Nothing,
            }),
            EditChoice::Nothing => {}
        }
        let others: Vec<(VendorId, String)> = c
            .vendors
            .iter()
            .filter(|o| o.id != id)
            .map(|o| (o.id, o.contact.name.clone()))
            .collect();
        if let Some(into) = merge_row(ui, "vendor", "vendor", &x.contact.name, &others, |_| None) {
            out.push(Action::Run {
                cmd: Command::VendorMerge { from: id, into },
                reset: Reset::EditedVendor,
            });
        }
    });
    if close {
        s.editing_vendor = None;
    }
    ui.add_space(6.0);
}

pub(crate) fn customers(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Customers");
    ui.checkbox(&mut s.show_inactive, "Show inactive");
    edit_customer(ui, c, s, out);
    egui::CollapsingHeader::new("New customer")
        .default_open(c.customers.is_empty())
        .show(ui, |ui| {
            contact_editor(ui, "new_customer", &mut s.new_customer, true);
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
                    for _ in 0..4 {
                        widgets::column_header(ui, "");
                    }
                    ui.end_row();
                    let show_inactive = s.show_inactive;
                    for x in c.customers.iter().filter(|x| x.active || show_inactive) {
                        list_name(ui, &x.contact.name, x.active);
                        ui.label(x.contact.email.as_str());
                        ui.label(x.contact.phone.as_str());
                        ui.label(terms_label(x.contact.terms_days));
                        money_label(ui, c.customer_balance(x.id), false);
                        if taxes {
                            customer_tax(ui, c, x, &mut s.exemption_notes, out);
                        }
                        if ui
                            .small_button("Edit")
                            .on_hover_text("Change their name, address, terms or notes")
                            .clicked()
                        {
                            s.editing_customer = Some((x.id, ContactInput::from_contact(&x.contact)));
                        }
                        if ui.small_button("Invoice").clicked() {
                            out.push(Action::InvoiceFor(x.id));
                        }
                        if ui.small_button("Receive payment").clicked() {
                            out.push(Action::PaymentFrom(x.id));
                        }
                        if ui
                            .small_button("Statement")
                            .on_hover_text("Save their statement for a period as a PDF")
                            .clicked()
                        {
                            out.push(Action::Statement(Some(x.id)));
                        }
                        ui.end_row();
                    }
                });
        });
}

pub(crate) fn vendors(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Vendors");
    ui.checkbox(&mut s.show_inactive, "Show inactive");
    edit_vendor(ui, c, s, out);
    egui::CollapsingHeader::new("New vendor")
        .default_open(c.vendors.is_empty())
        .show(ui, |ui| {
            contact_editor(ui, "new_vendor", &mut s.new_vendor, false);
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
                    for h in ["Name", "Email", "Phone", "Terms", "Balance", "", "", ""] {
                        widgets::column_header(ui, h);
                    }
                    ui.end_row();
                    let show_inactive = s.show_inactive;
                    for x in c.vendors.iter().filter(|x| x.active || show_inactive) {
                        list_name(ui, &x.contact.name, x.active);
                        ui.label(x.contact.email.as_str());
                        ui.label(x.contact.phone.as_str());
                        ui.label(terms_label(x.contact.terms_days));
                        money_label(ui, c.vendor_balance(x.id), false);
                        if ui
                            .small_button("Edit")
                            .on_hover_text("Change their name, address, terms or notes")
                            .clicked()
                        {
                            s.editing_vendor = Some((x.id, ContactInput::from_contact(&x.contact)));
                        }
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
                if ui
                    .add(egui::Button::selectable(
                        !current.exempt && current.group.is_none(),
                        "— (choose on each invoice)",
                    ))
                    .clicked()
                {
                    out.push(set(CustomerTax::default()));
                }
                for g in c
                    .tax_groups
                    .iter()
                    .filter(|g| g.active || current.group == Some(g.id))
                {
                    let on = !current.exempt && current.group == Some(g.id);
                    if ui
                        .add(egui::Button::selectable(on, g.name.as_str()))
                        .clicked()
                    {
                        out.push(set(CustomerTax {
                            group: Some(g.id),
                            exempt: false,
                            exemption: current.exemption.clone(),
                        }));
                    }
                }
                if ui
                    .add(egui::Button::selectable(current.exempt, "Exempt"))
                    .on_hover_text("Not charged sales tax, e.g. a reseller or nonprofit")
                    .clicked()
                {
                    out.push(set(CustomerTax {
                        group: current.group,
                        exempt: true,
                        exemption: current.exemption.clone(),
                    }));
                }
                ui.separator();
                if ui
                    .button("Find from address (ZIP+4)…")
                    .on_hover_text(
                        "Look up their tax location in your state's rate files and use its group",
                    )
                    .clicked()
                {
                    out.push(Action::FindTaxGroup(x.id));
                }
            });
        if current.exempt {
            let note = notes
                .entry(x.id)
                .or_insert_with(|| current.exemption.clone());
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

/// An item's fields, for New item and Edit.
fn item_fields(ui: &mut Ui, c: &Company, salt: &str, n: &mut NewItem) {
    egui::Grid::new(salt)
        .num_columns(4)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Name");
            field(ui, &mut n.name, 200.0, "");
            widgets::hint_label(ui, "Type");
            egui::ComboBox::from_id_salt((salt, "kind"))
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
            widgets::account_combo(ui, (salt, "acct"), c, &mut n.account, |k| {
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
}

/// The panel editing an item, above the list.
fn edit_item(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    let Some((id, input)) = s.editing_item.as_mut() else {
        return;
    };
    let id = *id;
    let Some(x) = c.item(id) else {
        s.editing_item = None;
        return;
    };
    let mut close = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(format!("Edit item: {}", x.spec.name)).strong());
        ui.label(
            RichText::new(
                "Changes apply to new invoices; invoices already recorded keep their lines.",
            )
            .color(theme::MUTED),
        );
        item_fields(ui, c, "edit_item", input);
        match edit_buttons(ui, x.active) {
            EditChoice::Save => match input.build() {
                Ok(spec) => out.push(Action::Run {
                    cmd: Command::ItemUpdate { id, spec },
                    reset: Reset::EditedItem,
                }),
                Err(e) => out.push(Action::Error(e)),
            },
            EditChoice::Cancel => close = true,
            EditChoice::SetActive(active) => out.push(Action::Run {
                cmd: Command::ItemSetActive { id, active },
                reset: Reset::Nothing,
            }),
            EditChoice::Nothing => {}
        }
        let others: Vec<(ItemId, String)> = c
            .items
            .iter()
            .filter(|o| o.id != id)
            .map(|o| (o.id, o.spec.name.clone()))
            .collect();
        if let Some(into) = merge_row(ui, "item", "item", &x.spec.name, &others, |_| None) {
            out.push(Action::Run {
                cmd: Command::ItemMerge { from: id, into },
                reset: Reset::EditedItem,
            });
        }
    });
    if close {
        s.editing_item = None;
    }
    ui.add_space(6.0);
}

pub(crate) fn items(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Products and Services");
    ui.checkbox(&mut s.show_inactive, "Show inactive");
    edit_item(ui, c, s, out);
    egui::CollapsingHeader::new("New item")
        .default_open(c.items.is_empty())
        .show(ui, |ui| {
            item_fields(ui, c, "new_item", &mut s.new_item);
            if ui.button("Add item").clicked() {
                match s.new_item.build() {
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
            for h in [
                "Name",
                "Type",
                "Description",
                "Rate",
                "Income account",
                "Taxable",
                "",
            ] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            let show_inactive = s.show_inactive;
            for x in c.items.iter().filter(|x| x.active || show_inactive) {
                list_name(ui, &x.spec.name, x.active);
                ui.label(x.spec.kind.label());
                ui.label(x.spec.description.as_str());
                money_label(ui, x.spec.rate, false);
                ui.label(c.account_name(x.spec.income_account));
                let mut taxable = x.spec.taxable;
                if ui
                    .checkbox(&mut taxable, "")
                    .on_hover_text(
                        "Charge sales tax on it. Invoices already recorded keep their tax.",
                    )
                    .changed()
                {
                    out.push(Action::Run {
                        cmd: Command::ItemSetTaxable { id: x.id, taxable },
                        reset: Reset::Nothing,
                    });
                }
                if ui
                    .small_button("Edit")
                    .on_hover_text("Change its name, description, price, account or tax")
                    .clicked()
                {
                    s.editing_item = Some((x.id, NewItem::from_item(x)));
                }
                ui.end_row();
            }
        });
}

// ----- classes -------------------------------------------------------------------------

/// Lists > Classes: departments, locations or lines of business that
/// transactions can be tagged with, for Profit & Loss by Class.
pub(crate) fn classes(ui: &mut Ui, c: &Company, s: &mut ListState, out: &mut Vec<Action>) {
    title(ui, "Classes");
    ui.label(
        RichText::new(
            "Tag transactions with a class (a shop, a location, a line of business) and \
             Reports > Profit & Loss by Class shows each one's income and expenses. \
             Forms show a Class box once you add one here.",
        )
        .color(theme::MUTED),
    );
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "New class");
        field(ui, &mut s.new_class, 200.0, "e.g. Craft fairs");
        if ui.button("Add class").clicked() {
            let name = s.new_class.trim().to_string();
            if name.is_empty() {
                out.push(Action::Error("Give the class a name.".into()));
            } else {
                out.push(Action::Run {
                    cmd: Command::ClassCreate { name },
                    reset: Reset::NewClass,
                });
            }
        }
    });
    ui.checkbox(&mut s.show_inactive, "Show inactive");
    ui.add_space(6.0);
    if c.classes.is_empty() {
        ui.label(RichText::new("No classes yet.").color(theme::MUTED));
        return;
    }
    let mut cancel = false;
    let mut rename = None;
    egui::Grid::new("classes")
        .striped(true)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            for h in ["Name", "Transactions", ""] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            let show_inactive = s.show_inactive;
            for k in c.classes.iter().filter(|k| k.active || show_inactive) {
                let used = c.transactions.iter().filter(|t| t.class == Some(k.id)).count();
                let renaming = s.editing_class.as_mut().filter(|(id, _)| *id == k.id);
                match renaming {
                    Some((_, name)) => {
                        field(ui, name, 200.0, "");
                        ui.label(used.to_string());
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                out.push(Action::Run {
                                    cmd: Command::ClassUpdate {
                                        id: k.id,
                                        name: name.trim().to_string(),
                                    },
                                    reset: Reset::EditedClass,
                                });
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    }
                    _ => {
                        list_name(ui, &k.name, k.active);
                        ui.label(used.to_string());
                        ui.horizontal(|ui| {
                            if ui.small_button("Rename").clicked() {
                                rename = Some((k.id, k.name.clone()));
                            }
                            let (label, hover) = if k.active {
                                (
                                    "Make inactive",
                                    "Leave it off the forms. Its transactions keep it.",
                                )
                            } else {
                                ("Make active", "List it on forms again.")
                            };
                            if ui.small_button(label).on_hover_text(hover).clicked() {
                                out.push(Action::Run {
                                    cmd: Command::ClassSetActive {
                                        id: k.id,
                                        active: !k.active,
                                    },
                                    reset: Reset::Nothing,
                                });
                            }
                        });
                    }
                }
                ui.end_row();
            }
        });
    if cancel {
        s.editing_class = None;
    }
    if rename.is_some() {
        s.editing_class = rename;
    }
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
                                if ledgercraft_reports::has_pdf(&t.body)
                                    && ui
                                        .small_button("PDF")
                                        .on_hover_text("Save it as a PDF to email or print")
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
    widgets::report_view(ui, &report, out, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_a_contact_keeps_every_field() {
        let contact = Contact {
            name: "Dana Ortiz".into(),
            company: "Maple Street Cafe".into(),
            email: "orders@maple.example".into(),
            phone: "801-555-0100".into(),
            address: Address {
                line1: "12 Maple Street".into(),
                line2: "Suite 4".into(),
                city: "Provo".into(),
                region: "UT".into(),
                postal_code: "84601-1100".into(),
                country: "USA".into(),
            },
            terms_days: 30,
            notes: "Delivers on Fridays".into(),
            ship_to: Address {
                line1: "40 Warehouse Row".into(),
                city: "Orem".into(),
                region: "UT".into(),
                ..Address::default()
            },
        };
        assert_eq!(ContactInput::from_contact(&contact).build(), Ok(contact));
    }

    #[test]
    fn editing_an_item_keeps_every_field() {
        let c = ledgercraft_engine::sample_company().unwrap();
        for item in &c.items {
            assert_eq!(NewItem::from_item(item).build(), Ok(item.spec.clone()));
        }
    }

    #[test]
    fn the_lists_draw_with_their_editors_open() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let customer = &c.customers[0];
        let vendor = &c.vendors[0];
        let item = &c.items[0];
        let mut s = ListState {
            show_inactive: true,
            editing_customer: Some((customer.id, ContactInput::from_contact(&customer.contact))),
            editing_vendor: Some((vendor.id, ContactInput::from_contact(&vendor.contact))),
            editing_item: Some((item.id, NewItem::from_item(item))),
            editing_account: Some((c.accounts[0].id, NewAccount::from_account(&c.accounts[0]))),
            ..ListState::default()
        };
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = Vec::new();
        crate::theme::test_frame(&ctx, |ui| {
            customers(ui, &c, &mut s, &mut out);
            vendors(ui, &c, &mut s, &mut out);
            items(ui, &c, &mut s, &mut out);
            accounts(ui, &c, &mut s, &mut out);
            classes(ui, &c, &mut s, &mut out);
        });
        assert!(out.is_empty());
        assert!(s.editing_customer.is_some() && s.editing_item.is_some());
    }
}
