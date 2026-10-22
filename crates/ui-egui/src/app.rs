use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use ledgercraft_engine::{sample_company, Command, EngineError, Outcome, Session};
use ledgercraft_file::FileError;
use ledgercraft_ledger::{CustomerId, TxnId};

use crate::bankimport::{self, ImportState};
use crate::company_info::{self, CompanyInfoForm, CompanyInfoOutcome};
use crate::forms::Forms;
use crate::licenses::{LicenseTab, LicenseWindow};
use crate::pages::{self, ListState, ReportState};
use crate::prefs::{self, Prefs};
use crate::recent::{AppState, WindowState};
use crate::reconcile::{self, ReconcileState};
use crate::salestax::{self, SalesTaxState};
use crate::security::{
    self, PasswordForm, PasswordOutcome, UnlockForm, UnlockOutcome, UpgradeChoice,
};
use crate::start::{self, NewCompanyForm, StartChoice, StartPage};
use crate::statement::{self, StatementForm, StatementOutcome};
use crate::{forms, theme, tutorial, Action, FormKind, Page, Reset};

/// Something waiting for the user to decide what to do with unsaved changes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    New,
    Open,
    /// Open `LedgerApp::open_target` (a recent company).
    OpenPath,
    Sample,
    Close,
    Quit,
}

pub struct LedgerApp {
    /// The open company; `None` shows the start page.
    session: Option<Session>,
    /// The Create a Company window, while it is open.
    create: Option<NewCompanyForm>,
    /// Asking for a protected file's password, while open.
    unlock: Option<UnlockForm>,
    /// File > Password…, while open.
    password: Option<PasswordForm>,
    /// File > Company Information…, while open.
    company_info: Option<CompanyInfoForm>,
    /// Customers > Create Statement…, while open.
    statement: Option<StatementForm>,
    /// Lists > Sales Tax, and its import window.
    sales_tax: SalesTaxState,
    /// The quarterly "check your sales tax rates" reminder was put off until
    /// the company is next opened.
    tax_reminder_later: bool,
    /// Offer to protect the unprotected file just opened.
    offer_protection: bool,
    /// What the last import brought in, shown until closed.
    import_report: Option<ledgercraft_file::import::ImportSummary>,
    start: StartPage,
    page: Page,
    forms: Forms,
    lists: ListState,
    reports: ReportState,
    reconcile: ReconcileState,
    bank_import: ImportState,
    status: String,
    status_is_error: bool,
    /// When the status message appeared (egui time, seconds); `None` until
    /// the next frame notes it.
    status_since: Option<f64>,
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
    /// Recent companies, window size and side bar, kept between launches.
    state: AppState,
    /// What is on disk, so the state is written only when it changes.
    saved_state: AppState,
    /// Where the state is kept; `None` keeps it in memory only (tests).
    state_path: Option<PathBuf>,
    /// The recent company waiting on `Pending::OpenPath`.
    open_target: Option<PathBuf>,
    /// Where to go back to when a transaction edit is saved or cancelled.
    edit_return: Option<Page>,
    show_nav: bool,
    title: String,
    /// The font and size last applied, so fonts are rebuilt only on a change.
    applied_text: Option<(theme::Font, theme::TextSize)>,
    /// Scale the window size limits were last set for (0 = not yet).
    window_scale: f32,
}

/// Smallest window the layout still works in, in ui points.
const MIN_WINDOW: egui::Vec2 = egui::vec2(900.0, 560.0);

/// How long a status message stays before the status bar goes back to
/// "Ready": long enough to read, short enough not to linger. Errors stay
/// longer.
const STATUS_SECONDS: f64 = 8.0;
const ERROR_SECONDS: f64 = 20.0;

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const INVOICE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
const CHECK: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);
const SIDE_BAR: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);

