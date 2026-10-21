//! What LedgerCraft shows when no company is open: the start page (create,
//! open, sample, tutorial) and the Create a Company window.

use std::path::PathBuf;

use eframe::egui::{self, RichText};
use ledgercraft_file::import::{self, ImportSummary, ListKind};
use ledgercraft_ledger::{Address, Company};

use crate::security::{self, Secret};
use crate::theme;

const ICON_PNG: &[u8] = include_bytes!("../../../assets/app-icon/ledgercraft-256.png");
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const NAME_MAX: usize = 100;
/// Largest import file read, so a wrong pick cannot exhaust memory.
const IMPORT_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// A file chosen for import: an IIF file, or a CSV holding one list.
struct ImportFile {
    path: PathBuf,
    /// `None` for IIF (which holds several lists).
    csv: Option<ListKind>,
}

impl ImportFile {
    fn label(&self) -> String {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match self.csv {
            None => format!("{name}  (IIF lists)"),
            Some(kind) => format!("{name}  ({})", kind.label()),
        }
    }
}

/// A button on the start page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StartChoice {
    Create,
    Open,
    Sample,
    Tutorial,
    /// One of the recent companies.
    Recent(PathBuf),
    /// "Remove From This List" on a recent company.
    Forget(PathBuf),
}

/// The start page. Holds the icon texture, loaded on first use.
#[derive(Default)]
pub(crate) struct StartPage {
    icon: Option<egui::TextureHandle>,
    icon_failed: bool,
}

impl StartPage {
    fn icon(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.icon.is_none() && !self.icon_failed {
            self.icon = load_icon(ctx);
            self.icon_failed = self.icon.is_none();
        }
        self.icon.clone()
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, recent: &[PathBuf]) -> Option<StartChoice> {
        let mut choice = None;
        let icon = self.icon(ui.ctx());
        // Less space above when the recent list needs the room.
        let share = if recent.is_empty() { 0.12 } else { 0.05 };
        let top = (ui.available_height() * share).max(16.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| Self::page(ui, icon, top, recent, &mut choice));
        choice
    }

    fn page(
        ui: &mut egui::Ui,
        icon: Option<egui::TextureHandle>,
        top: f32,
        recent: &[PathBuf],
        choice: &mut Option<StartChoice>,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(top);
            if let Some(icon) = icon {
                ui.add(egui::Image::new(&icon).fit_to_exact_size(egui::vec2(96.0, 96.0)));
                ui.add_space(8.0);
            }
            ui.label(RichText::new("LedgerCraft").size(30.0).strong().color(theme::ACCENT));
            ui.label(
                RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(theme::MUTED),
            );
            ui.add_space(4.0);
            ui.label("Open-source accounting for small businesses.");
            ui.add_space(20.0);

            egui::Frame::group(ui.style()).inner_margin(20.0).show(ui, |ui| {
                ui.set_width(360.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No company is open").strong().size(16.0));
                    ui.add_space(10.0);
                    let size = [320.0, 40.0];
                    let big = |text: &str| egui::Button::new(RichText::new(text).size(16.0));
                    if ui
                        .add_sized(size, big("Create a New Company…"))
                        .on_hover_text("Set up a new set of books: name, fiscal year and a starting chart of accounts.")
                        .clicked()
                    {
                        *choice = Some(StartChoice::Create);
                    }
                    ui.add_space(4.0);
                    if ui
                        .add_sized(size, big("Open a Company File…"))
                        .on_hover_text("Open books you saved earlier (a .lcbooks file).")
                        .clicked()
                    {
                        *choice = Some(StartChoice::Open);
                    }
                    ui.add_space(4.0);
                    if ui
                        .add_sized(size, big("Explore the Sample Company"))
                        .on_hover_text(
                            "Pine Ridge Woodworks: a small business with a few months of \
                             transactions, to practise on.",
                        )
                        .clicked()
                    {
                        *choice = Some(StartChoice::Sample);
                    }
                    ui.add_space(10.0);
                    if ui.link("New to LedgerCraft? Show the tutorial").clicked() {
                        *choice = Some(StartChoice::Tutorial);
                    }
                });
            });
            if !recent.is_empty() {
                ui.add_space(14.0);
                if let Some(picked) = recent_list(ui, recent) {
                    *choice = Some(picked);
                }
            }
            ui.add_space(16.0);
        });
    }
}

