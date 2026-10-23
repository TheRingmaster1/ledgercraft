//! File > Company Information…: the company's name and the contact details
//! printed on invoices, and how printed documents look (template and logo).

use eframe::egui::{self, RichText};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{Address, Company, Logo, PdfTemplate, PrintStyle, LOGO_MAX_BYTES};

use crate::theme;

/// What the window is editing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CompanyInfoForm {
    name: String,
    legal_name: String,
    email: String,
    phone: String,
    line1: String,
    line2: String,
    city: String,
    region: String,
    postal_code: String,
    country: String,
    template: PdfTemplate,
    logo: Option<Logo>,
    /// What the logo is: its file name and size, once chosen.
    logo_note: String,
    error: Option<String>,
}

/// The most room the logo preview takes, in points.
const PREVIEW_SIZE: egui::Vec2 = egui::vec2(240.0, 80.0);

/// The logo as a texture to show, decoded once and kept while the window is
/// open (keyed by the file's contents); `None` if it cannot be shown.
fn logo_texture(ctx: &egui::Context, bytes: &[u8]) -> Option<egui::TextureHandle> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    bytes.hash(&mut hasher);
    let id = egui::Id::NULL.with(("company_logo_preview", hasher.finish()));
    if let Some(cached) = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return cached;
    }
    let texture = image::load_from_memory(bytes).ok().map(|decoded| {
        let rgba = decoded.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        ctx.load_texture("company_logo", pixels, egui::TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

/// The logo on a white card, as it sits on paper: scaled to fit, never
/// enlarged.
fn logo_preview(ui: &mut egui::Ui, bytes: &[u8]) {
    let Some(texture) = logo_texture(ui.ctx(), bytes) else {
        ui.label(RichText::new("The logo cannot be shown here.").color(theme::MUTED));
        return;
    };
    let natural = texture.size_vec2();
    let scale = (PREVIEW_SIZE.x / natural.x)
        .min(PREVIEW_SIZE.y / natural.y)
        .min(1.0);
    egui::Frame::NONE
        .fill(egui::Color32::WHITE)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.image((texture.id(), natural * scale));
        });
}

/// What the user chose.
pub(crate) enum CompanyInfoOutcome {
    /// Boxed: a command is large, and Cancel carries nothing.
    Save(Box<Command>),
    Cancel,
}

impl CompanyInfoForm {
    pub(crate) fn from_company(c: &Company) -> Self {
        let info = &c.info;
        let a = &info.address;
        Self {
            name: info.name.clone(),
            legal_name: info.legal_name.clone(),
            email: info.email.clone(),
            phone: info.phone.clone(),
            line1: a.line1.clone(),
            line2: a.line2.clone(),
            city: a.city.clone(),
            region: a.region.clone(),
            postal_code: a.postal_code.clone(),
            country: a.country.clone(),
            template: info.print.template,
            logo: info.print.logo.clone(),
            logo_note: info
                .print
                .logo
                .as_ref()
                .and_then(|l| ledgercraft_reports::logo_size(&l.0).ok())
                .map(|(w, h)| format!("{w} x {h} pixels"))
                .unwrap_or_default(),
            error: None,
        }
    }

    /// Asks for a PNG or JPEG file and takes it as the logo, or says why not.
    fn choose_logo(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Image (PNG or JPEG)", &["png", "jpg", "jpeg"])
            .pick_file()
        else {
            return;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match std::fs::read(&path) {
            Ok(bytes) if bytes.len() > LOGO_MAX_BYTES => {
                self.error = Some(format!(
                    "{name} is larger than 1 MB: use a smaller image (a few hundred pixels \
                     across is plenty)."
                ));
            }
            Ok(bytes) => match ledgercraft_reports::logo_size(&bytes) {
                Ok((w, h)) => {
                    self.logo = Some(Logo(bytes));
                    self.logo_note = format!("{name}, {w} x {h} pixels");
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("{name}: {e}")),
            },
            Err(e) => self.error = Some(format!("Could not read {name}: {e}")),
        }
    }

    fn command(&self) -> Result<Command, String> {
        if self.name.trim().is_empty() {
            return Err("Enter the company's name.".into());
        }
        Ok(Command::CompanySetInfo {
            name: self.name.clone(),
            legal_name: self.legal_name.clone(),
            email: self.email.clone(),
            phone: self.phone.clone(),
            address: Address {
                line1: self.line1.clone(),
                line2: self.line2.clone(),
                city: self.city.clone(),
                region: self.region.clone(),
                postal_code: self.postal_code.clone(),
                country: self.country.clone(),
            },
            print: Some(PrintStyle {
                template: self.template,
                logo: self.logo.clone(),
            }),
        })
    }
}

pub(crate) fn window(
    ctx: &egui::Context,
    form: &mut CompanyInfoForm,
) -> Option<CompanyInfoOutcome> {
    let mut outcome = None;
    let mut open = true;
    egui::Window::new("Company Information")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Your company's name and the contact details printed on invoices.")
                    .color(theme::MUTED),
            );
            ui.add_space(6.0);
            egui::Grid::new("company_info_form")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    let rows: [(&str, &mut String, &str); 10] = [
                        ("Company name", &mut form.name, "e.g. Acme Repair"),
                        (
                            "Legal name",
                            &mut form.legal_name,
                            "if different, e.g. Acme Repair LLC",
                        ),
                        ("Email", &mut form.email, ""),
                        ("Phone", &mut form.phone, ""),
                        ("Street", &mut form.line1, ""),
                        ("Street, line 2", &mut form.line2, "suite, unit or PO box"),
                        ("City", &mut form.city, ""),
                        ("State / region", &mut form.region, ""),
                        ("Postal code", &mut form.postal_code, ""),
                        (
                            "Country",
                            &mut form.country,
                            "leave empty for your own country",
                        ),
                    ];
                    for (label, value, hint) in rows {
                        ui.label(label);
                        ui.add(
                            egui::TextEdit::singleline(value)
                                .hint_text(hint)
                                .desired_width(280.0),
                        );
                        ui.end_row();
                    }
                });
            ui.add_space(8.0);
            ui.separator();
            ui.label(RichText::new("Printed documents").strong());
            ui.label(
                RichText::new("Invoices, sales receipts, credit memos and statements.")
                    .color(theme::MUTED),
            );
            ui.add_space(4.0);
            egui::Grid::new("company_print")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Template");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("pdf_template")
                            .selected_text(form.template.label())
                            .show_ui(ui, |ui| {
                                for t in PdfTemplate::ALL {
                                    ui.selectable_value(&mut form.template, t, t.label())
                                        .on_hover_text(t.description());
                                }
                            });
                        ui.label(RichText::new(form.template.description()).color(theme::MUTED));
                    });
                    ui.end_row();
                    ui.label("Logo");
                    ui.horizontal(|ui| {
                        if form.logo.is_some() {
                            ui.label(form.logo_note.as_str());
                        } else {
                            ui.label(RichText::new("None").color(theme::MUTED));
                        }
                        if ui
                            .button("Choose…")
                            .on_hover_text("A PNG or JPEG, printed at the top left")
                            .clicked()
                        {
                            form.choose_logo();
                        }
                        if form.logo.is_some() && ui.button("Remove").clicked() {
                            form.logo = None;
                            form.logo_note.clear();
                        }
                    });
                    ui.end_row();
                    if let Some(logo) = &form.logo {
                        ui.label("");
                        logo_preview(ui, &logo.0);
                        ui.end_row();
                    }
                });
            if let Some(e) = &form.error {
                ui.add_space(4.0);
                ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save").strong()).clicked() {
                    match form.command() {
                        Ok(cmd) => outcome = Some(CompanyInfoOutcome::Save(Box::new(cmd))),
                        Err(e) => form.error = Some(e),
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(CompanyInfoOutcome::Cancel);
                }
            });
        });
    if !open {
        outcome = Some(CompanyInfoOutcome::Cancel);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_round_trips_the_company_details() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let form = CompanyInfoForm::from_company(&c);
        assert_eq!(form.name, "Pine Ridge Woodworks");
        let info = &c.info;
        let expected = Command::CompanySetInfo {
            name: info.name.clone(),
            legal_name: info.legal_name.clone(),
            email: info.email.clone(),
            phone: info.phone.clone(),
            address: info.address.clone(),
            print: Some(info.print.clone()),
        };
        assert_eq!(form.command(), Ok(expected));
    }

    #[test]
    fn a_name_is_required() {
        let form = CompanyInfoForm::default();
        assert!(form.command().is_err());
    }

    #[test]
    fn the_window_draws() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let mut form = CompanyInfoForm::from_company(&c);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut outcome = None;
        crate::theme::test_frame(&ctx, |ui| outcome = window(ui.ctx(), &mut form));
        assert!(outcome.is_none());
    }
}
