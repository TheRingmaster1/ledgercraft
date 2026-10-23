//! Logos for printed documents: a PNG or JPEG file turned into a PDF image.
//!
//! JPEG files go into the PDF as they are (every PDF reader decodes them);
//! only their size is read here. PNG files are decoded (any colour type, bit
//! depth and filter; not interlaced) into RGB pixels and, if the image has
//! any, its transparency, both compressed again for the PDF. Bad files are
//! refused with a message, never a crash.

use crate::pdf::{Image, ImageData};

/// The most pixels a logo may have (2,000 x 2,000).
const MAX_PIXELS: usize = 4_000_000;

/// A logo's width and height in pixels, or why it cannot be used.
pub fn logo_size(bytes: &[u8]) -> Result<(u32, u32), String> {
    decode(bytes).map(|image| (image.width, image.height))
}

/// A logo file as a PDF image.
pub(crate) fn decode(bytes: &[u8]) -> Result<Image, String> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        jpeg(bytes)
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        png(bytes)
    } else {
        Err("Choose a PNG or JPEG image.".into())
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn check_size(width: u32, height: u32) -> Result<(), String> {
    let pixels = usize::try_from(width)
        .ok()
        .zip(usize::try_from(height).ok())
        .and_then(|(w, h)| w.checked_mul(h));
    match pixels {
        Some(0) => Err("The image is empty.".into()),
        Some(n) if n <= MAX_PIXELS => Ok(()),
        _ => Err("The image is too large: use one under 2000 x 2000 pixels.".into()),
    }
}

/// Reads a JPEG's size and colour channels from its frame header.
fn jpeg(bytes: &[u8]) -> Result<Image, String> {
    let bad = || "This JPEG file could not be read.".to_string();
    let mut at = 2;
    loop {
        if bytes.get(at) != Some(&0xFF) {
            return Err(bad());
        }
        let marker = *bytes.get(at + 1).ok_or_else(bad)?;
        match marker {
            // Padding, and markers with no length.
            0xFF => at += 1,
            0x01 | 0xD0..=0xD7 => at += 2,
            0xD9 | 0xDA => return Err(bad()),
            _ => {
                let length = usize::from(u16_at(bytes, at + 2).ok_or_else(bad)?);
                // Start of frame (not 0xC4, 0xC8 or 0xCC, which share the range).
                if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                    let height = u16_at(bytes, at + 5).ok_or_else(bad)?;
                    let width = u16_at(bytes, at + 7).ok_or_else(bad)?;
                    let channels = *bytes.get(at + 9).ok_or_else(bad)?;
                    let gray = match channels {
                        1 => true,
                        3 => false,
                        _ => {
                            return Err("This JPEG is CMYK: save it as RGB (for the web) and \
                                        choose it again."
                                .into())
                        }
                    };
                    check_size(u32::from(width), u32::from(height))?;
                    return Ok(Image {
                        width: u32::from(width),
                        height: u32::from(height),
                        data: ImageData::Jpeg {
                            bytes: bytes.to_vec(),
                            gray,
                        },
                    });
                }
                at += 2 + length;
            }
        }
    }
}

/// The parts of a PNG file needed to decode it.
struct PngParts {
    width: u32,
    height: u32,
    depth: u8,
    colour: u8,
    data: Vec<u8>,
    palette: Vec<u8>,
    transparency: Vec<u8>,
}