/// The recently opened companies, newest first. Right-click one to remove it.
fn recent_list(ui: &mut egui::Ui, recent: &[PathBuf]) -> Option<StartChoice> {
    let mut choice = None;
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.set_width(360.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.label(RichText::new("Recent companies").strong().size(15.0));
                ui.add_space(4.0);
                for path in recent {
                    let (name, folder) = crate::recent::describe(path);
                    let found = path.exists();
                    let link = if found {
                        ui.link(RichText::new(name).size(15.0))
                            .on_hover_text(path.display().to_string())
                    } else {
                        ui.link(RichText::new(format!("{name}  (not found)")).color(theme::MUTED))
                            .on_hover_text("This file has been moved, renamed or deleted.")
                    };
                    if link.clicked() {
                        choice = Some(StartChoice::Recent(path.clone()));
                    }
                    link.context_menu(|ui| {
                        if ui.button("Remove From This List").clicked() {
                            ui.close();
                            choice = Some(StartChoice::Forget(path.clone()));
                        }
                    });
                    ui.label(RichText::new(folder).small().color(theme::MUTED));
                    ui.add_space(2.0);
                }
            });
        });
    choice
}

/// Decodes the app icon into a texture; `None` if it cannot be read.
fn load_icon(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let data = eframe::icon_data::from_png_bytes(ICON_PNG).ok()?;
    let width = usize::try_from(data.width).ok()?;
    let height = usize::try_from(data.height).ok()?;
    // ColorImage panics on a size mismatch, so check first.
    if width.checked_mul(height)?.checked_mul(4)? != data.rgba.len() {
        return None;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &data.rgba);
    Some(ctx.load_texture("ledgercraft-icon", image, egui::TextureOptions::LINEAR))
}

/// What the person typed into Create a Company.
pub(crate) struct NewCompanyForm {
    name: String,
    legal_name: String,
    /// 1 = January.
    fiscal_month: u32,
    starter_accounts: bool,
    email: String,
    phone: String,
    line1: String,
    city: String,
    region: String,
    postal_code: String,
    imports: Vec<ImportFile>,
    password: Secret,
    confirm: Secret,
    /// How strong `password` is, worked out when it changes.
    strength: Option<security::Strength>,
    /// Show the password and confirmation instead of dots.
    show_password: bool,
    /// "I understand a forgotten password cannot be recovered."
    understood: bool,
    /// The list the next CSV file holds.
    csv_kind: ListKind,
    error: Option<String>,
}

impl Default for NewCompanyForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            legal_name: String::new(),
            fiscal_month: 1,
            starter_accounts: true,
            email: String::new(),
            phone: String::new(),
            line1: String::new(),
            city: String::new(),
            region: String::new(),
            postal_code: String::new(),
            imports: Vec::new(),
            password: Secret::default(),
            confirm: Secret::default(),
            strength: None,
            show_password: false,
            understood: false,
            csv_kind: ListKind::Customers,
            error: None,
        }
    }
}

impl NewCompanyForm {
    /// Builds the company and brings in the chosen files, or says what is
    /// wrong. The summary is `None` when nothing was imported.
    fn build_with_imports(&self) -> Result<(Company, Option<ImportSummary>), String> {
        let mut company = self.build()?;
        if self.imports.is_empty() {
            return Ok((company, None));
        }
        let mut summary = ImportSummary::default();
        // IIF first, then CSV lists with accounts before the items that use them.
        let mut order: Vec<&ImportFile> = self.imports.iter().collect();
        order.sort_by_key(|f| {
            f.csv.map_or(0, |k| {
                1 + ListKind::ALL.iter().position(|x| *x == k).unwrap_or(0)
            })
        });
        for file in order {
            let text = read_import(&file.path)?;
            match file.csv {
                None => summary.merge(import::import_iif(&mut company, &text)),
                Some(kind) => {
                    let part = import::import_csv(&mut company, kind, &text)
                        .map_err(|e| format!("{}: {e}", file.label()))?;
                    summary.merge(part);
                }
            }
        }
        Ok((company, Some(summary)))
    }

