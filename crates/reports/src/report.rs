use ledgercraft_core::Money;
use ledgercraft_ledger::{AccountId, TxnId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Section,
    Detail,
    Subtotal,
    Total,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Text(String),
    Money(Money),
}

impl Cell {
    pub fn text(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(s) => s.clone(),
            Self::Money(m) => m.to_string(),
        }
    }

    fn csv(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(s) => s.clone(),
            Self::Money(m) => m.to_plain_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub label: String,
    pub indent: u8,
    pub cells: Vec<Cell>,
    /// The account behind the row, so a UI can drill down to its register.
    pub account: Option<AccountId>,
    /// The transaction behind the row, so a UI can open it.
    pub txn: Option<TxnId>,
}

/// A finished report: a label column plus value columns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub title: String,
    pub company: String,
    pub period: String,
    pub label_header: String,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

impl Report {
    pub(crate) fn new(title: &str, company: &str, period: String, columns: &[&str]) -> Self {
        Self {
            title: title.to_string(),
            company: company.to_string(),
            period,
            label_header: String::new(),
            columns: columns.iter().map(|c| (*c).to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub(crate) fn section(&mut self, label: &str, indent: u8) {
        self.rows.push(Row {
            kind: RowKind::Section,
            label: label.to_string(),
            indent,
            cells: Vec::new(),
            account: None,
            txn: None,
        });
    }

    pub(crate) fn line(
        &mut self,
        kind: RowKind,
        label: &str,
        indent: u8,
        cells: Vec<Cell>,
        account: Option<AccountId>,
    ) {
        self.rows.push(Row {
            kind,
            label: label.to_string(),
            indent,
            cells,
            account,
            txn: None,
        });
    }

    /// A detail row for one transaction.
    pub(crate) fn txn_line(&mut self, label: &str, cells: Vec<Cell>, txn: TxnId) {
        self.rows.push(Row {
            kind: RowKind::Detail,
            label: label.to_string(),
            indent: 0,
            cells,
            account: None,
            txn: Some(txn),
        });
    }

    pub(crate) fn money(
        &mut self,
        kind: RowKind,
        label: &str,
        indent: u8,
        values: &[Money],
        account: Option<AccountId>,
    ) {
        let cells = values.iter().map(|m| Cell::Money(*m)).collect();
        self.line(kind, label, indent, cells, account);
    }

    /// RFC 4180 CSV with a header row.
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        let mut header = vec![csv_field(&self.label_header)];
        header.extend(self.columns.iter().map(|c| csv_field(c)));
        out.push_str(&header.join(","));
        out.push_str("\r\n");
        for row in &self.rows {
            let label = format!("{}{}", "  ".repeat(usize::from(row.indent)), row.label);
            let mut fields = vec![csv_field(&label)];
            for i in 0..self.columns.len() {
                fields.push(csv_field(
                    &row.cells.get(i).map(Cell::csv).unwrap_or_default(),
                ));
            }
            out.push_str(&fields.join(","));
            out.push_str("\r\n");
        }
        out
    }

    /// A plain-text table for terminals.
    pub fn to_text(&self) -> String {
        let label_of = |r: &Row| format!("{}{}", "  ".repeat(usize::from(r.indent)), r.label);
        let mut label_w = self.label_header.chars().count();
        for r in &self.rows {
            label_w = label_w.max(label_of(r).chars().count());
        }
        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.chars().count()).collect();
        for r in &self.rows {
            for (i, w) in widths.iter_mut().enumerate() {
                if let Some(c) = r.cells.get(i) {
                    *w = (*w).max(c.text().chars().count());
                }
            }
        }
        let mut out = format!("{}\n{}\n{}\n\n", self.company, self.title, self.period);
        let mut header = format!("{:<label_w$}", self.label_header);
        for (c, &w) in self.columns.iter().zip(&widths) {
            header.push_str(&format!("  {c:>w$}"));
        }
        out.push_str(header.trim_end());
        out.push('\n');
        for r in &self.rows {
            let mut line = format!("{:<label_w$}", label_of(r));
            for (i, &w) in widths.iter().enumerate() {
                let text = r.cells.get(i).map(Cell::text).unwrap_or_default();
                match r.cells.get(i) {
                    Some(Cell::Text(_)) => line.push_str(&format!("  {text:<w$}")),
                    _ => line.push_str(&format!("  {text:>w$}")),
                }
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

fn csv_field(s: &str) -> String {
    // A leading =, + or @ would be read as a formula by spreadsheet apps.
    // (A leading - is left alone: it is how negative amounts are written.)
    let s = if s.starts_with(['=', '+', '@']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}
