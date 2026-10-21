//! Help > Licenses: the full licence texts, compiled into the binary so they
//! can be read without a network connection or the source tree.

use eframe::egui::{self, RichText};

use crate::theme;

const MIT: &str = include_str!("../../../LICENSE-MIT");
const APACHE: &str = include_str!("../../../LICENSE-APACHE");
const NOTICE: &str = include_str!("../../../NOTICE");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LicenseTab {
    #[default]
    Summary,
    Mit,
    Apache,
    Notice,
    Font,
}

impl LicenseTab {
    const ALL: [Self; 5] = [
        Self::Summary,
        Self::Mit,
        Self::Apache,
        Self::Notice,
        Self::Font,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Mit => "MIT License",
            Self::Apache => "Apache License 2.0",
            Self::Notice => "Notices",
            Self::Font => "Inter Font",
        }
    }
}

#[derive(Default)]
pub(crate) struct LicenseWindow {
    pub open: bool,
    tab: LicenseTab,
}

impl LicenseWindow {
    pub(crate) fn show_tab(&mut self, tab: LicenseTab) {
        self.open = true;
        self.tab = tab;
    }

    pub(crate) fn ui(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        egui::Window::new("Licenses")
            .open(&mut open)
            .collapsible(false)
            .default_size([640.0, 520.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for tab in LicenseTab::ALL {
                        ui.selectable_value(&mut self.tab, tab, tab.label());
                    }
                });
                ui.separator();
                match self.tab {
                    LicenseTab::Summary => summary(ui, &mut self.tab),
                    LicenseTab::Mit => text(ui, MIT),
                    LicenseTab::Apache => text(ui, APACHE),
                    LicenseTab::Notice => text(ui, NOTICE),
                    LicenseTab::Font => text(ui, theme::INTER_LICENSE),
                }
            });
        self.open = open;
    }
}

fn summary(ui: &mut egui::Ui, tab: &mut LicenseTab) {
    ui.label(RichText::new("LedgerCraft is free and open-source software.").strong());
    ui.add_space(4.0);
    ui.label(
        "You may use, copy, modify and distribute it under either the MIT License or the \
         Apache License 2.0, at your option. Both licences let you use LedgerCraft for any \
         purpose, including commercial use, at no cost.",
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.link("Read the MIT License").clicked() {
            *tab = LicenseTab::Mit;
        }
        ui.label("·");
        if ui.link("Read the Apache License 2.0").clicked() {
            *tab = LicenseTab::Apache;
        }
        ui.label("·");
        if ui.link("Read the notices").clicked() {
            *tab = LicenseTab::Notice;
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("No warranty").strong());
    ui.label(
        "LedgerCraft is provided \"as is\", without warranty of any kind. It is not tax, \
         legal or accounting advice. Check your books and filings with a qualified \
         professional.",
    );
    ui.add_space(8.0);
    ui.label(RichText::new("Third-party components").strong());
    ui.label(
        "LedgerCraft is built with open-source Rust libraries (such as egui, wgpu and serde), \
         each under its own permissive licence. A complete list with licence texts will be \
         included here in a later release.",
    );
    ui.horizontal(|ui| {
        ui.label("Text is set in Inter by Rasmus Andersson, under the SIL Open Font License.");
        if ui.link("Read it").clicked() {
            *tab = LicenseTab::Font;
        }
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "QuickBooks and Intuit are trademarks of Intuit Inc. LedgerCraft is an independent \
             project and is not affiliated with or endorsed by Intuit Inc.",
        )
        .small()
        .color(theme::MUTED),
    );
}

/// A scrollable, selectable view of a licence text.
fn text(ui: &mut egui::Ui, body: &str) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut shown = body;
            ui.add(
                egui::TextEdit::multiline(&mut shown)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
        });
}