    /// Builds the company, or says what is missing.
    fn build(&self) -> Result<Company, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Enter the company's name.".into());
        }
        if name.chars().count() > NAME_MAX {
            return Err(format!("The name can be at most {NAME_MAX} characters."));
        }
        let mut company = if self.starter_accounts {
            Company::with_default_accounts(name)
        } else {
            Company::new(name)
        };
        let info = &mut company.info;
        info.legal_name = self.legal_name.trim().to_string();
        info.fiscal_year_start_month = self.fiscal_month.clamp(1, 12);
        info.email = self.email.trim().to_string();
        info.phone = self.phone.trim().to_string();
        info.address = Address {
            line1: self.line1.trim().to_string(),
            city: self.city.trim().to_string(),
            region: self.region.trim().to_string(),
            postal_code: self.postal_code.trim().to_string(),
            ..Address::default()
        };
        Ok(company)
    }
}

fn read_import(path: &std::path::Path) -> Result<String, String> {
    let shown = path.display();
    let len = std::fs::metadata(path)
        .map_err(|e| format!("Could not read {shown}: {e}"))?
        .len();
    if len > IMPORT_MAX_BYTES {
        return Err(format!("{shown} is too large to import ({len} bytes)."));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {shown}: {e}"))?;
    // IIF files are often saved in the Windows code page, not UTF-8; keep
    // what can be read rather than refusing the file.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// What happened in the Create a Company window this frame.
pub(crate) enum CreateOutcome {
    /// The new company, what was imported into it (if anything), and the
    /// password to protect it with.
    Create(Box<Company>, Option<ImportSummary>, Secret),
    Cancel,
}

/// Draws Create a Company. Returns `None` while the person is still filling
/// it in.
/// Width of each column of Create a Company, in points.
const COLUMN_WIDTH: f32 = 430.0;
/// Space between the two columns.
const COLUMN_GAP: f32 = 28.0;
/// Room the window needs around its scrolling part: title bar, the line at
/// the top, the buttons and messages at the bottom, and margins.
const CREATE_CHROME: f32 = 190.0;

pub(crate) fn new_company_window(
    ctx: &egui::Context,
    form: &mut NewCompanyForm,
) -> Option<CreateOutcome> {
    let mut outcome = None;
    let mut open = true;
    // Two columns side by side when the app window is wide enough, one
    // otherwise; either way the fields scroll if the window is short, so the
    // buttons always stay in view.
    let screen = ctx.content_rect();
    let wide = screen.width() >= 2.0 * COLUMN_WIDTH + COLUMN_GAP + 60.0;
    let scroll_height = (screen.height() - CREATE_CHROME).max(120.0);
    egui::Window::new("Create a Company")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Tell LedgerCraft about the business. Only the name is required.");
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(scroll_height)
                .show(ui, |ui| {
                    if wide {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(COLUMN_WIDTH);
                                details_section(ui, form);
                            });
                            ui.add_space(COLUMN_GAP);
                            ui.vertical(|ui| {
                                ui.set_width(COLUMN_WIDTH);
                                password_section(ui, form);
                                ui.add_space(12.0);
                                import_section(ui, form);
                            });
                        });
                    } else {
                        ui.vertical(|ui| {
                            ui.set_width(COLUMN_WIDTH);
                            details_section(ui, form);
                            ui.add_space(8.0);
                            password_section(ui, form);
                            ui.add_space(8.0);
                            import_section(ui, form);
                        });
                    }
                });
            ui.separator();
            if let Some(error) = &form.error {
                ui.label(RichText::new(error.as_str()).color(theme::NEGATIVE));
            }
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Create Company").strong())
                    .clicked()
                {
                    outcome = create_clicked(form);
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(CreateOutcome::Cancel);
                }
                ui.label(
                    RichText::new("Next, LedgerCraft asks where to save the company file.")
                        .small()
                        .color(theme::MUTED),
                );
            });
        });
    if !open {
        outcome = Some(CreateOutcome::Cancel);
    }
    outcome
}

/// Create Company: checks the password, builds the company and runs the
/// imports, or notes what is wrong on the form.
fn create_clicked(form: &mut NewCompanyForm) -> Option<CreateOutcome> {
    let password = security::check_new(&form.password.0, &form.confirm.0, &[form.name.trim()])
        .and_then(|()| {
            if form.understood {
                Ok(())
            } else {
                Err("Tick the box to confirm you will keep the password safe.".to_string())
            }
        });
    match password.and_then(|()| form.build_with_imports()) {
        Ok((company, summary)) => {
            let secret = Secret(std::mem::take(&mut form.password.0));
            Some(CreateOutcome::Create(Box::new(company), summary, secret))
        }
        Err(e) => {
            form.error = Some(e);
            None
        }
    }
}

