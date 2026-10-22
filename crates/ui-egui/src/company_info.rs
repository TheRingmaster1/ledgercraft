//! File > Company Information…: the company's name and the contact details
//! printed on invoices.

use eframe::egui::{self, RichText};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{Address, Company};

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
    error: Option<String>,
}

/// What the user chose.
pub(crate) enum CompanyInfoOutcome {
    Save(Command),
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
            error: None,
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
        })
    }
}

pub(crate) fn window(ctx: &egui::Context, form: &mut CompanyInfoForm) -> Option<CompanyInfoOutcome> {
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
                        ("Legal name", &mut form.legal_name, "if different, e.g. Acme Repair LLC"),
                        ("Email", &mut form.email, ""),
                        ("Phone", &mut form.phone, ""),
                        ("Street", &mut form.line1, ""),
                        ("Street, line 2", &mut form.line2, "suite, unit or PO box"),
                        ("City", &mut form.city, ""),
                        ("State / region", &mut form.region, ""),
                        ("Postal code", &mut form.postal_code, ""),
                        ("Country", &mut form.country, "leave empty for your own country"),
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
            if let Some(e) = &form.error {
                ui.add_space(4.0);
                ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save").strong()).clicked() {
                    match form.command() {
                        Ok(cmd) => outcome = Some(CompanyInfoOutcome::Save(cmd)),
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
