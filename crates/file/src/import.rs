//! Bring existing lists into a company: the chart of accounts, customers,
//! vendors and products and services.
//!
//! Two sources are read:
//! * IIF files, the tab-separated list export of QuickBooks Desktop
//!   (File > Utilities > Export > Lists to IIF Files). IIF is a documented
//!   import/export format; only list records are read (`ACCNT`, `CUST`,
//!   `VEND`, `INVITEM`).
//! * CSV files with a header row, one list per file, as exported by most
//!   accounting software and spreadsheets. Columns are matched by name.
//!
//! Importing never stops at a bad row: each record that cannot be added is
//! skipped and listed with the reason, and everything else goes in.

use std::fmt;

use ledgercraft_ledger::{
    AccountType, Address, Company, Contact, ItemKind, ItemSpec, Money,
};

/// Which list a CSV file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Accounts,
    Customers,
    Vendors,
    Items,
}

impl ListKind {
    pub const ALL: [Self; 4] = [Self::Accounts, Self::Customers, Self::Vendors, Self::Items];

    pub fn label(self) -> &'static str {
        match self {
            Self::Accounts => "Chart of accounts",
            Self::Customers => "Customers",
            Self::Vendors => "Vendors",
            Self::Items => "Products and services",
        }
    }

    /// The columns LedgerCraft reads, for help text. The first is required.
    pub fn columns(self) -> &'static str {
        match self {
            Self::Accounts => "Name, Type, Number, Description",
            Self::Customers | Self::Vendors => {
                "Name, Company, Email, Phone, Street, City, State, Zip, Terms"
            }
            Self::Items => "Name, Type, Description, Rate, Income Account",
        }
    }
}

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub accounts: usize,
    pub customers: usize,
    pub vendors: usize,
    pub items: usize,
    /// One line per record that was not added, with the reason.
    pub skipped: Vec<String>,
}

impl ImportSummary {
    pub fn added(&self) -> usize {
        self.accounts + self.customers + self.vendors + self.items
    }

    pub fn merge(&mut self, other: Self) {
        self.accounts += other.accounts;
        self.customers += other.customers;
        self.vendors += other.vendors;
        self.items += other.items;
        self.skipped.extend(other.skipped);
    }
}

impl fmt::Display for ImportSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Imported {} accounts, {} customers, {} vendors and {} items",
            self.accounts, self.customers, self.vendors, self.items
        )?;
        if !self.skipped.is_empty() {
            write!(f, "; skipped {}", self.skipped.len())?;
        }
        Ok(())
    }
}

/// Refuse absurdly large inputs (hostile or corrupt files).
const MAX_RECORDS: usize = 100_000;

// ----- IIF ------------------------------------------------------------------

/// Reads list records from an IIF file into `company`.
pub fn import_iif(company: &mut Company, text: &str) -> ImportSummary {
    let mut summary = ImportSummary::default();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    // The column names for each record type, from its `!TYPE` header line.
    let mut headers: Vec<(String, Vec<String>)> = Vec::new();
    // Items name their income account, so add them after the accounts.
    let mut items: Vec<Record> = Vec::new();
    for (n, line) in text.lines().enumerate().take(MAX_RECORDS) {
        let line_no = n + 1;
        let fields: Vec<String> = line.split('\t').map(unquote).collect();
        let Some(tag) = fields.first() else { continue };
        if let Some(kind) = tag.strip_prefix('!') {
            headers.retain(|(k, _)| k != kind);
            headers.push((kind.to_string(), fields.iter().skip(1).map(|h| h.to_uppercase()).collect()));
            continue;
        }
        let Some((_, cols)) = headers.iter().find(|(k, _)| k == tag) else {
            continue;
        };
        let record = Record { line: line_no, cols: cols.clone(), values: fields.iter().skip(1).cloned().collect() };
        match tag.as_str() {
            "ACCNT" => add_account(company, &record, &mut summary),
            "CUST" => add_party(company, &record, true, &mut summary),
            "VEND" => add_party(company, &record, false, &mut summary),
            "INVITEM" => items.push(record),
            _ => {}
        }
    }
    for record in &items {
        add_item(company, record, &mut summary);
    }
    summary
}

