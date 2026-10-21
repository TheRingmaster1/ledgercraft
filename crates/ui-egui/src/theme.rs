//! LedgerCraft's own palette. Colours are defined here and nowhere else.

use std::sync::Arc;

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};

pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x1f, 0x6f, 0x8b);
pub(crate) const ACCENT_SOFT: Color32 = Color32::from_rgb(0xd6, 0xea, 0xf1);
pub(crate) const NEGATIVE: Color32 = Color32::from_rgb(0xb0, 0x30, 0x30);
pub(crate) const POSITIVE: Color32 = Color32::from_rgb(0x2e, 0x7d, 0x4f);
/// Cautions that are not errors, such as a weak password.
pub(crate) const WARNING: Color32 = Color32::from_rgb(0xb9, 0x6a, 0x00);
/// The empty part of a meter.
pub(crate) const TRACK: Color32 = Color32::from_rgb(0xdd, 0xe1, 0xe6);
pub(crate) const MUTED: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);
pub(crate) const NAV_BG: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x36);
pub(crate) const NAV_TEXT: Color32 = Color32::from_rgb(0xe8, 0xee, 0xf2);
pub(crate) const NAV_HOVER: Color32 = Color32::from_rgb(0x2c, 0x40, 0x4f);

/// The side bar is dark, so its selected and hovered items need dark-friendly
/// backgrounds (the light theme's pale selection makes white text unreadable).
pub(crate) fn nav_visuals(ui: &mut egui::Ui) {
    let v = ui.visuals_mut();
    v.selection.bg_fill = ACCENT;
    v.widgets.hovered.weak_bg_fill = NAV_HOVER;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.weak_bg_fill = ACCENT;
}

/// Inter by Rasmus Andersson, Regular and Light. SIL Open Font License 1.1;
/// see assets/fonts/OFL.txt and ATTRIBUTION.md.
const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
const INTER_LIGHT: &[u8] = include_bytes!("../../../assets/fonts/Inter-Light.ttf");
/// The font's licence, which must travel with it (shown in Help > Licenses).
pub(crate) const INTER_LICENSE: &str = include_str!("../../../assets/fonts/OFL.txt");

/// The text font, chosen in Help > Preferences > Text: one LedgerCraft
/// bundles, or one already installed on this computer. Installed fonts are
/// read from their usual place at run time, never copied or bundled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Font {
    /// egui's built-in Ubuntu Light, as in LedgerCraft 0.2.
    Classic,
    Inter,
    #[default]
    InterLight,
    // Windows
    SegoeUi,
    SegoeUiSemilight,
    SegoeUiLight,
    Calibri,
    CalibriLight,
    Verdana,
    Tahoma,
    Arial,
    // Linux
    DejaVuSans,
    NotoSans,
    LiberationSans,
    Ubuntu,
    Cantarell,
}

/// Biggest font file LedgerCraft will read.
const MAX_FONT_BYTES: u64 = 50 * 1024 * 1024;

