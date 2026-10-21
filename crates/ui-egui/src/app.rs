use std::path::PathBuf;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use ledgercraft_engine::{sample_company, Session};

use crate::forms::Forms;
use crate::licenses::{LicenseTab, LicenseWindow};
use crate::pages::{self, ListState, ReportState};
use crate::prefs::{self, Prefs};
use crate::start::{self, NewCompanyForm, StartChoice, StartPage};
use crate::{forms, theme, tutorial, Action, FormKind, Page, Reset};

/// Something waiting for the user to decide what to do with unsaved changes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    New,
    Open,
    Sample,
    Close,
    Quit,
}

pub struct LedgerApp {
    /// The open company; `None` shows the start page.
    session: Option<Session>,
    /// The Create a Company window, while it is open.
    create: Option<NewCompanyForm>,
    start: StartPage,
    page: Page,
    forms: Forms,
    lists: ListState,
    reports: ReportState,
    status: String,
    status_is_error: bool,
    pending: Option<Pending>,
    allow_close: bool,
    show_about: bool,
    show_prefs: bool,
    show_tutorial: bool,
    tutorial: tutorial::Tutorial,
    prefs: Prefs,
    /// What is on disk, so changes are saved once rather than every frame.
    saved_prefs: Prefs,
    licenses: LicenseWindow,
    show_nav: bool,
    title: String,
    /// Scale the window size limits were last set for (0 = not yet).
    window_scale: f32,
}

/// Smallest window the layout still works in, in ui points.
const MIN_WINDOW: egui::Vec2 = egui::vec2(900.0, 560.0);

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const INVOICE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
const CHECK: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);
const SIDE_BAR: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);

impl LedgerApp {
    /// `None` starts with no company open, on the start page.
    pub fn new(cc: &eframe::CreationContext<'_>, session: Option<Session>) -> Self {
        theme::apply(&cc.egui_ctx);
        let prefs = Prefs::load();
        Self {
            show_tutorial: prefs.show_tutorial,
            prefs,
            saved_prefs: prefs,
            ..Self::blank(session)
        }
    }

    #[cfg(test)]
    fn with_session(session: Session) -> Self {
        Self::blank(Some(session))
    }

    fn blank(session: Option<Session>) -> Self {
        Self {
            session,
            create: None,
            start: StartPage::default(),
            page: Page::Home,
            forms: Forms::default(),
            lists: ListState::default(),
            reports: ReportState::default(),
            status: "Ready".into(),
            status_is_error: false,
            pending: None,
            allow_close: false,
            show_about: false,
            show_prefs: false,
            show_tutorial: false,
            tutorial: tutorial::Tutorial::default(),
            prefs: Prefs::default(),
            saved_prefs: Prefs::default(),
            licenses: LicenseWindow::default(),
            show_nav: true,
            title: String::new(),
            window_scale: 0.0,
        }
    }

