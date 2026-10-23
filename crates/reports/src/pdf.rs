//! A small PDF writer: enough for printable documents such as invoices.
//!
//! Pages are US Letter. Text uses Helvetica and Helvetica-Bold, two of the
//! standard fonts every PDF reader provides, so no font is embedded and the
//! files stay small. Text is encoded as WinAnsi (Windows-1252), which covers
//! English and Western European text and the €, £ and other common symbols;
//! anything else prints as "?". Positions are in points (1/72 inch) measured
//! from the top-left corner, the way the page is read.

/// US Letter, in points.
pub(crate) const PAGE_WIDTH: f32 = 612.0;
pub(crate) const PAGE_HEIGHT: f32 = 792.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rgb(pub f32, pub f32, pub f32);

/// How text sits against its `x`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Left,
    Right,
}

/// A font choice: bold or not, and its size in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Font {
    pub bold: bool,
    pub size: f32,
}

impl Font {
    pub(crate) const fn regular(size: f32) -> Self {
        Self { bold: false, size }
    }

    pub(crate) const fn bold(size: f32) -> Self {
        Self { bold: true, size }
    }
}

/// One page's drawing commands.
#[derive(Default)]
pub(crate) struct Page {
    ops: Vec<String>,
}

impl Page {
    /// Draws text with its baseline at `y`; `x` is its left edge, or its
    /// right edge when aligned right.
    pub(crate) fn text(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        font: Font,
        color: Rgb,
        align: Align,
    ) {
        let x = match align {
            Align::Left => x,
            Align::Right => x - width(text, font),
        };
        let name = if font.bold { "F2" } else { "F1" };
        self.ops.push(format!(
            "BT /{name} {:.1} Tf {} {x:.2} {:.2} Td ({}) Tj ET",
            font.size,
            fill(color),
            PAGE_HEIGHT - y,
            literal(text)
        ));
    }

    /// A filled rectangle; `y` is its top edge.
    pub(crate) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
        self.ops.push(format!(
            "{} {x:.2} {:.2} {w:.2} {h:.2} re f",
            fill(color),
            PAGE_HEIGHT - y - h
        ));
    }

    /// Draws image number `index` (counting from 0, in the list given to
    /// [`finish_with`]) `w` by `h` points, its top-left corner at `x`, `y`.
    pub(crate) fn image(&mut self, index: usize, x: f32, y: f32, w: f32, h: f32) {
        self.ops.push(format!(
            "q {w:.2} 0 0 {h:.2} {x:.2} {:.2} cm /Im{} Do Q",
            PAGE_HEIGHT - y - h,
            index + 1
        ));
    }

    /// A horizontal rule from `x1` to `x2`.
    pub(crate) fn rule(&mut self, x1: f32, x2: f32, y: f32, color: Rgb, thickness: f32) {
        let Rgb(r, g, b) = color;
        let y = PAGE_HEIGHT - y;
        self.ops.push(format!(
            "{r:.3} {g:.3} {b:.3} RG {thickness:.2} w {x1:.2} {y:.2} m {x2:.2} {y:.2} l S"
        ));
    }
}

fn fill(Rgb(r, g, b): Rgb) -> String {
    format!("{r:.3} {g:.3} {b:.3} rg")
}

/// Text as WinAnsi bytes; characters it cannot show become "?".
pub(crate) fn encode(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| match u32::from(ch) {
            code @ (32..=126 | 160..=255) => u8::try_from(code).unwrap_or(b'?'),
            _ => winansi_extra(ch).unwrap_or(b'?'),
        })
        .collect()
}

/// The Windows-1252 characters outside Latin-1 (bytes 0x80 to 0x9F).
fn winansi_extra(ch: char) -> Option<u8> {
    Some(match ch {
        '€' => 0x80,
        '‚' => 0x82,
        'ƒ' => 0x83,
        '„' => 0x84,
        '…' => 0x85,
        '†' => 0x86,
        '‡' => 0x87,
        'ˆ' => 0x88,
        '‰' => 0x89,
        'Š' => 0x8A,
        '‹' => 0x8B,
        'Œ' => 0x8C,
        'Ž' => 0x8E,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        '˜' => 0x98,
        '™' => 0x99,
        'š' => 0x9A,
        '›' => 0x9B,
        'œ' => 0x9C,
        'ž' => 0x9E,
        'Ÿ' => 0x9F,
        '\t' => b' ',
        _ => return None,
    })
}

/// A PDF string literal: brackets and backslashes escaped, and every byte
/// outside printable ASCII written as an octal escape, so the file stays
/// plain text.
fn literal(text: &str) -> String {
    let mut out = String::new();
    for b in encode(text) {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(char::from(b));
            }
            32..=126 => out.push(char::from(b)),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out
}