/// The company's name, fiscal year, contact details and starting chart of
/// accounts.
fn details_section(ui: &mut egui::Ui, form: &mut NewCompanyForm) {
    ui.label(RichText::new("Company").strong());
    egui::Grid::new("new_company")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            let field = |ui: &mut egui::Ui, text: &mut String, hint: &str| {
                ui.add_sized(
                    [280.0, 22.0],
                    egui::TextEdit::singleline(text).hint_text(hint),
                );
            };
            ui.label(RichText::new("Company name").strong());
            field(ui, &mut form.name, "e.g. Acme Repair");
            ui.end_row();
            ui.label("Legal name");
            field(
                ui,
                &mut form.legal_name,
                "if different, e.g. Acme Repair LLC",
            );
            ui.end_row();
            ui.label("Fiscal year starts").on_hover_text(
                "The first month of your financial year. Most small businesses use January.",
            );
            let index = usize::try_from(form.fiscal_month.saturating_sub(1)).unwrap_or(0);
            let current = MONTHS.get(index).copied().unwrap_or("January");
            egui::ComboBox::from_id_salt("fiscal_month")
                .selected_text(current)
                .width(160.0)
                .show_ui(ui, |ui| {
                    for (month, label) in (1u32..).zip(MONTHS) {
                        ui.selectable_value(&mut form.fiscal_month, month, label);
                    }
                });
            ui.end_row();
            ui.label("Email");
            field(ui, &mut form.email, "");
            ui.end_row();
            ui.label("Phone");
            field(ui, &mut form.phone, "");
            ui.end_row();
            ui.label("Street");
            field(ui, &mut form.line1, "");
            ui.end_row();
            ui.label("City");
            field(ui, &mut form.city, "");
            ui.end_row();
            ui.label("State / region");
            field(ui, &mut form.region, "");
            ui.end_row();
            ui.label("Postal code");
            field(ui, &mut form.postal_code, "");
            ui.end_row();
        });
    ui.add_space(12.0);
    ui.label(RichText::new("Chart of accounts").strong());
    ui.radio_value(
        &mut form.starter_accounts,
        true,
        "Start with a standard small-business chart of accounts (recommended)",
    );
    ui.radio_value(
        &mut form.starter_accounts,
        false,
        "Start with no accounts; I will add my own",
    );
}

/// The company file's password. Every new company is encrypted with one.
fn password_section(ui: &mut egui::Ui, form: &mut NewCompanyForm) {
    ui.label(RichText::new("Password").strong());
    ui.label(
        "The company file is encrypted with this password, on disk and in every backup \
         copy. You will need it each time you open the company.",
    );
    egui::Grid::new("new_company_password")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Password");
            let typed = security::password_with_peek(
                ui,
                &mut form.password.0,
                &format!(
                    "at least {} characters",
                    ledgercraft_file::MIN_PASSWORD_CHARS
                ),
                &mut form.show_password,
            );
            if typed.changed() {
                form.strength = security::strength(&form.password.0, &[form.name.trim()]);
            }
            ui.end_row();
            ui.label("");
            security::strength_meter(ui, form.strength);
            ui.end_row();
            ui.label("Confirm");
            security::confirm_field(
                ui,
                &mut form.confirm.0,
                &form.password.0,
                form.show_password,
            );
            ui.end_row();
        });
    security::passphrase_tip(ui);
    security::forgotten_password_note(ui);
    ui.checkbox(
        &mut form.understood,
        "I understand, and I will keep the password safe",
    );
}