fn png_parts(bytes: &[u8]) -> Result<PngParts, String> {
    let bad = || "This PNG file could not be read.".to_string();
    let mut parts = PngParts {
        width: 0,
        height: 0,
        depth: 0,
        colour: 0,
        data: Vec::new(),
        palette: Vec::new(),
        transparency: Vec::new(),
    };
    let mut at = 8;
    while at < bytes.len() {
        let length = usize::try_from(u32_at(bytes, at).ok_or_else(bad)?).map_err(|_| bad())?;
        let kind = bytes.get(at + 4..at + 8).ok_or_else(bad)?;
        let body = bytes.get(at + 8..at + 8 + length).ok_or_else(bad)?;
        match kind {
            b"IHDR" => {
                parts.width = u32_at(body, 0).ok_or_else(bad)?;
                parts.height = u32_at(body, 4).ok_or_else(bad)?;
                parts.depth = *body.get(8).ok_or_else(bad)?;
                parts.colour = *body.get(9).ok_or_else(bad)?;
                if body.get(12) != Some(&0) {
                    return Err("This PNG is interlaced: save it without interlacing and \
                                choose it again."
                        .into());
                }
            }
            b"PLTE" => parts.palette = body.to_vec(),
            b"tRNS" => parts.transparency = body.to_vec(),
            b"IDAT" => parts.data.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        at += 12 + length;
    }
    if parts.data.is_empty() {
        return Err(bad());
    }
    Ok(parts)
}

/// The Paeth predictor from the PNG specification.
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn png(bytes: &[u8]) -> Result<Image, String> {
    let bad = || "This PNG file could not be read.".to_string();
    let parts = png_parts(bytes)?;
    check_size(parts.width, parts.height)?;
    let channels: usize = match parts.colour {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return Err(bad()),
    };
    let depth = usize::from(parts.depth);
    if !matches!(depth, 1 | 2 | 4 | 8 | 16) {
        return Err(bad());
    }
    let width = usize::try_from(parts.width).map_err(|_| bad())?;
    let height = usize::try_from(parts.height).map_err(|_| bad())?;
    let bits = channels * depth;
    // Bytes per pixel for the filters (at least one), and per row.
    let step = bits.div_ceil(8).max(1);
    let stride = (width * bits).div_ceil(8);
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&parts.data).map_err(|_| bad())?;

    let mut rows: Vec<Vec<u8>> = Vec::with_capacity(height);
    let mut previous = vec![0u8; stride];
    for y in 0..height {
        let start = y * (stride + 1);
        let filter = *raw.get(start).ok_or_else(bad)?;
        let line = raw.get(start + 1..start + 1 + stride).ok_or_else(bad)?;
        let mut row = line.to_vec();
        for i in 0..stride {
            let left = if i >= step { row[i - step] } else { 0 };
            let up = previous[i];
            let up_left = if i >= step { previous[i - step] } else { 0 };
            let x = row[i];
            row[i] = match filter {
                0 => x,
                1 => x.wrapping_add(left),
                2 => x.wrapping_add(up),
                3 => {
                    let average = (u16::from(left) + u16::from(up)) / 2;
                    x.wrapping_add(u8::try_from(average).unwrap_or(0))
                }
                4 => x.wrapping_add(paeth(left, up, up_left)),
                _ => return Err(bad()),
            };
        }
        previous.clone_from(&row);
        rows.push(row);
    }

    // Sample `c` of pixel `x` in a row, scaled to 0-255 (palette indexes as
    // they are).
    let max = (1u32 << depth.min(8)) - 1;
    let sample = |row: &[u8], x: usize, c: usize| -> u8 {
        let n = x * channels + c;
        match depth {
            16 => row.get(n * 2).copied().unwrap_or(0),
            8 => row.get(n).copied().unwrap_or(0),
            _ => {
                let bit = n * depth;
                let byte = row.get(bit / 8).copied().unwrap_or(0);
                let shift = 8 - depth - bit % 8;
                let value = u32::from(byte >> shift) & max;
                if parts.colour == 3 {
                    u8::try_from(value).unwrap_or(0)
                } else {
                    u8::try_from(value * 255 / max).unwrap_or(0)
                }
            }
        }
    };

    let mut rgb = Vec::with_capacity(width * height * 3);
    let mut alpha = Vec::with_capacity(width * height);
    for row in &rows {
        for x in 0..width {
            let (r, g, b, a) = match parts.colour {
                0 => {
                    let v = sample(row, x, 0);
                    (v, v, v, 255)
                }
                2 => (sample(row, x, 0), sample(row, x, 1), sample(row, x, 2), 255),
                3 => {
                    let i = usize::from(sample(row, x, 0));
                    let colour = parts.palette.get(i * 3..i * 3 + 3).ok_or_else(bad)?;
                    let a = parts.transparency.get(i).copied().unwrap_or(255);
                    (colour[0], colour[1], colour[2], a)
                }
                4 => {
                    let v = sample(row, x, 0);
                    (v, v, v, sample(row, x, 1))
                }
                _ => (
                    sample(row, x, 0),
                    sample(row, x, 1),
                    sample(row, x, 2),
                    sample(row, x, 3),
                ),
            };
            rgb.extend_from_slice(&[r, g, b]);
            alpha.push(a);
        }
    }
    let alpha = if alpha.iter().all(|a| *a == 255) {
        None
    } else {
        Some(miniz_oxide::deflate::compress_to_vec_zlib(&alpha, 6))
    };
    Ok(Image {
        width: parts.width,
        height: parts.height,
        data: ImageData::Rgb {
            pixels: miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6),
            alpha,
        },
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A PNG file from raw (already filtered) rows: CRCs are left zero,
    /// which the reader does not check.
    pub(crate) fn png_file(width: u32, height: u32, depth: u8, colour: u8, rows: &[u8]) -> Vec<u8> {
        png_file_with(width, height, depth, colour, rows, &[])
    }

    pub(crate) fn png_file_with(
        width: u32,
        height: u32,
        depth: u8,
        colour: u8,
        rows: &[u8],
        extra: &[(&[u8; 4], Vec<u8>)],
    ) -> Vec<u8> {
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut chunk = |kind: &[u8], body: &[u8]| {
            out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(body);
            out.extend_from_slice(&[0, 0, 0, 0]);
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[depth, colour, 0, 0, 0]);
        chunk(b"IHDR", &ihdr);
        for (kind, body) in extra {
            chunk(*kind, body);
        }
        chunk(
            b"IDAT",
            &miniz_oxide::deflate::compress_to_vec_zlib(rows, 6),
        );
        chunk(b"IEND", &[]);
        out
    }

    fn pixels(image: &Image) -> (Vec<u8>, Option<Vec<u8>>) {
        match &image.data {
            ImageData::Rgb { pixels, alpha } => (
                miniz_oxide::inflate::decompress_to_vec_zlib(pixels).unwrap(),
                alpha
                    .as_ref()
                    .map(|a| miniz_oxide::inflate::decompress_to_vec_zlib(a).unwrap()),
            ),
            ImageData::Jpeg { .. } => (Vec::new(), None),
        }
    }

    #[test]
    fn png_rows_are_unfiltered() {
        // 2 x 2 RGB: row 1 unfiltered, row 2 "up" (adds the row above).
        let rows = [0, 10, 20, 30, 40, 50, 60, 2, 1, 1, 1, 2, 2, 2];
        let image = decode(&png_file(2, 2, 8, 2, &rows)).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        let (rgb, alpha) = pixels(&image);
        assert_eq!(rgb, vec![10, 20, 30, 40, 50, 60, 11, 21, 31, 42, 52, 62]);
        assert!(alpha.is_none(), "no transparency, no mask");
    }

    #[test]
    fn transparency_and_palettes_come_through() {
        // RGBA: one clear pixel, one solid, "sub" filtered (adds the left).
        let rows = [1, 255, 0, 0, 0, 0, 0, 0, 255];
        let (rgb, alpha) = pixels(&decode(&png_file(2, 1, 8, 6, &rows)).unwrap());
        assert_eq!(rgb, vec![255, 0, 0, 255, 0, 0]);
        assert_eq!(alpha, Some(vec![0, 255]));

        // A 2-bit palette: indexes 0, 1, 2, 1 packed in one byte.
        let palette = vec![0, 0, 0, 255, 255, 255, 31, 111, 139];
        let file = png_file_with(
            4,
            1,
            2,
            3,
            &[0, 0b0001_1001],
            &[(b"PLTE", palette), (b"tRNS", vec![0])],
        );
        let (rgb, alpha) = pixels(&decode(&file).unwrap());
        assert_eq!(
            rgb,
            vec![0, 0, 0, 255, 255, 255, 31, 111, 139, 255, 255, 255]
        );
        assert_eq!(alpha, Some(vec![0, 255, 255, 255]));
    }

    #[test]
    fn jpeg_size_is_read_from_its_frame_header() {
        // SOI, an APP0 segment, then SOF0: 8-bit, 120 high, 300 wide, RGB.
        let mut file = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        file.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 120, 0x01, 0x2C, 3]);
        file.extend_from_slice(&[0; 9]);
        assert_eq!(logo_size(&file), Ok((300, 120)));
        file[17] = 4;
        assert!(logo_size(&file).unwrap_err().contains("CMYK"));
    }

    #[test]
    fn bad_files_are_refused_not_crashed_on() {
        assert!(logo_size(b"GIF89a").is_err());
        assert!(logo_size(&[0xFF, 0xD8, 0xFF]).is_err());
        let mut png = png_file(2, 2, 8, 2, &[0, 1, 2, 3, 4, 5, 6]);
        assert!(logo_size(&png).is_err(), "rows missing");
        png.truncate(30);
        assert!(logo_size(&png).is_err());
        assert!(logo_size(&png_file(0, 0, 8, 2, &[])).is_err());
    }
}
