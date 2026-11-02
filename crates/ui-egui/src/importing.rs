//! Bringing existing books in: choosing IIF and CSV list files (Create a
//! Company and File > Import Lists…), and File > Opening Balances….

use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText, Ui};
use ledgercraft_core::{Date, Money};
use ledgercraft_engine::Command;
use ledgercraft_file::import::{ImportText, ListKind};
use ledgercraft_ledger::{
    takes_opening_balance, AccountId, Company, CustomerId, OpeningBalances, VendorId,
};

use crate::widgets::{self, field, money_label};
use crate::{theme, Action, Reset};

/// Largest import file read, so a wrong pick cannot exhaust memory.
const IMPORT_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// A file chosen for import: an IIF file, or a CSV holding one list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImportFile {
    pub path: PathBuf,
    /// `None` for IIF (which holds several lists).
    pub csv: Option<ListKind>,
}

impl ImportFile {
    fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn label(&self) -> String {
        match self.csv {
            None => format!("{}  (IIF lists)", self.name()),
            Some(kind) => format!("{}  ({})", self.name(), kind.label()),
        }
    }
}

/// The files picked, and the list the next CSV file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImportPicks {
    pub files: Vec<ImportFile>,
    pub csv_kind: ListKind,
}

impl Default for ImportPicks {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            csv_kind: ListKind::Customers,
        }
    }
}

impl ImportPicks {
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Reads every file picked.
    pub(crate) fn read(&self) -> Result<Vec<ImportText>, String> {
        self.files
            .iter()
            .map(|f| {
                Ok(ImportText {
                    list: f.csv,
                    text: read_import(&f.path)?,
                    name: f.name(),
                })
            })
            .collect()
    }
}

fn read_import(path: &Path) -> Result<String, String> {
    let shown = path.display();
    let len = std::fs::metadata(path)
        .map_err(|e| format!("Could not read {shown}: {e}"))?
        .len();
    if len > IMPORT_MAX_BYTES {
        return Err(format!("{shown} is too large to import ({len} bytes)."));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {shown}: {e}"))?;
    // IIF files are often saved in the Windows code page, not UTF-8; keep
    // what can be read rather than refusing the file.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Add IIF File…, a CSV list choice with Add CSV File…, and the files
/// picked so far (each with a remove button).
pub(crate) fn picker(ui: &mut Ui, salt: &str, picks: &mut ImportPicks) {
    ui.horizontal(|ui| {
        let iif = ui.button("Add IIF File…").on_hover_text(
            "An IIF list export, for example from QuickBooks Desktop: File > Utilities > \
             Export > Lists to IIF Files. Brings in accounts, customers, vendors and items.",
        );
        if iif.clicked() {
            if let Some(path) = crate::dialogs::pick_file(
                crate::dialogs::Folder::ListImport,
                None,
                &[("IIF file", &["iif", "txt"])],
            ) {
                picks.files.push(ImportFile { path, csv: None });
            }
        }
        ui.separator();
        egui::ComboBox::from_id_salt(("csv_kind", salt))
            .selected_text(picks.csv_kind.label())
            .width(170.0)
            .show_ui(ui, |ui| {
                for kind in ListKind::ALL {
                    ui.selectable_value(&mut picks.csv_kind, kind, kind.label());
                }
            });
        let csv = ui.button("Add CSV File…").on_hover_text(format!(
            "A CSV file with a header row holding this list. Columns read: {}. Only the \
             name is required.",
            picks.csv_kind.columns()
        ));
        if csv.clicked() {
            if let Some(path) = crate::dialogs::pick_file(
                crate::dialogs::Folder::ListImport,
                None,
                &[("CSV file", &["csv", "txt"])],
            ) {
                picks.files.push(ImportFile {
                    path,
                    csv: Some(picks.csv_kind),
                });
            }
        }
    });
    let mut remove = None;
    for (i, file) in picks.files.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("•  {}", file.label()));
            if ui
                .small_button("x")
                .on_hover_text("Don't import this file")
                .clicked()
            {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove.filter(|i| *i < picks.files.len()) {
        picks.files.remove(i);
    }
}

// ----- File > Import Lists… -------------------------------------------------------

#[derive(Default)]
pub(crate) struct ImportListsForm {
    pub picks: ImportPicks,
    error: Option<String>,
}

impl ImportListsForm {
    pub(crate) fn set_error(&mut self, e: String) {
        self.error = Some(e);
    }
}

pub(crate) enum ImportListsOutcome {
    /// Boxed: a command is large.
    Import(Box<Command>),
    Cancel,
}

pub(crate) fn import_lists_window(
    ctx: &egui::Context,
    form: &mut ImportListsForm,
) -> Option<ImportListsOutcome> {
    let mut outcome = None;
    let mut open = true;
    egui::Window::new("Import Lists")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_max_width(560.0);
            ui.label(
                RichText::new(
                    "Add accounts, customers, vendors and products and services from IIF or \\
                     CSV files to this company. Anything with a name already in the company \\
                     is skipped and listed afterwards, so importing the same file twice adds \\
                     nothing. Edit > Undo takes the whole import back.",
                )
                .color(theme::MUTED),
            );
            ui.add_space(8.0);
            picker(ui, "import_lists", &mut form.picks);
            if let Some(e) = &form.error {
                ui.add_space(4.0);
                ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let import = ui.add_enabled(
                    !form.picks.is_empty(),
                    egui::Button::new(RichText::new("Import").strong()),
                );
                if import.clicked() {
                    match form.picks.read() {
                        Ok(files) => {
                            outcome = Some(ImportListsOutcome::Import(Box::new(
                                Command::ImportLists { files },
                            )));
                        }
                        Err(e) => form.error = Some(e),
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(ImportListsOutcome::Cancel);
                }
            });
        });
    if !open {
        outcome = Some(ImportListsOutcome::Cancel);
    }
    outcome
}