impl Font {
    /// Fonts that come with LedgerCraft, always available.
    pub(crate) const BUNDLED: [Self; 3] = [Self::Classic, Self::Inter, Self::InterLight];
    /// Fonts used when this computer has them installed.
    const SYSTEM: [Self; 13] = [
        Self::SegoeUi,
        Self::SegoeUiSemilight,
        Self::SegoeUiLight,
        Self::Calibri,
        Self::CalibriLight,
        Self::Verdana,
        Self::Tahoma,
        Self::Arial,
        Self::DejaVuSans,
        Self::NotoSans,
        Self::LiberationSans,
        Self::Ubuntu,
        Self::Cantarell,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic (Ubuntu Light)",
            Self::Inter => "Inter",
            Self::InterLight => "Inter Light",
            Self::SegoeUi => "Segoe UI",
            Self::SegoeUiSemilight => "Segoe UI Semilight",
            Self::SegoeUiLight => "Segoe UI Light",
            Self::Calibri => "Calibri",
            Self::CalibriLight => "Calibri Light",
            Self::Verdana => "Verdana",
            Self::Tahoma => "Tahoma",
            Self::Arial => "Arial",
            Self::DejaVuSans => "DejaVu Sans",
            Self::NotoSans => "Noto Sans",
            Self::LiberationSans => "Liberation Sans",
            Self::Ubuntu => "Ubuntu",
            Self::Cantarell => "Cantarell",
        }
    }

    /// Where an installed font may be, on the systems that have it.
    fn system_files(self) -> Vec<std::path::PathBuf> {
        use std::path::PathBuf;
        let windows = |file: &str| -> Vec<PathBuf> {
            windows_fonts_dir()
                .map(|dir| dir.join(file))
                .into_iter()
                .collect()
        };
        let linux = |paths: &[&str]| -> Vec<PathBuf> {
            if cfg!(target_os = "linux") {
                paths.iter().map(PathBuf::from).collect()
            } else {
                Vec::new()
            }
        };
        match self {
            Self::Classic | Self::Inter | Self::InterLight => Vec::new(),
            Self::SegoeUi => windows("segoeui.ttf"),
            Self::SegoeUiSemilight => windows("segoeuisl.ttf"),
            Self::SegoeUiLight => windows("segoeuil.ttf"),
            Self::Calibri => windows("calibri.ttf"),
            Self::CalibriLight => windows("calibril.ttf"),
            Self::Verdana => windows("verdana.ttf"),
            Self::Tahoma => windows("tahoma.ttf"),
            Self::Arial => windows("arial.ttf"),
            Self::DejaVuSans => linux(&[
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
                "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            ]),
            Self::NotoSans => linux(&[
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/google-noto-sans-fonts/NotoSans-Regular.ttf",
            ]),
            Self::LiberationSans => linux(&[
                "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
                "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf",
            ]),
            Self::Ubuntu => linux(&[
                "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
                "/usr/share/fonts/ubuntu/Ubuntu-R.ttf",
                "/usr/share/fonts/TTF/Ubuntu-R.ttf",
            ]),
            Self::Cantarell => linux(&[
                "/usr/share/fonts/cantarell/Cantarell-VF.otf",
                "/usr/share/fonts/opentype/cantarell/Cantarell-VF.otf",
                "/usr/share/fonts/abattis-cantarell-vf-fonts/Cantarell-VF.otf",
            ]),
        }
    }

    /// The font file to read, if this font is installed here.
    fn installed_file(self) -> Option<std::path::PathBuf> {
        self.system_files().into_iter().find(|p| p.is_file())
    }

    /// The name its data is registered under in egui.
    fn key(self) -> String {
        format!("ledgercraft-{self:?}")
    }

    /// The egui family that draws text in just this font (with egui's fonts
    /// behind it), registered by [`set_text`].
    pub(crate) fn family(self) -> egui::FontFamily {
        egui::FontFamily::Name(self.key().into())
    }

    /// Whether this font can be used on this computer.
    pub(crate) fn is_available(self) -> bool {
        Self::BUNDLED.contains(&self) || installed().contains(&self)
    }

    /// The font to hand egui; `None` for Classic (egui's own) or a font that
    /// cannot be found or read here.
    fn data(self) -> Option<Arc<egui::FontData>> {
        match self {
            Self::Classic => None,
            Self::Inter => Some(Arc::new(egui::FontData::from_static(INTER))),
            Self::InterLight => Some(Arc::new(egui::FontData::from_static(INTER_LIGHT))),
            _ => {
                let bytes = read_font(&self.installed_file()?)?;
                Some(Arc::new(egui::FontData::from_owned(bytes)))
            }
        }
    }
}

/// The installed fonts found on this computer, looked up once.
pub(crate) fn installed() -> &'static [Font] {
    static FOUND: std::sync::OnceLock<Vec<Font>> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| {
        // Read each one once: a font is offered only if egui can draw it.
        Font::SYSTEM
            .into_iter()
            .filter(|f| f.installed_file().as_deref().and_then(read_font).is_some())
            .collect()
    })
}