/// How wide `text` is when drawn, in points.
pub(crate) fn width(text: &str, font: Font) -> f32 {
    let table: &[u16] = if font.bold {
        &HELVETICA_BOLD
    } else {
        &HELVETICA
    };
    let units: u32 = encode(text)
        .iter()
        .map(|&b| {
            usize::from(b)
                .checked_sub(32)
                .and_then(|i| table.get(i))
                .map_or(0, |&w| u32::from(w))
        })
        .sum();
    // Widths are in thousandths of the font size.
    let units = units as f32;
    units * font.size / 1000.0
}

/// Breaks text into lines no wider than `max`, at spaces where it can and
/// inside a word only when the word alone is too wide. Line breaks in the
/// text are kept.
pub(crate) fn wrap(text: &str, font: Font, max: f32) -> Vec<String> {
    let fits = |chars: &[char]| width(&chars.iter().collect::<String>(), font) <= max;
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut current = String::new();
        for word in paragraph.split(' ') {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if width(&candidate, font) <= max {
                current = candidate;
                continue;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut rest: Vec<char> = word.chars().collect();
            while rest.len() > 1 && !fits(&rest) {
                let mut n = rest.len() - 1;
                while n > 1 && !fits(&rest[..n]) {
                    n -= 1;
                }
                let tail = rest.split_off(n);
                lines.push(rest.into_iter().collect());
                rest = tail;
            }
            current = rest.into_iter().collect();
        }
        lines.push(current);
    }
    lines
}

/// A picture for the pages, such as a logo, ready to put in the file.
pub(crate) struct Image {
    pub width: u32,
    pub height: u32,
    pub data: ImageData,
}

pub(crate) enum ImageData {
    /// A JPEG file as it is (PDF readers decode it); `gray` for one channel.
    Jpeg { bytes: Vec<u8>, gray: bool },
    /// 8-bit RGB pixels, zlib-compressed, and the same for its transparency
    /// (one byte a pixel) when it has any.
    Rgb {
        pixels: Vec<u8>,
        alpha: Option<Vec<u8>>,
    },
}

/// An image object: its dictionary and data.
fn image_object(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

/// The finished file, with `images` for the pages to draw. `title` shows
/// in the reader's title bar.
pub(crate) fn finish_with(pages: &[Page], title: &str, images: &[Image]) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", 6 + 2 * i))
        .collect();
    objects.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .into_bytes(),
    );
    for base in ["Helvetica", "Helvetica-Bold"] {
        objects.push(
            format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>"
            )
            .into_bytes(),
        );
    }
    objects.push(
        format!(
            "<< /Title ({}) /Producer (LedgerCraft {}) >>",
            literal(title),
            env!("CARGO_PKG_VERSION")
        )
        .into_bytes(),
    );
    // Images come after the pages: each takes one object, and one more for
    // its transparency if it has any.
    let mut next = 6 + 2 * pages.len();
    let mut numbers = Vec::new();
    for image in images {
        let own = next;
        next += 1;
        let mask = match &image.data {
            ImageData::Rgb { alpha: Some(_), .. } => {
                next += 1;
                Some(own + 1)
            }
            _ => None,
        };
        numbers.push((own, mask));
    }
    let xobjects = if images.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = numbers
            .iter()
            .enumerate()
            .map(|(i, (own, _))| format!("/Im{} {own} 0 R", i + 1))
            .collect();
        format!(" /XObject << {} >>", names.join(" "))
    };
    for (i, page) in pages.iter().enumerate() {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >>{xobjects} >> /Contents {} 0 R >>",
                7 + 2 * i
            )
            .into_bytes(),
        );
        let body = page.ops.join("\n");
        let mut stream = format!("<< /Length {} >>\nstream\n", body.len()).into_bytes();
        stream.extend_from_slice(body.as_bytes());
        stream.extend_from_slice(b"\nendstream");
        objects.push(stream);
    }
    for (image, (_, mask)) in images.iter().zip(&numbers) {
        let size = format!(
            "/Type /XObject /Subtype /Image /Width {} /Height {} /BitsPerComponent 8",
            image.width, image.height
        );
        match &image.data {
            ImageData::Jpeg { bytes, gray } => {
                let space = if *gray { "/DeviceGray" } else { "/DeviceRGB" };
                let dict = format!("{size} /ColorSpace {space} /Filter /DCTDecode");
                objects.push(image_object(&dict, bytes));
            }
            ImageData::Rgb { pixels, alpha } => {
                let smask = mask.map_or_else(String::new, |m| format!(" /SMask {m} 0 R"));
                let dict = format!("{size} /ColorSpace /DeviceRGB /Filter /FlateDecode{smask}");
                objects.push(image_object(&dict, pixels));
                if let Some(alpha) = alpha {
                    let dict = format!("{size} /ColorSpace /DeviceGray /Filter /FlateDecode");
                    objects.push(image_object(&dict, alpha));
                }
            }
        }
    }

    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(object);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

