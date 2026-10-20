//! The Tutorial window: shown at launch until the person ticks "Never show
//! again", and any time from Help > Tutorial. The lessons themselves are not
//! written yet, so the window is blank for now.

use eframe::egui;

use crate::prefs::Prefs;

/// Size of the (for now empty) lesson area.
const CONTENT_SIZE: egui::Vec2 = egui::vec2(480.0, 300.0);

/// The Tutorial window. The checkbox edits `prefs`; the app saves them.
pub(crate) fn window(ctx: &egui::Context, open: &mut bool, prefs: &mut Prefs) {
    egui::Window::new("Tutorial")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.allocate_space(CONTENT_SIZE);
            ui.separator();
            let mut never = !prefs.show_tutorial;
            if ui.checkbox(&mut never, "Never show again").changed() {
                prefs.show_tutorial = !never;
            }
        });
}
