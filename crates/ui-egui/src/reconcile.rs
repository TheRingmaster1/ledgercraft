//! Banking > Reconcile: tick off a bank or credit card statement against the
//! books. The page keeps what is ticked until the reconciliation is saved;
//! the engine refuses it unless the cleared items match the statement exactly.

use std::collections::BTreeSet;

use eframe::egui::{self, RichText, Ui};
use ledgercraft_core::{Date, Money};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{AccountId, AccountType, Company, ReconcileItem, TxnId};
use ledgercraft_reports::reconciliation_report;

use crate::widgets::{self, field, money_label};
use crate::{currency, theme, Action, Reset};

/// What is typed and ticked on the Reconcile page.
#[derive(Default)]
pub(crate) struct ReconcileState {
    account: Option<AccountId>,
    statement_date: String,
    ending_balance: String,
    cleared: BTreeSet<TxnId>,
    /// How many reconciliations the account had when last drawn; one more
    /// means the last was just saved, so its report is shown.
    known: Option<(AccountId, usize)>,
    /// The reconciliation whose report is shown (index, oldest first).
    report: Option<usize>,
    /// Undo Last Reconciliation was clicked once; a second click confirms.
    confirm_undo: bool,
}

impl ReconcileState {
    /// Starts reconciling `account` (from a register, say).
    pub(crate) fn start(&mut self, account: AccountId) {
        if self.account != Some(account) {
            *self = Self {
                account: Some(account),
                ..Self::default()
            };
        }
    }
}

/// Width of the summary's amount column.
const SUMMARY_AMOUNT_WIDTH: f32 = 140.0;

fn is_reconcilable(kind: AccountType) -> bool {
    matches!(kind, AccountType::Bank | AccountType::CreditCard)
}

/// The two lists' headings, by account type.
fn list_titles(kind: Option<AccountType>) -> (&'static str, &'static str) {
    match kind {
        Some(AccountType::CreditCard) => ("Charges and cash advances", "Payments and credits"),
        _ => ("Deposits and other credits", "Checks and payments"),
    }
}

pub(crate) fn page(ui: &mut Ui, c: &Company, s: &mut ReconcileState, out: &mut Vec<Action>) {
    // The two lists share what height the window has; the page scrolls as a
    // whole when a report is open below them.
    let list_height = (ui.available_height() - 300.0).max(140.0);
    egui::ScrollArea::vertical()
        .id_salt("rec_page")
        .auto_shrink(false)
        .show(ui, |ui| body(ui, c, s, out, list_height));
}

