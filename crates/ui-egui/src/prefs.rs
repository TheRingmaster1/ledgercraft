//! Help > Preferences: look-and-feel settings that belong to the person, not
//! the company file. Kept as a small JSON file in the user's config folder
//! (`LEDGERCRAFT_CONFIG_DIR` overrides it). A missing or damaged file just
//! means the defaults.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::currency::Currency;
use crate::theme::{self, Font, TextSize};

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
    /// How amounts are shown ($, €, £, …). Display only; nothing is converted.
    pub(crate) currency: Currency,
    /// Open the company that was open last when the app starts.
    pub(crate) reopen_last: bool,
    /// The text font and size.
    pub(crate) font: Font,
    pub(crate) text_size: TextSize,
    /// How dark ordinary text is: a grey level, 0 = black.
    pub(crate) text_grey: u8,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            stripe_grey: DEFAULT_STRIPE_GREY,
            show_tutorial: true,
            hover_menus: false,
            currency: Currency::Usd,
            reopen_last: true,
            font: Font::default(),
            text_size: TextSize::Standard,
            text_grey: 0,
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

    /// General's Reset To Default: the row shading, Open Menus On Hover and
    /// Open The Last Company go back to their defaults; the tutorial setting
    /// is left as it is.
    pub(crate) fn reset_global(&mut self) {
        let defaults = Self::default();
        self.stripe_grey = defaults.stripe_grey;
        self.hover_menus = defaults.hover_menus;
        self.reopen_last = defaults.reopen_last;
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
    Some(config_dir()?.join(FILE_NAME))
}

/// `$LEDGERCRAFT_CONFIG_DIR`, else the platform's usual per-user config
/// folder: where LedgerCraft keeps its small per-user files.
pub(crate) fn config_dir() -> Option<PathBuf> {
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
    Some(dir)
}

fn load_from(path: &Path) -> Prefs {
    read_json(path)
}

fn save_to(path: &Path, prefs: &Prefs) -> std::io::Result<()> {
    write_json(path, prefs)
}

/// Reads a small JSON settings file; the defaults if it is missing, too big
/// or cannot be read.
pub(crate) fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    let too_big = fs::metadata(path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(true);
    if too_big {
        return T::default();
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes a temporary file next to the target and renames it over, so a crash
/// mid-write leaves the old file rather than half a file.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
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
/// A page of Help > Preferences, listed down the left of the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PrefsPage {
    #[default]
    General,
    Currency,
    Text,
    Files,
}

impl PrefsPage {
    const ALL: [Self; 4] = [Self::General, Self::Currency, Self::Text, Self::Files];

    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Currency => "Currency",
            Self::Text => "Text",
            Self::Files => "Files",
        }
    }

    /// The line under the page title.
    fn summary(self) -> &'static str {
        match self {
            Self::General => "Row shading, menus and what happens at startup.",
            Self::Currency => "How amounts are shown.",
            Self::Text => "The font and size of all text.",
            Self::Files => "Opening company files by double-clicking them.",
        }
    }
}

/// Size of the Preferences window's content. Fixed, so new settings go on
/// a new page instead of making the window ever taller.
const WINDOW_SIZE: egui::Vec2 = egui::vec2(680.0, 420.0);
/// Width of the page list on the left, and the space between it and the page.
const LIST_WIDTH: f32 = 160.0;
const PAGE_GAP: f32 = 16.0;
/// Where the open page is remembered for the session.
const PAGE_ID: &str = "prefs_page";

/// The Preferences window: pages listed on the left, the open page's
/// settings on the right (scrolling if they ever need to). Changes apply
/// straight away; the app saves them.
pub(crate) fn window(ctx: &egui::Context, open: &mut bool, prefs: &mut Prefs) {
    let id = egui::Id::new(PAGE_ID);
    let mut page = ctx
        .data(|d| d.get_temp::<PrefsPage>(id))
        .unwrap_or_default();
    egui::Window::new("Preferences")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .fixed_size(WINDOW_SIZE)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                page_list(ui, &mut page);
                ui.add_space(PAGE_GAP);
                ui.vertical(|ui| {
                    // The settings side takes the rest of the window.
                    ui.set_width(WINDOW_SIZE.x - LIST_WIDTH - PAGE_GAP);
                    ui.label(RichText::new(page.label()).heading().color(theme::ACCENT));
                    ui.label(RichText::new(page.summary()).small().color(theme::MUTED));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("prefs_scroll")
                        .auto_shrink(false)
                        .show(ui, |ui| match page {
                            PrefsPage::General => general_section(ui, prefs),
                            PrefsPage::Currency => currency_section(ui, prefs),
                            PrefsPage::Text => text_section(ui, prefs),
                            PrefsPage::Files => crate::assoc::section(ui),
                        });
                });
            });
        });
    ctx.data_mut(|d| d.insert_temp(id, page));
}

