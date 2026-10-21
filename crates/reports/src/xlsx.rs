//! Export a [`Report`] as a formatted Excel workbook (.xlsx).
//!
//! Unlike CSV, the workbook keeps the report's look: money as `#,##0.00`
//! with red negatives, bold sections and totals, indented accounts, total
//! rules, sized columns and a frozen header row.

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};

use crate::{Cell, Report, RowKind};

/// Excel number format for money, with an optional currency symbol shown in
/// front: `"$"#,##0.00;[Red]-"$"#,##0.00`.
fn money_format(symbol: &str) -> String {
    if symbol.is_empty() {
        "#,##0.00;[Red]-#,##0.00".to_string()
    } else {
        let quoted = format!("\"{}\"", symbol.replace('"', ""));
        format!("{quoted}#,##0.00;[Red]-{quoted}#,##0.00")
    }
}
const MUTED: u32 = 0x6B7280;
const ACCENT: u32 = 0x1F6F8B;
/// Rows above the column headers: company, title, period, blank.
const HEADER_ROW: u32 = 4;

impl Report {
    /// The report as the bytes of an .xlsx file. Amounts are numbers Excel can
    /// add up, shown with `currency` (such as `"$"`, or `""` for none) in front.
    pub fn to_xlsx(&self, currency: &str) -> Result<Vec<u8>, String> {
        build(self, &money_format(currency))
            .map_err(|e| format!("could not build the Excel file: {e}"))
    }
}

fn build(r: &Report, money_fmt: &str) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let ws = workbook.add_worksheet();
    ws.set_name(sheet_name(&r.title))?;

    // Title block.
    ws.write_string_with_format(
        0,
        0,
        &r.company,
        &Format::new().set_bold().set_font_size(14),
    )?;
    let title = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_font_color(Color::RGB(ACCENT));
    ws.write_string_with_format(1, 0, &r.title, &title)?;
    ws.write_string_with_format(
        2,
        0,
        &r.period,
        &Format::new().set_font_color(Color::RGB(MUTED)),
    )?;

    // Column headers. Money columns are right-aligned to sit over their numbers.
    let header = Format::new()
        .set_bold()
        .set_border_bottom(FormatBorder::Thin);
    ws.write_string_with_format(HEADER_ROW, 0, &r.label_header, &header)?;
    for (i, name) in r.columns.iter().enumerate() {
        let fmt = if is_money_column(r, i) {
            header.clone().set_align(FormatAlign::Right)
        } else {
            header.clone()
        };
        ws.write_string_with_format(HEADER_ROW, col(i + 1), name, &fmt)?;
    }

    // Body.
    let mut row = HEADER_ROW + 1;
    for line in &r.rows {
        write_row(ws, row, line, r.columns.len(), money_fmt)?;
        row = row.saturating_add(1);
    }

    // Column widths from the content, within sensible limits.
    let label_width = r
        .rows
        .iter()
        .map(|l| l.label.chars().count() + usize::from(l.indent) * 3)
        .chain([r.label_header.chars().count(), 12])
        .max()
        .unwrap_or(12)
        .min(60);
    ws.set_column_width(0, width(label_width + 2))?;
    for i in 0..r.columns.len() {
        let content = r
            .rows
            .iter()
            .filter_map(|l| l.cells.get(i))
            .map(|c| c.text().chars().count())
            .chain(r.columns.get(i).map(|h| h.chars().count()))
            .max()
            .unwrap_or(10)
            .clamp(10, 50);
        ws.set_column_width(col(i + 1), width(content + 3))?;
    }

    ws.set_freeze_panes(HEADER_ROW + 1, 0)?;
    workbook.save_to_buffer()
}

fn write_row(
    ws: &mut Worksheet,
    row: u32,
    line: &crate::Row,
    columns: usize,
    money_fmt: &str,
) -> Result<(), XlsxError> {
    let strong = matches!(
        line.kind,
        RowKind::Section | RowKind::Subtotal | RowKind::Total
    );
    let mut label = Format::new().set_indent(line.indent);
    if strong {
        label = label.set_bold();
    }
    ws.write_string_with_format(row, 0, &line.label, &label)?;

    if line.kind == RowKind::Section {
        return Ok(());
    }
    let mut money = Format::new().set_num_format(money_fmt);
    match line.kind {
        RowKind::Subtotal => money = money.set_bold().set_border_top(FormatBorder::Thin),
        RowKind::Total => {
            money = money
                .set_bold()
                .set_border_top(FormatBorder::Thin)
                .set_border_bottom(FormatBorder::Double)
        }
        RowKind::Detail | RowKind::Section => {}
    }
    for i in 0..columns {
        let c = col(i + 1);
        match line.cells.get(i) {
            Some(Cell::Money(m)) => {
                // Cents to a decimal number; exact for any realistic amount.
                let value = m.cents() as f64 / 100.0;
                ws.write_number_with_format(row, c, value, &money)?;
            }
            Some(Cell::Text(t)) => {
                ws.write_string(row, c, t)?;
            }
            _ if line.kind != RowKind::Detail => {
                // Keep the total rules continuous across empty cells.
                ws.write_blank(row, c, &money)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// True when a column holds money in any row.
fn is_money_column(r: &Report, i: usize) -> bool {
    r.rows
        .iter()
        .any(|l| matches!(l.cells.get(i), Some(Cell::Money(_))))
}

/// Excel sheet names: at most 31 characters, none of `[]:*?/\`.
fn sheet_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') {
                '-'
            } else {
                c
            }
        })
        .take(31)
        .collect();
    let trimmed = cleaned.trim().trim_matches('\'').to_string();
    if trimmed.is_empty() {
        "Report".to_string()
    } else {
        trimmed
    }
}

fn col(i: usize) -> u16 {
    u16::try_from(i).unwrap_or(u16::MAX)
}

fn width(chars: usize) -> f64 {
    f64::from(u32::try_from(chars).unwrap_or(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_names_are_valid() {
        assert_eq!(
            sheet_name("Register: 1000 · Checking"),
            "Register- 1000 · Checking"
        );
        assert_eq!(sheet_name(""), "Report");
        assert!(sheet_name(&"x".repeat(50)).chars().count() <= 31);
    }

    #[test]
    fn builds_a_workbook() {
        let mut r = Report::new("Profit & Loss", "Test Co", "2026".into(), &["Amount"]);
        r.section("Income", 0);
        r.money(
            RowKind::Detail,
            "Sales",
            1,
            &[ledgercraft_core::Money::from_cents(12_345)],
            None,
        );
        r.money(
            RowKind::Total,
            "Net Income",
            0,
            &[ledgercraft_core::Money::from_cents(12_345)],
            None,
        );
        let bytes = r.to_xlsx("$").unwrap();
        assert_eq!(money_format("$"), "\"$\"#,##0.00;[Red]-\"$\"#,##0.00");
        assert_eq!(money_format(""), "#,##0.00;[Red]-#,##0.00");
        // An .xlsx file is a zip archive.
        assert_eq!(bytes.get(0..2), Some(&b"PK"[..]));
    }
}