// ----- File > Opening Balances… -----------------------------------------------------

/// The Opening Balances page: a date, and an amount for each account,
/// customer and vendor (as typed).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct OpeningState {
    pub date: String,
    pub accounts: Vec<(AccountId, String)>,
    pub customers: Vec<(CustomerId, String)>,
    pub vendors: Vec<(VendorId, String)>,
}

impl OpeningState {
    /// A blank entry for every active account, customer and vendor that can
    /// take an opening balance, as of the first day of the current year.
    pub(crate) fn for_company(c: &Company) -> Self {
        let today = Date::today();
        let start = Date::from_ymd(today.year(), 1, 1).unwrap_or(today);
        Self {
            date: start.add_days(-1).to_string(),
            accounts: c
                .accounts_in_tree_order()
                .into_iter()
                .filter(|(a, _)| {
                    a.active
                        && takes_opening_balance(a.kind)
                        && !a.name.eq_ignore_ascii_case(ledgercraft_ledger::OPENING_BALANCE_EQUITY)
                })
                .map(|(a, _)| (a.id, String::new()))
                .collect(),
            customers: c
                .customers
                .iter()
                .filter(|x| x.active)
                .map(|x| (x.id, String::new()))
                .collect(),
            vendors: c
                .vendors
                .iter()
                .filter(|x| x.active)
                .map(|x| (x.id, String::new()))
                .collect(),
        }
    }

    /// The command entering what was typed, or what is wrong with it.
    pub(crate) fn build(&self) -> Result<Command, String> {
        let date = Date::parse(&self.date).map_err(|e| format!("As of: {e}"))?;
        fn amounts<T: Copy>(rows: &[(T, String)]) -> Result<Vec<(T, Money)>, String> {
            rows.iter()
                .filter(|(_, text)| !text.trim().is_empty())
                .map(|(id, text)| {
                    Money::parse(text)
                        .map(|m| (*id, m))
                        .map_err(|e| format!("{text:?}: {e}"))
                })
                .collect()
        }
        let balances = OpeningBalances {
            accounts: amounts(&self.accounts)?,
            customers: amounts(&self.customers)?,
            vendors: amounts(&self.vendors)?,
        };
        if balances.is_empty() {
            return Err("Enter at least one opening balance.".into());
        }
        Ok(Command::CompanyOpeningBalances { date, balances })
    }

    /// Fills in amounts from a CSV file with Name and Balance columns (an
    /// account's name or number, a customer's or a vendor's name). Says
    /// which names matched nothing.
    pub(crate) fn load_csv(&mut self, c: &Company, text: &str) -> Result<Vec<String>, String> {
        let rows = ledgercraft_file::import::parse_rows(text);
        let mut rows = rows.into_iter();
        let header: Vec<String> = rows
            .next()
            .ok_or("the file is empty")?
            .iter()
            .map(|h| h.trim().to_uppercase())
            .collect();
        let col = |names: &[&str]| header.iter().position(|h| names.contains(&h.as_str()));
        let name_col = col(&["NAME", "ACCOUNT", "CUSTOMER", "VENDOR"])
            .ok_or("no Name column")?;
        let amount_col = col(&["BALANCE", "AMOUNT", "OPENING BALANCE"])
            .ok_or("no Balance column")?;
        let mut unmatched = Vec::new();
        for row in rows {
            let (Some(name), Some(amount)) = (row.get(name_col), row.get(amount_col)) else {
                continue;
            };
            let (name, amount) = (name.trim(), amount.trim().to_string());
            if name.is_empty() {
                continue;
            }
            let account = c.account_by_name(name).map(|a| a.id);
            let customer = c
                .customers
                .iter()
                .find(|x| x.contact.name.eq_ignore_ascii_case(name))
                .map(|x| x.id);
            let vendor = c
                .vendors
                .iter()
                .find(|x| x.contact.name.eq_ignore_ascii_case(name))
                .map(|x| x.id);
            let slot = account
                .and_then(|id| self.accounts.iter_mut().find(|(a, _)| *a == id))
                .map(|(_, t)| t)
                .or_else(|| {
                    customer
                        .and_then(|id| self.customers.iter_mut().find(|(x, _)| *x == id))
                        .map(|(_, t)| t)
                })
                .or_else(|| {
                    vendor
                        .and_then(|id| self.vendors.iter_mut().find(|(x, _)| *x == id))
                        .map(|(_, t)| t)
                });
            match slot {
                Some(text) => *text = amount,
                None => unmatched.push(name.to_string()),
            }
        }
        Ok(unmatched)
    }

