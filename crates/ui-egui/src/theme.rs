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
