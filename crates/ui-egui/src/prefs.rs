//! Help > Preferences: look-and-feel settings that belong to the person, not
//! the company file. Kept as a small JSON file in the user's config folder
//! (`LEDGERCRAFT_CONFIG_DIR` overrides it). A missing or damaged file just
//! means the defaults.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};

use crate::theme;

const FILE_NAME: &str = "preferences.json";
/// Far more than any preferences file needs; anything bigger is not ours.
const MAX_BYTES: u64 = 64 * 1024;

/// Lightest and darkest shading offered, as grey levels (0 black, 255 white).
/// Darker than this and the black row text gets hard to read.
pub(crate) const STRIPE_GREY_RANGE: std::ops::RangeInclusive<u8> = 160..=250;
/// The default shading: the middle of the picker's range.
const DEFAULT_STRIPE_GREY: u8 =
    ((*STRIPE_GREY_RANGE.start() as u16 + *STRIPE_GREY_RANGE.end() as u16) / 2) as u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Prefs {
    /// Grey level behind every other row of every list in the app.
    pub(crate) stripe_grey: u8,
    /// Open the Tutorial window when the app starts. Cleared by its
    /// "Never show again" box; Help > Tutorial sets it again.
    pub(crate) show_tutorial: bool,
    /// Open the top menus (File, Edit, …) when the pointer rests on them,
    /// instead of on a click.
    pub(crate) hover_menus: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            stripe_grey: DEFAULT_STRIPE_GREY,
            show_tutorial: true,
            hover_menus: false,
        }
    }
}

impl Prefs {
    pub(crate) fn stripe(&self) -> Color32 {
        let grey = self
            .stripe_grey
            .clamp(*STRIPE_GREY_RANGE.start(), *STRIPE_GREY_RANGE.end());
        Color32::from_gray(grey)
    }

    /// Global's Reset To Default: the row shading and Open Menus On Hover go
    /// back to their defaults; the tutorial setting is left as it is.
    pub(crate) fn reset_global(&mut self) {
        let defaults = Self::default();
        self.stripe_grey = defaults.stripe_grey;
        self.hover_menus = defaults.hover_menus;
    }

    /// The saved preferences, or the defaults if there are none or they
    /// cannot be read.
    pub(crate) fn load() -> Self {
        path().map(|p| load_from(&p)).unwrap_or_default()
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        let path = path().ok_or("no home folder to keep preferences in")?;
        save_to(&path, self).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// `$LEDGERCRAFT_CONFIG_DIR/preferences.json`, else the platform's usual
/// per-user config folder.
fn path() -> Option<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if let Some(dir) = var("LEDGERCRAFT_CONFIG_DIR") {
        dir
    } else if cfg!(windows) {
        var("APPDATA")?.join("LedgerCraft")
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library/Application Support/LedgerCraft")
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".config")))?
            .join("ledgercraft")
    };
    Some(dir.join(FILE_NAME))
}

fn load_from(path: &Path) -> Prefs {
    let too_big = fs::metadata(path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(true);
    if too_big {
        return Prefs::default();
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes a temporary file next to the target and renames it over, so a crash
/// mid-write leaves the old preferences rather than half a file.
fn save_to(path: &Path, prefs: &Prefs) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(prefs).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    let written = fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(json.as_bytes())?;
        f.sync_all()
    });
    if let Err(e) = written.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// Width of the shaded preview rows in the Preferences window.
const PREVIEW_WIDTH: f32 = 280.0;

/// The Preferences window. Changes apply straight away; the app saves them.
pub(crate) fn window(ctx: &egui::Context, open: &mut bool, prefs: &mut Prefs) {
    egui::Window::new("Preferences")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            // Sections, styled like the selected side bar item. Only Global
            // for now; selecting it shows its settings, clicking again hides them.
            let id = egui::Id::new("prefs_global_selected");
            let mut global = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
            ui.scope(|ui| {
                ui.visuals_mut().selection.bg_fill = theme::ACCENT;
                let text = RichText::new("Global").size(15.0);
                let text = if global {
                    text.color(Color32::WHITE)
                } else {
                    text
                };
                if ui.selectable_label(global, text).clicked() {
                    global = !global;
                    ui.data_mut(|d| d.insert_temp(id, global));
                }
            });
            if global {
                ui.add_space(4.0);
                global_section(ui, prefs);
            }
        });
}

/// Global preferences: the row shading used by every list, with a preview.
fn global_section(ui: &mut egui::Ui, prefs: &mut Prefs) {
    ui.horizontal(|ui| {
        ui.label("Row Shading");
        grey_picker(ui, &mut prefs.stripe_grey);
    });
    ui.add_space(4.0);
    // A preview of how the shaded rows will look, in a 2 pt black box.
    egui::Frame::none()
        .stroke(egui::Stroke::new(2.0_f32, Color32::BLACK))
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.set_width(PREVIEW_WIDTH);
            for (i, label) in ["Checking", "Savings", "Customers owe you"]
                .into_iter()
                .enumerate()
            {
                let background = ui.painter().add(egui::Shape::Noop);
                let row = ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(label);
                });
                if i % 2 == 1 {
                    let rect = row.response.rect;
                    let rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(PREVIEW_WIDTH, rect.height()),
                    );
                    let shape = egui::Shape::rect_filled(
                        rect.expand2(egui::vec2(0.0, 2.0)),
                        2.0,
                        prefs.stripe(),
                    );
                    ui.painter().set(background, shape);
                }
            }
        });
    ui.add_space(6.0);
    ui.checkbox(&mut prefs.hover_menus, "Open Menus On Hover")
        .on_hover_text("Open File, Edit, View and the other top menus when the pointer rests on them, without clicking");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Reset To Default").clicked() {
            prefs.reset_global();
        }
        ui.label(
            RichText::new("Saved automatically.")
                .small()
                .color(theme::MUTED),
        );
    });
}

