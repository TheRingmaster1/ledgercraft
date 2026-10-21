//! Banking > Import Bank Transactions: read a bank or credit card download
//! (OFX, QFX, QBO or CSV), decide what each line is (a transaction already
//! recorded, a new one in a category, or nothing), and bring it in with one
//! command that one Undo reverses. Then reconcile against the same statement.

use std::collections::BTreeSet;

use eframe::egui::{self, RichText, Ui};
use ledgercraft_core::Date;
use ledgercraft_engine::Command;
use ledgercraft_file::bankfeed::{read_statement, Statement};
use ledgercraft_ledger::{AccountId, AccountType, Company, FeedAction, FeedEntry, FeedLine, TxnId};

use crate::widgets::{self, money_label};
use crate::{currency, theme, Action, Reset};

/// What to do with one downloaded line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Match,
    Add,
    Skip,
}

struct Row {
    line: FeedLine,
    /// Imported before: shown, but never brought in again.
    duplicate: bool,
    /// The recorded transaction it most likely is.
    candidate: Option<TxnId>,
    choice: Choice,
    category: Option<AccountId>,
    remember: bool,
}

/// What the Import Bank Transactions page holds.
#[derive(Default)]
pub(crate) struct ImportState {
    account: Option<AccountId>,
    file_name: String,
    statement: Option<Statement>,
    rows: Vec<Row>,
    /// Set when the import command is sent: the number of imported lines
    /// then, so the next frame can tell whether it went through.
    sent: Option<usize>,
    /// The last import went through: offer to reconcile.
    done: Option<String>,
    /// How many lines the books held as imported when the page last looked;
    /// a change it did not cause (Undo, Redo) means the rows are stale.
    links_seen: usize,
}

/// Keeps the page in step with the books: notices an import going through,
/// and an Undo or Redo of one, which brings the download's lines back.
fn sync(c: &Company, s: &mut ImportState) {
    let now = c.feed_links.len();
    if let Some(before) = s.sent.take() {
        if let (true, Some(account)) = (now > before, s.account) {
            let count = now - before;
            s.done = Some(format!(
                "{count} lines imported into {}.",
                c.account_name(account)
            ));
            s.rows.clear();
        }
    } else if now != s.links_seen {
        s.done = None;
        if let (Some(account), Some(statement)) = (s.account, &s.statement) {
            s.rows = build_rows(c, account, statement);
        }
    }
    s.links_seen = now;
}

fn is_feed_account(kind: AccountType) -> bool {
    matches!(kind, AccountType::Bank | AccountType::CreditCard)
}

/// Decides each line's first choice: imported before, a match, or an add
/// (with a rule's category when one applies).
fn build_rows(c: &Company, account: AccountId, statement: &Statement) -> Vec<Row> {
    let mut taken = BTreeSet::new();
    statement
        .lines
        .iter()
        .map(|line| {
            let duplicate = c.is_imported(account, &line.fit_id);
            let candidate = if duplicate {
                None
            } else {
                c.suggest_match(account, line, &taken)
            };
            if let Some(id) = candidate {
                taken.insert(id);
            }
            let category = c.rule_for(line).map(|r| r.category);
            Row {
                line: line.clone(),
                duplicate,
                candidate,
                choice: if candidate.is_some() {
                    Choice::Match
                } else {
                    Choice::Add
                },
                category,
                remember: false,
            }
        })
        .collect()
}

pub(crate) fn page(ui: &mut Ui, c: &Company, s: &mut ImportState, out: &mut Vec<Action>) {
    egui::ScrollArea::both()
        .id_salt("import_page")
        .auto_shrink(false)
        .show(ui, |ui| body(ui, c, s, out));
}