impl LedgerApp {
    /// `None` starts with no company open, on the start page. `unlock` is a
    /// password-protected file to ask the password for straight away.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        session: Option<Session>,
        unlock: Option<PathBuf>,
    ) -> Self {
        theme::apply(&cc.egui_ctx);
        let prefs = Prefs::load();
        let state_path = AppState::path();
        let state = state_path
            .as_deref()
            .map(AppState::load_from)
            .unwrap_or_default();
        let mut app = Self {
            show_tutorial: prefs.show_tutorial,
            prefs,
            saved_prefs: prefs,
            unlock: unlock.map(UnlockForm::new),
            show_nav: state.side_bar,
            saved_state: state.clone(),
            state,
            state_path,
            ..Self::blank(session)
        };
        let opened = app
            .session
            .as_ref()
            .and_then(|s| s.path())
            .map(Path::to_path_buf);
        if let Some(path) = opened {
            // Opened from the command line or by double-clicking the file.
            app.remember(&path);
        } else if app.session.is_none() && app.unlock.is_none() && app.prefs.reopen_last {
            // Nothing asked for: carry on with the company that was open last.
            if let Some(last) = app.state.recent.first().filter(|p| p.exists()).cloned() {
                app.open_path(last);
            }
        }
        app
    }

    #[cfg(test)]
    fn with_session(session: Session) -> Self {
        Self::blank(Some(session))
    }

    fn blank(session: Option<Session>) -> Self {
        Self {
            session,
            create: None,
            import_report: None,
            unlock: None,
            password: None,
            company_info: None,
            statement: None,
            sales_tax: SalesTaxState::default(),
            tax_reminder_later: false,
            offer_protection: false,
            start: StartPage::default(),
            page: Page::Home,
            forms: Forms::default(),
            lists: ListState::default(),
            reports: ReportState::default(),
            reconcile: ReconcileState::default(),
            bank_import: ImportState::default(),
            status: "Ready".into(),
            status_is_error: false,
            status_since: None,
            pending: None,
            allow_close: false,
            show_about: false,
            show_prefs: false,
            show_tutorial: false,
            tutorial: tutorial::Tutorial::default(),
            prefs: Prefs::default(),
            saved_prefs: Prefs::default(),
            licenses: LicenseWindow::default(),
            state: AppState::default(),
            saved_state: AppState::default(),
            state_path: None,
            open_target: None,
            edit_return: None,
            show_nav: true,
            title: String::new(),
            window_scale: 0.0,
            applied_text: None,
        }
    }

    /// Set the minimum window size, and on the first frame fit the window to
    /// its monitor. egui-winit (since 0.29) converts `ViewportBuilder` sizes with the
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
            // The size the window had when the app last closed, if known.
            let saved = self.state.window;
            let (inner, monitor) =
                ctx.input(|i| (i.viewport().inner_rect, i.viewport().monitor_size));
            if let (Some(inner), Some(monitor)) = (inner, monitor) {
                let wanted = saved.and_then(|w| w.size()).unwrap_or(inner.size());
                let fitted = wanted.min(monitor * 0.9).max(MIN_WINDOW);
                if fitted != inner.size() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(fitted));
                }
            }
            if saved.is_some_and(|w| w.maximized) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
        }
        self.window_scale = scale;
    }

    /// Row Shading applies to every striped list in the app: egui's striped
    /// grids take their colour from `faint_bg_color` (and nothing else does).
    fn apply_prefs(&mut self, ctx: &egui::Context) {
        crate::currency::set(self.prefs.currency);
        let text = (self.prefs.font, self.prefs.text_size);
        if self.applied_text != Some(text) {
            theme::set_text(ctx, text.0, text.1);
            self.applied_text = Some(text);
        }
        let ink = theme::text_colour(self.prefs.text_grey);
        if ctx.global_style().visuals.override_text_color != Some(ink) {
            ctx.global_style_mut(|s| s.visuals.override_text_color = Some(ink));
        }
        let hinting = self.prefs.hinting;
        if ctx.global_style().visuals.text_options.font_hinting != hinting {
            ctx.global_style_mut(|s| s.visuals.text_options.font_hinting = hinting);
        }
        let stripe = self.prefs.stripe();
        if ctx.global_style().visuals.faint_bg_color != stripe {
            ctx.global_style_mut(|s| s.visuals.faint_bg_color = stripe);
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

    /// Puts a company file at the top of the recent list.
    fn remember(&mut self, path: &Path) {
        self.state.remember(path);
    }

    /// Notes the window's size, so the next launch opens at the same size.
    fn note_window(&mut self, ctx: &egui::Context) {
        let (inner, maximized, minimized) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect, v.maximized, v.minimized)
        });
        if minimized == Some(true) {
            return;
        }
        let maximized = maximized == Some(true);
        let previous = self.state.window;
        let window = match (maximized, previous, inner) {
            // Maximized: keep the size to restore to, just note the maximizing.
            (true, Some(before), _) => WindowState {
                maximized: true,
                ..before
            },
            (_, _, Some(rect)) => WindowState {
                width: rect.width(),
                height: rect.height(),
                maximized,
            },
            _ => return,
        };
        if window.size().is_some() || window.maximized {
            self.state.window = Some(window);
        }
    }

    /// Writes the recent list, window size and side bar when they change.
    fn save_state(&mut self) {
        self.state.side_bar = self.show_nav;
        if self.state == self.saved_state {
            return;
        }
        let written = self.state_path.as_deref().map(|p| self.state.save_to(p));
        if let Some(Err(e)) = written {
            self.error(format!("Could not save the recent companies list: {e}"));
        }
        self.saved_state = self.state.clone();
    }

    /// Opens a company from the recent list, or says it is gone.
    fn open_recent(&mut self, path: PathBuf) {
        if path.exists() {
            self.open_path(path);
        } else {
            self.error(format!(
                "{} was not found. It may have been moved, renamed or deleted; it is \
                 removed from the recent list.",
                path.display()
            ));
            self.state.forget(&path);
        }
    }

    /// Help > Tutorial > Show at Next Launch: undo "Never show again".
    fn tutorial_at_next_launch(&mut self) {
        self.prefs.show_tutorial = true;
        self.info("The tutorial will open the next time LedgerCraft starts.");
    }

    fn info(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_is_error = false;
        self.status_since = None;
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_is_error = true;
        self.status_since = None;
    }

    /// Clears the status message once it has been shown long enough.
    /// Returns the seconds it has left, so the app can wake up to clear it.
    fn expire_status(&mut self, now: f64) -> Option<f64> {
        let since = *self.status_since.get_or_insert(now);
        if self.status == "Ready" {
            return None;
        }
        let lasts = if self.status_is_error {
            ERROR_SECONDS
        } else {
            STATUS_SECONDS
        };
        let left = since + lasts - now;
        if left > 0.0 {
            Some(left)
        } else {
            self.info("Ready");
            None
        }
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
            Pending::OpenPath => {
                if let Some(path) = self.open_target.take() {
                    self.open_recent(path);
                }
            }
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
                let protected = s.is_protected();
                self.replace_session(s);
                self.remember(&path);
                self.info(format!("Opened {}", file_name(&path)));
                // A file from before encryption (or with its password removed):
                // offer to upgrade it.
                self.offer_protection = !protected;
            }
            Err(EngineError::File(FileError::PasswordRequired)) => {
                self.unlock = Some(UnlockForm::new(path))
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
        self.reconcile = ReconcileState::default();
        self.bank_import = ImportState::default();
        self.company_info = None;
        self.statement = None;
        self.sales_tax = SalesTaxState::default();
        self.tax_reminder_later = false;
        self.page = Page::Home;
    }

    fn replace_session(&mut self, s: Session) {
        self.session = Some(s);
        self.create = None;
        self.offer_protection = false;
        self.password = None;
        self.forms = Forms::default();
        self.lists = ListState::default();
        self.reconcile = ReconcileState::default();
        self.bank_import = ImportState::default();
        self.company_info = None;
        self.statement = None;
        self.sales_tax = SalesTaxState::default();
        self.tax_reminder_later = false;
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
        let name = format!(
            "{}.{}",
            session.company().info.name,
            ledgercraft_file::EXTENSION
        );
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
                self.remember(&path);
                self.info(format!("Saved {}", file_name(&path)));
                true
            }
            Err(e) => {
                self.error(e.to_string());
                false
            }
        }
    }

    fn export_csv(&mut self, name: &str, csv: &str) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(name)
            .save_file()
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
                self.run(cmd, reset);
            }
            Action::Go(page) => self.page = page,
            Action::Reconcile(account) => {
                self.reconcile.start(account);
                self.page = Page::Reconcile;
            }
            Action::ReconcileStatement {
                account,
                date,
                balance,
            } => {
                self.reconcile.start_statement(account, date, balance);
                self.page = Page::Reconcile;
            }
            Action::InvoiceFor(id) => {
                self.forms.invoice.customer = Some(id);
                self.forms.invoice.tax_group = self
                    .session
                    .as_ref()
                    .and_then(|s| forms::default_tax_group(s.company(), Some(id)));
                let terms = self
                    .session
                    .as_ref()
                    .and_then(|s| s.company().customer(id))
                    .map(|c| c.contact.terms_days);
                if let (Some(t), Ok(d)) = (
                    terms,
                    ledgercraft_core::Date::parse(&self.forms.invoice.date),
                ) {
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
            Action::Open(id) => self.open_txn(id),
            Action::EditCancelled => {
                self.info("Edit cancelled; nothing was changed.");
                if let Some(page) = self.edit_return.take() {
                    self.page = page;
                }
            }
            Action::ExportCsv { name, csv } => self.export_csv(&name, &csv),
            Action::ExportXlsx { name, data } => match data {
                Ok(bytes) => self.export_xlsx(&name, &bytes),
                Err(e) => self.error(e),
            },
            Action::InvoicePdf(id) => self.save_invoice_pdf(id),
            Action::Statement(customer) => self.statement = Some(StatementForm::new(customer)),
            Action::FindTaxGroup(id) => self.find_tax_group(id),
            Action::SstFilesUsed {
                state,
                rates,
                boundary,
            } => {
                self.state.sst_files.insert(state, (rates, boundary));
                self.save_state();
            }
            Action::SaveInvoicePdf(kind, cmd) => {
                let editing = match &cmd {
                    Command::TxnUpdate { id, .. } => Some(*id),
                    _ => None,
                };
                let reset = Reset::Form(kind);
                let id = match editing {
                    // Nothing changed: no edit to record (or undo), just the PDF.
                    Some(id) if self.changes_nothing(&cmd) => {
                        self.forms.reset(kind);
                        if let Some(page) = self.edit_return.take() {
                            self.page = page;
                        }
                        Some(id)
                    }
                    _ => self.run(cmd, reset).and_then(|outcome| {
                        editing.or_else(|| outcome.created.as_deref().and_then(parse_txn_id))
                    }),
                };
                if let Some(id) = id {
                    self.save_invoice_pdf(id);
                }
            }
        }
    }

    /// Runs a command, reporting what happened; the outcome when it worked.
    fn run(&mut self, cmd: Command, reset: Reset) -> Option<Outcome> {
        let label = cmd.undo_label();
        let was_edit = matches!(cmd, Command::TxnUpdate { .. });
        let Some(session) = self.session.as_mut() else {
            self.error("Open or create a company first.");
            return None;
        };
        match session.execute(cmd) {
            Ok(outcome) => {
                if let Reset::Form(kind) = reset {
                    self.forms.reset(kind);
                }
                self.lists.reset(reset);
                self.sales_tax.reset(reset);
                if was_edit {
                    self.info(format!(
                        "{label}: changes saved. Edit > Undo puts the original back."
                    ));
                    if let Some(page) = self.edit_return.take() {
                        self.page = page;
                    }
                } else {
                    match &outcome.created {
                        Some(id) => self.info(format!("{label} recorded ({id})")),
                        None => self.info(format!("{label}: done")),
                    }
                }
                Some(outcome)
            }
            Err(e) => {
                self.error(e.to_string());
                None
            }
        }
    }

    /// True when an edit would leave the recorded transaction exactly as it is.
    fn changes_nothing(&self, cmd: &Command) -> bool {
        let Command::TxnUpdate {
            id,
            date,
            number,
            memo,
            body,
        } = cmd
        else {
            return false;
        };
        self.session
            .as_ref()
            .and_then(|s| s.company().txn(*id))
            .is_some_and(|t| {
                t.date == *date
                    && t.number == number.trim()
                    && t.memo.trim() == memo.trim()
                    && t.body == *body
            })
    }

    /// Opens the rate import for a customer: their ZIP from their address,
    /// and the files last used for their state (or the only state used).
    fn find_tax_group(&mut self, id: CustomerId) {
        let Some(customer) = self.session.as_ref().and_then(|s| s.company().customer(id)) else {
            return;
        };
        let address = &customer.contact.address;
        let state = ledgercraft_file::sst::state_fips(&address.region)
            .map(ledgercraft_file::sst::state_abbr);
        let files = match state {
            Some(abbr) => self.state.sst_files.get(abbr).cloned(),
            None if self.state.sst_files.len() == 1 => {
                self.state.sst_files.values().next().cloned()
            }
            None => None,
        };
        let name = customer.contact.name.clone();
        let zip = address.postal_code.clone();
        self.sales_tax.import = Some(salestax::ImportWindow::for_customer(id, &name, &zip, files));
    }

    /// Asks where to save a recorded invoice as a PDF, and writes it.
    fn save_invoice_pdf(&mut self, id: TxnId) {
        let Some(company) = self.session.as_ref().map(Session::company) else {
            return;
        };
        let name = ledgercraft_reports::invoice_pdf_name(company, id);
        let bytes = match ledgercraft_reports::invoice_pdf(
            company,
            id,
            crate::currency::current().symbol(),
        ) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.error(e);
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => self.info(format!("Saved {}", file_name(&path))),
            Err(e) => self.error(format!("Could not write {}: {e}", path.display())),
        }
    }

    /// Asks where to save a customer's statement as a PDF, and writes it.
    /// Returns whether it was saved.
    fn save_statement_pdf(
        &mut self,
        customer: CustomerId,
        from: ledgercraft_core::Date,
        to: ledgercraft_core::Date,
    ) -> bool {
        let Some(company) = self.session.as_ref().map(Session::company) else {
            return false;
        };
        let name = ledgercraft_reports::statement_pdf_name(company, customer, to);
        let bytes = match ledgercraft_reports::statement_pdf(
            company,
            customer,
            from,
            to,
            crate::currency::current().symbol(),
        ) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.error(e);
                return false;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .set_file_name(name)
            .save_file()
        else {
            return false;
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.info(format!("Saved {}", file_name(&path)));
                true
            }
            Err(e) => {
                self.error(format!("Could not write {}: {e}", path.display()));
                false
            }
        }
    }

    /// Opens a recorded transaction in its form, ready to change.
    fn open_txn(&mut self, id: TxnId) {
        let Some(t) = self
            .session
            .as_ref()
            .and_then(|s| s.company().txn(id))
            .cloned()
        else {
            self.error(format!("Transaction {id} was not found."));
            return;
        };
        match self.forms.load(&t) {
            Ok(kind) => {
                // Back to the list, register or report it was opened from.
                if !matches!(self.page, Page::Form(_)) {
                    self.edit_return = Some(self.page);
                }
                self.page = Page::Form(kind);
                let number = if t.number.is_empty() {
                    String::new()
                } else {
                    format!(" {}", t.number)
                };
                self.info(format!("Editing {}{number}.", t.body.label()));
            }
            Err(e) => self.error(e),
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

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let mut tops = Vec::new();
                let has_company = self.session.is_some();
                if has_company
                    && !self.show_nav
                    && ui
                        .button(">")
                        .on_hover_text("Show side bar (Ctrl+B)")
                        .clicked()
                {
                    self.show_nav = true;
                }
                tops.push(
                    ui.menu_button("File", |ui| {
                        if ui.button("New Company…").clicked() {
                            ui.close();
                            self.request(Pending::New, ctx);
                        }
                        if ui.button("Open Company…").clicked() {
                            ui.close();
                            self.request(Pending::Open, ctx);
                        }
                        let recent = self.state.recent.clone();
                        if recent.is_empty() {
                            ui.add_enabled(false, egui::Button::new("Open Previous Company"));
                        } else {
                            ui.menu_button("Open Previous Company", |ui| {
                                for path in recent {
                                    let (name, folder) = crate::recent::describe(&path);
                                    if ui.button(name).on_hover_text(folder).clicked() {
                                        ui.close();
                                        self.open_target = Some(path);
                                        self.request(Pending::OpenPath, ctx);
                                    }
                                }
                            });
                        }
                        if ui.button("Open Sample Company").clicked() {
                            ui.close();
                            self.request(Pending::Sample, ctx);
                        }
                        ui.separator();
                        if ui
                            .add_enabled(has_company, egui::Button::new("Save"))
                            .clicked()
                        {
                            ui.close();
                            self.save();
                        }
                        if ui
                            .add_enabled(has_company, egui::Button::new("Save As…"))
                            .clicked()
                        {
                            ui.close();
                            self.save_as();
                        }
                        let info = ui
                            .add_enabled(has_company, egui::Button::new("Company Information…"))
                            .on_hover_text(
                                "The company's name, address, phone and email, as printed on invoices.",
                            );
                        if info.clicked() {
                            ui.close();
                            self.company_info = self
                                .session
                                .as_ref()
                                .map(|s| CompanyInfoForm::from_company(s.company()));
                        }
                        let password = ui
                            .add_enabled(has_company, egui::Button::new("Password…"))
                            .on_hover_text(
                            "Set, change or remove the password that encrypts this company file.",
                        );
                        if password.clicked() {
                            ui.close();
                            self.password = Some(PasswordForm::default());
                        }
                        let close = ui
                            .add_enabled(has_company, egui::Button::new("Close Company"))
                            .on_hover_text("Close the books and go back to the start page.");
                        if close.clicked() {
                            ui.close();
                            self.request(Pending::Close, ctx);
                        }
                        ui.separator();
                        if ui.button("Exit").clicked() {
                            ui.close();
                            self.request(Pending::Quit, ctx);
                        }
                    })
                    .response,
                );
                tops.push(
                    ui.menu_button("Edit", |ui| {
                        let undo = self
                            .session
                            .as_ref()
                            .and_then(Session::undo_label)
                            .map(|l| format!("Undo {l}"));
                        if ui
                            .add_enabled(
                                undo.is_some(),
                                egui::Button::new(undo.unwrap_or_else(|| "Undo".into())),
                            )
                            .clicked()
                        {
                            ui.close();
                            self.undo();
                        }
                        let redo = self
                            .session
                            .as_ref()
                            .and_then(Session::redo_label)
                            .map(|l| format!("Redo {l}"));
                        if ui
                            .add_enabled(
                                redo.is_some(),
                                egui::Button::new(redo.unwrap_or_else(|| "Redo".into())),
                            )
                            .clicked()
                        {
                            ui.close();
                            self.redo();
                        }
                    })
                    .response,
                );
                tops.push(
                    ui.menu_button("View", |ui| {
                        let side_bar =
                            egui::Checkbox::new(&mut self.show_nav, "Side Bar    Ctrl+B");
                        if ui.add_enabled(has_company, side_bar).clicked() {
                            ui.close();
                        }
                    })
                    .response,
                );
                // Lists, Customers, Vendors, Banking and Reports work on a
                // company, so their items are greyed out until one is open.
                // (Disabled inside each menu rather than around the bar, so
                // the menus stay part of the same bar for Open Menus On Hover.)
                tops.push(
                    ui.menu_button("Lists", |ui| {
                        if !has_company {
                            ui.disable();
                        }
                        for (label, page) in [
                            ("Chart of Accounts", Page::Accounts),
                            ("Customers", Page::Customers),
                            ("Vendors", Page::Vendors),
                            ("Products and Services", Page::Items),
                            ("Sales Tax", Page::SalesTax),
                        ] {
                            if ui.button(label).clicked() {
                                ui.close();
                                self.page = page;
                            }
                        }
                    })
                    .response,
                );
                tops.push(
                    ui.menu_button("Customers", |ui| {
                        if !has_company {
                            ui.disable();
                        }
                        for kind in [
                            FormKind::Invoice,
                            FormKind::SalesReceipt,
                            FormKind::ReceivePayment,
                            FormKind::CreditMemo,
                            FormKind::Refund,
                            FormKind::Deposit,
                        ] {
                            if ui.button(kind.title()).clicked() {
                                ui.close();
                                self.page = Page::Form(kind);
                            }
                        }
                        ui.separator();
                        if ui.button("Create Statement…").clicked() {
                            ui.close();
                            self.statement = Some(StatementForm::new(None));
                        }
                    })
                    .response,
                );
                tops.push(
                    ui.menu_button("Vendors", |ui| {
                        if !has_company {
                            ui.disable();
                        }
                        for kind in [FormKind::Bill, FormKind::PayBills] {
                            if ui.button(kind.title()).clicked() {
                                ui.close();
                                self.page = Page::Form(kind);
                            }
                        }
                    })
                    .response,
                );
                tops.push(
                    ui.menu_button("Banking", |ui| {
                        if !has_company {
                            ui.disable();
                        }
                        for kind in [FormKind::Check, FormKind::Deposit, FormKind::Journal] {
                            if ui.button(kind.title()).clicked() {
                                ui.close();
                                self.page = Page::Form(kind);
                            }
                        }
                        ui.separator();
                        if ui.button("Import Bank Transactions…").clicked() {
                            ui.close();
                            self.page = Page::BankImport;
                        }
                        if ui.button("Reconcile…").clicked() {
                            ui.close();
                            self.page = Page::Reconcile;
                        }
                    })
                    .response,
                );
                // A real menu: each report opens directly, rather than
                // duplicating the side bar's "Reports" link.
                tops.push(
                    ui.menu_button("Reports", |ui| {
                        if !has_company {
                            ui.disable();
                        }
                        for kind in ledgercraft_reports::ReportKind::ALL {
                            if ui.button(kind.label()).clicked() {
                                ui.close();
                                self.reports.kind = kind;
                                self.page = Page::Reports;
                            }
                        }
                    })
                    .response,
                );
                tops.push(
                    ui.menu_button("Help", |ui| {
                        if ui.button("Preferences…").clicked() {
                            ui.close();
                            self.show_prefs = true;
                        }
                        ui.menu_button("Tutorial", |ui| {
                            if ui.button("Show Tutorial").clicked() {
                                ui.close();
                                self.show_tutorial = true;
                            }
                            if ui.button("Show at Next Launch").clicked() {
                                ui.close();
                                self.tutorial_at_next_launch();
                            }
                        });
                        if ui.button("Licenses").clicked() {
                            ui.close();
                            self.licenses.show_tab(LicenseTab::Summary);
                        }
                        ui.separator();
                        if ui.button("About LedgerCraft").clicked() {
                            ui.close();
                            self.show_about = true;
                        }
                    })
                    .response,
                );
                if self.prefs.hover_menus {
                    open_menus_on_hover(ui, &tops);
                }
            });
        });
    }

    fn nav(&mut self, ui: &mut egui::Ui) {
        let has_company = self.session.is_some();
        let mut open = self.show_nav && has_company;
        let mut hide = false;
        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(190.0)
            .frame(egui::Frame::NONE.fill(theme::NAV_BG).inner_margin(10.0))
            .show_collapsible(ui, &mut open, |ui| {
                theme::nav_visuals(ui);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("LedgerCraft")
                            .size(20.0)
                            .strong()
                            .color(theme::NAV_TEXT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let hide_button =
                            egui::Button::new(RichText::new("<").color(theme::NAV_TEXT))
                                .frame(false);
                        if ui
                            .add(hide_button)
                            .on_hover_text("Hide side bar (Ctrl+B)")
                            .clicked()
                        {
                            hide = true;
                        }
                    });
                });
                ui.add_space(12.0);
                let items: [(&str, Page); 9] = [
                    ("Home", Page::Home),
                    ("Chart of Accounts", Page::Accounts),
                    ("Customers", Page::Customers),
                    ("Vendors", Page::Vendors),
                    ("Products & Services", Page::Items),
                    ("Transactions", Page::Transactions),
                    ("Reports", Page::Reports),
                    ("Reconcile", Page::Reconcile),
                    ("Journal Entry", Page::Form(FormKind::Journal)),
                ];
                for (label, page) in items {
                    let selected = self.page == page;
                    let text = RichText::new(label).size(15.0).color(theme::NAV_TEXT);
                    if ui
                        .selectable_label(selected, text)
                        .on_hover_text(page.hint())
                        .clicked()
                    {
                        self.page = page;
                    }
                }
                ui.add_space(16.0);
                ui.label(RichText::new("Create").color(theme::NAV_TEXT).small());
                for kind in [
                    FormKind::Invoice,
                    FormKind::SalesReceipt,
                    FormKind::ReceivePayment,
                    FormKind::CreditMemo,
                    FormKind::Bill,
                    FormKind::PayBills,
                    FormKind::Check,
                    FormKind::Deposit,
                ] {
                    let selected = self.page == Page::Form(kind);
                    let text = RichText::new(kind.title()).color(theme::NAV_TEXT);
                    if ui
                        .selectable_label(selected, text)
                        .on_hover_text(kind.hint())
                        .clicked()
                    {
                        self.page = Page::Form(kind);
                    }
                }
            });
        // The panel can be dragged open or shut; keep the setting in step.
        if has_company {
            self.show_nav = open && !hide;
        }
    }

    /// The status message on the left, the company file on the right. The
    /// file side gets at most half the bar; both shorten with "…" rather than
    /// overlap when the window is narrow, and hovering shows them in full.
    fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| self.status_row(ui));
        });
    }

    fn status_row(&self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (text, full_path) = match &self.session {
                None => ("No company open".to_string(), None),
                Some(session) => {
                    let file = session
                        .path()
                        .map(file_name)
                        .unwrap_or_else(|| "Not saved yet".into());
                    let dirty = if session.is_dirty() {
                        "  • unsaved changes"
                    } else {
                        ""
                    };
                    let lock = if session.is_protected() {
                        "  • password protected"
                    } else {
                        "  • not password protected"
                    };
                    let full = session.path().map(|p| p.display().to_string());
                    (format!("{file}{lock}{dirty}"), full)
                }
            };
            let height = ui.available_height();
            let right_width = (ui.available_width() * 0.5).max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, height),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let label = ui.add(
                        egui::Label::new(RichText::new(text).color(theme::MUTED))
                            .wrap_mode(egui::TextWrapMode::Truncate),
                    );
                    if let Some(full) = full_path {
                        label.on_hover_text(full);
                    }
                },
            );
            ui.add_space(12.0);
            // The message takes whatever is left, from the left edge.
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let color = if self.status_is_error {
                    theme::NEGATIVE
                } else {
                    theme::MUTED
                };
                ui.add(
                    egui::Label::new(RichText::new(self.status.as_str()).color(color))
                        .wrap_mode(egui::TextWrapMode::Truncate),
                )
                .on_hover_text(self.status.as_str());
            });
        });
    }

    /// The unlock prompt and File > Password….
    fn security_windows(&mut self, ctx: &egui::Context) {
        let unlocked = match self.unlock.as_mut() {
            Some(form) => security::unlock_window(ctx, form),
            None => None,
        };
        match unlocked {
            Some(UnlockOutcome::Opened(session)) => {
                let path = self.unlock.take().map(|f| f.path.clone());
                self.replace_session(*session);
                if let Some(path) = path {
                    self.remember(&path);
                    self.info(format!("Opened {}", file_name(&path)));
                }
            }
            Some(UnlockOutcome::Failed(e)) => {
                self.unlock = None;
                self.error(format!("Could not open the company: {e}"));
            }
            Some(UnlockOutcome::Cancel) => self.unlock = None,
            None => {}
        }

        let file_name = self
            .session
            .as_ref()
            .filter(|s| self.offer_protection && !s.is_protected())
            .and_then(|s| s.path())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        if let Some(name) = file_name {
            match security::upgrade_window(ctx, &name) {
                Some(UpgradeChoice::SetPassword) => {
                    self.offer_protection = false;
                    self.password = Some(PasswordForm::default());
                }
                Some(UpgradeChoice::NotNow) => {
                    self.offer_protection = false;
                    self.info("This company file is not password protected. Use File > Password… to protect it.");
                }
                None => {}
            }
        }

        let info = self
            .company_info
            .as_mut()
            .and_then(|form| company_info::window(ctx, form));
        match info {
            Some(CompanyInfoOutcome::Save(cmd)) => {
                if self.run(*cmd, Reset::Nothing).is_some() {
                    self.company_info = None;
                }
            }
            Some(CompanyInfoOutcome::Cancel) => self.company_info = None,
            None => {}
        }

        let chosen = match (self.statement.as_mut(), self.session.as_ref()) {
            (Some(form), Some(session)) => statement::window(ctx, form, session.company()),
            _ => None,
        };
        match chosen {
            Some(StatementOutcome::Save { customer, from, to }) => {
                if self.save_statement_pdf(customer, from, to) {
                    self.statement = None;
                }
            }
            Some(StatementOutcome::Cancel) => self.statement = None,
            None => {}
        }

        // Lists > Sales Tax > Import Rates from State Files…
        let mut tax_actions = Vec::new();
        let keep = match (self.sales_tax.import.as_mut(), self.session.as_ref()) {
            (Some(w), Some(session)) => {
                salestax::import_window(ctx, w, session.company(), &mut tax_actions)
            }
            _ => false,
        };
        if !keep {
            self.sales_tax.import = None;
        }
        for action in tax_actions {
            self.apply(action);
        }

        // The quarterly reminder to check the sales tax rates.
        let today = ledgercraft_ledger::Date::today();
        let review = self
            .session
            .as_ref()
            .and_then(|s| s.company().tax_review_due(today))
            .filter(|_| !self.tax_reminder_later && self.unlock.is_none() && self.create.is_none());
        if let Some(review) = review {
            match salestax::reminder_window(ctx, &review) {
                Some(salestax::Reminder::Review) => {
                    self.page = Page::SalesTax;
                    self.tax_reminder_later = true;
                }
                Some(salestax::Reminder::UpToDate) => {
                    self.run(Command::TaxMarkChecked { on: today }, Reset::Nothing);
                }
                Some(salestax::Reminder::Later) => self.tax_reminder_later = true,
                None => {}
            }
        }

        let changed = match (self.password.as_mut(), self.session.as_ref()) {
            (Some(form), Some(session)) => security::password_window(ctx, form, session),
            (Some(_), None) => Some(PasswordOutcome::Cancel),
            (None, _) => None,
        };
        match changed {
            Some(PasswordOutcome::Apply(new)) => {
                self.password = None;
                let protect = new.is_some();
                let result = self
                    .session
                    .as_mut()
                    .map(|s| s.set_password(new.as_deref()));
                let mut new = new;
                if let Some(p) = new.as_mut() {
                    zeroize::Zeroize::zeroize(p);
                }
                match result {
                    Some(Ok(())) if protect => {
                        self.info("Password saved. The company file is encrypted.")
                    }
                    Some(Ok(())) => {
                        self.info("Password removed. The company file is no longer encrypted.")
                    }
                    Some(Err(e)) => self.error(format!("Could not change the password: {e}")),
                    None => {}
                }
            }
            Some(PasswordOutcome::Cancel) => self.password = None,
            None => {}
        }
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
            Some(start::CreateOutcome::Create(company, summary, password)) => {
                let name = company.info.name.clone();
                let mut session = Session::new(*company);
                // No location yet, so this only sets the key; the file is
                // written encrypted when it is saved just below.
                if let Err(e) = session.set_password(Some(&password.0)) {
                    self.error(format!("Could not protect the company: {e}"));
                    return;
                }
                drop(password);
                self.replace_session(session);
                match &summary {
                    Some(s) => self.info(format!("Created {name}. {s}.")),
                    None => self.info(format!("Created {name}.")),
                }
                self.import_report = summary;
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
        start::import_report_window(ctx, &mut self.import_report);
        self.security_windows(ctx);
        let mut open = self.show_about;
        let mut view_licenses = false;
        egui::Window::new("About LedgerCraft")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!("LedgerCraft {}", env!("CARGO_PKG_VERSION"))).strong(),
                );
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
        let target = tutorial::window(
            ctx,
            &mut self.show_tutorial,
            &mut self.prefs,
            &mut self.tutorial,
        );
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