/// The page list, styled like the app's side bar: pages stacked top to
/// bottom in a column of fixed width.
fn page_list(ui: &mut egui::Ui, page: &mut PrefsPage) {
    let margin = 8.0;
    let inner = egui::vec2(LIST_WIDTH - 2.0 * margin, WINDOW_SIZE.y - 2.0 * margin);
    egui::Frame::none()
        .fill(theme::NAV_BG)
        .rounding(4.0)
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                inner,
                egui::Layout::top_down_justified(egui::Align::Center),
                |ui| {
                    ui.set_min_size(inner);
                    theme::nav_visuals(ui);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for p in PrefsPage::ALL {
                        let text = RichText::new(p.label()).size(15.0).color(theme::NAV_TEXT);
                        let item = ui.add_sized(
                            [inner.x, 34.0],
                            egui::SelectableLabel::new(*page == p, text),
                        );
                        if item.clicked() {
                            *page = p;
                        }
                    }
                },
            );
        });
}

/// The text font and size, applied to the whole app straight away.
fn text_section(ui: &mut egui::Ui, prefs: &mut Prefs) {
    egui::Grid::new("prefs_text")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Font");
            let shown = if prefs.font.is_available() {
                prefs.font.label().to_string()
            } else {
                // Chosen on another computer: Classic is used here instead.
                format!("{} (not on this computer)", prefs.font.label())
            };
            // Each font's name is shown in that font. Only fonts on offer here
            // have a family registered; egui stops on an unregistered one.
            let in_font = |font: Font, text: String| {
                if font.is_available() {
                    RichText::new(text).family(font.family())
                } else {
                    RichText::new(text)
                }
            };
            egui::ComboBox::from_id_salt("prefs_font")
                .selected_text(in_font(prefs.font, shown))
                .width(220.0)
                .height(360.0)
                .show_ui(ui, |ui| {
                    ui.label(
                        RichText::new("Included with LedgerCraft")
                            .small()
                            .color(theme::MUTED),
                    );
                    for font in Font::BUNDLED {
                        let name = in_font(font, font.label().to_string());
                        ui.selectable_value(&mut prefs.font, font, name);
                    }
                    let installed = theme::installed();
                    if !installed.is_empty() {
                        ui.separator();
                        ui.label(
                            RichText::new("On this computer")
                                .small()
                                .color(theme::MUTED),
                        );
                        for font in installed {
                            let name = in_font(*font, font.label().to_string());
                            ui.selectable_value(&mut prefs.font, *font, name);
                        }
                    }
                });
            ui.end_row();
            ui.label("Size");
            ui.horizontal(|ui| {
                for size in TextSize::ALL {
                    ui.radio_value(&mut prefs.text_size, size, size.label());
                }
            });
            ui.end_row();
            ui.label("Darkness").on_hover_text(
                "Softer text makes heavy fonts such as Verdana, Tahoma and Arial look lighter.",
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("Darker").small().color(theme::MUTED));
                ui.add(
                    egui::Slider::new(&mut prefs.text_grey, theme::TEXT_GREY_RANGE)
                        .show_value(false),
                );
                ui.label(RichText::new("Softer").small().color(theme::MUTED));
                if ui
                    .add_enabled(prefs.text_grey != 0, egui::Button::new("Default"))
                    .clicked()
                {
                    prefs.text_grey = 0;
                }
            });
            ui.end_row();
        });
    ui.add_space(4.0);
    egui::Frame::none()
        .stroke(egui::Stroke::new(1.0_f32, theme::MUTED))
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.set_width(PREVIEW_WIDTH);
            ui.label("Invoice 1001 for Pine Ridge Woodworks: $1,234.56");
            ui.label(RichText::new("Due in 30 days.").small().color(theme::MUTED));
        });
    ui.label(
        RichText::new(
            "Changes apply straight away. Fonts under \"On this computer\" are read from \
             your system, not included with LedgerCraft. Classic at Standard size is how \
             LedgerCraft 0.2 looked.",
        )
        .small()
        .color(theme::MUTED),
    );
}

