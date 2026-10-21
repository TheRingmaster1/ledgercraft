//! Reading bank and credit card downloads: OFX (SGML 1.x and XML 2.x, which
//! also covers Quicken's QFX and QuickBooks Web Connect QBO) and CSV.
//! Produces [`FeedLine`]s for the ledger to match or add; it never changes
//! the books itself.

use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::FeedLine;

use crate::import::parse_csv;

/// What a download holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Statement {
    pub lines: Vec<FeedLine>,
    /// The bank's ending balance and its date, when the file says (OFX does).
    pub ending_balance: Option<Money>,
    pub balance_date: Option<Date>,
    /// Lines that could not be read, with the reason.
    pub skipped: Vec<String>,
}

/// Biggest download read (a few years of daily transactions is far less).
const MAX_BYTES: usize = 20 * 1024 * 1024;

/// Reads a download, choosing OFX or CSV from its contents.
pub fn read_statement(text: &str) -> Result<Statement, String> {
    if text.len() > MAX_BYTES {
        return Err("the file is too large to be a bank download".into());
    }
    let upper = text.to_ascii_uppercase();
    if upper.contains("<OFX") || upper.contains("OFXHEADER") {
        read_ofx(text)
    } else {
        read_csv(text)
    }
}

// ----- OFX ----------------------------------------------------------------------

/// The `<TAG>value` pairs of an OFX file in order, for both SGML (no closing
/// tags) and XML. Closing tags come back as `/TAG` with no value.
fn ofx_tags(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('>') else {
            break;
        };
        let tag = rest[..close].trim().to_ascii_uppercase();
        rest = &rest[close + 1..];
        let end = rest.find('<').unwrap_or(rest.len());
        let value = decode_entities(rest[..end].trim());
        out.push((tag, value));
    }
    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// `20260131`, `20260131120000.000[-5:EST]` and the like.
fn ofx_date(s: &str) -> Option<Date> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).take(8).collect();
    if digits.len() < 8 {
        return None;
    }
    let year = digits.get(0..4)?.parse().ok()?;
    let month = digits.get(4..6)?.parse().ok()?;
    let day = digits.get(6..8)?.parse().ok()?;
    Date::from_ymd(year, month, day).ok()
}

fn read_ofx(text: &str) -> Result<Statement, String> {
    let mut statement = Statement::default();
    let mut current: Option<FeedLine> = None;
    let mut in_balance = false;
    let mut row = 0;
    for (tag, value) in ofx_tags(text) {
        match tag.as_str() {
            "STMTTRN" => {
                // SGML files often leave the previous transaction unclosed.
                if let Some(line) = current.take() {
                    statement.lines.push(line);
                }
                row += 1;
                current = Some(FeedLine {
                    date: Date::today(),
                    amount: Money::ZERO,
                    payee: String::new(),
                    memo: String::new(),
                    number: String::new(),
                    fit_id: String::new(),
                });
            }
            "/STMTTRN" | "/BANKTRANLIST" => {
                if let Some(line) = current.take() {
                    statement.lines.push(line);
                }
            }
            "LEDGERBAL" => {
                if let Some(line) = current.take() {
                    statement.lines.push(line);
                }
                in_balance = true;
            }
            "/LEDGERBAL" => in_balance = false,
            _ => {}
        }
        if let Some(line) = current.as_mut() {
            match tag.as_str() {
                "DTPOSTED" => match ofx_date(&value) {
                    Some(date) => line.date = date,
                    None => statement
                        .skipped
                        .push(format!("Transaction {row}: date {value:?}")),
                },
                "TRNAMT" => match Money::parse(&value) {
                    Ok(amount) => line.amount = amount,
                    Err(e) => statement.skipped.push(format!("Transaction {row}: {e}")),
                },
                "NAME" | "PAYEE" => line.payee = value,
                "MEMO" => line.memo = value,
                "CHECKNUM" => line.number = value,
                "FITID" => line.fit_id = value,
                _ => {}
            }
        } else if in_balance {
            match tag.as_str() {
                "BALAMT" => statement.ending_balance = Money::parse(&value).ok(),
                "DTASOF" => statement.balance_date = ofx_date(&value),
                _ => {}
            }
        }
    }
    // SGML files may never close the last transaction.
    if let Some(line) = current.take() {
        statement.lines.push(line);
    }
    if statement.lines.is_empty() {
        return Err("no transactions were found in this OFX file".into());
    }
    Ok(statement)
}

