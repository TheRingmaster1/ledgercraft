use std::path::PathBuf;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use ledgercraft_engine::{sample_company, Session};
use ledgercraft_ledger::Company;

use crate::forms::Forms;
use crate::licenses::{LicenseTab, LicenseWindow};
use crate::pages::{self, ListState, ReportState};
use crate::{forms, theme, Action, FormKind, Page, Reset};

/// Something waiting for the user to decide what to do with unsaved changes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    New,
    Open,
    Sample,
    Quit,
}

pub struct LedgerApp {
    session: Session,
    page: Page,
    forms: Forms,
    lists: ListState,
    reports: ReportState,
    status: String,
    status_is_error: bool,
    pending: Option<Pending>,
    allow_close: bool,
    show_about: bool,
    licenses: LicenseWindow,
    show_nav: bool,
    title: String,
}

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const INVOICE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
const CHECK: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);
const SIDE_BAR: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);

impl LedgerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, session: Session) -> Self {
        theme::apply(&cc.egui_ctx);
        Self::with_session(session)
    }

    fn with_session(session: Session) -> Self {
        Self {
            session,
            page: Page::Home,
            forms: Forms::default(),
            lists: ListState::default(),
            reports: ReportState::default(),
            status: "Ready".into(),
            status_is_error: false,
            pending: None,
            allow_close: false,
            show_about: false,
            licenses: LicenseWindow::default(),
            show_nav: true,
            title: String::new(),
        }
    }

    fn info(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_is_error = false;
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_is_error = true;
    }

    // ----- files -----------------------------------------------------------

    fn request(&mut self, what: Pending, ctx: &egui::Context) {
        if self.session.is_dirty() {
            self.pending = Some(what);
        } else {
            self.carry_out(what, ctx);
        }
    }

    fn carry_out(&mut self, what: Pending, ctx: &egui::Context) {
        match what {
            Pending::New => {
                self.replace_session(Session::new(Company::with_default_accounts("My Company")));
                self.info("New company created with a starter chart of accounts. Rename it in Company ▸ Settings.");
            }
            Pending::Sample => match sample_company() {
                Ok(c) => {
                    self.replace_session(Session::new(c));
                    self.info("Opened the sample company. Changes are yours to keep or discard.");
                }
                Err(e) => self.error(format!("Could not build the sample: {e}")),
            },
            Pending::Open => {
                let picked = rfd::FileDialog::new()
                    .add_filter("LedgerCraft company", &[ledgercraft_file::EXTENSION])
                    .pick_file();
                if let Some(path) = picked {
                    self.open_path(path);
                }
            }
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn open_path(&mut self, path: PathBuf) {
        match Session::open(&path) {
            Ok(s) => {
                self.replace_session(s);
                self.info(format!("Opened {}", path.display()));
            }
            Err(e) => self.error(format!("Could not open {}: {e}", path.display())),
        }
    }

    fn replace_session(&mut self, s: Session) {
        self.session = s;
        self.forms = Forms::default();
        self.lists = ListState::default();
        self.page = Page::Home;
    }

    /// Saves; returns true when the books are safely on disk.
    fn save(&mut self) -> bool {
        if self.session.path().is_none() {
            return self.save_as();
        }
        match self.session.save() {
            Ok(()) => {
                self.info("Saved");
                true
            }
            Err(e) => {
                self.error(e.to_string());
                false
            }
        }
    }

    fn save_as(&mut self) -> bool {
        let name = format!("{}.{}", self.session.company().info.name, ledgercraft_file::EXTENSION);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LedgerCraft company", &[ledgercraft_file::EXTENSION])
            .set_file_name(name)
            .save_file()
        else {
            return false;
        };
        match self.session.save_as(&path) {
            Ok(()) => {
                self.info(format!("Saved to {}", path.display()));
                true
            }
            Err(e) => {
                self.error(e.to_string());
                false
            }
        }
    }

    fn export_csv(&mut self, name: &str, csv: &str) {
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name(name).save_file()
        else {
            return;
        };
               match std::fs::write(&path, format!("\u{feff}{csv}")) {

            Ok(()) => self.info(format!("Exported {}", path.display())),
            Err(e) => self.error(format!("Could not write {}: {e}", path.display())),
        }
    }

    fn undo(&mut self) {
        match self.session.undo() {
            Some(label) => self.info(format!("Undid {label}")),
            None => self.info("Nothing to undo"),
        }
    }

    fn redo(&mut self) {
        match self.session.redo() {
            Some(label) => self.info(format!("Redid {label}")),
            None => self.info("Nothing to redo"),
        }
    }

    // ----- actions ------------------------------------------------------------

    fn apply(&mut self, action: Action) {
        match action {
            Action::Run { cmd, reset } => {
                let label = cmd.undo_label();
                match self.session.execute(cmd) {
                    Ok(outcome) => {
                        if let Reset::Form(kind) = reset {
                            self.forms.reset(kind);
                        }
                        self.lists.reset(reset);
                        match outcome.created {
                            Some(id) => self.info(format!("{label} recorded ({id})")),
                            None => self.info(format!("{label}: done")),
                        }
                    }
                    Err(e) => self.error(e.to_string()),
                }
            }
            Action::Go(page) => self.page = page,
            Action::InvoiceFor(id) => {
                self.forms.invoice.customer = Some(id);
                let terms = self.session.company().customer(id).map(|c| c.contact.terms_days);
                if let (Some(t), Ok(d)) = (terms, ledgercraft_core::Date::parse(&self.forms.invoice.date)) {
                    self.forms.invoice.due = d.add_days(i64::from(t)).to_string();
                }
                self.page = Page::Form(FormKind::Invoice);
            }
            Action::PaymentFrom(id) => {
                if self.forms.receive.customer != Some(id) {
                    self.forms.receive.amounts.clear();
                }
                self.forms.receive.customer = Some(id);
                self.page = Page::Form(FormKind::ReceivePayment);
            }
            Action::BillFrom(id) => {
                self.forms.bill.vendor = Some(id);
                self.page = Page::Form(FormKind::Bill);
            }
            Action::PayVendor(id) => {
                if self.forms.pay_bills.vendor != Some(id) {
                    self.forms.pay_bills.amounts.clear();
                }
                self.forms.pay_bills.vendor = Some(id);
                self.page = Page::Form(FormKind::PayBills);
            }
            Action::Error(e) => self.error(e),
            Action::ExportCsv { name, csv } => self.export_csv(&name, &csv),
            Action::ClearStatus => self.info("Ready"),
        }
    }

    // ----- chrome ----------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let pressed = |s: &KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(s));
        if pressed(&SAVE) {
            self.save();
        }
        if pressed(&OPEN) {
            self.request(Pending::Open, ctx);
        }
        if pressed(&NEW) {
            self.request(Pending::New, ctx);
        }
        // While typing, Ctrl+Z / Ctrl+Y belong to the text field.
        let typing = ctx.memory(|m| m.focused().is_some());
        if !typing && pressed(&UNDO) {
            self.undo();
        }
        if !typing && pressed(&REDO) {
            self.redo();
        }
        if pressed(&INVOICE) {
            self.page = Page::Form(FormKind::Invoice);
        }
        if pressed(&CHECK) {
            self.page = Page::Form(FormKind::Check);
        }

        if pressed(&SIDE_BAR) {
            self.show_nav = !self.show_nav;
        }
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                if !self.show_nav && ui.button(">").on_hover_text("Show navigation bar (Cmd+B)").clicked() {
                    self.show_nav = true;
                }
                ui.menu_button("File", |ui| {
                    if ui.button("New Company").clicked() {
                        ui.close_menu();
                        self.request(Pending::New, ctx);
                    }
                    if ui.button("Open Company…").clicked() {
                        ui.close_menu();
                        self.request(Pending::Open, ctx);
                    }
                    if ui.button("Open Sample Company").clicked() {
                        ui.close_menu();
                        self.request(Pending::Sample, ctx);
                    }
                    ui.separator();
                    if ui.button("Save").clicked() {
                        ui.close_menu();
                        self.save();
                    }
                    if ui.button("Save As…").clicked() {
                        ui.close_menu();
                        self.save_as();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ui.close_menu();
                        self.request(Pending::Quit, ctx);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let undo = self.session.undo_label().map(|l| format!("Undo {l}"));
                    if ui.add_enabled(undo.is_some(), egui::Button::new(undo.unwrap_or_else(|| "Undo".into()))).clicked() {
                        ui.close_menu();
                        self.undo();
                    }
                    let redo = self.session.redo_label().map(|l| format!("Redo {l}"));
                    if ui.add_enabled(redo.is_some(), egui::Button::new(redo.unwrap_or_else(|| "Redo".into()))).clicked() {
                        ui.close_menu();
                        self.redo();
                    }
                });
                                ui.menu_button("View", |ui| {
                    if ui.checkbox(&mut self.show_nav, "Side Bar    Ctrl+B").clicked() {
                        ui.close_menu();
                    }
                });

                ui.menu_button("Lists", |ui| {
                    for (label, page) in [
                        ("Chart of Accounts", Page::Accounts),
                        ("Customers", Page::Customers),
                        ("Vendors", Page::Vendors),
                        ("Products and Services", Page::Items),
                    ] {
                        if ui.button(label).clicked() {
                            ui.close_menu();
                            self.page = page;
                        }
                    }
                });
                ui.menu_button("Customers", |ui| {
                    for kind in [FormKind::Invoice, FormKind::ReceivePayment, FormKind::Deposit] {
                        if ui.button(kind.title()).clicked() {
                            ui.close_menu();
                            self.page = Page::Form(kind);
                        }
                    }
                });
                ui.menu_button("Vendors", |ui| {
                    for kind in [FormKind::Bill, FormKind::PayBills] {
                        if ui.button(kind.title()).clicked() {
                            ui.close_menu();
                            self.page = Page::Form(kind);
                        }
                    }
                });
                ui.menu_button("Banking", |ui| {
                    for kind in [FormKind::Check, FormKind::Deposit, FormKind::Journal] {
                        if ui.button(kind.title()).clicked() {
                            ui.close_menu();
                            self.page = Page::Form(kind);
                        }
                    }
                });
                ui.menu_button("Reports", |ui| {
                    for kind in ledgercraft_reports::ReportKind::ALL {
                        if ui.button(kind.label()).clicked() {
                            ui.close_menu();
                            self.reports.kind = kind;
                            self.page = Page::Reports;
                        }
                    }
                });
                ui.menu_button("Help", |ui| {
                                        if ui.button("Licenses").clicked() {
                        ui.close_menu();
                        self.licenses.show_tab(LicenseTab::Summary);
                    }
                    ui.separator();
                    
                    if ui.button("About LedgerCraft").clicked() {
                        ui.close_menu();
                        self.show_about = true;
                    }
                });
            });
        });
    }

    fn nav(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(190.0)
            .frame(egui::Frame::none().fill(theme::NAV_BG).inner_margin(10.0))
            .show_animated(ctx, self.show_nav, |ui| {
                theme::nav_visuals(ui);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("LedgerCraft").size(20.0).strong().color(theme::NAV_TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let hide = egui::Button::new(RichText::new("<").color(theme::NAV_TEXT)).frame(false);
                        if ui.add(hide).on_hover_text("Hide side bar (Ctrl+B)").clicked() {
                            self.show_nav = false;
                        }
                    });
                });
                ui.add_space(12.0);
                let items: [(&str, Page); 8] = [
                    ("Home", Page::Home),
                    ("Chart of Accounts", Page::Accounts),
                    ("Customers", Page::Customers),
                    ("Vendors", Page::Vendors),
                    ("Products & Services", Page::Items),
                    ("Transactions", Page::Transactions),
                    ("Reports", Page::Reports),
                    ("Journal Entry", Page::Form(FormKind::Journal)),
                ];
                for (label, page) in items {
                    let selected = self.page == page;
                    let text = RichText::new(label).size(15.0).color(theme::NAV_TEXT);
                                        if ui.selectable_label(selected, text).on_hover_text(page.hint()).clicked() {
                        self.page = page;

                    }
                }
                ui.add_space(16.0);
                ui.label(RichText::new("Create").color(theme::NAV_TEXT).small());
                for kind in [FormKind::Invoice, FormKind::ReceivePayment, FormKind::Bill, FormKind::PayBills, FormKind::Check, FormKind::Deposit] {
                    let selected = self.page == Page::Form(kind);
                    let text = RichText::new(kind.title()).color(theme::NAV_TEXT);
                                        if ui.selectable_label(selected, text).on_hover_text(kind.hint()).clicked() {
                        self.page = Page::Form(kind);

                    }
                }
            });
    }

    fn status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let color = if self.status_is_error { theme::NEGATIVE } else { theme::MUTED };
                ui.label(RichText::new(self.status.as_str()).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let file = self
                        .session
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Not saved yet".into());
                    let dirty = if self.session.is_dirty() { "  • unsaved changes" } else { "" };
                    ui.label(RichText::new(format!("{file}{dirty}")).color(theme::MUTED));
                });
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(what) = self.pending {
            let mut choice = None;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Your company file has changes that are not saved yet.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Save").strong()).clicked() {
                            choice = Some(0);
                        }
                        if ui.button("Don't save").clicked() {
                            choice = Some(1);
                        }
                        if ui.button("Cancel").clicked() {
                            choice = Some(2);
                        }
                    });
                });
            match choice {
                Some(0) => {
                    self.pending = None;
                    if self.save() {
                        self.carry_out(what, ctx);
                    }
                }
                Some(1) => {
                    self.pending = None;
                    self.carry_out(what, ctx);
                }
                Some(_) => self.pending = None,
                None => {}
            }
        }
        let mut open = self.show_about;
        let mut view_licenses = false;
        egui::Window::new("About LedgerCraft").open(&mut open).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new(format!("LedgerCraft {}", env!("CARGO_PKG_VERSION"))).strong());
            ui.label("Open-source, double-entry small-business accounting in pure Rust.");
            ui.horizontal(|ui| {
                ui.label("Dual-licensed under MIT or Apache-2.0.");
                view_licenses = ui.link("View licenses").clicked();
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "LedgerCraft is an independent project, not affiliated with or endorsed by \
                     Intuit Inc. QuickBooks is a trademark of Intuit Inc.",
                )
                .small()
                .color(theme::MUTED),
            );
        });
                self.show_about = open;
        if view_licenses {
            self.licenses.show_tab(LicenseTab::Summary);
        }
        self.licenses.ui(ctx);

    }
}