/// The currency amounts are shown in, with a preview.
fn currency_section(ui: &mut egui::Ui, prefs: &mut Prefs) {
    ui.horizontal(|ui| {
        ui.label("Show amounts in");
        egui::ComboBox::from_id_salt("prefs_currency")
            .selected_text(prefs.currency.label())
            .width(230.0)
            .show_ui(ui, |ui| {
                for c in Currency::ALL {
                    ui.selectable_value(&mut prefs.currency, c, c.label());
                }
            });
    });
    let symbol = prefs.currency.symbol();
    let sample = |cents: i64| ledgercraft_core::Money::from_cents(cents).format_with(symbol);
    ui.label(format!(
        "Example: {}   and a negative amount: {}",
        sample(123_456),
        sample(-8_910)
    ));
    ui.label(
        RichText::new(
            "This changes how amounts are shown, in the app and in Excel exports. It does \
             not convert amounts between currencies: choose the currency your books are \
             kept in.",
        )
        .small()
        .color(theme::MUTED),
    );
}

/// General preferences: the row shading used by every list (with a
/// preview), Open Menus On Hover and Open The Last Company At Startup.
fn general_section(ui: &mut egui::Ui, prefs: &mut Prefs) {
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
    ui.checkbox(&mut prefs.reopen_last, "Open The Last Company At Startup")
        .on_hover_text("When LedgerCraft starts, open the company that was open last (asking for its password if it has one)");
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
            currency: Currency::Eur,
            reopen_last: false,
            font: Font::InterLight,
            text_size: TextSize::Larger,
            text_grey: 60,
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
    fn currency_defaults_to_dollars_for_older_files() {
        let path = temp_file("currency");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"stripe_grey\": 190}").unwrap();
        assert_eq!(load_from(&path).currency, Currency::Usd);
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
    // turned the tutorial back on. It resets what General shows, and only that.
    #[test]
    fn reset_global_resets_shading_and_hover_but_not_the_tutorial() {
        let mut prefs = Prefs {
            stripe_grey: 170,
            show_tutorial: false,
            hover_menus: true,
            currency: Currency::Gbp,
            reopen_last: false,
            font: Font::Inter,
            text_size: TextSize::Largest,
            text_grey: 90,
        };
        prefs.reset_global();
        // The currency is not a General setting, so Reset leaves it.
        assert_eq!(prefs.currency, Currency::Gbp);
        assert_eq!(prefs.stripe_grey, Prefs::default().stripe_grey);
        assert!(!prefs.hover_menus);
        assert!(prefs.reopen_last);
        assert!(!prefs.show_tutorial);
    }

    #[test]
    fn older_files_get_the_default_text() {
        let path = temp_file("text");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"stripe_grey\": 190}").unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.font, Font::InterLight);
        assert_eq!(loaded.text_size, TextSize::Standard);
        assert_eq!(loaded.text_grey, 0, "older files keep black text");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn older_files_reopen_the_last_company() {
        let path = temp_file("reopen");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"stripe_grey\": 190}").unwrap();
        assert!(load_from(&path).reopen_last);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn every_page_draws_in_the_fixed_window() {
        let labels: std::collections::BTreeSet<_> =
            PrefsPage::ALL.iter().map(|p| p.label()).collect();
        assert_eq!(labels.len(), PrefsPage::ALL.len());
        for page in PrefsPage::ALL {
            let ctx = egui::Context::default();
            // As the app does at start: fonts (and each font's family) first.
            theme::apply(&ctx);
            ctx.data_mut(|d| d.insert_temp(egui::Id::new(PAGE_ID), page));
            let mut open = true;
            let mut prefs = Prefs::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                window(ctx, &mut open, &mut prefs);
            });
            assert!(open);
            assert_eq!(
                prefs,
                Prefs::default(),
                "{page:?} changed a setting by drawing"
            );
        }
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
