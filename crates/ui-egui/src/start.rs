//! What LedgerCraft shows when no company is open: the start page (create,
//! open, sample, tutorial) and the Create a Company window.

use eframe::egui::{self, RichText};
use ledgercraft_ledger::{Address, Company};

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

/// A button on the start page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StartChoice {
    Create,
    Open,
    Sample,
    Tutorial,
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

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) -> Option<StartChoice> {
        let mut choice = None;
        let icon = self.icon(ui.ctx());
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.12).max(16.0));
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
                        choice = Some(StartChoice::Create);
                    }
                    ui.add_space(4.0);
                    if ui
                        .add_sized(size, big("Open a Company File…"))
                        .on_hover_text("Open books you saved earlier (a .lcbooks file).")
                        .clicked()
                    {
                        choice = Some(StartChoice::Open);
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
                        choice = Some(StartChoice::Sample);
                    }
                    ui.add_space(10.0);
                    if ui.link("New to LedgerCraft? Show the tutorial").clicked() {
                        choice = Some(StartChoice::Tutorial);
                    }
                });
            });
        });
        choice
    }
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
            error: None,
        }
    }
}

impl NewCompanyForm {
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

/// What happened in the Create a Company window this frame.
pub(crate) enum CreateOutcome {
    Create(Company),
    Cancel,
}

/// Draws Create a Company. Returns `None` while the person is still filling
/// it in.
pub(crate) fn new_company_window(ctx: &egui::Context, form: &mut NewCompanyForm) -> Option<CreateOutcome> {
    let mut outcome = None;
    let mut open = true;
    egui::Window::new("Create a Company")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label("Tell LedgerCraft about the business. Only the name is required.");
            ui.add_space(8.0);
            egui::Grid::new("new_company").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                let field = |ui: &mut egui::Ui, text: &mut String, hint: &str| {
                    ui.add_sized([280.0, 22.0], egui::TextEdit::singleline(text).hint_text(hint));
                };
                ui.label(RichText::new("Company name").strong());
                field(ui, &mut form.name, "e.g. Acme Repair");
                ui.end_row();
                ui.label("Legal name");
                field(ui, &mut form.legal_name, "if different, e.g. Acme Repair LLC");
                ui.end_row();
                ui.label("Fiscal year starts").on_hover_text(
                    "The first month of your financial year. Most small businesses use January.",
                );
                let index = usize::try_from(form.fiscal_month.saturating_sub(1)).unwrap_or(0);
                let current = MONTHS.get(index).copied().unwrap_or("January");
                egui::ComboBox::from_id_salt("fiscal_month").selected_text(current).width(160.0).show_ui(ui, |ui| {
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
            ui.add_space(8.0);
            ui.label(RichText::new("Chart of accounts").strong());
            ui.radio_value(
                &mut form.starter_accounts,
                true,
                "Start with a standard small-business chart of accounts (recommended)",
            );
            ui.radio_value(&mut form.starter_accounts, false, "Start with no accounts; I will add my own");
            if let Some(error) = &form.error {
                ui.add_space(6.0);
                ui.label(RichText::new(error.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Create Company").strong()).clicked() {
                    match form.build() {
                        Ok(company) => outcome = Some(CreateOutcome::Create(company)),
                        Err(e) => form.error = Some(e),
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(CreateOutcome::Cancel);
                }
            });
            ui.label(
                RichText::new("Next, LedgerCraft asks where to save the company file.")
                    .small()
                    .color(theme::MUTED),
            );
        });
    if !open {
        outcome = Some(CreateOutcome::Cancel);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_required() {
        let form = NewCompanyForm { name: "   ".into(), ..NewCompanyForm::default() };
        assert!(form.build().is_err());
        let long = NewCompanyForm { name: "x".repeat(NAME_MAX + 1), ..NewCompanyForm::default() };
        assert!(long.build().is_err());
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

        let empty = NewCompanyForm { name: "Blank Co".into(), starter_accounts: false, ..NewCompanyForm::default() };
        assert!(empty.build().unwrap().accounts.is_empty());
    }
}