/// One row: column names and values, matched case-insensitively.
struct Record {
    line: usize,
    cols: Vec<String>,
    values: Vec<String>,
}

impl Record {
    /// The first non-empty value among the given column names.
    fn get(&self, names: &[&str]) -> &str {
        for name in names {
            let found = self
                .cols
                .iter()
                .position(|c| c.eq_ignore_ascii_case(name))
                .and_then(|i| self.values.get(i))
                .map(|v| v.trim())
                .filter(|v| !v.is_empty());
            if let Some(v) = found {
                return v;
            }
        }
        ""
    }
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    match t.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => t.to_string(),
    }
}

/// Account types by IIF code or by LedgerCraft's own label.
fn account_type(text: &str) -> Option<AccountType> {
    let t = text.trim().to_uppercase();
    let by_code = match t.as_str() {
        "BANK" => Some(AccountType::Bank),
        "AR" => Some(AccountType::AccountsReceivable),
        "OCASSET" => Some(AccountType::OtherCurrentAsset),
        "FIXASSET" => Some(AccountType::FixedAsset),
        "OASSET" => Some(AccountType::OtherAsset),
        "AP" => Some(AccountType::AccountsPayable),
        "CCARD" => Some(AccountType::CreditCard),
        "OCLIAB" => Some(AccountType::OtherCurrentLiability),
        "LTLIAB" => Some(AccountType::LongTermLiability),
        "EQUITY" => Some(AccountType::Equity),
        "INC" => Some(AccountType::Income),
        "COGS" => Some(AccountType::CostOfGoodsSold),
        "EXP" => Some(AccountType::Expense),
        "EXINC" => Some(AccountType::OtherIncome),
        "EXEXP" => Some(AccountType::OtherExpense),
        _ => None,
    };
    by_code.or_else(|| AccountType::ALL.into_iter().find(|k| k.label().eq_ignore_ascii_case(text.trim())))
}

fn add_account(company: &mut Company, r: &Record, summary: &mut ImportSummary) {
    let name = r.get(&["NAME", "ACCOUNT", "ACCOUNT NAME"]);
    let type_text = r.get(&["ACCNTTYPE", "TYPE", "ACCOUNT TYPE"]);
    if name.is_empty() {
        summary.skipped.push(format!("Line {}: account with no name", r.line));
        return;
    }
    if type_text.eq_ignore_ascii_case("NONPOSTING") {
        summary.skipped.push(format!("{name}: non-posting accounts (such as purchase orders) are not used"));
        return;
    }
    let Some(kind) = account_type(type_text) else {
        summary.skipped.push(format!("{name}: unknown account type {type_text:?}"));
        return;
    };
    let number = r.get(&["ACCNUM", "NUMBER", "ACCOUNT NUMBER", "NUM"]);
    let description = r.get(&["DESC", "DESCRIPTION"]);
    match company.add_account(number, name, kind, description) {
        Ok(_) => summary.accounts += 1,
        Err(e) => summary.skipped.push(format!("{name}: {e}")),
    }
}

/// Payment terms such as "Net 30" or "30" become days; anything else 0.
fn terms_days(text: &str) -> u32 {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    digits.parse::<u32>().ok().filter(|d| *d <= 3650).unwrap_or(0)
}

