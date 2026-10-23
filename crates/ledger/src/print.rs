//! How printed documents (invoices, receipts, credit memos, statements)
//! look: a template, and the company's logo.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The look of printed documents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfTemplate {
    /// Coloured headings and filled column bands.
    #[default]
    Classic,
    /// Lighter: coloured headings over thin rules, no filled bands.
    Modern,
    /// Black and white, for printing.
    Simple,
}

impl PdfTemplate {
    pub const ALL: [Self; 3] = [Self::Classic, Self::Modern, Self::Simple];

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Modern => "Modern",
            Self::Simple => "Simple",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Classic => "Coloured headings and filled column bands.",
            Self::Modern => "Lighter: coloured headings over thin rules.",
            Self::Simple => "Black and white, for printing.",
        }
    }
}

/// The largest logo file accepted, in bytes.
pub const LOGO_MAX_BYTES: usize = 1_000_000;

/// A logo image file, PNG or JPEG, as it was chosen. It is kept in the
/// company file as base64 text.
#[derive(Clone, PartialEq, Eq)]
pub struct Logo(pub Vec<u8>);

impl fmt::Debug for Logo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Logo({} bytes)", self.0.len())
    }
}

impl Serialize for Logo {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64_encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Logo {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        base64_decode(&text)
            .map(Logo)
            .ok_or_else(|| serde::de::Error::custom("the logo is not valid base64"))
    }
}

/// How printed documents look, as set in Company Information.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintStyle {
    #[serde(default)]
    pub template: PdfTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<Logo>,
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, with padding.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                let index = (n >> (18 - 6 * i)) & 63;
                out.push(char::from(ALPHABET[index as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes standard base64 (padding optional, whitespace ignored); `None`
/// if it is not base64.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for ch in text.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buffer >> bits) & 0xFF).ok()?);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for text in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            let encoded = base64_encode(text.as_bytes());
            assert_eq!(base64_decode(&encoded).unwrap(), text.as_bytes());
        }
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&base64_encode(&all)).unwrap(), all);
        assert!(base64_decode("not*base64").is_none());
    }

    #[test]
    fn a_logo_is_saved_as_base64_text() {
        let style = PrintStyle {
            template: PdfTemplate::Modern,
            logo: Some(Logo(vec![0x89, b'P', b'N', b'G'])),
        };
        let json = serde_json::to_string(&style).unwrap();
        assert_eq!(json, r#"{"template":"modern","logo":"iVBORw=="}"#);
        let back: PrintStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(back, style);
        // Older files have neither: the classic look, no logo.
        let old: PrintStyle = serde_json::from_str("{}").unwrap();
        assert_eq!(old, PrintStyle::default());
    }
}
