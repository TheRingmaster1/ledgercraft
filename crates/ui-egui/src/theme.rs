//! LedgerCraft's own palette. Colours are defined here and nowhere else.

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

/// The text font, chosen in Help > Preferences > Text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Font {
    /// egui's built-in Ubuntu Light, as in LedgerCraft 0.2.
    #[default]
    Classic,
    Inter,
    InterLight,
}

impl Font {
    pub(crate) const ALL: [Self; 3] = [Self::Classic, Self::Inter, Self::InterLight];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic (Ubuntu Light)",
            Self::Inter => "Inter",
            Self::InterLight => "Inter Light",
        }
    }

    fn bytes(self) -> Option<&'static [u8]> {
        match self {
            Self::Classic => None,
            Self::Inter => Some(INTER),
            Self::InterLight => Some(INTER_LIGHT),
        }
    }
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
/// behind a chosen font for any character it lacks, and for monospace.
/// Rebuilds the font atlas, so call it only when the choice changes.
pub(crate) fn set_text(ctx: &egui::Context, font: Font, size: TextSize) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = font.bytes() {
        let name = "LedgerCraft text".to_owned();
        fonts
            .font_data
            .insert(name.clone(), egui::FontData::from_static(bytes));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, name);
    }
    ctx.set_fonts(fonts);
    let body = size.body();
    ctx.style_mut(|style| text_styles(style, body));
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
    let mut style = (*ctx.style()).clone();
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
    ctx.set_style(style);
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
        for font in Font::ALL {
            for size in TextSize::ALL {
                let ctx = egui::Context::default();
                apply(&ctx);
                set_text(&ctx, font, size);
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.label("Accounts Receivable $1,234.56 • € £ …");
                    });
                });
                let style = ctx.style();
                let body = style.text_styles.get(&egui::TextStyle::Body);
                assert_eq!(body.map(|f| f.size), Some(size.body()), "{font:?} {size:?}");
            }
        }
    }

    #[test]
    fn the_default_looks_like_0_2() {
        assert_eq!(Font::default(), Font::Classic);
        assert_eq!(TextSize::default().body(), 12.5);
    }
}