/// "Pine Ridge Woodworks.lcbooks" from a full path, for the status bar.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Help > Preferences > Open Menus On Hover. egui opens a top-level menu on
/// a click, or on hover once another menu is open; this opens the hovered one
/// straight away, as a click would (opening one closes any other). It closes
/// like any menu: by choosing an item or clicking elsewhere.
fn open_menus_on_hover(ui: &egui::Ui, tops: &[egui::Response]) {
    let Some(button) = tops.iter().find(|b| b.hovered()) else {
        return;
    };
    let popup = egui::Popup::default_response_id(button);
    if !egui::Popup::is_id_open(ui.ctx(), popup) {
        egui::Popup::open_id(ui.ctx(), popup);
        ui.ctx().request_repaint();
    }
}

impl eframe::App for LedgerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        // Never lose work on close.
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && self.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        } else if close_requested {
            // Really closing: remember the window for next time.
            self.note_window(ctx);
            self.save_state();
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
        let now = ctx.input(|i| i.time);
        if let Some(left) = self.expire_status(now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
        }
        self.apply_prefs(ctx);
        self.save_prefs(ctx);

        self.shortcuts(ctx);
        // Panels first, the central page last.
        self.menu_bar(ui);
        self.status_bar(ui);
        self.nav(ui);

        let mut actions: Vec<Action> = Vec::new();
        let page = self.page;
        let mut start_choice = None;
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(session) = self.session.as_ref() else {
                start_choice = self.start.ui(ui, &self.state.recent);
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
                Page::Reconcile => reconcile::page(ui, company, &mut self.reconcile, &mut actions),
                Page::BankImport => {
                    bankimport::page(ui, company, &mut self.bank_import, &mut actions)
                }
                _ => {
                    let stripe = self.prefs.stripe();
                    let salt = ("page", format!("{page:?}"));
                    egui::ScrollArea::both()
                        .id_salt(salt)
                        .show(ui, |ui| match page {
                            Page::Home => pages::home(ui, company, stripe, &mut actions),
                            Page::Items => pages::items(ui, company, &mut self.lists, &mut actions),
                            Page::SalesTax => {
                                salestax::page(ui, company, &mut self.sales_tax, &mut actions)
                            }
                            Page::Form(FormKind::Invoice) => {
                                forms::invoice(ui, company, &mut self.forms.invoice, &mut actions)
                            }
                            Page::Form(FormKind::ReceivePayment) => forms::receive_payment(
                                ui,
                                company,
                                &mut self.forms.receive,
                                &mut actions,
                            ),
                            Page::Form(FormKind::Bill) => {
                                forms::bill(ui, company, &mut self.forms.bill, &mut actions)
                            }
                            Page::Form(FormKind::PayBills) => forms::pay_bills(
                                ui,
                                company,
                                &mut self.forms.pay_bills,
                                &mut actions,
                            ),
                            Page::Form(FormKind::Check) => {
                                forms::check(ui, company, &mut self.forms.check, &mut actions)
                            }
                            Page::Form(FormKind::Deposit) => {
                                forms::deposit(ui, company, &mut self.forms.deposit, &mut actions)
                            }
                            Page::Form(FormKind::Journal) => {
                                forms::journal(ui, company, &mut self.forms.journal, &mut actions)
                            }
                            Page::Form(FormKind::SalesReceipt) => forms::sales_receipt(
                                ui,
                                company,
                                &mut self.forms.sales_receipt,
                                &mut actions,
                            ),
                            Page::Form(FormKind::CreditMemo) => forms::credit_memo(
                                ui,
                                company,
                                &mut self.forms.credit_memo,
                                &mut actions,
                            ),
                            Page::Form(FormKind::Refund) => {
                                forms::refund(ui, company, &mut self.forms.refund, &mut actions)
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
            Some(StartChoice::Recent(path)) => {
                self.open_target = Some(path);
                self.request(Pending::OpenPath, ctx);
            }
            Some(StartChoice::Forget(path)) => self.state.forget(&path),
            None => {}
        }
        self.dialogs(ctx);
        self.save_state();
    }
}

/// The transaction id in an outcome's `created`, such as `T42`.
fn parse_txn_id(created: &str) -> Option<TxnId> {
    created.strip_prefix('T')?.parse().ok().map(TxnId)
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
    fn every_sample_invoice_makes_a_pdf() {
        let c = sample_company().unwrap();
        let invoices: Vec<_> = c
            .transactions
            .iter()
            .filter(|t| matches!(t.body, ledgercraft_ledger::TxnBody::Invoice(_)))
            .collect();
        assert_eq!(invoices.len(), 18);
        for t in invoices {
            let pdf = ledgercraft_reports::invoice_pdf(&c, t.id, "$").unwrap();
            assert!(pdf.starts_with(b"%PDF-1.4"));
        }
    }

    #[test]
    fn a_pdf_of_an_unchanged_invoice_records_no_edit() {
        let c = sample_company().unwrap();
        let t = c
            .transactions
            .iter()
            .find(|t| matches!(t.body, ledgercraft_ledger::TxnBody::Invoice(_)))
            .cloned()
            .unwrap();
        let app = LedgerApp::with_session(Session::new(c));
        let mut cmd = Command::TxnUpdate {
            id: t.id,
            date: t.date,
            number: t.number.clone(),
            memo: t.memo.clone(),
            body: t.body.clone(),
        };
        assert!(app.changes_nothing(&cmd));
        if let Command::TxnUpdate { memo, .. } = &mut cmd {
            *memo = "Changed".into();
        }
        assert!(!app.changes_nothing(&cmd));
    }

    #[test]
    fn created_transaction_ids_are_read_back() {
        assert_eq!(parse_txn_id("T42"), Some(TxnId(42)));
        assert_eq!(parse_txn_id("C4"), None);
        assert_eq!(parse_txn_id("T"), None);
    }

    #[test]
    fn clear_status_resets_the_status_bar() {
        let mut app = LedgerApp::with_session(Session::new(new_company("Clear Co")));
        app.error("Invoice 1001 is missing a customer");
        app.apply(Action::ClearStatus);
        assert_eq!(app.status, "Ready");
        assert!(!app.status_is_error);
    }

    #[test]
    fn status_messages_clear_themselves() {
        let mut app = LedgerApp::blank(None);
        app.info("Saved Pine Ridge.lcbooks");
        assert_eq!(app.expire_status(100.0), Some(STATUS_SECONDS));
        assert!(app.expire_status(100.0 + STATUS_SECONDS - 1.0).is_some());
        assert_eq!(app.expire_status(100.0 + STATUS_SECONDS), None);
        assert_eq!(app.status, "Ready");
        // Errors stay longer.
        app.error("Could not save");
        app.expire_status(200.0);
        assert!(app.expire_status(200.0 + STATUS_SECONDS + 1.0).is_some());
        assert!(app.status_is_error);
        app.expire_status(200.0 + ERROR_SECONDS);
        assert_eq!(app.status, "Ready");
        assert!(!app.status_is_error);
    }

    #[test]
    fn the_status_bar_shows_the_file_name_not_the_whole_path() {
        let path = std::env::temp_dir().join("Pine Ridge Woodworks.lcbooks");
        assert_eq!(file_name(&path), "Pine Ridge Woodworks.lcbooks");
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
        let renamed = app.session.as_mut().map(|s| {
            s.execute(Command::CompanyRename {
                name: "Dirty Co 2".into(),
            })
            .is_ok()
        });
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
    fn opening_an_unprotected_file_offers_to_protect_it() {
        let dir = std::env::temp_dir().join(format!("lc-offer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain.lcbooks");
        ledgercraft_file::save(&plain, &new_company("Plain Co"), None).unwrap();
        let mut app = LedgerApp::blank(None);
        app.open_path(plain);
        assert!(app.session.is_some());
        assert!(app.offer_protection);

        let locked = dir.join("locked.lcbooks");
        let protection = ledgercraft_file::Protection::new("already locked").unwrap();
        ledgercraft_file::save(&locked, &new_company("Locked Co"), Some(&protection)).unwrap();
        let mut app = LedgerApp::blank(None);
        app.open_path(locked);
        assert!(
            app.session.is_none(),
            "a protected file waits for its password"
        );
        assert!(app.unlock.is_some());
        assert!(!app.offer_protection);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn with_no_company_the_books_cannot_change() {
        let mut app = LedgerApp::blank(None);
        app.apply(Action::Run {
            cmd: Command::CompanyRename {
                name: "Nobody".into(),
            },
            reset: Reset::Nothing,
        });
        assert!(app.status_is_error);
        app.undo();
        assert!(!app.save());
    }

    #[test]
    fn opened_companies_go_to_the_top_of_the_recent_list() {
        let dir = std::env::temp_dir().join(format!("lc-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.lcbooks");
        let b = dir.join("b.lcbooks");
        ledgercraft_file::save(&a, &new_company("A Co"), None).unwrap();
        ledgercraft_file::save(&b, &new_company("B Co"), None).unwrap();
        let mut app = LedgerApp::blank(None);
        app.state_path = Some(dir.join("state.json"));
        app.open_path(a.clone());
        app.open_path(b.clone());
        app.open_path(a.clone());
        assert_eq!(app.state.recent, vec![a.clone(), b.clone()]);
        app.save_state();
        let saved = AppState::load_from(&dir.join("state.json"));
        assert_eq!(saved.recent, vec![a, b]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_recent_company_is_reported_and_forgotten() {
        let ctx = egui::Context::default();
        let gone = std::env::temp_dir().join(format!("lc-gone-{}.lcbooks", std::process::id()));
        let mut app = LedgerApp::blank(None);
        app.state.remember(&gone);
        app.open_target = Some(gone);
        app.request(Pending::OpenPath, &ctx);
        assert!(app.session.is_none());
        assert!(app.status_is_error);
        assert!(app.state.recent.is_empty());
    }

    #[test]
    fn opening_a_recent_company_with_unsaved_changes_asks_first() {
        let ctx = egui::Context::default();
        let mut app = LedgerApp::with_session(Session::new(new_company("Busy Co")));
        let renamed = app.session.as_mut().map(|s| {
            s.execute(Command::CompanyRename {
                name: "Busy Co 2".into(),
            })
            .is_ok()
        });
        assert_eq!(renamed, Some(true));
        app.open_target = Some(std::env::temp_dir().join("elsewhere.lcbooks"));
        app.request(Pending::OpenPath, &ctx);
        assert!(app.pending == Some(Pending::OpenPath));
        assert!(app.session.is_some());
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