fn add_party(company: &mut Company, r: &Record, customer: bool, summary: &mut ImportSummary) {
    let name = r.get(&["NAME", "CUSTOMER", "VENDOR", "DISPLAY NAME", "FULL NAME"]);
    if name.is_empty() {
        summary.skipped.push(format!("Line {}: name is missing", r.line));
        return;
    }
    let contact = Contact {
        name: name.to_string(),
        company: r.get(&["COMPANYNAME", "COMPANY", "COMPANY NAME"]).to_string(),
        email: r.get(&["EMAIL", "E-MAIL"]).to_string(),
        phone: r.get(&["PHONE1", "PHONE", "MAIN PHONE"]).to_string(),
        address: Address {
            line1: r.get(&["BADDR1", "ADDR1", "STREET", "ADDRESS", "BILLING ADDRESS", "ADDRESS 1"]).to_string(),
            line2: r.get(&["BADDR2", "ADDR2", "ADDRESS 2"]).to_string(),
            city: r.get(&["CITY", "BILLING CITY"]).to_string(),
            region: r.get(&["STATE", "REGION", "PROVINCE", "BILLING STATE"]).to_string(),
            postal_code: r.get(&["ZIP", "POSTAL CODE", "ZIP CODE", "BILLING ZIP"]).to_string(),
            country: r.get(&["COUNTRY"]).to_string(),
        },
        terms_days: terms_days(r.get(&["TERMS", "PAYMENT TERMS"])),
        notes: r.get(&["NOTE", "NOTES"]).to_string(),
    };
    let result = if customer {
        company.add_customer(contact).map(|_| ())
    } else {
        company.add_vendor(contact).map(|_| ())
    };
    match result {
        Ok(()) if customer => summary.customers += 1,
        Ok(()) => summary.vendors += 1,
        Err(e) => summary.skipped.push(format!("{name}: {e}")),
    }
}

fn add_item(company: &mut Company, r: &Record, summary: &mut ImportSummary) {
    let name = r.get(&["NAME", "ITEM", "PRODUCT/SERVICE NAME", "PRODUCT/SERVICE"]);
    if name.is_empty() {
        summary.skipped.push(format!("Line {}: item with no name", r.line));
        return;
    }
    let kind = match r.get(&["INVITEMTYPE", "TYPE", "ITEM TYPE"]).to_uppercase().as_str() {
        "SERV" | "SERVICE" => ItemKind::Service,
        "OTHC" | "OTHER CHARGE" => ItemKind::OtherCharge,
        _ => ItemKind::NonInventory,
    };
    let rate_text = r.get(&["PRICE", "RATE", "SALES PRICE", "SALES PRICE/RATE"]);
    let rate = match Money::parse_or_zero(rate_text) {
        Ok(m) => m,
        Err(_) => {
            summary.skipped.push(format!("{name}: price {rate_text:?} is not an amount"));
            return;
        }
    };
    let account_name = r.get(&["ACCNT", "INCOME ACCOUNT", "ACCOUNT"]);
    let account = company
        .account_by_name(account_name)
        .filter(|a| a.active && !a.kind.needs_party())
        .map(|a| a.id)
        .or_else(|| company.first_account_of(AccountType::Income));
    let Some(income_account) = account else {
        summary.skipped.push(format!("{name}: no income account to record its sales in"));
        return;
    };
    let spec = ItemSpec {
        name: name.to_string(),
        description: r.get(&["DESC", "DESCRIPTION", "SALES DESCRIPTION"]).to_string(),
        kind,
        rate,
        income_account,
    };
    match company.add_item(spec) {
        Ok(_) => summary.items += 1,
        Err(e) => summary.skipped.push(format!("{name}: {e}")),
    }
}

// ----- CSV --------------------------------------------------------------------

/// Reads one list from a CSV file with a header row into `company`.
pub fn import_csv(company: &mut Company, kind: ListKind, text: &str) -> Result<ImportSummary, String> {
    let rows = parse_csv(text);
    let mut rows = rows.into_iter();
    let Some(header) = rows.next() else {
        return Err("the file is empty".into());
    };
    let cols: Vec<String> = header.iter().map(|h| h.trim().to_uppercase()).collect();
    let mut summary = ImportSummary::default();
    for (i, values) in rows.enumerate().take(MAX_RECORDS) {
        if values.iter().all(|v| v.trim().is_empty()) {
            continue;
        }
        let record = Record { line: i + 2, cols: cols.clone(), values };
        match kind {
            ListKind::Accounts => add_account(company, &record, &mut summary),
            ListKind::Customers => add_party(company, &record, true, &mut summary),
            ListKind::Vendors => add_party(company, &record, false, &mut summary),
            ListKind::Items => add_item(company, &record, &mut summary),
        }
    }
    Ok(summary)
}