fn body(ui: &mut Ui, c: &Company, s: &mut ImportState, out: &mut Vec<Action>) {
    ui.label(
        RichText::new("Import Bank Transactions")
            .heading()
            .color(theme::ACCENT),
    );
    ui.label(
        RichText::new(
            "Bring in a download from your bank or card website (OFX, QFX, QBO or CSV). \
             Each line is matched to a transaction you already recorded, or added as a new \
             one; nothing changes until you click Import.",
        )
        .color(theme::MUTED),
    );
    ui.add_space(6.0);

    sync(c, s);

    let before = s.account;
    ui.horizontal_wrapped(|ui| {
        widgets::hint_label(ui, "Account");
        widgets::account_combo(ui, "import_account", c, &mut s.account, is_feed_account);
        if ui.button("Choose File…").clicked() {
            choose_file(c, s, out);
        }
        if !s.file_name.is_empty() {
            ui.label(RichText::new(s.file_name.as_str()).color(theme::MUTED));
        }
    });
    let Some(account) = s.account.filter(|a| c.can_reconcile(*a)) else {
        ui.add_space(8.0);
        ui.label("Choose the bank or credit card account the download is for, then the file.");
        rules(ui, c, out);
        return;
    };
    if s.account != before {
        if let Some(statement) = &s.statement {
            s.rows = build_rows(c, account, statement);
        }
        s.done = None;
    }

    if let Some(done) = s.done.clone() {
        finished(ui, c, account, s, &done, out);
    }
    if s.rows.is_empty() {
        if s.done.is_none() {
            ui.add_space(8.0);
            ui.label("Choose File… to pick the download you saved from your bank.");
        }
        rules(ui, c, out);
        return;
    }

    lines_table(ui, c, s);
    import_button(ui, c, account, s, out);
    rules(ui, c, out);
}

fn choose_file(c: &Company, s: &mut ImportState, out: &mut Vec<Action>) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Bank download", &["ofx", "qfx", "qbo", "csv", "txt"])
        .pick_file()
    else {
        return;
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let read = std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| read_statement(&String::from_utf8_lossy(&bytes)));
    match read {
        Ok(statement) => {
            if !statement.skipped.is_empty() {
                out.push(Action::Error(format!(
                    "{} line(s) of {name} could not be read: {}",
                    statement.skipped.len(),
                    statement.skipped.join("; ")
                )));
            }
            s.rows = match s.account {
                Some(account) => build_rows(c, account, &statement),
                None => Vec::new(),
            };
            s.statement = Some(statement);
            s.file_name = name;
            s.done = None;
            s.links_seen = c.feed_links.len();
        }
        Err(e) => out.push(Action::Error(format!("Could not read {name}: {e}"))),
    }
}

fn lines_table(ui: &mut Ui, c: &Company, s: &mut ImportState) {
    ui.add_space(8.0);
    egui::Grid::new("import_lines")
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for h in ["Date", "Payee / memo", "Amount", "Action"] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for (i, row) in s.rows.iter_mut().enumerate() {
                let muted = |text: String| {
                    if row.duplicate || row.choice == Choice::Skip {
                        RichText::new(text).color(theme::MUTED)
                    } else {
                        RichText::new(text)
                    }
                };
                ui.label(muted(row.line.date.to_string()));
                let mut text = row.line.description().to_string();
                if !row.line.number.trim().is_empty() {
                    text = format!("{text}  #{}", row.line.number.trim());
                }
                ui.label(muted(text));
                money_label(ui, row.line.amount, false);
                if row.duplicate {
                    ui.label(RichText::new("Already imported").color(theme::MUTED));
                    ui.end_row();
                    continue;
                }
                let matched = row.candidate.and_then(|id| c.txn(id));
                let label = |choice: Choice| match (choice, matched) {
                    (Choice::Match, Some(t)) => {
                        format!("Match {} {} ({})", c.txn_type_label(t), t.number, t.date)
                    }
                    (Choice::Match, None) => "Match".to_string(),
                    (Choice::Add, _) => "Add".to_string(),
                    (Choice::Skip, _) => "Skip".to_string(),
                };
                // The choice, with the category under it when adding, so the row
                // stays as narrow as the window.
                ui.vertical(|ui| {
                    egui::ComboBox::from_id_salt(("import_choice", i))
                        .selected_text(label(row.choice))
                        .width(250.0)
                        .show_ui(ui, |ui| {
                            if matched.is_some() {
                                ui.selectable_value(&mut row.choice, Choice::Match, label(Choice::Match));
                            }
                            ui.selectable_value(&mut row.choice, Choice::Add, "Add as a new transaction");
                            ui.selectable_value(&mut row.choice, Choice::Skip, "Skip");
                        });
                    if row.choice == Choice::Add {
                        ui.horizontal_wrapped(|ui| {
                            widgets::account_combo(
                                ui,
                                ("import_category", i),
                                c,
                                &mut row.category,
                                |k| !k.needs_party(),
                            );
                            ui.checkbox(&mut row.remember, "Remember")
                                .on_hover_text("Use this category for this payee next time");
                        });
                    }
                });
                ui.end_row();
            }
        });
}