/// "Bring in existing data": choose IIF and CSV files to import.
fn import_section(ui: &mut egui::Ui, form: &mut NewCompanyForm) {
    ui.label(RichText::new("Bring in existing data (optional)").strong());
    ui.label(
        "Moving from other accounting software? Import your lists now: chart of accounts, \
         customers, vendors, and products and services. Opening balances and past \
         transactions are not imported yet; enter balances with a journal entry.",
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let iif = ui.button("Add IIF File…").on_hover_text(
            "An IIF list export, for example from QuickBooks Desktop: File > Utilities > \
             Export > Lists to IIF Files. Brings in accounts, customers, vendors and items.",
        );
        if iif.clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("IIF file", &["iif", "txt"])
                .pick_file()
            {
                form.imports.push(ImportFile { path, csv: None });
            }
        }
        ui.separator();
        let current = form.csv_kind.label();
        egui::ComboBox::from_id_salt("csv_kind")
            .selected_text(current)
            .width(170.0)
            .show_ui(ui, |ui| {
                for kind in ListKind::ALL {
                    ui.selectable_value(&mut form.csv_kind, kind, kind.label());
                }
            });
        let csv = ui.button("Add CSV File…").on_hover_text(format!(
            "A CSV file with a header row holding this list. Columns read: {}. Only the \
             name is required.",
            form.csv_kind.columns()
        ));
        if csv.clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV file", &["csv", "txt"])
                .pick_file()
            {
                form.imports.push(ImportFile {
                    path,
                    csv: Some(form.csv_kind),
                });
            }
        }
    });
    let mut remove = None;
    for (i, file) in form.imports.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("•  {}", file.label()));
            if ui
                .small_button("x")
                .on_hover_text("Don't import this file")
                .clicked()
            {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        if i < form.imports.len() {
            form.imports.remove(i);
        }
    }
}

/// Shows what an import brought in and what it skipped, until closed.
pub(crate) fn import_report_window(ctx: &egui::Context, report: &mut Option<ImportSummary>) {
    let Some(summary) = report.as_ref() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    egui::Window::new("Import Results")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(460.0)
        .show(ctx, |ui| {
            egui::Grid::new("import_counts")
                .num_columns(2)
                .spacing([24.0, 4.0])
                .show(ui, |ui| {
                    for (label, n) in [
                        ("Accounts", summary.accounts),
                        ("Customers", summary.customers),
                        ("Vendors", summary.vendors),
                        ("Products and services", summary.items),
                    ] {
                        ui.label(label);
                        ui.label(RichText::new(n.to_string()).strong());
                        ui.end_row();
                    }
                });
            if !summary.skipped.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Not imported ({})", summary.skipped.len())).strong(),
                );
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for line in &summary.skipped {
                            ui.label(RichText::new(line.as_str()).color(theme::MUTED));
                        }
                    });
                ui.label(
                    RichText::new(
                        "Fix these in the source file and add them by hand, or leave them out.",
                    )
                    .small()
                    .color(theme::MUTED),
                );
            }
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if !open || close {
        *report = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_required() {
        let form = NewCompanyForm {
            name: "   ".into(),
            ..NewCompanyForm::default()
        };
        assert!(form.build().is_err());
        let long = NewCompanyForm {
            name: "x".repeat(NAME_MAX + 1),
            ..NewCompanyForm::default()
        };
        assert!(long.build().is_err());
    }

    #[test]
    fn imports_run_after_the_company_is_built() {
        let dir = std::env::temp_dir().join(format!("lc-start-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("customers.csv");
        std::fs::write(&csv, "Name,Terms\nAcme,Net 30\n").unwrap();
        let form = NewCompanyForm {
            name: "Import Co".into(),
            imports: vec![ImportFile {
                path: csv,
                csv: Some(ListKind::Customers),
            }],
            ..NewCompanyForm::default()
        };
        let (company, summary) = form.build_with_imports().unwrap();
        assert_eq!(summary.map(|s| s.customers), Some(1));
        assert_eq!(company.customers.len(), 1);

        let missing = NewCompanyForm {
            name: "Missing Co".into(),
            imports: vec![ImportFile {
                path: dir.join("nope.csv"),
                csv: Some(ListKind::Vendors),
            }],
            ..NewCompanyForm::default()
        };
        assert!(missing.build_with_imports().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn builds_the_company_as_entered() {
        let form = NewCompanyForm {
            name: "  Acme Repair ".into(),
            legal_name: "Acme Repair LLC".into(),
            fiscal_month: 7,
            city: "Provo".into(),
            ..NewCompanyForm::default()
        };
        let company = form.build().unwrap();
        assert_eq!(company.info.name, "Acme Repair");
        assert_eq!(company.info.legal_name, "Acme Repair LLC");
        assert_eq!(company.info.fiscal_year_start_month, 7);
        assert_eq!(company.info.address.city, "Provo");
        assert!(!company.accounts.is_empty());

        let empty = NewCompanyForm {
            name: "Blank Co".into(),
            starter_accounts: false,
            ..NewCompanyForm::default()
        };
        assert!(empty.build().unwrap().accounts.is_empty());
    }
}