/// A horizontal light-to-dark grey bar: click or drag to pick a grey level.
fn grey_picker(ui: &mut egui::Ui, grey: &mut u8) {
    let (lo, hi) = (*STRIPE_GREY_RANGE.start(), *STRIPE_GREY_RANGE.end());
    let size = egui::vec2(180.0, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    // Left is lightest, right is darkest.
    let at = |t: f32| hi as f32 - t * (hi - lo) as f32;
    if let Some(pos) = response.interact_pointer_pos() {
        let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        *grey = at(t).round().clamp(lo as f32, hi as f32) as u8;
    }
    const STEPS: usize = 36;
    let step = rect.width() / STEPS as f32;
    for i in 0..STEPS {
        let x = rect.left() + i as f32 * step;
        let cell = egui::Rect::from_x_y_ranges(x..=x + step + 0.5, rect.y_range());
        let level = at((i as f32 + 0.5) / STEPS as f32).round() as u8;
        ui.painter()
            .rect_filled(cell, 0.0, Color32::from_gray(level));
    }
    ui.painter()
        .rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, theme::MUTED));
    // Marker at the current level.
    let t = (hi as f32 - (*grey).clamp(lo, hi) as f32) / (hi - lo) as f32;
    let x = rect.left() + t * rect.width();
    let marker = egui::Rect::from_x_y_ranges(x - 2.0..=x + 2.0, rect.y_range().expand(3.0));
    ui.painter().rect_filled(marker, 1.0, theme::ACCENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ledgercraft-prefs-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join(FILE_NAME)
    }

    #[test]
    fn round_trip_through_disk() {
        let path = temp_file("round-trip");
        let prefs = Prefs {
            stripe_grey: 190,
            show_tutorial: false,
            hover_menus: true,
        };
        save_to(&path, &prefs).unwrap();
        assert_eq!(load_from(&path), prefs);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn missing_or_damaged_file_gives_defaults() {
        let path = temp_file("damaged");
        assert_eq!(load_from(&path), Prefs::default());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load_from(&path), Prefs::default());
        fs::write(&path, "{\"stripe_grey\": 999}").unwrap();
        assert_eq!(load_from(&path), Prefs::default());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let path = temp_file("fields");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"future_setting\": true}").unwrap();
        assert_eq!(load_from(&path), Prefs::default());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn tutorial_shows_until_turned_off() {
        assert!(Prefs::default().show_tutorial);
        let path = temp_file("tutorial");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A file saved before the setting existed still shows the tutorial.
        fs::write(&path, "{\"stripe_grey\": 190}").unwrap();
        assert!(load_from(&path).show_tutorial);
        let off = Prefs {
            show_tutorial: false,
            ..Prefs::default()
        };
        save_to(&path, &off).unwrap();
        assert!(!load_from(&path).show_tutorial);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn hover_menus_are_off_by_default() {
        assert!(!Prefs::default().hover_menus);
        let path = temp_file("hover");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A file saved before the setting existed keeps click-to-open menus.
        fs::write(&path, "{\"stripe_grey\": 190, \"show_tutorial\": false}").unwrap();
        let loaded = load_from(&path);
        assert!(!loaded.hover_menus);
        assert!(!loaded.show_tutorial);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    // Regression: Reset To Default replaced every preference, so it also
    // turned the tutorial back on. It resets what Global shows, and only that.
    #[test]
    fn reset_global_resets_shading_and_hover_but_not_the_tutorial() {
        let mut prefs = Prefs {
            stripe_grey: 170,
            show_tutorial: false,
            hover_menus: true,
        };
        prefs.reset_global();
        assert_eq!(prefs.stripe_grey, Prefs::default().stripe_grey);
        assert!(!prefs.hover_menus);
        assert!(!prefs.show_tutorial);
    }

    #[test]
    fn default_is_the_middle_of_the_picker() {
        assert_eq!(Prefs::default().stripe_grey, 205);
    }

    #[test]
    fn stripe_stays_readable() {
        assert_eq!(
            Prefs {
                stripe_grey: 0,
                ..Prefs::default()
            }
            .stripe(),
            Color32::from_gray(*STRIPE_GREY_RANGE.start())
        );
        assert_eq!(
            Prefs {
                stripe_grey: 255,
                ..Prefs::default()
            }
            .stripe(),
            Color32::from_gray(*STRIPE_GREY_RANGE.end())
        );
    }
}