fn body(ui: &mut Ui, c: &Company, s: &mut ReconcileState, out: &mut Vec<Action>, list_height: f32) {
    ui.label(RichText::new("Reconcile").heading().color(theme::ACCENT));
    ui.label(
        RichText::new(
            "Tick each item that appears on your statement. When the difference is $0.00 \
             the books agree with the bank, and you can reconcile.",
        )
        .color(theme::MUTED),
    );
    ui.add_space(6.0);
    let before = s.account;
    egui::Grid::new("rec_head")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Account");
            widgets::account_combo(ui, "rec_account", c, &mut s.account, is_reconcilable);
            widgets::hint_label(ui, "Statement date");
            field(ui, &mut s.statement_date, 110.0, "YYYY-MM-DD");
            ui.end_row();
            ui.label("");
            ui.label("");
            widgets::hint_label(ui, "Ending balance");
            field(ui, &mut s.ending_balance, 110.0, "from statement");
            ui.end_row();
        });
    if s.account != before {
        let account = s.account;
        *s = ReconcileState {
            account,
            ..ReconcileState::default()
        };
    }
    let Some(account) = s.account.filter(|a| c.can_reconcile(*a)) else {
        ui.add_space(8.0);
        ui.label("Choose a bank or credit card account to reconcile.");
        return;
    };
    if s.statement_date.trim().is_empty() {
        s.statement_date = Date::today().to_string();
    }
    let kind = c.account(account).map(|a| a.kind);

    // A reconciliation just saved: start afresh and show its report.
    let count = c.reconciliations_for(account).len();
    if let Some((known_account, known)) = s.known {
        if known_account == account && count > known {
            *s = ReconcileState {
                account: Some(account),
                report: count.checked_sub(1),
                ..ReconcileState::default()
            };
            s.statement_date = Date::today().to_string();
        }
    }
    s.known = Some((account, count));

    let beginning = c.reconciled_balance(account);
    match c.last_reconciliation(account) {
        Some(last) => ui.label(format!(
            "Last reconciled for the statement ending {}. Beginning balance {}.",
            last.statement_date,
            currency::format(beginning)
        )),
        None => ui.label(format!(
            "Not reconciled before. Beginning balance {}.",
            currency::format(beginning)
        )),
    };

    let Ok(through) = Date::parse(&s.statement_date) else {
        ui.label(RichText::new("Enter the statement date as YYYY-MM-DD.").color(theme::NEGATIVE));
        return;
    };
    let items = c.unreconciled(account, through);
    // Forget ticks on items no longer listed (voided, or dated later now).
    let listed: BTreeSet<TxnId> = items.iter().map(|i| i.txn).collect();
    s.cleared.retain(|id| listed.contains(id));

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Mark All").clicked() {
            s.cleared = listed.clone();
        }
        if ui.button("Unmark All").clicked() {
            s.cleared.clear();
        }
    });
    let (up_title, down_title) = list_titles(kind);
    let (ups, downs): (Vec<&ReconcileItem>, Vec<&ReconcileItem>) =
        items.iter().partition(|i| !i.amount.is_negative());
    ui.columns(2, |cols| {
        if let [left, right] = cols {
            item_list(
                left,
                "rec_up",
                up_title,
                c,
                &ups,
                &mut s.cleared,
                list_height,
            );
            item_list(
                right,
                "rec_down",
                down_title,
                c,
                &downs,
                &mut s.cleared,
                list_height,
            );
        }
    });

    // The summary, and Reconcile Now once it balances.
    let picked = |want_up: bool| -> (usize, Money) {
        items
            .iter()
            .filter(|i| s.cleared.contains(&i.txn) && (!i.amount.is_negative()) == want_up)
            .fold((0, Money::ZERO), |(n, total), i| {
                (n + 1, total.saturating_add(i.amount))
            })
    };
    let (n_up, up) = picked(true);
    let (n_down, down) = picked(false);
    let cleared_balance = beginning.saturating_add(up).saturating_add(down);
    // Nothing typed yet: no ending balance, so no difference to show.
    let ending = if s.ending_balance.trim().is_empty() {
        None
    } else {
        Some(Money::parse(&s.ending_balance))
    };
    ui.add_space(8.0);
    // Amounts in one right-aligned column of fixed width.
    let amount = |ui: &mut Ui, text: RichText| {
        let height = ui.spacing().interact_size.y;
        ui.allocate_ui_with_layout(
            egui::vec2(SUMMARY_AMOUNT_WIDTH, height),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(text);
            },
        );
    };
    let money = |m: Money| RichText::new(currency::format(m));
    egui::Grid::new("rec_summary")
        .num_columns(2)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            ui.label("Beginning balance");
            amount(ui, money(beginning));
            ui.end_row();
            ui.label(format!("{n_up} {}", up_title.to_lowercase()));
            amount(ui, money(up));
            ui.end_row();
            ui.label(format!("{n_down} {}", down_title.to_lowercase()));
            amount(ui, money(down));
            ui.end_row();
            ui.label(RichText::new("Cleared balance").strong());
            amount(ui, money(cleared_balance).strong());
            ui.end_row();
            ui.label("Ending balance");
            match &ending {
                Some(Ok(e)) => amount(ui, money(*e)),
                Some(Err(_)) => amount(ui, RichText::new("not a number").color(theme::NEGATIVE)),
                None => amount(ui, RichText::new("—").color(theme::MUTED)),
            }
            ui.end_row();
            ui.label(RichText::new("Difference").strong());
            match &ending {
                Some(Ok(e)) => {
                    let difference = e.saturating_sub(cleared_balance);
                    let colour = if difference.is_zero() {
                        theme::POSITIVE
                    } else {
                        theme::NEGATIVE
                    };
                    amount(ui, money(difference).strong().color(colour));
                }
                _ => amount(ui, RichText::new("")),
            }
            ui.end_row();
        });
    ui.add_space(6.0);
    let balances = matches!(&ending, Some(Ok(e)) if *e == cleared_balance);
    guidance(ui, ending.as_ref(), balances);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let now = ui
            .add_enabled(
                balances,
                egui::Button::new(RichText::new("Reconcile Now").strong()),
            )
            .on_disabled_hover_text("Tick items until the difference is $0.00.");
        if now.clicked() {
            if let Some(Ok(ending_balance)) = ending {
                out.push(Action::Run {
                    cmd: Command::AccountReconcile {
                        account,
                        statement_date: through,
                        ending_balance,
                        cleared: s.cleared.iter().copied().collect(),
                    },
                    reset: Reset::Nothing,
                });
            }
        }
        if count > 0 {
            let label = if s.confirm_undo {
                "Click again to undo it"
            } else {
                "Undo Last Reconciliation"
            };
            let undo = ui.button(label).on_hover_text(
                "Un-reconciles the items of the last statement, so they can be changed or \
                 reconciled again.",
            );
            if undo.clicked() {
                if s.confirm_undo {
                    out.push(Action::Run {
                        cmd: Command::AccountUndoReconciliation { account },
                        reset: Reset::Nothing,
                    });
                    s.confirm_undo = false;
                    s.report = None;
                } else {
                    s.confirm_undo = true;
                }
            }
        }
    });

    history(ui, c, account, s, out);
}

