//! What LedgerCraft remembers between launches that is not a preference:
//! the recently opened company files, the window size and the side bar.
//! Kept as `state.json` next to the preferences. A missing or damaged file
//! just means a fresh start.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::prefs;

const FILE_NAME: &str = "state.json";
/// How many company files the start page and File menu list.
pub(crate) const MAX_RECENT: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct AppState {
    /// Most recent first. Only paths: no company names or data, so the list
    /// says nothing about a protected company that its file name does not.
    pub(crate) recent: Vec<PathBuf>,
    /// The window's size when the app last closed, in ui points.
    pub(crate) window: Option<WindowState>,
    pub(crate) side_bar: bool,
    /// The Streamlined Sales Tax files last imported from, by state
    /// abbreviation (`UT`): (rate file, boundary file). Used to find a
    /// customer's tax group from their ZIP+4.
    pub(crate) sst_files: std::collections::BTreeMap<String, (PathBuf, PathBuf)>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recent: Vec::new(),
            window: None,
            side_bar: true,
            sst_files: std::collections::BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WindowState {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) maximized: bool,
}

impl WindowState {
    /// A size worth restoring: finite and not absurd.
    pub(crate) fn size(&self) -> Option<eframe::egui::Vec2> {
        let ok = |v: f32| v.is_finite() && (100.0..=20_000.0).contains(&v);
        if ok(self.width) && ok(self.height) {
            Some(eframe::egui::vec2(self.width, self.height))
        } else {
            None
        }
    }
}

impl AppState {
    /// Where the state is kept, if there is a home folder.
    pub(crate) fn path() -> Option<PathBuf> {
        Some(prefs::config_dir()?.join(FILE_NAME))
    }

    pub(crate) fn load_from(path: &Path) -> Self {
        let mut state: Self = prefs::read_json(path);
        state.recent.truncate(MAX_RECENT);
        state
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<(), String> {
        prefs::write_json(path, self).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Puts `path` at the top of the recent list (once), keeping the list short.
    pub(crate) fn remember(&mut self, path: &Path) {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.recent.retain(|p| !same_file(p, &path));
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }

    pub(crate) fn forget(&mut self, path: &Path) {
        self.recent.retain(|p| !same_file(p, path));
    }
}

/// Windows and macOS file names ignore case; Linux's do not.
fn same_file(a: &Path, b: &Path) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}

/// The company file's name without `.lcbooks`, and the folder it is in.
pub(crate) fn describe(path: &Path) -> (String, String) {
    let name = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let folder = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    (name, folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ledgercraft-state-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(FILE_NAME)
    }

    #[test]
    fn newest_first_without_duplicates_and_capped() {
        let dir = std::env::temp_dir();
        let mut state = AppState::default();
        for i in 0..MAX_RECENT + 3 {
            state.remember(&dir.join(format!("company {i}.lcbooks")));
        }
        assert_eq!(state.recent.len(), MAX_RECENT);
        let again = dir.join("company 5.lcbooks");
        state.remember(&again);
        assert_eq!(state.recent.len(), MAX_RECENT);
        assert_eq!(state.recent[0], again);
        assert_eq!(state.recent.iter().filter(|p| **p == again).count(), 1);
        state.forget(&again);
        assert!(!state.recent.contains(&again));
    }

    #[test]
    fn relative_paths_are_made_absolute() {
        let mut state = AppState::default();
        state.remember(Path::new("books.lcbooks"));
        assert!(state.recent[0].is_absolute());
    }

    #[test]
    fn round_trip_and_damaged_files() {
        let path = temp_file("round-trip");
        let mut state = AppState {
            window: Some(WindowState {
                width: 1400.0,
                height: 900.0,
                maximized: false,
            }),
            side_bar: false,
            ..AppState::default()
        };
        state.remember(&std::env::temp_dir().join("a.lcbooks"));
        state
            .sst_files
            .insert("UT".into(), (PathBuf::from("UTR.csv"), PathBuf::from("UTB.csv")));
        state.save_to(&path).unwrap();
        assert_eq!(AppState::load_from(&path), state);
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(AppState::load_from(&path), AppState::default());
        assert!(AppState::default().side_bar);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn silly_window_sizes_are_ignored() {
        let size = |width, height| {
            WindowState {
                width,
                height,
                maximized: false,
            }
            .size()
        };
        assert!(size(1280.0, 800.0).is_some());
        assert!(size(0.0, 800.0).is_none());
        assert!(size(f32::NAN, 800.0).is_none());
        assert!(size(1280.0, 1.0e9).is_none());
    }
}