// ----- CSV ----------------------------------------------------------------------

/// Which column holds what, found from the header row's names.
#[derive(Default)]
struct Columns {
    date: Option<usize>,
    amount: Option<usize>,
    debit: Option<usize>,
    credit: Option<usize>,
    payee: Option<usize>,
    memo: Option<usize>,
    number: Option<usize>,
    id: Option<usize>,
}

impl Columns {
    fn from_header(header: &[String]) -> Self {
        let mut c = Self::default();
        for (i, name) in header.iter().enumerate() {
            let n = name.trim().to_lowercase();
            let slot = match n.as_str() {
                "date" | "posted date" | "posting date" | "transaction date" | "trans date"
                | "post date" => &mut c.date,
                "amount" | "transaction amount" => &mut c.amount,
                "debit" | "debits" | "withdrawal" | "withdrawals" | "money out" | "paid out" => {
                    &mut c.debit
                }
                "credit" | "credits" | "deposit" | "deposits" | "money in" | "paid in" => {
                    &mut c.credit
                }
                "description" | "payee" | "name" | "merchant" | "details" => &mut c.payee,
                "memo" | "notes" | "note" => &mut c.memo,
                "check number" | "check" | "check #" | "cheque number" | "number" | "ref"
                | "reference" => &mut c.number,
                "id" | "transaction id" | "fitid" => &mut c.id,
                _ => continue,
            };
            if slot.is_none() {
                *slot = Some(i);
            }
        }
        c
    }
}