/// What to do next, in plain words, under the summary.
fn guidance(ui: &mut Ui, ending: Option<&Result<Money, ledgercraft_core::CoreError>>, balances: bool) {
    let (lead, text, colour) = match ending {
        None => (
            "Next: ",
            "find the ending balance printed on your bank or card statement and type it into \
             the Ending balance box at the top of this page.",
            theme::MUTED,
        ),
        Some(Err(_)) => (
            "Check the ending balance: ",
            "it should be a number, such as 1234.56.",
            theme::NEGATIVE,
        ),
        Some(Ok(_)) if balances => (
            "Done checking: ",
            "the books match your statement. Click Reconcile Now to record it.",
            theme::POSITIVE,
        ),
        Some(Ok(_)) => (
            "Next: ",
            "tick each item in the two lists that also appears on your statement. The \
             Difference should reach $0.00. If it will not, look for an amount entered \
             wrongly, an item entered twice, or a bank fee or interest not recorded yet.",
            theme::MUTED,
        ),
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(RichText::new(lead).strong().color(colour));
        ui.label(RichText::new(text).color(colour));
    });
}

/// One of the two lists of items to tick.
fn item_list(
    ui: &mut Ui,
    salt: &str,
    title: &str,
    c: &Company,
    items: &[&ReconcileItem],
    cleared: &mut BTreeSet<TxnId>,
    height: f32,
) {
    ui.label(RichText::new(title).strong());
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            if items.is_empty() {
                ui.label(RichText::new("Nothing to reconcile.").color(theme::MUTED));
                return;
            }
            egui::Grid::new(salt)
                .striped(true)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    for h in ["", "Date", "Num", "Name", "Amount"] {
                        widgets::column_header(ui, h);
                    }
                    ui.end_row();
                    for item in items {
                        let mut ticked = cleared.contains(&item.txn);
                        if ui.checkbox(&mut ticked, "").changed() {
                            if ticked {
                                cleared.insert(item.txn);
                            } else {
                                cleared.remove(&item.txn);
                            }
                        }
                        let t = c.txn(item.txn);
                        ui.label(item.date.to_string());
                        ui.label(t.map(|t| t.number.clone()).unwrap_or_default());
                        ui.label(
                            t.and_then(|t| t.body.party())
                                .map(|p| c.party_name(p))
                                .unwrap_or_default(),
                        );
                        money_label(ui, item.amount.abs(), false);
                        ui.end_row();
                    }
                });
        });
}

/// Earlier reconciliations; the chosen (or just saved) one's report opens in
/// a window.
fn history(
    ui: &mut Ui,
    c: &Company,
    account: AccountId,
    s: &mut ReconcileState,
    out: &mut Vec<Action>,
) {
    let recs = c.reconciliations_for(account);
    if recs.is_empty() {
        return;
    }
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Previous reconciliations").strong());
    ui.horizontal_wrapped(|ui| {
        for (i, r) in recs.iter().enumerate().rev().take(12) {
            let text = format!(
                "{}  {}",
                r.statement_date,
                currency::format(r.ending_balance)
            );
            if ui
                .selectable_label(s.report == Some(i), text)
                .on_hover_text("Open this reconciliation's report")
                .clicked()
            {
                s.report = Some(i);
            }
        }
    });
    let Some(index) = s.report.filter(|i| *i < recs.len()) else {
        return;
    };
    let report = reconciliation_report(c, account, index);
    // The report opens in its own window, with its export buttons.
    let mut open = true;
    let mut close = false;
    egui::Window::new("Reconciliation Report")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([680.0, 520.0])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button("Export to Excel…").clicked() {
                    out.push(Action::ExportXlsx {
                        name: "reconciliation.xlsx".into(),
                        data: report.to_xlsx(currency::current().symbol()),
                    });
                }
                if ui.button("Export CSV…").clicked() {
                    out.push(Action::ExportCsv {
                        name: "reconciliation.csv".into(),
                        csv: report.to_csv(),
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = ui.button("Close").clicked();
                });
            });
            ui.separator();
            widgets::report_view(ui, &report, out);
        });
    if !open || close {
        s.report = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_draws_and_waits_for_a_zero_difference() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let checking = c.account_by_name("Checking").unwrap().id;
        let mut s = ReconcileState::default();
        s.start(checking);
        s.ending_balance = "1.23".into();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = Vec::new();
        crate::theme::test_frame(&ctx, |ui| page(ui, &c, &mut s, &mut out));
        // Nothing ticked and a difference: nothing is reconciled.
        assert!(out.is_empty());
        assert_eq!(s.known, Some((checking, 0)));
        assert!(
            !s.statement_date.is_empty(),
            "the statement date defaults to today"
        );
    }

    #[test]
    fn guidance_draws_for_every_step() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let bad = Money::parse("abc");
        let good = Money::parse("12.00");
        crate::theme::test_frame(&ctx, |ui| {
            guidance(ui, None, false);
            guidance(ui, Some(&bad), false);
            guidance(ui, Some(&good), false);
            guidance(ui, Some(&good), true);
        });
    }

    #[test]
    fn credit_cards_get_their_own_list_titles() {
        assert_eq!(
            list_titles(Some(AccountType::CreditCard)).0,
            "Charges and cash advances"
        );
        assert_eq!(
            list_titles(Some(AccountType::Bank)).1,
            "Checks and payments"
        );
    }
}