// Advance widths of the WinAnsi characters 32 to 255, in thousandths of the
// font size, from Adobe's Core 14 font metrics (see ATTRIBUTION.md).
#[rustfmt::skip]
const HELVETICA: [u16; 224] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, 350,
    556, 350, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 350, 611, 350,
    350, 222, 222, 333, 333, 350, 556, 1000, 333, 1000, 500, 333, 944, 350, 500, 667,
    278, 333, 556, 556, 556, 556, 260, 556, 333, 737, 370, 556, 584, 333, 737, 333,
    400, 584, 333, 333, 333, 556, 537, 278, 333, 333, 365, 556, 834, 834, 834, 611,
    667, 667, 667, 667, 667, 667, 1000, 722, 667, 667, 667, 667, 278, 278, 278, 278,
    722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667, 611,
    556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556, 278, 278, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556, 500,
];
#[rustfmt::skip]
const HELVETICA_BOLD: [u16; 224] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611,
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584, 350,
    556, 350, 278, 556, 500, 1000, 556, 556, 333, 1000, 667, 333, 1000, 350, 611, 350,
    350, 278, 278, 500, 500, 350, 556, 1000, 333, 1000, 556, 333, 944, 350, 500, 667,
    278, 333, 556, 556, 556, 556, 280, 556, 333, 737, 370, 556, 584, 333, 737, 333,
    400, 584, 333, 333, 333, 611, 556, 278, 333, 333, 365, 556, 834, 834, 834, 611,
    722, 722, 722, 722, 722, 722, 1000, 722, 667, 667, 667, 667, 278, 278, 278, 278,
    722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667, 611,
    556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556, 278, 278, 278, 278,
    611, 611, 611, 611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611, 556,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_measured_with_the_font_metrics() {
        // "Hello" in Helvetica: H 722, e 556, l 222, l 222, o 556 = 2278 units.
        assert!((width("Hello", Font::regular(10.0)) - 22.78).abs() < 0.001);
        assert!(width("Hello", Font::bold(10.0)) > width("Hello", Font::regular(10.0)));
    }

    #[test]
    fn winansi_covers_the_common_symbols() {
        assert_eq!(encode("€£é—"), vec![0x80, 0xA3, 0xE9, 0x97]);
        assert_eq!(encode("✓"), vec![b'?']);
        assert_eq!(literal("(a\\b) €"), "\\(a\\\\b\\) \\200");
    }

    #[test]
    fn long_text_wraps_and_long_words_break() {
        let font = Font::regular(10.0);
        let lines = wrap("one two three four five six", font, 60.0);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| width(l, font) <= 60.0));
        let word = wrap(&"x".repeat(80), font, 50.0);
        assert!(word.len() > 1);
        assert!(word.iter().all(|l| width(l, font) <= 50.0));
        assert_eq!(word.concat(), "x".repeat(80));
        assert_eq!(wrap("a\nb", font, 100.0), vec!["a", "b"]);
    }

    #[test]
    fn the_cross_reference_table_points_at_each_object() {
        let mut page = Page::default();
        let black = Rgb(0.0, 0.0, 0.0);
        page.text(
            50.0,
            60.0,
            "Hi (there)",
            Font::bold(12.0),
            black,
            Align::Left,
        );
        page.rule(50.0, 100.0, 70.0, Rgb(0.5, 0.5, 0.5), 1.0);
        let pdf = finish_with(&[page, Page::default()], "Test", &[]);
        let text = String::from_utf8(pdf).unwrap();
        assert!(text.starts_with("%PDF-1.4\n") && text.ends_with("%%EOF\n"));
        let xref_at: usize = text.lines().rev().nth(1).unwrap().parse().unwrap();
        assert!(text[xref_at..].starts_with("xref\n"));
        let entries: Vec<usize> = text[xref_at..]
            .lines()
            .skip(3)
            .take_while(|l| l.ends_with(" n "))
            .map(|l| l[..10].parse().unwrap())
            .collect();
        assert_eq!(
            entries.len(),
            9,
            "catalog, pages, 2 fonts, info, 2 x (page, content)"
        );
        for (i, at) in entries.iter().enumerate() {
            assert!(text[*at..].starts_with(&format!("{} 0 obj", i + 1)));
        }
        assert!(text.contains("/Count 2"));
        assert!(text.contains("(Hi \\(there\\)) Tj"));
    }
}