/// `C:\Windows\Fonts` (from WINDIR) on Windows; `None` elsewhere.
fn windows_fonts_dir() -> Option<std::path::PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let root = std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
    Some(root.join("Fonts"))
}

/// Reads an installed font file, if it is a sensible size and a font egui
/// can use. egui stops the app on a font it cannot read, so check first.
fn read_font(path: &std::path::Path) -> Option<Vec<u8>> {
    let size = std::fs::metadata(path).ok()?.len();
    if size > MAX_FONT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    usable(&bytes).then_some(bytes)
}

/// Tables holding embedded bitmap or colour glyphs. egui 0.37 draws those
/// glyphs as images in their own colour, not the text colour, so a font that
/// has them (such as Windows' regular Calibri) shows letters that are white
/// or missing. Such fonts are not offered until egui handles them.
const IMAGE_GLYPH_TABLES: [&[u8; 4]; 4] = [b"EBDT", b"CBDT", b"sbix", b"COLR"];

/// Whether egui's font reader (skrifa) accepts these bytes and draws every
/// glyph as an outline in the text colour.
fn usable(bytes: &[u8]) -> bool {
    let Ok(font) = skrifa::FontRef::new(bytes) else {
        return false;
    };
    IMAGE_GLYPH_TABLES
        .iter()
        .all(|tag| font.table_data(skrifa::raw::types::Tag::new(tag)).is_none())
}

/// Text darkness, as a grey level for ordinary text: 0 is black (as in
/// LedgerCraft 0.2), higher is softer. egui has no font hinting to adjust,
/// so softening the colour is what thins heavy fonts such as Verdana.
pub(crate) const TEXT_GREY_RANGE: std::ops::RangeInclusive<u8> = 0..=110;

/// The colour of ordinary text for a darkness setting, kept in range so
/// text always stays readable on the light background.
pub(crate) fn text_colour(grey: u8) -> Color32 {
    Color32::from_gray(grey.clamp(*TEXT_GREY_RANGE.start(), *TEXT_GREY_RANGE.end()))
}

/// How big text is, chosen in Help > Preferences > Text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TextSize {
    /// egui's own sizes, as in LedgerCraft 0.2.
    #[default]
    Standard,
    Larger,
    Largest,
}

impl TextSize {
    pub(crate) const ALL: [Self; 3] = [Self::Standard, Self::Larger, Self::Largest];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Larger => "Larger",
            Self::Largest => "Largest",
        }
    }

    /// Body text size in points.
    fn body(self) -> f32 {
        match self {
            Self::Standard => 12.5,
            Self::Larger => 13.5,
            Self::Largest => 14.5,
        }
    }
}

/// Sets the text font and size for the whole app. egui's own fonts stay
/// behind a chosen font for any character it lacks, and for monospace. A
/// font that cannot be found or read falls back to Classic. Rebuilds the
/// font atlas, so call it only when the choice changes.
///
/// Every font on offer also gets its own named family ([`Font::family`]), so
/// the font list in Preferences can show each name in its own font. egui
/// stops the app if text asks for a family that is not registered, so the
/// families are always registered (one that cannot be read is backed by
/// Classic), and [`apply`] registers them before the first frame.
pub(crate) fn set_text(ctx: &egui::Context, font: Font, size: TextSize) {
    let mut fonts = egui::FontDefinitions::default();
    let fallback: Vec<String> = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut chosen = None;
    for offered in Font::BUNDLED.iter().chain(installed()).copied() {
        let mut family = fallback.clone();
        if let Some(data) = offered.data() {
            let key = offered.key();
            fonts.font_data.insert(key.clone(), data);
            family.insert(0, key.clone());
            if offered == font {
                chosen = Some(key);
            }
        }
        fonts.families.insert(offered.family(), family);
    }
    if let Some(key) = chosen {
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, key);
    }
    ctx.set_fonts(fonts);
    let body = size.body();
    ctx.global_style_mut(|style| text_styles(style, body));
}