/// RFC 4180 CSV: quoted fields may hold commas, quotes ("") and line breaks.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                if rows.len() > MAX_RECORDS {
                    return rows;
                }
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const IIF: &str = "!ACCNT\tNAME\tACCNTTYPE\tDESC\tACCNUM\n\
ACCNT\tChecking\tBANK\t\t1000\n\
ACCNT\tShop Supplies\tEXP\tGlue and sandpaper\t6210\n\
ACCNT\tPurchase Orders\tNONPOSTING\t\t\n\
ACCNT\tFurniture Sales\tINC\t\t4050\n\
!CUST\tNAME\tBADDR1\tPHONE1\tEMAIL\tTERMS\n\
CUST\t\"Smith, Jane\"\t12 Elm St\t555-0100\tjane@example.com\tNet 15\n\
!VEND\tNAME\tADDR1\tTERMS\n\
VEND\tTimber Co\t9 Mill Rd\tDue on receipt\n\
!INVITEM\tNAME\tINVITEMTYPE\tDESC\tPRICE\tACCNT\n\
INVITEM\tBookshelf\tPART\tOak bookshelf\t450.00\tFurniture Sales\n\
INVITEM\tDelivery\tSERV\tLocal delivery\t40\tNo Such Account\n";

    #[test]
    fn iif_lists_come_in() {
        let mut c = Company::new("Import Co");
        let s = import_iif(&mut c, IIF);
        assert_eq!((s.accounts, s.customers, s.vendors, s.items), (3, 1, 1, 2), "{s:?}");
        assert_eq!(s.skipped.len(), 1); // the non-posting account
        let jane = c.customers.first().unwrap();
        assert_eq!(jane.contact.name, "Smith, Jane");
        assert_eq!(jane.contact.terms_days, 15);
        assert_eq!(c.vendors.first().unwrap().contact.terms_days, 0);
        let shelf = c.items.iter().find(|i| i.spec.name == "Bookshelf").unwrap();
        assert_eq!(shelf.spec.rate, Money::from_dollars(450));
        assert_eq!(c.account_name(shelf.spec.income_account), "4050 · Furniture Sales");
    }

    #[test]
    fn duplicates_are_skipped_not_fatal() {
        let mut c = Company::with_default_accounts("Starter Co");
        let s = import_iif(&mut c, IIF);
        // "Checking" and number 1000 already exist in the starter chart.
        assert!(s.skipped.iter().any(|m| m.starts_with("Checking")));
        assert!(s.accounts >= 2);
    }

    #[test]
    fn csv_customers_with_quotes_and_line_breaks() {
        let csv = "\u{feff}Name,Email,Terms,City\r\n\
\"Lee, Dana\",dana@example.com,Net 30,Lehi\r\n\
\"Quote \"\"Q\"\" Co\",,30,\"Provo\nUT\"\r\n\
,missing@example.com,,\r\n";
        let mut c = Company::new("CSV Co");
        let s = import_csv(&mut c, ListKind::Customers, csv).unwrap();
        assert_eq!(s.customers, 2);
        assert_eq!(s.skipped.len(), 1);
        assert_eq!(c.customers[1].contact.name, "Quote \"Q\" Co");
        assert_eq!(c.customers[1].contact.address.city, "Provo\nUT");
        assert_eq!(c.customers[0].contact.terms_days, 30);
    }

    #[test]
    fn csv_accounts_by_label() {
        let csv = "Number,Name,Type\n1010,Petty Cash,Bank\n6999,Odd,Mystery\n";
        let mut c = Company::new("CSV Co");
        let s = import_csv(&mut c, ListKind::Accounts, csv).unwrap();
        assert_eq!(s.accounts, 1);
        assert!(s.skipped[0].contains("unknown account type"));
        assert!(import_csv(&mut c, ListKind::Accounts, "").is_err());
    }
}