    /// Set the minimum window size, and on the first frame fit the window to
    /// its monitor. egui-winit 0.29 converts `ViewportBuilder` sizes with the
    /// primary monitor's scale, so with a Retina main display and a 1x
    /// external one the window opened at double size and could not shrink.
    /// Viewport commands use the window's own scale, so set the limits here
    /// and again whenever the window moves to a monitor with another scale.
    fn fit_window(&mut self, ctx: &egui::Context) {
        let scale = ctx.pixels_per_point();
        if scale == self.window_scale {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(MIN_WINDOW));
        if self.window_scale == 0.0 {
            let (inner, monitor) =
                ctx.input(|i| (i.viewport().inner_rect, i.viewport().monitor_size));
            if let (Some(inner), Some(monitor)) = (inner, monitor) {
                let fitted = inner.size().min(monitor * 0.9).max(MIN_WINDOW);
                if fitted != inner.size() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(fitted));
                }
            }
        }
        self.window_scale = scale;
    }

    /// Row Shading applies to every striped list in the app: egui's striped
    /// grids take their colour from `faint_bg_color` (and nothing else does).
    fn apply_prefs(&self, ctx: &egui::Context) {
        let stripe = self.prefs.stripe();
        if ctx.style().visuals.faint_bg_color != stripe {
            ctx.style_mut(|s| s.visuals.faint_bg_color = stripe);
        }
    }

    /// Save changed preferences once the mouse is released, so dragging the
    /// shading picker writes the file once rather than every frame.
    fn save_prefs(&mut self, ctx: &egui::Context) {
        if self.prefs == self.saved_prefs || ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        if let Err(e) = self.prefs.save() {
            self.error(format!("Could not save preferences: {e}"));
        }
        self.saved_prefs = self.prefs;
    }

    /// Help > Tutorial > Show at Next Launch: undo "Never show again".
    fn tutorial_at_next_launch(&mut self) {
        self.prefs.show_tutorial = true;
        self.info("The tutorial will open the next time LedgerCraft starts.");
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

    /// True when the open company has unsaved changes.
    fn is_dirty(&self) -> bool {
        self.session.as_ref().is_some_and(Session::is_dirty)
    }

    fn request(&mut self, what: Pending, ctx: &egui::Context) {
        if self.is_dirty() {
            self.pending = Some(what);
        } else {
            self.carry_out(what, ctx);
        }
    }

    fn carry_out(&mut self, what: Pending, ctx: &egui::Context) {
        match what {
            // Opens Create a Company; the open company stays until one is created.
            Pending::New => self.create = Some(NewCompanyForm::default()),
            Pending::Close => self.close_company(),
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

    /// Back to the start page with no company open.
    fn close_company(&mut self) {
        if self.session.take().is_some() {
            self.info("Company closed.");
        }
        self.forms = Forms::default();
        self.lists = ListState::default();
        self.page = Page::Home;
    }

    fn replace_session(&mut self, s: Session) {
        self.session = Some(s);
        self.create = None;
        self.forms = Forms::default();
        self.lists = ListState::default();
        self.page = Page::Home;
    }

    /// Saves; returns true when the books are safely on disk.
    fn save(&mut self) -> bool {
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        if session.path().is_none() {
            return self.save_as();
        }
        let result = session.save();
        match result {
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
        let Some(session) = self.session.as_ref() else {
            return false;
        };
        let name = format!("{}.{}", session.company().info.name, ledgercraft_file::EXTENSION);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LedgerCraft company", &[ledgercraft_file::EXTENSION])
            .set_file_name(name)
            .save_file()
        else {
            return false;
        };
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        let result = session.save_as(&path);
        match result {
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
        // Excel on Windows reads a CSV without a byte-order mark as the local
        // ANSI code page, which garbles non-ASCII characters. The UTF-8 BOM
        // tells it the file is UTF-8.
        match std::fs::write(&path, format!("\u{feff}{csv}")) {
            Ok(()) => self.info(format!("Exported {}", path.display())),
            Err(e) => self.error(format!("Could not write {}: {e}", path.display())),
        }
    }

    fn export_xlsx(&mut self, name: &str, bytes: &[u8]) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Excel workbook", &["xlsx"])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => self.info(format!("Exported {}", path.display())),
            Err(e) => self.error(format!("Could not write {}: {e}", path.display())),
        }
    }

    fn undo(&mut self) {
        match self.session.as_mut().and_then(Session::undo) {
            Some(label) => self.info(format!("Undid {label}")),
            None => self.info("Nothing to undo"),
        }
    }

    fn redo(&mut self) {
        match self.session.as_mut().and_then(Session::redo) {
            Some(label) => self.info(format!("Redid {label}")),
            None => self.info("Nothing to redo"),
        }
    }

    // ----- actions ------------------------------------------------------------

    fn apply(&mut self, action: Action) {
        match action {
            Action::Run { cmd, reset } => {
                let label = cmd.undo_label();
                let Some(session) = self.session.as_mut() else {
                    self.error("Open or create a company first.");
                    return;
                };
                let result = session.execute(cmd);
                match result {
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
                let terms = self
                    .session
                    .as_ref()
                    .and_then(|s| s.company().customer(id))
                    .map(|c| c.contact.terms_days);
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
            Action::ClearStatus => self.info("Ready"),
            Action::ExportCsv { name, csv } => self.export_csv(&name, &csv),
            Action::ExportXlsx { name, data } => match data {
                Ok(bytes) => self.export_xlsx(&name, &bytes),
                Err(e) => self.error(e),
            }

        }
    }

    // ----- chrome ----------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let pressed = |s: &KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(s));
        let has_company = self.session.is_some();
        if has_company && pressed(&SAVE) {
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
        if !typing && has_company && pressed(&UNDO) {
            self.undo();
        }
        if !typing && has_company && pressed(&REDO) {
            self.redo();
        }
        if has_company && pressed(&INVOICE) {
            self.page = Page::Form(FormKind::Invoice);
        }
        if has_company && pressed(&CHECK) {
            self.page = Page::Form(FormKind::Check);
        }
        if has_company && pressed(&SIDE_BAR) {
            self.show_nav = !self.show_nav;
        }
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let mut tops = Vec::new();
                let has_company = self.session.is_some();
                if has_company && !self.show_nav && ui.button(">").on_hover_text("Show side bar (Ctrl+B)").clicked() {
                    self.show_nav = true;
                }
                tops.push(ui.menu_button("File", |ui| {
                    if ui.button("New Company…").clicked() {
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
                    if ui.add_enabled(has_company, egui::Button::new("Save")).clicked() {
                        ui.close_menu();
                        self.save();
                    }
                    if ui.add_enabled(has_company, egui::Button::new("Save As…")).clicked() {
                        ui.close_menu();
                        self.save_as();
                    }
                    let close = ui
                        .add_enabled(has_company, egui::Button::new("Close Company"))
                        .on_hover_text("Close the books and go back to the start page.");
                    if close.clicked() {
                        ui.close_menu();
                        self.request(Pending::Close, ctx);
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ui.close_menu();
                        self.request(Pending::Quit, ctx);
                    }
                }).response);
                tops.push(ui.menu_button("Edit", |ui| {
                    let undo = self.session.as_ref().and_then(Session::undo_label).map(|l| format!("Undo {l}"));
                    if ui.add_enabled(undo.is_some(), egui::Button::new(undo.unwrap_or_else(|| "Undo".into()))).clicked() {
                        ui.close_menu();
                        self.undo();
                    }
                    let redo = self.session.as_ref().and_then(Session::redo_label).map(|l| format!("Redo {l}"));
                    if ui.add_enabled(redo.is_some(), egui::Button::new(redo.unwrap_or_else(|| "Redo".into()))).clicked() {
                        ui.close_menu();
                        self.redo();
                    }
                }).response);
                tops.push(ui.menu_button("View", |ui| {
                    let side_bar = egui::Checkbox::new(&mut self.show_nav, "Side Bar    Ctrl+B");
                    if ui.add_enabled(has_company, side_bar).clicked() {
                        ui.close_menu();
                    }
                }).response);
                // Lists, Customers, Vendors, Banking and Reports work on a
                // company, so their items are greyed out until one is open.
                // (Disabled inside each menu rather than around the bar, so
                // the menus stay part of the same bar for Open Menus On Hover.)
                tops.push(ui.menu_button("Lists", |ui| {
                    if !has_company {
                        ui.disable();
                    }
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
                }).response);
                tops.push(ui.menu_button("Customers", |ui| {
                    if !has_company {
                        ui.disable();
                    }
                    for kind in [FormKind::Invoice, FormKind::ReceivePayment, FormKind::Deposit] {
                        if ui.button(kind.title()).clicked() {
                            ui.close_menu();
                            self.page = Page::Form(kind);
                        }
                    }
                }).response);
                tops.push(ui.menu_button("Vendors", |ui| {
                    if !has_company {
                        ui.disable();
                    }
                    for kind in [FormKind::Bill, FormKind::PayBills] {
                        if ui.button(kind.title()).clicked() {
                            ui.close_menu();
                            self.page = Page::Form(kind);
                        }
                    }
                }).response);
                tops.push(ui.menu_button("Banking", |ui| {
                    if !has_company {
                        ui.disable();
                    }
                    for kind in [FormKind::Check, FormKind::Deposit, FormKind::Journal] {
                        if ui.button(kind.title()).clicked() {
                            ui.close_menu();
                            self.page = Page::Form(kind);
                        }
                    }
                }).response);
                // A real menu: each report opens directly, rather than
                // duplicating the side bar's "Reports" link.
                tops.push(ui.menu_button("Reports", |ui| {
                    if !has_company {
                        ui.disable();
                    }
                    for kind in ledgercraft_reports::ReportKind::ALL {
                        if ui.button(kind.label()).clicked() {
                            ui.close_menu();
                            self.reports.kind = kind;
                            self.page = Page::Reports;
                        }
                    }
                }).response);
                tops.push(ui.menu_button("Help", |ui| {
                    if ui.button("Preferences…").clicked() {
                        ui.close_menu();
                        self.show_prefs = true;
                    }
                    ui.menu_button("Tutorial", |ui| {
                        if ui.button("Show Tutorial").clicked() {
                            ui.close_menu();
                            self.show_tutorial = true;
                        }
                        if ui.button("Show at Next Launch").clicked() {
                            ui.close_menu();
                            self.tutorial_at_next_launch();
                        }
                    });
                    if ui.button("Licenses").clicked() {
                        ui.close_menu();
                        self.licenses.show_tab(LicenseTab::Summary);
                    }
                    ui.separator();
                    if ui.button("About LedgerCraft").clicked() {
                        ui.close_menu();
                        self.show_about = true;
                    }
                }).response);
                if self.prefs.hover_menus {
                    open_menus_on_hover(ui, &tops);
                }
            });
        });
    }

    fn nav(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(190.0)
            .frame(egui::Frame::none().fill(theme::NAV_BG).inner_margin(10.0))
            .show_animated(ctx, self.show_nav && self.session.is_some(), |ui| {
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
                    let text = match &self.session {
                        None => "No company open".to_string(),
                        Some(session) => {
                            let file = session
                                .path()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "Not saved yet".into());
                            let dirty = if session.is_dirty() { "  • unsaved changes" } else { "" };
                            format!("{file}{dirty}")
                        }
                    };
                    ui.label(RichText::new(text).color(theme::MUTED));
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
        let created = match self.create.as_mut() {
            Some(form) => start::new_company_window(ctx, form),
            None => None,
        };
        match created {
            Some(start::CreateOutcome::Create(company)) => {
                let name = company.info.name.clone();
                self.replace_session(Session::new(company));
                self.info(format!("Created {name}."));
                // Ask where it lives straight away, as desktop apps do.
                if !self.save_as() && !self.status_is_error {
                    self.info(format!(
                        "Created {name}. It is not saved yet: use File > Save As to choose where it lives."
                    ));
                }
            }
            Some(start::CreateOutcome::Cancel) => self.create = None,
            None => {}
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
        prefs::window(ctx, &mut self.show_prefs, &mut self.prefs);
        let target = tutorial::window(ctx, &mut self.show_tutorial, &mut self.prefs, &mut self.tutorial);
        match target {
            Some(tutorial::Target::Page(page)) if self.session.is_some() => self.page = page,
            Some(tutorial::Target::Page(_)) => {
                self.info("Open or create a company first, or explore the sample company.");
            }
            Some(tutorial::Target::Sample) => self.request(Pending::Sample, ctx),
            None => {}
        }
    }
}

/// Help > Preferences > Open Menus On Hover. egui opens a top-level menu on a
/// click, or on hover only once another menu is open; this opens the hovered
/// one straight away (where a click would) and closes the open menu when the
/// pointer leaves both the menu bar and the menu.
fn open_menus_on_hover(ui: &egui::Ui, tops: &[egui::Response]) {
    use egui::menu::{BarState, MenuResponse, MenuRoot};
    let Some(pointer) = ui.ctx().pointer_hover_pos() else {
        return;
    };
    let bar_id = ui.id();
    let mut bar = BarState::load(ui.ctx(), bar_id);
    let response = match bar.as_ref() {
        None => match tops.iter().find(|b| b.hovered()) {
            Some(button) => {
                let margin = egui::Frame::menu(ui.style()).total_margin().left;
                let gap = ui.style().spacing.menu_spacing;
                MenuResponse::Create(
                    button.rect.left_bottom() + egui::vec2(-margin, gap),
                    button.id,
                )
            }
            None => return,
        },
        Some(open) => {
            // A little slack so crossing the gap under the bar does not close it.
            let over_bar = ui.min_rect().expand(6.0).contains(pointer);
            if over_bar || open.menu_state.read().area_contains(pointer) {
                return;
            }
            MenuResponse::Close
        }
    };
    MenuRoot::handle_menu_response(&mut bar, response);
    bar.store(ui.ctx(), bar_id);
    ui.ctx().request_repaint();
}

impl eframe::App for LedgerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Never lose work on close.
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
        let title = match &self.session {
            Some(session) => format!("LedgerCraft — {}", session.title()),
            None => "LedgerCraft".to_string(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        self.fit_window(ctx);
        self.apply_prefs(ctx);
        self.save_prefs(ctx);

        self.shortcuts(ctx);
        self.menu_bar(ctx);
        self.status_bar(ctx);
        self.nav(ctx);

        let mut actions: Vec<Action> = Vec::new();
        let page = self.page;
        let mut start_choice = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(session) = self.session.as_ref() else {
                start_choice = self.start.ui(ui);
                return;
            };
            let company = session.company();
            match page {
                // Pages with their own scroll areas.
                Page::Accounts => pages::accounts(ui, company, &mut self.lists, &mut actions),
                Page::Register(id) => pages::register(ui, company, id, &mut actions),
                Page::Customers => pages::customers(ui, company, &mut self.lists, &mut actions),
                Page::Vendors => pages::vendors(ui, company, &mut self.lists, &mut actions),
                Page::Transactions => pages::transactions(ui, company, &mut actions),
                Page::Reports => pages::reports(ui, company, &mut self.reports, &mut actions),
                _ => {
                    let stripe = self.prefs.stripe();
                    let salt = ("page", format!("{page:?}"));
                    egui::ScrollArea::vertical().id_salt(salt).show(ui, |ui| match page {
                        Page::Home => pages::home(ui, company, stripe, &mut actions),
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
        match start_choice {
            Some(StartChoice::Create) => self.request(Pending::New, ctx),
            Some(StartChoice::Open) => self.request(Pending::Open, ctx),
            Some(StartChoice::Sample) => self.request(Pending::Sample, ctx),
            Some(StartChoice::Tutorial) => self.show_tutorial = true,
            None => {}
        }
        self.dialogs(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_engine::{new_company, Command};

    impl LedgerApp {
        fn title_for_test(&self) -> String {
            match &self.session {
                Some(session) => format!("LedgerCraft — {}", session.title()),
                None => "LedgerCraft".to_string(),
            }
        }
    }

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

    #[test]
    fn close_company_returns_to_the_start_page() {
        let ctx = egui::Context::default();
        let mut app = LedgerApp::with_session(Session::new(new_company("Close Co")));
        app.page = Page::Customers;
        app.request(Pending::Close, &ctx);
        assert!(app.session.is_none());
        assert_eq!(app.page, Page::Home);
        assert_eq!(app.title_for_test(), "LedgerCraft");
    }

    #[test]
    fn closing_with_unsaved_changes_asks_first() {
        let ctx = egui::Context::default();
        let mut app = LedgerApp::with_session(Session::new(new_company("Dirty Co")));
        let renamed = app
            .session
            .as_mut()
            .map(|s| s.execute(Command::CompanyRename { name: "Dirty Co 2".into() }).is_ok());
        assert_eq!(renamed, Some(true));
        app.request(Pending::Close, &ctx);
        assert!(app.session.is_some(), "closed without asking");
        assert!(app.pending == Some(Pending::Close));
    }

    #[test]
    fn new_company_opens_the_create_window_and_keeps_the_open_books() {
        let ctx = egui::Context::default();
        let mut app = LedgerApp::with_session(Session::new(new_company("Keep Co")));
        app.request(Pending::New, &ctx);
        assert!(app.create.is_some());
        assert!(app.session.is_some());
    }

    #[test]
    fn with_no_company_the_books_cannot_change() {
        let mut app = LedgerApp::blank(None);
        app.apply(Action::Run {
            cmd: Command::CompanyRename { name: "Nobody".into() },
            reset: Reset::Nothing,
        });
        assert!(app.status_is_error);
        app.undo();
        assert!(!app.save());
    }

    #[test]
    fn show_at_next_launch_turns_the_tutorial_back_on() {
        let mut app = LedgerApp::with_session(Session::new(new_company("Tutorial Co")));
        app.prefs.show_tutorial = false;
        app.tutorial_at_next_launch();
        assert!(app.prefs.show_tutorial);
        // Only flags the next launch; it does not open the window now.
        assert!(!app.show_tutorial);
    }
}
