//! The look of printed documents: the colours a template uses, how column
//! headings are drawn, and the letterhead with the company's logo.

use ledgercraft_ledger::{Company, PdfTemplate};

use crate::invoice_pdf::{ACCENT, GREEN, INK, LEFT, LINE_HEIGHT, MUTED, RED, RIGHT, WHITE};
use crate::pdf::{Align, Font, Image, Page, Rgb};

/// A template's colours and style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    /// Headings, labels and column bands.
    pub accent: Rgb,
    /// Column headings on a filled band (Classic), or over a rule.
    pub band: bool,
    /// VOID.
    pub warning: Rgb,
    /// "Paid in full".
    pub good: Rgb,
}

impl Look {
    pub(crate) fn of(template: PdfTemplate) -> Self {
        match template {
            PdfTemplate::Classic => Self {
                accent: ACCENT,
                band: true,
                warning: RED,
                good: GREEN,
            },
            PdfTemplate::Modern => Self {
                accent: ACCENT,
                band: false,
                warning: RED,
                good: GREEN,
            },
            PdfTemplate::Simple => Self {
                accent: INK,
                band: false,
                warning: INK,
                good: INK,
            },
        }
    }

    /// A row of column headings `(x, label, align)` with its top at `y`, and
    /// at `size` points; returns the y below it.
    pub(crate) fn headings(&self, p: &mut Page, y: f32, size: f32, cells: &[(f32, &str, Align)]) -> f32 {
        let font = Font::bold(size);
        let baseline = y + 13.5;
        if self.band {
            p.rect(LEFT, y, RIGHT - LEFT, 20.0, self.accent);
            for (x, label, align) in cells {
                p.text(*x, baseline, label, font, WHITE, *align);
            }
        } else {
            for (x, label, align) in cells {
                p.text(*x, baseline, label, font, self.accent, *align);
            }
            p.rule(LEFT, RIGHT, y + 19.0, self.accent, 1.2);
        }
        y + 20.0
    }
}

/// The company's logo as a PDF image, if it has one that can be read (a
/// logo that cannot is left off rather than stopping the PDF).
pub(crate) fn logo(c: &Company) -> Option<Image> {
    c.info
        .print
        .logo
        .as_ref()
        .and_then(|l| crate::logo::decode(&l.0).ok())
}

/// The most room the logo takes at the top left, in points.
const LOGO_WIDTH: f32 = 180.0;
const LOGO_HEIGHT: f32 = 60.0;

/// A `width` x `height` pixel image scaled to fit the logo's room, keeping
/// its shape, and never enlarged.
#[allow(clippy::cast_precision_loss)]
fn fit(width: u32, height: u32) -> (f32, f32) {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    let scale = (LOGO_WIDTH / w).min(LOGO_HEIGHT / h).min(1.0);
    (w * scale, h * scale)
}

/// The top left of a document's first page: the logo (image 0) if there is
/// one, then the company's name and contact lines. Returns the y below.
pub(crate) fn letterhead(p: &mut Page, logo: Option<&Image>, company: &str, lines: &[String]) -> f32 {
    let mut name_y = 64.0;
    if let Some(image) = logo {
        let (w, h) = fit(image.width, image.height);
        p.image(0, LEFT, 40.0, w, h);
        name_y = 40.0 + h + 22.0;
    }
    p.text(LEFT, name_y, company, Font::bold(18.0), INK, Align::Left);
    let mut y = name_y + 18.0;
    for l in lines {
        p.text(LEFT, y, l, Font::regular(9.5), MUTED, Align::Left);
        y += LINE_HEIGHT;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logos_fit_their_room_and_keep_their_shape() {
        let (w, h) = fit(1200, 400);
        assert!((w - 180.0).abs() < 0.01 && (h - 60.0).abs() < 0.01);
        let (w, h) = fit(300, 600);
        assert!((h - 60.0).abs() < 0.01 && (w - 30.0).abs() < 0.01);
        // Small ones are not blown up.
        assert_eq!(fit(40, 20), (40.0, 20.0));
    }
}
