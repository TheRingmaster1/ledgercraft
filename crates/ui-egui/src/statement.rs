//! Customers > Create Statement…: pick a customer and a period, then save
//! their statement as a PDF.

use eframe::egui::{self, RichText};
use ledgercraft_core::Date;
use ledgercraft_ledger::{Company, CustomerId};

use crate::{currency, theme};

/// What the window is choosing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StatementForm {
    pub(crate) customer: Option<CustomerId>,
    from: String,
    to: String,
    error: Option<String>,
}

/// What the user chose.
pub(crate) enum StatementOutcome {
    Save {
        customer: CustomerId,
        from: Date,
        to: Date,
    },
    Cancel,
}

impl StatementForm {
    /// This month so far, for `customer` if one was picked.
    pub(crate) fn new(customer: Option<CustomerId>) -> Self {
        let today = Date::today();
        Self {
            customer,
            from: today.first_of_month().to_string(),
            to: today.to_string(),
            error: None,
        }
    }

    fn choice(&self) -> Result<StatementOutcome, String> {
        let customer = self.customer.ok_or("Choose a customer.")?;
        let from = Date::parse(&self.from).map_err(|e| format!("From: {e}"))?;
        let to = Date::parse(&self.to).map_err(|e| format!("To: {e}"))?;
        if from > to {
            return Err("From is after To.".into());
        }
        Ok(StatementOutcome::Save { customer, from, to })
    }
}

pub(crate) fn window(
    ctx: &egui::Context,
    form: &mut StatementForm,
    c: &Company,
) -> Option<StatementOutcome> {
    let mut outcome = None;
    let mut open = true;
    egui::Window::new("Create Statement")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "A statement lists a customer's charges and payments for a period, \
                     with what they owe and how overdue it is.",
                )
                .color(theme::MUTED),
            );
            ui.add_space(6.0);
            egui::Grid::new("statement_form")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Customer");
                    let chosen = form
                        .customer
                        .and_then(|id| c.customer(id))
                        .map_or("Choose…".to_string(), |x| x.contact.name.clone());
                    egui::ComboBox::from_id_salt("statement_customer")
                        .selected_text(chosen)
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            let current = form.customer;
                            for x in c
                                .customers
                                .iter()
                                .filter(|x| x.active || current == Some(x.id))
                            {
                                let id = x.id;
                                ui.selectable_value(
                                    &mut form.customer,
                                    Some(id),
                                    x.contact.name.as_str(),
                                );
                            }
                        });
                    ui.end_row();
                    ui.label("From");
                    ui.add(egui::TextEdit::singleline(&mut form.from).desired_width(110.0));
                    ui.end_row();
                    ui.label("To");
                    ui.add(egui::TextEdit::singleline(&mut form.to).desired_width(110.0));
                    ui.end_row();
                    if let Some(id) = form.customer {
                        ui.label("Owes today");
                        let owed = c.customer_balance(id);
                        ui.label(owed.format_with(currency::current().symbol()));
                        ui.end_row();
                    }
                });
            if let Some(e) = &form.error {
                ui.add_space(4.0);
                ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save as PDF…").strong()).clicked() {
                    match form.choice() {
                        Ok(choice) => outcome = Some(choice),
                        Err(e) => form.error = Some(e),
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(StatementOutcome::Cancel);
                }
            });
        });
    if !open {
        outcome = Some(StatementOutcome::Cancel);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_period_must_make_sense() {
        let mut f = StatementForm::new(None);
        assert_eq!(f.choice().err().as_deref(), Some("Choose a customer."));
        f.customer = Some(CustomerId(1));
        assert!(f.choice().is_ok());
        f.from = "2026-05-01".into();
        f.to = "2026-04-01".into();
        assert_eq!(f.choice().err().as_deref(), Some("From is after To."));
        f.to = "not a date".into();
        assert!(f.choice().err().unwrap_or_default().starts_with("To:"));
    }
}
