//! Password windows: unlocking a protected company file, and File >
//! Password… for setting, changing or removing a company's password.
//! Typed passwords are wiped from memory when these windows close.

use std::path::PathBuf;

use eframe::egui::{self, RichText};
use ledgercraft_engine::{EngineError, Session};
use ledgercraft_file::{check_new_password, FileError, MIN_PASSWORD_CHARS};
use zeroize::{Zeroize, Zeroizing};

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
        egui::TextEdit::singleline(text)
            .password(true)
            .hint_text(hint),
    )
}

/// The lowest strength accepted for a new password, on zxcvbn's 0 to 4
/// scale: 3 means "safely unguessable" (more than 10^8 guesses), which, with
/// Argon2id slowing every guess down, makes guessing the password impractical.
pub(crate) const MIN_STRENGTH: u8 = 3;

/// Longest prefix the strength estimate looks at: past this, a password is
/// strong anyway, and the estimate's work grows with the length.
const STRENGTH_MAX_CHARS: usize = 64;

/// How hard `password` is to guess, 0 (trivial) to 4 (very hard), or `None`
/// when nothing has been typed. Common passwords, names, dictionary words,
/// dates, keyboard runs and the `context` words (such as the company name)
/// all count against it.
pub(crate) fn strength(password: &str, context: &[&str]) -> Option<u8> {
    if password.is_empty() {
        return None;
    }
    let head: Zeroizing<String> =
        Zeroizing::new(password.chars().take(STRENGTH_MAX_CHARS).collect());
    Some(u8::from(zxcvbn::zxcvbn(&head, context).score()))
}

/// Checks a new password and its confirmation. `context` is words the
/// password should not lean on, such as the company name.
pub(crate) fn check_new(password: &str, confirm: &str, context: &[&str]) -> Result<(), String> {
    check_new_password(password)?;
    if strength(password, context).unwrap_or(0) < MIN_STRENGTH {
        return Err(
            "That password is too easy to guess. Try four or more unrelated words, \
                    or make it longer and less predictable."
                .into(),
        );
    }
    if password != confirm {
        return Err("The two passwords do not match.".into());
    }
    Ok(())
}

/// A four-part bar and a word for how strong the new password is. Nothing
/// is shown until something has been typed.
pub(crate) fn strength_meter(ui: &mut egui::Ui, strength: Option<u8>) {
    let Some(score) = strength else {
        return;
    };
    let (word, colour) = match score {
        0 | 1 => ("Too easy to guess", theme::NEGATIVE),
        2 => ("Weak", theme::WARNING),
        3 => ("Good", theme::POSITIVE),
        _ => ("Strong", theme::POSITIVE),
    };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(160.0, 8.0), egui::Sense::hover());
        let gap = 3.0;
        let part = (rect.width() - 3.0 * gap) / 4.0;
        for i in 0..4_u8 {
            let left = rect.left() + f32::from(i) * (part + gap);
            let piece = egui::Rect::from_min_size(
                egui::pos2(left, rect.top()),
                egui::vec2(part, rect.height()),
            );
            // Even the weakest shows one red part, so the bar never looks empty.
            let fill = if i < score.max(1) {
                colour
            } else {
                theme::TRACK
            };
            ui.painter().rect_filled(piece, 2.0, fill);
        }
        ui.label(RichText::new(word).small().color(colour));
    });
}

/// The passphrase tip shown wherever a password is set.
pub(crate) fn passphrase_tip(ui: &mut egui::Ui) {
    ui.label(
        RichText::new(format!(
            "At least {MIN_PASSWORD_CHARS} characters. Tip: four or more unrelated words \
             (\"maple trumpet orbit gravel\") are strong and easy to remember."
        ))
        .small()
        .color(theme::MUTED),
    );
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
        Self {
            path,
            password: String::new(),
            error: None,
            focused: false,
        }
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
    /// How strong `new` is, worked out when it changes.
    strength: Option<u8>,
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
    let company_name = session.company().info.name.as_str();
    let title = if protected {
        "Change Password"
    } else {
        "Set a Password"
    };
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
            egui::Grid::new("password_form")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    if protected {
                        ui.label("Current password");
                        password_field(ui, &mut form.current, "");
                        ui.end_row();
                    }
                    if !form.remove {
                        ui.label("New password");
                        let typed = password_field(
                            ui,
                            &mut form.new,
                            &format!("at least {MIN_PASSWORD_CHARS} characters"),
                        );
                        if typed.changed() {
                            form.strength = strength(&form.new, &[company_name]);
                        }
                        ui.end_row();
                        ui.label("");
                        strength_meter(ui, form.strength);
                        ui.end_row();
                        ui.label("Confirm");
                        password_field(ui, &mut form.confirm, "type it again");
                        ui.end_row();
                    }
                });
            if protected {
                ui.checkbox(
                    &mut form.remove,
                    "Remove the password (the file will no longer be encrypted)",
                );
            }
            ui.add_space(4.0);
            if !form.remove {
                passphrase_tip(ui);
                forgotten_password_note(ui);
            }
            if let Some(error) = &form.error {
                ui.label(RichText::new(error.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let label = if form.remove {
                    "Remove Password"
                } else {
                    "Save Password"
                };
                if ui.button(RichText::new(label).strong()).clicked() {
                    form.error = None;
                    if protected && !opens_saved_file(session, &form.current) {
                        form.error = Some("The current password is not correct.".into());
                    } else if form.remove {
                        outcome = Some(PasswordOutcome::Apply(None));
                    } else {
                        match check_new(&form.new, &form.confirm, &[company_name]) {
                            Ok(()) => {
                                outcome = Some(PasswordOutcome::Apply(Some(form.new.clone())))
                            }
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
                if ui
                    .button(RichText::new("Set a Password…").strong())
                    .clicked()
                {
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

    const STRONG: &str = "Tq7#vL9!pWz2@kR4";

    #[test]
    fn new_passwords_must_be_long_enough_and_match() {
        assert!(
            check_new("Tq7#vL9!pW", "Tq7#vL9!pW", &[]).is_err(),
            "too short"
        );
        assert!(
            check_new(STRONG, "Tq7#vL9!pWz2@kR5", &[]).is_err(),
            "mismatch"
        );
        assert!(check_new(STRONG, STRONG, &[]).is_ok());
    }

    #[test]
    fn common_and_patterned_passwords_are_refused() {
        for weak in [
            "password1234",
            "aaaaaaaaaaaa",
            "qwertyuiop12",
            "123456789012",
        ] {
            assert!(check_new(weak, weak, &[]).is_err(), "{weak} was accepted");
            assert!(strength(weak, &[]).unwrap_or(0) < MIN_STRENGTH, "{weak}");
        }
    }

    #[test]
    fn strength_is_none_until_typed_and_high_for_random_passwords() {
        assert_eq!(strength("", &[]), None);
        assert_eq!(strength(STRONG, &[]), Some(4));
        // Very long input is fine and still strong.
        let long = STRONG.repeat(20);
        assert!(strength(&long, &[]).unwrap_or(0) >= MIN_STRENGTH);
    }
}