impl eframe::App for LedgerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Never lose work on close.
        if ctx.input(|i| i.viewport().close_requested()) && self.session.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
        let title = format!("LedgerCraft — {}", self.session.title());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        self.shortcuts(ctx);
        self.menu_bar(ctx);
        self.status_bar(ctx);
        self.nav(ctx);

        let mut actions: Vec<Action> = Vec::new();
        let page = self.page;
        egui::CentralPanel::default().show(ctx, |ui| {
            let company = self.session.company();
            match page {
                // Pages with their own scroll areas.
                Page::Accounts => pages::accounts(ui, company, &mut self.lists, &mut actions),
                Page::Register(id) => pages::register(ui, company, id, &mut actions),
                Page::Customers => pages::customers(ui, company, &mut self.lists, &mut actions),
                Page::Vendors => pages::vendors(ui, company, &mut self.lists, &mut actions),
                Page::Transactions => pages::transactions(ui, company, &mut actions),
                Page::Reports => pages::reports(ui, company, &mut self.reports, &mut actions),
                _ => {
                    let salt = ("page", format!("{page:?}"));
                    egui::ScrollArea::vertical().id_salt(salt).show(ui, |ui| match page {
                        Page::Home => pages::home(ui, company, &mut actions),
                        Page::Items => pages::items(ui, company, &mut self.lists, &mut actions),
                        Page::Form(FormKind::Invoice) => {
                            forms::invoice(ui, company, &mut self.forms.invoice, &mut actions)
                        }
                        Page::Form(FormKind::ReceivePayment) => {
                            forms::receive_payment(ui, company, &mut self.forms.receive, &mut actions)
                        }
                        Page::Form(FormKind::Bill) => {
                            forms::bill(ui, company, &mut self.forms.bill, &mut actions)
                        }
                        Page::Form(FormKind::PayBills) => {
                            forms::pay_bills(ui, company, &mut self.forms.pay_bills, &mut actions)
                        }
                        Page::Form(FormKind::Check) => {
                            forms::check(ui, company, &mut self.forms.check, &mut actions)
                        }
                        Page::Form(FormKind::Deposit) => {
                            forms::deposit(ui, company, &mut self.forms.deposit, &mut actions)
                        }
                        Page::Form(FormKind::Journal) => {
                            forms::journal(ui, company, &mut self.forms.journal, &mut actions)
                        }
                        _ => {}
                    });
                }
            }
        });
        for action in actions {
            self.apply(action);
        }
        self.dialogs(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_engine::new_company;

    // Regression: Clear on a form sent ClearStatus, which hit a `panic!` and
    // closed the app.
    #[test]
    fn clear_status_resets_the_status_bar() {
        let mut app = LedgerApp::with_session(Session::new(new_company("Clear Co")));
        app.error("Invoice 1001 is missing a customer");
        app.apply(Action::ClearStatus);
        assert_eq!(app.status, "Ready");
        assert!(!app.status_is_error);
    }
}
