//! File > Closing Date…: close the books through a date, so nothing dated
//! then or before can be added, changed or voided, with an optional
//! password to allow it.

use eframe::egui::{self, RichText};
use ledgercraft_core::Date;
use ledgercraft_engine::Command;
use ledgercraft_ledger::Company;

use crate::theme;

/// What the window is editing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClosingForm {
    /// The books are (to be) closed.
    closed: bool,
    date: String,
    /// The books already have a closing date password.
    has_password: bool,
    /// The password is kept as it is.
    keep_password: bool,
    password: String,
    confirm: String,
    /// The closing date password, typed to unlock.
    unlock: String,
    error: Option<String>,
}

/// What the user chose.
pub(crate) enum ClosingOutcome {
    /// Boxed: a command is large.
    Save(Box<Command>),
    /// Enter the closing date password.
    Unlock(String),
    Cancel,
}

impl ClosingForm {
    pub(crate) fn from_company(c: &Company) -> Self {
        let closing = c.closing.as_ref();
        Self {
            closed: closing.is_some(),
            date: closing.map_or_else(String::new, |x| x.date.to_string()),
            has_password: closing.is_some_and(|x| x.password.is_some()),
            keep_password: true,
            ..Self::default()
        }
    }

    pub(crate) fn set_error(&mut self, e: String) {
        self.error = Some(e);
    }

    fn command(&self) -> Result<Command, String> {
        let date = if self.closed {
            Some(Date::parse(&self.date).map_err(|e| format!("Closing date: {e}"))?)
        } else {
            None
        };
        let keep_password = self.has_password && self.keep_password;
        let password = if keep_password || self.password.is_empty() {
            None
        } else if self.password != self.confirm {
            return Err("The two passwords are not the same.".into());
        } else {
            Some(self.password.clone())
        };
        Ok(Command::CompanySetClosing {
            date,
            password,
            keep_password,
        })
    }
}

/// `locked`: the books have a closing date password not yet entered.
pub(crate) fn window(
    ctx: &egui::Context,
    form: &mut ClosingForm,
    locked: bool,
) -> Option<ClosingOutcome> {
    let mut outcome = None;
    let mut open = true;
    egui::Window::new("Closing Date")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.label(
                RichText::new(
                    "Close the books once a period is finished (taxes filed, reports sent to \
                     your accountant): transactions dated on or before the closing date can \
                     no longer be added, changed or voided.",
                )
                .color(theme::MUTED),
            );
            ui.add_space(8.0);
            if locked {
                ui.label(
                    "These books have a closing date password. Enter it to change the closing \
                     date, or to change closed transactions until you close the company.",
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut form.unlock)
                            .password(true)
                            .desired_width(200.0),
                    );
                    if ui.button("Unlock").clicked() {
                        outcome = Some(ClosingOutcome::Unlock(form.unlock.clone()));
                    }
                });
                ui.add_space(8.0);
            }
            ui.add_enabled_ui(!locked, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut form.closed, "Close the books through");
                    ui.add_enabled(
                        form.closed,
                        egui::TextEdit::singleline(&mut form.date)
                            .hint_text("YYYY-MM-DD")
                            .desired_width(110.0),
                    );
                });
                ui.add_space(6.0);
                if form.has_password {
                    ui.checkbox(&mut form.keep_password, "Keep the closing date password");
                }
                if !form.has_password || !form.keep_password {
                    ui.label(
                        RichText::new(
                            "Password (optional): needed to change closed transactions. Leave \
                             it empty to need only a change of the closing date.",
                        )
                        .color(theme::MUTED),
                    );
                    egui::Grid::new("closing_password")
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Password");
                            ui.add(
                                egui::TextEdit::singleline(&mut form.password)
                                    .password(true)
                                    .desired_width(200.0),
                            );
                            ui.end_row();
                            ui.label("Again");
                            ui.add(
                                egui::TextEdit::singleline(&mut form.confirm)
                                    .password(true)
                                    .desired_width(200.0),
                            );
                            ui.end_row();
                        });
                }
            });
            if let Some(e) = &form.error {
                ui.add_space(4.0);
                ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let save =
                    ui.add_enabled(!locked, egui::Button::new(RichText::new("Save").strong()));
                if save.clicked() {
                    match form.command() {
                        Ok(cmd) => outcome = Some(ClosingOutcome::Save(Box::new(cmd))),
                        Err(e) => form.error = Some(e),
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(ClosingOutcome::Cancel);
                }
            });
        });
    if !open {
        outcome = Some(ClosingOutcome::Cancel);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_builds_the_closing_command() {
        let mut f = ClosingForm {
            closed: true,
            date: "2026-06-30".into(),
            password: "year end".into(),
            confirm: "year end".into(),
            ..ClosingForm::default()
        };
        assert_eq!(
            f.command(),
            Ok(Command::CompanySetClosing {
                date: Date::parse("2026-06-30").ok(),
                password: Some("year end".into()),
                keep_password: false,
            })
        );
        f.confirm = "year and".into();
        assert!(f.command().is_err());
        f.closed = false;
        f.password.clear();
        assert_eq!(
            f.command(),
            Ok(Command::CompanySetClosing {
                date: None,
                password: None,
                keep_password: false,
            })
        );
    }

    #[test]
    fn the_window_draws_locked_and_unlocked() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let mut form = ClosingForm::from_company(&c);
        assert!(form.closed);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        for locked in [false, true] {
            let mut outcome = None;
            crate::theme::test_frame(&ctx, |ui| outcome = window(ui.ctx(), &mut form, locked));
            assert!(outcome.is_none());
        }
    }
}