fn import_button(
    ui: &mut Ui,
    c: &Company,
    account: AccountId,
    s: &mut ImportState,
    out: &mut Vec<Action>,
) {
    let live = s.rows.iter().filter(|r| !r.duplicate);
    let (mut matches, mut adds, mut skips) = (0, 0, 0);
    let mut missing = 0;
    for r in live {
        match r.choice {
            Choice::Match => matches += 1,
            Choice::Add if r.category.is_none() => missing += 1,
            Choice::Add => adds += 1,
            Choice::Skip => skips += 1,
        }
    }
    let duplicates = s.rows.iter().filter(|r| r.duplicate).count();
    ui.add_space(8.0);
    ui.label(format!(
        "{matches} to match, {adds} to add, {skips} to skip, {duplicates} imported before."
    ));
    if missing > 0 {
        ui.label(
            RichText::new(format!(
                "Next: choose a category for the {missing} line(s) to add, or set them to Skip."
            ))
            .color(theme::NEGATIVE),
        );
    }
    let ready = missing == 0 && matches + adds > 0;
    let import = ui
        .add_enabled(ready, egui::Button::new(RichText::new("Import").strong()))
        .on_disabled_hover_text("Choose a category for each line to add, or skip it.");
    if import.clicked() {
        let entries: Vec<FeedEntry> = s
            .rows
            .iter()
            .filter(|r| !r.duplicate)
            .filter_map(|r| {
                let action = match (r.choice, r.candidate, r.category) {
                    (Choice::Match, Some(txn), _) => FeedAction::Match { txn },
                    (Choice::Add, _, Some(category)) => FeedAction::Add {
                        category,
                        remember: r.remember,
                    },
                    _ => return None,
                };
                Some(FeedEntry {
                    line: r.line.clone(),
                    action,
                })
            })
            .collect();
        s.sent = Some(c.feed_links.len());
        out.push(Action::Run {
            cmd: Command::BankImport { account, entries },
            reset: Reset::Nothing,
        });
    }
}

/// After an import: what happened, and reconciling against the statement.
fn finished(
    ui: &mut Ui,
    c: &Company,
    account: AccountId,
    s: &ImportState,
    done: &str,
    out: &mut Vec<Action>,
) {
    ui.add_space(8.0);
    ui.label(RichText::new(done).color(theme::POSITIVE));
    let balance = s.statement.as_ref().and_then(|st| {
        let amount = st.ending_balance.and_then(|b| c.feed_amount(account, b))?;
        Some((st.balance_date.unwrap_or_else(Date::today), amount))
    });
    match balance {
        Some((date, amount)) => {
            ui.label(format!(
                "The download says the balance was {} on {date}.",
                currency::format(amount)
            ));
            if ui
                .button("Reconcile with This Statement")
                .on_hover_text(
                    "Open Reconcile with the statement date and ending balance filled in",
                )
                .clicked()
            {
                out.push(Action::ReconcileStatement {
                    account,
                    date,
                    balance: amount,
                });
            }
        }
        None => {
            if ui.button("Reconcile…").clicked() {
                out.push(Action::Reconcile(account));
            }
        }
    }
}

