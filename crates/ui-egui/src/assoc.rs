//! Double-clicking a `.lcbooks` company file opens it in LedgerCraft.
//!
//! * Windows: Help > Preferences > Files registers the file type for the
//!   current user (under `HKCU\Software\Classes`, so no administrator rights
//!   are needed), using the `reg` tool that ships with Windows.
//! * Linux: `install.sh` from the release package registers a MIME type and
//!   the desktop entry.
//! * macOS: the app bundle will declare it (with the macOS package).

use eframe::egui;

/// Preferences > Files.
pub(crate) fn section(ui: &mut egui::Ui) {
    #[cfg(windows)]
    windows::section(ui);
    #[cfg(not(windows))]
    {
        use crate::theme;
        use eframe::egui::RichText;
        let text = if cfg!(target_os = "linux") {
            "Double-clicking a .lcbooks company file opens it in LedgerCraft once LedgerCraft \
             is installed with install.sh from the Linux release package."
        } else {
            "Double-clicking .lcbooks company files will open LedgerCraft once the macOS \
             package is available."
        };
        ui.label(RichText::new(text).color(theme::MUTED));
    }
}

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use eframe::egui::{self, RichText};

    use crate::theme;

    const CLASSES: &str = r"HKCU\Software\Classes";
    const PROG_ID: &str = "LedgerCraft.Company";
    /// Run `reg` without flashing a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub(super) fn section(ui: &mut egui::Ui) {
        let id = egui::Id::new("prefs_files_result");
        ui.label("Open .lcbooks company files in LedgerCraft when you double-click them.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .button("Use LedgerCraft for .lcbooks Files")
                .on_hover_text("Registers the file type for your Windows account only; no administrator rights needed")
                .clicked()
            {
                let result = match register() {
                    Ok(()) => (
                        true,
                        "Done. Double-clicking a .lcbooks file now opens it in LedgerCraft."
                            .to_string(),
                    ),
                    Err(e) => (false, format!("Could not set this up: {e}")),
                };
                ui.data_mut(|d| d.insert_temp(id, result));
            }
            if ui.button("Stop").clicked() {
                let result = match unregister() {
                    Ok(()) => (true, "LedgerCraft no longer opens .lcbooks files.".to_string()),
                    Err(e) => (false, format!("Could not undo this: {e}")),
                };
                ui.data_mut(|d| d.insert_temp(id, result));
            }
        });
        if let Some((ok, message)) = ui.data(|d| d.get_temp::<(bool, String)>(id)) {
            let colour = if ok { theme::POSITIVE } else { theme::NEGATIVE };
            ui.label(RichText::new(message).color(colour));
        }
        ui.label(
            RichText::new(
                "This points at the copy of LedgerCraft you are running now. If you move it, \
                 or build it somewhere else, click the button again.",
            )
            .small()
            .color(theme::MUTED),
        );
    }

    fn register() -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("could not find ledgercraft.exe ({e})"))?
            .display()
            .to_string();
        let ext = ledgercraft_file::EXTENSION;
        let entries = [
            (format!(r"{CLASSES}\.{ext}"), PROG_ID.to_string()),
            (
                format!(r"{CLASSES}\{PROG_ID}"),
                "LedgerCraft company file".to_string(),
            ),
            (
                format!(r"{CLASSES}\{PROG_ID}\DefaultIcon"),
                format!("\"{exe}\",0"),
            ),
            (
                format!(r"{CLASSES}\{PROG_ID}\shell\open\command"),
                format!("\"{exe}\" \"%1\""),
            ),
        ];
        for (key, value) in &entries {
            reg(&["add", key.as_str(), "/ve", "/d", value.as_str(), "/f"])?;
        }
        Ok(())
    }

    fn unregister() -> Result<(), String> {
        let ext_key = format!(r"{CLASSES}\.{}", ledgercraft_file::EXTENSION);
        let prog_key = format!(r"{CLASSES}\{PROG_ID}");
        for key in [&ext_key, &prog_key] {
            // Deleting a key that is not there fails; that is fine, as long
            // as the key is gone afterwards.
            let _ = reg(&["delete", key.as_str(), "/f"]);
            if reg(&["query", key.as_str()]).is_ok() {
                return Err(format!("{key} could not be removed"));
            }
        }
        Ok(())
    }

    fn reg(args: &[&str]) -> Result<(), String> {
        let out = Command::new("reg")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("could not run reg.exe ({e})"))?;
        if out.status.success() {
            Ok(())
        } else {
            let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if said.is_empty() {
                "reg.exe reported an error".to_string()
            } else {
                said
            })
        }
    }
}