fn read_csv(text: &str) -> Result<Statement, String> {
    let rows = parse_csv(text);
    let mut rows = rows.into_iter();
    let header = rows.next().ok_or("the file is empty")?;
    let cols = Columns::from_header(&header);
    let date_col = cols
        .date
        .ok_or("no Date column was found in the header row")?;
    if cols.amount.is_none() && cols.debit.is_none() && cols.credit.is_none() {
        return Err("no Amount (or Debit and Credit) column was found in the header row".into());
    }
    let mut statement = Statement::default();
    for (i, row) in rows.enumerate() {
        let n = i + 2;
        if row.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let cell = |col: Option<usize>| {
            col.and_then(|c| row.get(c))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        let Ok(date) = Date::parse(&cell(Some(date_col))) else {
            statement.skipped.push(format!("Row {n}: no readable date"));
            continue;
        };
        let amount = match cols.amount {
            Some(_) => Money::parse(&cell(cols.amount)),
            None => {
                let out = Money::parse_or_zero(&cell(cols.debit)).map(Money::abs);
                let into = Money::parse_or_zero(&cell(cols.credit)).map(Money::abs);
                out.and_then(|o| into.map(|n| n.saturating_sub(o)))
            }
        };
        let Ok(amount) = amount else {
            statement
                .skipped
                .push(format!("Row {n}: no readable amount"));
            continue;
        };
        let payee = cell(cols.payee);
        let number = cell(cols.number);
        // Without a bank id, one is made from the line itself, so the same
        // file imported twice is still recognised.
        let fit_id = match cell(cols.id) {
            id if !id.is_empty() => id,
            _ => format!("csv:{date}:{}:{payee}:{number}", amount.to_plain_string()),
        };
        statement.lines.push(FeedLine {
            date,
            amount,
            payee,
            memo: cell(cols.memo),
            number,
            fit_id,
        });
    }
    if statement.lines.is_empty() {
        return Err("no transactions were found in this CSV file".into());
    }
    Ok(statement)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SGML: &str = "OFXHEADER:100\nDATA:OFXSGML\nVERSION:102\n\n<OFX>\n<BANKMSGSRSV1>\n\
<STMTTRNRS><STMTRS><BANKTRANLIST>\n\
<STMTTRN>\n<TRNTYPE>CHECK\n<DTPOSTED>20260107120000[-5:EST]\n<TRNAMT>-200.00\n\
<FITID>A1\n<CHECKNUM>101\n<NAME>CHECK 101\n\
<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20260112\n<TRNAMT>500.00\n<FITID>A2\n\
<NAME>Smith &amp; Co\n<MEMO>Invoice 1001\n\
</BANKTRANLIST>\n<LEDGERBAL><BALAMT>300.00<DTASOF>20260131</LEDGERBAL>\n\
</STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>\n";

    const XML: &str = r#"<?xml version="1.0"?><?OFX OFXHEADER="200" VERSION="220"?>
<OFX><CREDITCARDMSGSRSV1><CCSTMTTRNRS><CCSTMTRS><BANKTRANLIST>
<STMTTRN><TRNTYPE>DEBIT</TRNTYPE><DTPOSTED>20260301</DTPOSTED><TRNAMT>-40.00</TRNAMT>
<FITID>C1</FITID><NAME>Diner</NAME></STMTTRN>
</BANKTRANLIST><LEDGERBAL><BALAMT>-40.00</BALAMT><DTASOF>20260331</DTASOF></LEDGERBAL>
</CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>"#;

    #[test]
    fn sgml_ofx_reads_lines_and_the_balance() {
        let s = read_statement(SGML).unwrap();
        assert_eq!(s.lines.len(), 2);
        let first = &s.lines[0];
        assert_eq!(first.date, Date::parse("2026-01-07").unwrap());
        assert_eq!(first.amount, Money::from_cents(-20_000));
        assert_eq!(
            (first.number.as_str(), first.fit_id.as_str()),
            ("101", "A1")
        );
        assert_eq!(s.lines[1].payee, "Smith & Co");
        assert_eq!(s.lines[1].memo, "Invoice 1001");
        assert_eq!(s.ending_balance, Some(Money::from_cents(30_000)));
        assert_eq!(s.balance_date, Some(Date::parse("2026-01-31").unwrap()));
    }

    #[test]
    fn xml_ofx_credit_card_reads() {
        let s = read_statement(XML).unwrap();
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.lines[0].amount, Money::from_cents(-4_000));
        assert_eq!(s.lines[0].payee, "Diner");
        assert_eq!(s.ending_balance, Some(Money::from_cents(-4_000)));
    }

    #[test]
    fn csv_with_one_amount_column() {
        let csv = "Date,Description,Amount,Check Number\n\
                   2026-01-07,CHECK 101,-200.00,101\n\
                   01/12/2026,\"Smith, Co\",500.00,\n";
        let s = read_statement(csv).unwrap();
        assert_eq!(s.lines.len(), 2);
        assert_eq!(s.lines[1].date, Date::parse("2026-01-12").unwrap());
        assert_eq!(s.lines[1].payee, "Smith, Co");
        assert_eq!(s.lines[0].number, "101");
        // Made-up ids are stable, so a second import is recognised.
        assert_eq!(
            read_statement(csv).unwrap().lines[0].fit_id,
            s.lines[0].fit_id
        );
    }

    #[test]
    fn csv_with_debit_and_credit_columns() {
        let csv =
            "Posted Date,Payee,Debit,Credit\n2026-02-01,Rent,1200.00,\n2026-02-03,Sale,,75.50\n";
        let s = read_statement(csv).unwrap();
        assert_eq!(s.lines[0].amount, Money::from_cents(-120_000));
        assert_eq!(s.lines[1].amount, Money::from_cents(7_550));
    }

    #[test]
    fn the_sample_download_in_docs_reads() {
        let text = include_str!("../../../docs/samples/pine-ridge-checking.ofx");
        let s = read_statement(text).unwrap();
        assert_eq!(s.lines.len(), 5);
        assert_eq!(s.lines[2].payee, "VALLEY POWER & LIGHT");
        assert_eq!(s.lines[4].amount, Money::from_cents(125));
        assert!(s.skipped.is_empty());
    }

    #[test]
    fn unreadable_files_say_why() {
        assert!(read_statement("").is_err());
        assert!(read_statement("Name,Phone\nDana,555\n").is_err());
        assert!(read_statement("<OFX></OFX>").is_err());
        let s = read_statement("Date,Amount\nsoon,5\n2026-01-01,abc\n2026-01-02,3\n").unwrap();
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.skipped.len(), 2);
    }
}
