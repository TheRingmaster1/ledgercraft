//! Password windows: unlocking a protected company file, and File >
//! Password… for setting, changing or removing a company's password.
//! Typed passwords are wiped from memory when these windows close.

use std::path::PathBuf;

use eframe::egui::{self, RichText};
use ledgercraft_engine::{EngineError, Session};
use ledgercraft_file::{check_new_password, FileError, MIN_PASSWORD_CHARS};
use zeroize::Zeroize;

use crate::theme;

/// A typed password, wiped from memory when dropped.
#[derive(Default)]
pub(crate) struct Secret(pub(crate) String);

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// A password field: dots instead of letters.
pub(crate) fn password_field(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    ui.add_sized(
        [260.0, 22.0],
        egui::TextEdit::singleline(text).password(true).hint_text(hint),
    )
}

/// Checks a new password and its confirmation.
pub(crate) fn check_new(password: &str, confirm: &str) -> Result<(), String> {
    check_new_password(password)?;
    if password != confirm {
        return Err("The two passwords do not match.".into());
    }
    Ok(())
}

/// The note shown wherever a password is set.
pub(crate) fn forgotten_password_note(ui: &mut egui::Ui) {
    ui.label(
        RichText::new(
            "If you forget this password, nobody can open the file, not even the LedgerCraft \
             team: there is no recovery. Keep it in a password manager.",
        )
        .small()
        .color(theme::MUTED),
    );
}

// ----- unlock -----------------------------------------------------------------

/// Asks for the password of a protected company file.
pub(crate) struct UnlockForm {
    pub(crate) path: PathBuf,
    password: String,
    error: Option<String>,
    focused: bool,
}

impl UnlockForm {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path, password: String::new(), error: None, focused: false }
    }
}

impl Drop for UnlockForm {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

pub(crate) enum UnlockOutcome {
    Opened(Box<Session>),
    Cancel,
    /// Could not open for a reason other than the password.
    Failed(String),
}

pub(crate) fn unlock_window(ctx: &egui::Context, form: &mut UnlockForm) -> Option<UnlockOutcome> {
    let mut outcome = None;
    let mut open = true;
    let name = form
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| form.path.display().to_string());
    egui::Window::new("Password Required")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("{name} is password protected."));
            ui.add_space(6.0);
            let field = password_field(ui, &mut form.password, "Password");
            if !form.focused {
                field.request_focus();
                form.focused = true;
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(error) = &form.error {
                ui.label(RichText::new(error.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let open_clicked = ui.button(RichText::new("Open").strong()).clicked();
                if (open_clicked || enter) && !form.password.is_empty() {
                    match Session::open_with_password(&form.path, &form.password) {
                        Ok(session) => outcome = Some(UnlockOutcome::Opened(Box::new(session))),
                        Err(EngineError::File(FileError::WrongPassword)) => {
                            form.password.zeroize();
                            form.password.clear();
                            form.error = Some("That password is not correct. Try again.".into());
                            form.focused = false;
                        }
                        Err(e) => outcome = Some(UnlockOutcome::Failed(e.to_string())),
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(UnlockOutcome::Cancel);
                }
            });
        });
    if !open {
        outcome = Some(UnlockOutcome::Cancel);
    }
    outcome
}

// ----- File > Password… -------------------------------------------------------

/// Set, change or remove the open company's password.
#[derive(Default)]
pub(crate) struct PasswordForm {
    current: String,
    new: String,
    confirm: String,
    remove: bool,
    error: Option<String>,
}

impl Drop for PasswordForm {
    fn drop(&mut self) {
        self.current.zeroize();
        self.new.zeroize();
        self.confirm.zeroize();
    }
}

pub(crate) enum PasswordOutcome {
    /// Protect with this password, or remove protection (`None`).
    Apply(Option<String>),
    Cancel,
}

/// Whether `password` opens the saved file (true when there is no file yet).
fn opens_saved_file(session: &Session, password: &str) -> bool {
    match session.path() {
        Some(path) => ledgercraft_file::load(path, Some(password)).is_ok(),
        None => true,
    }
}

pub(crate) fn password_window(
    ctx: &egui::Context,
    form: &mut PasswordForm,
    session: &Session,
) -> Option<PasswordOutcome> {
    let mut outcome = None;
    let mut open = true;
    let protected = session.is_protected();
    let title = if protected { "Change Password" } else { "Set a Password" };
    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            if protected {
                ui.label("This company file is password protected and encrypted.");
            } else {
                ui.label(
                    "This company file is not password protected: anyone who gets a copy can \
                     read it. Set a password to encrypt it.",
                );
            }
            ui.add_space(6.0);
            egui::Grid::new("password_form").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                if protected {
                    ui.label("Current password");
                    password_field(ui, &mut form.current, "");
                    ui.end_row();
                }
                if !form.remove {
                    ui.label("New password");
                    password_field(ui, &mut form.new, &format!("at least {MIN_PASSWORD_CHARS} characters"));
                    ui.end_row();
                    ui.label("Confirm");
                    password_field(ui, &mut form.confirm, "type it again");
                    ui.end_row();
                }
            });
            if protected {
                ui.checkbox(&mut form.remove, "Remove the password (the file will no longer be encrypted)");
            }
            ui.add_space(4.0);
            if !form.remove {
                forgotten_password_note(ui);
            }
            if let Some(error) = &form.error {
                ui.label(RichText::new(error.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let label = if form.remove { "Remove Password" } else { "Save Password" };
                if ui.button(RichText::new(label).strong()).clicked() {
                    form.error = None;
                    if protected && !opens_saved_file(session, &form.current) {
                        form.error = Some("The current password is not correct.".into());
                    } else if form.remove {
                        outcome = Some(PasswordOutcome::Apply(None));
                    } else {
                        match check_new(&form.new, &form.confirm) {
                            Ok(()) => outcome = Some(PasswordOutcome::Apply(Some(form.new.clone()))),
                            Err(e) => form.error = Some(e),
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(PasswordOutcome::Cancel);
                }
            });
        });
    if !open {
        outcome = Some(PasswordOutcome::Cancel);
    }
    outcome
}

// ----- upgrading older files ----------------------------------------------------

/// What the person chose when offered protection for an unprotected file.
pub(crate) enum UpgradeChoice {
    SetPassword,
    NotNow,
}

/// Shown after opening a company file that has no password (made before
/// LedgerCraft encrypted files, or with its password removed).
pub(crate) fn upgrade_window(ctx: &egui::Context, file_name: &str) -> Option<UpgradeChoice> {
    let mut choice = None;
    let mut open = true;
    egui::Window::new("Protect This Company?")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label(format!(
                "{file_name} is not password protected, so anyone who gets a copy of it can \
                 read your books."
            ));
            ui.add_space(4.0);
            ui.label(
                "Set a password to upgrade it to an encrypted file. The file is re-saved \
                 encrypted straight away; nothing else about the books changes.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Set a Password…").strong()).clicked() {
                    choice = Some(UpgradeChoice::SetPassword);
                }
                if ui.button("Not Now").clicked() {
                    choice = Some(UpgradeChoice::NotNow);
                }
            });
            ui.label(
                RichText::new("You can do this any time with File > Password….")
                    .small()
                    .color(theme::MUTED),
            );
        });
    if !open {
        choice = Some(UpgradeChoice::NotNow);
    }
    choice
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_passwords_must_be_long_enough_and_match() {
        assert!(check_new("short", "short").is_err());
        assert!(check_new("long enough one", "long enough two").is_err());
        assert!(check_new("long enough", "long enough").is_ok());
    }
}
