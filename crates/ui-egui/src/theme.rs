//! LedgerCraft's own palette. Colours are defined here and nowhere else.

use eframe::egui::{self, Color32};

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

/// Inter, the text font, by Rasmus Andersson. SIL Open Font License 1.1;
/// see assets/fonts/OFL.txt and ATTRIBUTION.md.
const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
/// The font's licence, which must travel with it (shown in Help > Licenses).
pub(crate) const INTER_LICENSE: &str = include_str!("../../../assets/fonts/OFL.txt");

/// Inter for all text, the same on every system. egui's own fonts stay
/// behind it for any character Inter lacks, and for monospace.
fn fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("Inter".to_owned(), egui::FontData::from_static(INTER));
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".to_owned());
    ctx.set_fonts(fonts);
}

/// Text sizes in points. Slightly larger than egui's defaults: small text
/// drawn without hinting reads better with a little more room.
fn text_styles(style: &mut egui::Style) {
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Small, FontId::new(10.5, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(19.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ]
    .into();
}

pub(crate) fn apply(ctx: &egui::Context) {
    fonts(ctx);
    let mut style = (*ctx.style()).clone();
    text_styles(&mut style);
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
    fn inter_is_a_truetype_font_with_its_licence() {
        // TrueType outlines start with version 1.0 (00 01 00 00).
        assert_eq!(INTER.get(..4), Some(&[0, 1, 0, 0][..]));
        assert!(INTER_LICENSE.contains("SIL OPEN FONT LICENSE"));
    }

    #[test]
    fn text_lays_out_in_inter() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Accounts Receivable $1,234.56 • € £ …");
            });
        });
        let style = ctx.style();
        let body = style.text_styles.get(&egui::TextStyle::Body);
        assert_eq!(body.map(|f| f.size), Some(13.5));
    }
}