/// The saved rules, with Delete.
fn rules(ui: &mut Ui, c: &Company, out: &mut Vec<Action>) {
    if c.bank_rules.is_empty() {
        return;
    }
    ui.add_space(12.0);
    ui.separator();
    ui.label(RichText::new("Rules").strong());
    ui.label(
        RichText::new("When a payee contains this text, new lines are added in this category.")
            .small()
            .color(theme::MUTED),
    );
    egui::Grid::new("bank_rules")
        .striped(true)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (index, rule) in c.bank_rules.iter().enumerate() {
                ui.label(format!("\"{}\"", rule.contains));
                ui.label(c.account_name(rule.category));
                if ui.small_button("Delete").clicked() {
                    out.push(Action::Run {
                        cmd: Command::BankRemoveRule { index },
                        reset: Reset::Nothing,
                    });
                }
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledgercraft_core::Money;
    use ledgercraft_ledger::{AccountLine, Check, TxnBody};

    #[test]
    fn rows_start_as_match_add_or_already_imported() {
        let mut c = Company::with_default_accounts("Rows Co");
        let checking = c.account_by_name("Checking").unwrap().id;
        let rent = c.account_by_name("Rent").unwrap().id;
        let check = TxnBody::Check(Check {
            payee: None,
            pay_from: checking,
            lines: vec![AccountLine {
                account: rent,
                description: String::new(),
                amount: Money::from_cents(20_000),
            }],
        });
        let date = Date::parse("2026-01-05").unwrap();
        let id = c.post(date, "101", "", check).unwrap();
        let line = |amount: i64, fit: &str| FeedLine {
            date,
            amount: Money::from_cents(amount),
            payee: "x".into(),
            memo: String::new(),
            number: String::new(),
            fit_id: fit.into(),
        };
        let statement = Statement {
            lines: vec![line(-20_000, "A"), line(-20_000, "B"), line(500, "C")],
            ..Statement::default()
        };
        let rows = build_rows(&c, checking, &statement);
        assert_eq!(rows[0].choice, Choice::Match);
        assert_eq!(rows[0].candidate, Some(id));
        // The same transaction is not offered to a second line.
        assert_eq!(rows[1].choice, Choice::Add);
        assert_eq!(rows[2].choice, Choice::Add);
        assert!(rows.iter().all(|r| !r.duplicate));
    }

    #[test]
    fn the_sample_download_matches_the_sample_company() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let checking = c.account_by_name("Checking").unwrap().id;
        let text = include_str!("../../../docs/samples/pine-ridge-checking.ofx");
        let statement = read_statement(text).unwrap();
        let rows = build_rows(&c, checking, &statement);
        // Check 1001 and the February deposit are already in the books.
        assert_eq!(rows.iter().filter(|r| r.choice == Choice::Match).count(), 2);
        assert_eq!(rows.iter().filter(|r| r.choice == Choice::Add).count(), 3);
    }

    #[test]
    fn the_page_draws_in_a_narrow_window() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let checking = c.account_by_name("Checking").unwrap().id;
        let text = include_str!("../../../docs/samples/pine-ridge-checking.ofx");
        let statement = read_statement(text).unwrap();
        let mut s = ImportState {
            account: Some(checking),
            rows: build_rows(&c, checking, &statement),
            statement: Some(statement),
            ..ImportState::default()
        };
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = Vec::new();
        crate::theme::test_frame_sized(&ctx, egui::vec2(480.0, 600.0), |ui| {
            page(ui, &c, &mut s, &mut out);
        });
        assert!(out.is_empty());
        assert_eq!(s.rows.len(), 5);
    }

    #[test]
    fn undoing_an_import_brings_the_lines_back() {
        let before = ledgercraft_engine::sample_company().unwrap();
        let checking = before.account_by_name("Checking").unwrap().id;
        let text = include_str!("../../../docs/samples/pine-ridge-checking.ofx");
        let statement = read_statement(text).unwrap();
        let mut s = ImportState {
            account: Some(checking),
            rows: build_rows(&before, checking, &statement),
            statement: Some(statement.clone()),
            ..ImportState::default()
        };
        sync(&before, &mut s);

        // Import the two lines that match recorded transactions.
        let mut after = before.clone();
        let entries = s
            .rows
            .iter()
            .filter_map(|r| {
                Some(FeedEntry {
                    line: r.line.clone(),
                    action: FeedAction::Match { txn: r.candidate? },
                })
            })
            .collect();
        assert_eq!(after.import_feed(checking, entries).unwrap(), 2);
        s.sent = Some(before.feed_links.len());
        sync(&after, &mut s);
        assert!(s.done.is_some() && s.rows.is_empty());

        // Undo puts the books back: the lines show again, ready to import.
        sync(&before, &mut s);
        assert!(s.done.is_none());
        assert_eq!(s.rows.len(), 5);
        assert!(s.rows.iter().all(|r| !r.duplicate));
    }
}