/// Text sizes from the body size. At Standard these are egui's defaults
/// (small 9, body 12.5, heading 18, monospace 12).
fn text_styles(style: &mut egui::Style, body: f32) {
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(body - 3.5, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(body, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(body, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(body + 5.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(body - 0.5, FontFamily::Monospace),
        ),
    ]
    .into();
}

pub(crate) fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    let mut v = egui::Visuals::light();
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke.color = ACCENT;
    v.hyperlink_color = ACCENT;
    // One dark text colour everywhere; text with its own colour (side bar,
    // links, muted hints, negative amounts) keeps it.
    v.override_text_color = Some(v.strong_text_color());
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    ctx.set_global_style(style);
    // The default text, and every font's own family, before the first frame.
    set_text(ctx, Font::default(), TextSize::default());
}

/// Runs one headless frame for a test. A real app hands the frame's font
/// texture updates to the GPU; a test has none, so it clears them (egui 0.36+
/// stops on updates that are dropped unhandled).
#[cfg(test)]
pub(crate) fn test_frame(ctx: &egui::Context, add_contents: impl FnMut(&mut egui::Ui)) {
    let mut output = ctx.run_ui(egui::RawInput::default(), add_contents);
    output.textures_delta.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inter_fonts_are_truetype_with_their_licence() {
        // TrueType outlines start with version 1.0 (00 01 00 00).
        for bytes in [INTER, INTER_LIGHT] {
            assert_eq!(bytes.get(..4), Some(&[0, 1, 0, 0][..]));
        }
        assert!(INTER_LICENSE.contains("SIL OPEN FONT LICENSE"));
    }

    #[test]
    fn every_font_and_size_lays_out_text() {
        for font in Font::BUNDLED {
            for size in TextSize::ALL {
                let ctx = egui::Context::default();
                apply(&ctx);
                set_text(&ctx, font, size);
                test_frame(&ctx, |ui| {
                    ui.label("Accounts Receivable $1,234.56 • € £ …");
                });
                let style = ctx.global_style();
                let body = style.text_styles.get(&egui::TextStyle::Body);
                assert_eq!(body.map(|f| f.size), Some(size.body()), "{font:?} {size:?}");
            }
        }
    }

    #[test]
    fn every_offered_font_has_a_family_to_show_its_name_in() {
        let ctx = egui::Context::default();
        apply(&ctx);
        test_frame(&ctx, |ui| {
            for font in Font::BUNDLED.iter().chain(installed()) {
                ui.label(egui::RichText::new(font.label()).family(font.family()));
            }
        });
    }

    #[test]
    fn text_darkness_stays_readable() {
        assert_eq!(text_colour(0), Color32::BLACK);
        assert_eq!(text_colour(200), Color32::from_gray(*TEXT_GREY_RANGE.end()));
    }

    #[test]
    fn the_default_is_inter_light_at_standard_size() {
        assert_eq!(Font::default(), Font::InterLight);
        assert_eq!(TextSize::default().body(), 12.5);
    }

    #[test]
    fn only_real_fonts_are_used() {
        // Inter is outlines only, so it passes the image-glyph check too.
        assert!(usable(INTER));
        assert!(usable(INTER_LIGHT));
        assert!(!usable(b"not a font at all"));
        assert!(!usable(&[]));
        let missing = std::env::temp_dir().join("ledgercraft-no-such-font.ttf");
        assert!(read_font(&missing).is_none());
    }

    #[test]
    fn installed_fonts_are_real_and_bundled_ones_always_available() {
        for font in Font::BUNDLED {
            assert!(font.is_available());
        }
        // Whatever this computer has must load (or the test machine has none).
        for font in installed() {
            assert!(
                font.data().is_some(),
                "{font:?} was listed but cannot be read"
            );
            let ctx = egui::Context::default();
            set_text(&ctx, *font, TextSize::Standard);
            test_frame(&ctx, |ui| {
                ui.label("Accounts Receivable $1,234.56");
            });
        }
    }
}