    fn total<T>(rows: &[(T, String)]) -> Money {
        rows.iter()
            .filter_map(|(_, t)| Money::parse(t).ok())
            .fold(Money::ZERO, Money::saturating_add)
    }
}

/// One section of the page: a heading, then a name and amount box per row.
fn amount_rows<T: Copy>(
    ui: &mut Ui,
    salt: &str,
    heading: &str,
    hint: &str,
    rows: &mut [(T, String)],
    name: impl Fn(T) -> String,
) {
    ui.add_space(8.0);
    ui.label(RichText::new(heading).strong());
    ui.label(RichText::new(hint).small().color(theme::MUTED));
    if rows.is_empty() {
        ui.label(RichText::new("None yet.").color(theme::MUTED));
        return;
    }
    egui::Grid::new(("opening", salt))
        .striped(true)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            for (id, text) in rows.iter_mut() {
                ui.label(name(*id));
                field(ui, text, 110.0, "0.00");
                ui.end_row();
            }
        });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Total").strong());
        money_label(ui, OpeningState::total(rows), true);
    });
}

pub(crate) fn opening_page(ui: &mut Ui, c: &Company, s: &mut OpeningState, out: &mut Vec<Action>) {
    ui.label(RichText::new("Opening Balances").heading().color(theme::ACCENT));
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Where the books stood on the day before LedgerCraft takes over: the balance of \\
             each bank account, loan and card on your last statement, and what each customer \\
             owed you and you owed each vendor. Leave a box empty for nothing. Customers \\
             get an opening invoice and vendors an opening bill, so payments apply to them; \\
             everything is balanced against Opening Balance Equity.",
        )
        .color(theme::MUTED),
    );
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "As of");
        field(ui, &mut s.date, 110.0, "YYYY-MM-DD");
        ui.separator();
        let load = ui.button("Fill From CSV…").on_hover_text(
            "A CSV file with Name and Balance columns: an account's name or number, or a \\
             customer's or vendor's name, and its balance",
        );
        if load.clicked() {
            if let Some(path) = crate::dialogs::pick_file(
                crate::dialogs::Folder::ListImport,
                None,
                &[("CSV file", &["csv", "txt"])],
            ) {
                match read_import(&path).and_then(|text| s.load_csv(c, &text)) {
                    Ok(unmatched) if unmatched.is_empty() => out.push(Action::ClearStatus),
                    Ok(unmatched) => out.push(Action::Error(format!(
                        "Filled in what matched. Not found: {}.",
                        unmatched.join(", ")
                    ))),
                    Err(e) => out.push(Action::Error(e)),
                }
            }
        }
    });
    amount_rows(
        ui,
        "accounts",
        "Accounts",
        "Bank, savings, cards, loans and other balance sheet accounts. Positive is the \\
         usual balance: money in the bank, or what you owe on a card or loan. An \\
         overdrawn account is negative.",
        &mut s.accounts,
        |id| c.account_full_name(id),
    );
    amount_rows(
        ui,
        "customers",
        "What customers owe you",
        "Each becomes an opening invoice, due on the customer's terms.",
        &mut s.customers,
        |id| {
            c.customer(id)
                .map(|x| x.contact.name.clone())
                .unwrap_or_default()
        },
    );
    amount_rows(
        ui,
        "vendors",
        "What you owe vendors",
        "Each becomes an opening bill, due on the vendor's terms.",
        &mut s.vendors,
        |id| {
            c.vendor(id)
                .map(|x| x.contact.name.clone())
                .unwrap_or_default()
        },
    );
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new("Enter Opening Balances").strong())
            .on_hover_text("Record them. Edit > Undo takes them all back.")
            .clicked()
        {
            match s.build() {
                Ok(cmd) => out.push(Action::Run {
                    cmd,
                    reset: Reset::OpeningBalances,
                }),
                Err(e) => out.push(Action::Error(e)),
            }
        }
        if ui.button("Clear").clicked() {
            *s = OpeningState::for_company(c);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_balances_fill_from_csv_and_build() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let mut s = OpeningState::for_company(&c);
        assert!(s.build().is_err(), "nothing entered");
        let csv = "Name,Balance\\nChecking,\\"1,250.00\\"\\nMaple Street Cafe,300\\nNobody,5\\n";
        let unmatched = s.load_csv(&c, csv).unwrap();
        assert_eq!(unmatched, ["Nobody"]);
        let cmd = s.build().unwrap();
        assert!(
            matches!(&cmd, Command::CompanyOpeningBalances { balances, .. }
                if balances.accounts.len() == 1 && balances.customers.len() == 1),
            "{cmd:?}"
        );
        s.date = "someday".into();
        assert!(s.build().is_err());
    }

    #[test]
    fn the_page_draws_without_actions() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let mut s = OpeningState::for_company(&c);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = Vec::new();
        crate::theme::test_frame(&ctx, |ui| opening_page(ui, &c, &mut s, &mut out));
        assert!(out.is_empty());
    }
}
