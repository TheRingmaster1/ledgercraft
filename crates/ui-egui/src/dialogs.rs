//! The system's file dialogs, starting where you last were.
//!
//! Each kind of file has its own remembered folder (company files, exports,
//! PDFs, bank downloads…), kept with the recent companies in `state.json`.
//! A dialog opens in that folder, and the folder of whatever was chosen
//! becomes the new one.
//!
//! On Linux the dialogs are the desktop's own, through xdg-desktop-portal.
//! Without a portal no dialog appears at all, so when one is missing and a
//! dialog comes back empty, the app says what to install.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

/// The kinds of file with a remembered folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Folder {
    /// Company files (.lcbooks).
    Company,
    /// CSV and Excel exports.
    Export,
    /// Invoice, receipt and statement PDFs.
    Pdf,
    /// Bank and card downloads.
    BankDownload,
    /// IIF and CSV list files.
    ListImport,
    /// Sales tax rate and boundary files.
    TaxFiles,
    /// Logo images.
    Logo,
    /// Company file backups.
    Backup,
}

static FOLDERS: Mutex<BTreeMap<Folder, PathBuf>> = Mutex::new(BTreeMap::new());
static PROBLEM: Mutex<Option<String>> = Mutex::new(None);

/// Starts from the folders saved last time.
pub(crate) fn set_folders(folders: BTreeMap<Folder, PathBuf>) {
    *FOLDERS.lock().unwrap_or_else(PoisonError::into_inner) = folders;
}

/// The folders to save for next time.
pub(crate) fn folders() -> BTreeMap<Folder, PathBuf> {
    FOLDERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn remembered(folder: Folder) -> Option<PathBuf> {
    FOLDERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&folder)
        .filter(|dir| dir.is_dir())
        .cloned()
}

fn remember(folder: Folder, chosen: &Path) {
    if let Some(dir) = chosen.parent().filter(|d| !d.as_os_str().is_empty()) {
        FOLDERS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(folder, dir.to_path_buf());
    }
}

/// A message for the status bar when a file dialog could not be shown,
/// once.
pub(crate) fn take_problem() -> Option<String> {
    PROBLEM
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
}

/// After a dialog came back with nothing: if that was because there is no
/// dialog to show, say so.
fn check_nothing_chosen() {
    if let Some(message) = missing_dialog_message() {
        *PROBLEM.lock().unwrap_or_else(PoisonError::into_inner) = Some(message);
    }
}

/// A dialog starting in `folder`'s remembered folder, or `start` when
/// given (the open company's own folder, say).
fn dialog(folder: Folder, start: Option<&Path>) -> rfd::FileDialog {
    let dir = start
        .filter(|d| d.is_dir())
        .map(Path::to_path_buf)
        .or_else(|| remembered(folder));
    match dir {
        Some(dir) => rfd::FileDialog::new().set_directory(dir),
        None => rfd::FileDialog::new(),
    }
}

/// Asks for a file to open of the kinds in `filters` (`(name, extensions)`).
pub(crate) fn pick_file(
    folder: Folder,
    title: Option<&str>,
    filters: &[(&str, &[&str])],
) -> Option<PathBuf> {
    pick_file_in(folder, title, filters, None)
}

/// [`pick_file`], starting in `start` rather than the remembered folder.
pub(crate) fn pick_file_in(
    folder: Folder,
    title: Option<&str>,
    filters: &[(&str, &[&str])],
    start: Option<&Path>,
) -> Option<PathBuf> {
    let mut d = dialog(folder, start);
    if let Some(title) = title {
        d = d.set_title(title);
    }
    for (name, extensions) in filters {
        d = d.add_filter(*name, extensions);
    }
    let chosen = d.pick_file();
    match &chosen {
        Some(path) => remember(folder, path),
        None => check_nothing_chosen(),
    }
    chosen
}

/// Asks where to save a file named `name` of the kind in `filter`.
pub(crate) fn save_file(
    folder: Folder,
    filter: (&str, &[&str]),
    name: &str,
    start: Option<&Path>,
) -> Option<PathBuf> {
    let chosen = dialog(folder, start)
        .add_filter(filter.0, filter.1)
        .set_file_name(name)
        .save_file();
    match &chosen {
        Some(path) => remember(folder, path),
        None => check_nothing_chosen(),
    }
    chosen
}

/// Why no file dialog can be shown, if that is so: on Linux, no
/// xdg-desktop-portal with a file chooser.
fn missing_dialog_message() -> Option<String> {
    if cfg!(target_os = "linux") && !linux_file_chooser_found(&data_dirs()) {
        Some(
            "No file dialog could be shown: LedgerCraft uses your desktop's file chooser \
             (xdg-desktop-portal). Install xdg-desktop-portal and the one for your desktop \
             (xdg-desktop-portal-gtk for GNOME, Xfce, Cinnamon and most others; \
             xdg-desktop-portal-kde for Plasma), then log out and back in. Meanwhile a \
             company file opens from a terminal: ledgercraft path/to/books.lcbooks"
                .into(),
        )
    } else {
        None
    }
}

/// `$XDG_DATA_DIRS`, or its default.
fn data_dirs() -> Vec<PathBuf> {
    let dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.split(':')
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Whether a portal is installed (its D-Bus service) with a back end that
/// offers a file chooser.
fn linux_file_chooser_found(data_dirs: &[PathBuf]) -> bool {
    let service = data_dirs.iter().any(|d| {
        d.join("dbus-1/services/org.freedesktop.portal.Desktop.service")
            .is_file()
    });
    let chooser = data_dirs.iter().any(|d| {
        std::fs::read_dir(d.join("xdg-desktop-portal/portals"))
            .map(|entries| {
                entries.filter_map(Result::ok).any(|e| {
                    std::fs::read_to_string(e.path())
                        .is_ok_and(|text| text.contains("org.freedesktop.impl.portal.FileChooser"))
                })
            })
            .unwrap_or(false)
    });
    service && chooser
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chosen_folders_are_remembered_by_kind() {
        let dir = std::env::temp_dir();
        set_folders(BTreeMap::new());
        remember(Folder::Pdf, &dir.join("Invoice 1001.pdf"));
        assert_eq!(remembered(Folder::Pdf), Some(dir.clone()));
        assert_eq!(remembered(Folder::Export), None);
        // A folder that no longer exists is not offered.
        set_folders(BTreeMap::from([(
            Folder::Logo,
            dir.join("gone-away-folder"),
        )]));
        assert_eq!(remembered(Folder::Logo), None);
        set_folders(BTreeMap::new());
    }

    #[test]
    fn a_portal_needs_its_service_and_a_file_chooser() {
        let root = std::env::temp_dir().join(format!("lc-portal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dirs = vec![root.clone()];
        assert!(!linux_file_chooser_found(&dirs));
        std::fs::create_dir_all(root.join("dbus-1/services")).unwrap();
        std::fs::write(
            root.join("dbus-1/services/org.freedesktop.portal.Desktop.service"),
            "",
        )
        .unwrap();
        assert!(
            !linux_file_chooser_found(&dirs),
            "a service but no back end"
        );
        std::fs::create_dir_all(root.join("xdg-desktop-portal/portals")).unwrap();
        std::fs::write(
            root.join("xdg-desktop-portal/portals/gtk.portal"),
            "[portal]\nInterfaces=org.freedesktop.impl.portal.FileChooser;\n",
        )
        .unwrap();
        assert!(linux_file_chooser_found(&dirs));
        let _ = std::fs::remove_dir_all(&root);
    }
}
