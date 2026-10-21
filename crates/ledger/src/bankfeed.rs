//! Bank feeds: bringing the lines of a bank or credit card download into the
//! books. Each line is matched to a transaction already recorded, added as a
//! new one in a chosen category, or skipped. Every imported line's bank id is
//! remembered, so importing the same download again adds nothing twice.
//! Rules ("payee contains Valley Power -> Utilities") suggest categories.

use std::collections::BTreeSet;

use ledgercraft_core::{Date, Money};
use serde::{Deserialize, Serialize};

use crate::{
    AccountId, AccountLine, AccountType, Check, Company, Deposit, JournalEntry, JournalLine,
    LedgerError, LedgerResult, TxnBody, TxnId,
};

/// How many days a bank's date may differ from the books' for a match.
pub const MATCH_DAYS: i64 = 10;

/// One line of a bank or credit card download.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedLine {
    pub date: Date,
    /// Money into the account is positive (a deposit, a card payment or
    /// refund); money out is negative (a check, a withdrawal, a card charge).
    pub amount: Money,
    #[serde(default)]
    pub payee: String,
    #[serde(default)]
    pub memo: String,
    /// Check or reference number, if the bank gives one.
    #[serde(default)]
    pub number: String,
    /// The bank's own id for the line (OFX FITID), used to avoid duplicates.
    #[serde(default)]
    pub fit_id: String,
}

impl FeedLine {
    /// The payee, or the memo if there is no payee.
    pub fn description(&self) -> &str {
        if self.payee.trim().is_empty() {
            self.memo.trim()
        } else {
            self.payee.trim()
        }
    }
}

/// What to do with one downloaded line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FeedAction {
    /// It is this transaction, already in the books.
    Match {
        txn: TxnId,
    },
    /// Record it as a new transaction in this category (an income, expense
    /// or other account, not Accounts Receivable or Payable).
    Add {
        category: AccountId,
        /// Remember the payee and category as a rule for next time.
        #[serde(default)]
        remember: bool,
    },
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedEntry {
    pub line: FeedLine,
    pub action: FeedAction,
}

/// A downloaded line that was matched or added, kept so it is never
/// imported twice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedLink {
    pub account: AccountId,
    pub fit_id: String,
    pub txn: TxnId,
}

/// "If the payee contains this text, the category is that account."
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankRule {
    pub contains: String,
    pub category: AccountId,
}

impl Company {
    /// Whether a line with this bank id was imported into the account before.
    pub fn is_imported(&self, account: AccountId, fit_id: &str) -> bool {
        let fit_id = fit_id.trim();
        !fit_id.is_empty()
            && self
                .feed_links
                .iter()
                .any(|l| l.account == account && l.fit_id == fit_id)
    }

    /// Whether a transaction is already tied to a downloaded line.
    pub fn is_feed_matched(&self, account: AccountId, txn: TxnId) -> bool {
        self.feed_links
            .iter()
            .any(|l| l.account == account && l.txn == txn)
    }

    /// A downloaded amount in the account's own sign, as [`Company::amount_in`]
    /// reports transactions: for a credit card, a charge (money out of the
    /// card's point of view) raises the balance owed.
    pub fn feed_amount(&self, account: AccountId, amount: Money) -> Option<Money> {
        let kind = self.account(account)?.kind;
        if kind.is_debit_normal() {
            Some(amount)
        } else {
            amount.checked_neg()
        }
    }

    /// The recorded transaction a downloaded line most likely is: the same
    /// amount in the account, dated within [`MATCH_DAYS`], not already tied
    /// to a download and not in `taken`. An equal check number wins, then the
    /// nearest date.
    pub fn suggest_match(
        &self,
        account: AccountId,
        line: &FeedLine,
        taken: &BTreeSet<TxnId>,
    ) -> Option<TxnId> {
        let wanted = self.feed_amount(account, line.amount)?;
        let number = line.number.trim();
        self.transactions
            .iter()
            .filter(|t| !t.voided && !taken.contains(&t.id))
            .filter(|t| !self.is_feed_matched(account, t.id))
            .filter(|t| t.date.days_until(line.date).abs() <= MATCH_DAYS)
            .filter(|t| self.amount_in(&t.body, account) == Some(wanted))
            .min_by_key(|t| {
                let same_number = !number.is_empty() && t.number.trim() == number;
                (!same_number, t.date.days_until(line.date).abs(), t.id)
            })
            .map(|t| t.id)
    }

    /// The category a rule suggests for a line, if any (the first match).
    pub fn rule_for(&self, line: &FeedLine) -> Option<&BankRule> {
        let text = format!("{} {}", line.payee, line.memo).to_lowercase();
        self.bank_rules
            .iter()
            .find(|r| !r.contains.trim().is_empty() && text.contains(&r.contains.to_lowercase()))
    }

    /// Removes a rule by its place in the list.
    pub fn remove_bank_rule(&mut self, index: usize) -> LedgerResult<()> {
        if index >= self.bank_rules.len() {
            return Err(LedgerError::NotFound(format!("Rule {}", index + 1)));
        }
        self.bank_rules.remove(index);
        Ok(())
    }

    /// Brings downloaded lines into a bank or credit card account. Lines
    /// whose bank id was imported before are passed over. Returns how many
    /// lines were matched or added. All or nothing: an error part-way leaves
    /// the books unchanged (the engine discards the whole command).
    pub fn import_feed(
        &mut self,
        account: AccountId,
        entries: Vec<FeedEntry>,
    ) -> LedgerResult<usize> {
        let kind = self
            .require_kind(
                account,
                |k| matches!(k, AccountType::Bank | AccountType::CreditCard),
                "a bank or credit card account",
            )?
            .kind;
        let mut seen = BTreeSet::new();
        let mut done = 0;
        for entry in entries {
            let line = entry.line;
            let fit_id = line.fit_id.trim().to_string();
            if !fit_id.is_empty()
                && (self.is_imported(account, &fit_id) || !seen.insert(fit_id.clone()))
            {
                continue;
            }
            let txn = match entry.action {
                FeedAction::Skip => continue,
                FeedAction::Match { txn } => {
                    let t = self
                        .txn(txn)
                        .filter(|t| !t.voided)
                        .ok_or_else(|| LedgerError::NotFound(format!("Transaction {txn}")))?;
                    if self.is_feed_matched(account, txn) {
                        return Err(LedgerError::Invalid(format!(
                            "{txn} is already matched to a downloaded line"
                        )));
                    }
                    let wanted = self.feed_amount(account, line.amount);
                    if self.amount_in(&t.body, account) != wanted || wanted.is_none() {
                        return Err(LedgerError::Invalid(format!(
                            "{txn} does not have the amount of the line from {} ({})",
                            line.date, line.amount
                        )));
                    }
                    txn
                }
                FeedAction::Add { category, remember } => {
                    let body = feed_body(kind, account, category, &line)?;
                    let id = self.post(line.date, &line.number, line.memo.trim(), body)?;
                    let payee = line.description().to_string();
                    if remember
                        && !payee.is_empty()
                        && !self
                            .bank_rules
                            .iter()
                            .any(|r| r.contains.eq_ignore_ascii_case(&payee))
                    {
                        self.bank_rules.push(BankRule {
                            contains: payee,
                            category,
                        });
                    }
                    id
                }
            };
            if !fit_id.is_empty() {
                self.feed_links.push(FeedLink {
                    account,
                    fit_id,
                    txn,
                });
            }
            done += 1;
        }
        Ok(done)
    }
}

/// The transaction a new downloaded line becomes. Money out is a check (or
/// card charge) to the category; money in is a deposit for a bank, or, for a
/// credit card (a payment or refund), a journal entry lowering the balance.
fn feed_body(
    kind: AccountType,
    account: AccountId,
    category: AccountId,
    line: &FeedLine,
) -> LedgerResult<TxnBody> {
    let description = line.description().to_string();
    let amount = line.amount;
    if amount.is_zero() {
        return Err(LedgerError::NotPositive("The amount"));
    }
    if amount.is_negative() {
        let out = amount.checked_neg().ok_or(LedgerError::Overflow)?;
        return Ok(TxnBody::Check(Check {
            payee: None,
            pay_from: account,
            lines: vec![AccountLine {
                account: category,
                description,
                amount: out,
            }],
        }));
    }
    Ok(match kind {
        AccountType::Bank => TxnBody::Deposit(Deposit {
            deposit_to: account,
            lines: vec![AccountLine {
                account: category,
                description,
                amount,
            }],
        }),
        _ => TxnBody::Journal(JournalEntry {
            lines: vec![
                JournalLine {
                    account,
                    debit: amount,
                    credit: Money::ZERO,
                    memo: description.clone(),
                    party: None,
                },
                JournalLine {
                    account: category,
                    debit: Money::ZERO,
                    credit: amount,
                    memo: description,
                    party: None,
                },
            ],
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Check, Company};

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn dollars(n: i64) -> Money {
        Money::from_cents(n * 100)
    }

    fn line(date: &str, amount: i64, payee: &str, number: &str, fit_id: &str) -> FeedLine {
        FeedLine {
            date: d(date),
            amount: dollars(amount),
            payee: payee.into(),
            memo: String::new(),
            number: number.into(),
            fit_id: fit_id.into(),
        }
    }

    /// Checking with check 101 for 200 rent on 2026-01-05.
    fn books() -> (Company, AccountId, TxnId) {
        let mut c = Company::with_default_accounts("Feed Co");
        let checking = c.account_by_name("Checking").unwrap().id;
        let rent = c.account_by_name("Rent").unwrap().id;
        let check = TxnBody::Check(Check {
            payee: None,
            pay_from: checking,
            lines: vec![AccountLine {
                account: rent,
                description: String::new(),
                amount: dollars(200),
            }],
        });
        let id = c.post(d("2026-01-05"), "101", "", check).unwrap();
        (c, checking, id)
    }

    #[test]
    fn a_line_matches_the_recorded_transaction() {
        let (c, checking, check) = books();
        let none = BTreeSet::new();
        // Same amount (money out), three days later, same check number.
        let l = line("2026-01-08", -200, "CHECK 101", "101", "A1");
        assert_eq!(c.suggest_match(checking, &l, &none), Some(check));
        // Too far apart, or a different amount: no match.
        assert_eq!(
            c.suggest_match(checking, &line("2026-02-20", -200, "", "", "A2"), &none),
            None
        );
        assert_eq!(
            c.suggest_match(checking, &line("2026-01-05", -201, "", "", "A3"), &none),
            None
        );
        // Already taken by another line in this download.
        let taken: BTreeSet<TxnId> = [check].into();
        assert_eq!(c.suggest_match(checking, &l, &taken), None);
    }

    #[test]
    fn importing_matches_adds_and_never_doubles() {
        let (mut c, checking, check) = books();
        let utilities = c.account_by_name("Utilities").unwrap().id;
        let sales = c.account_by_name("Sales").unwrap().id;
        let entries = vec![
            FeedEntry {
                line: line("2026-01-07", -200, "CHECK 101", "101", "A1"),
                action: FeedAction::Match { txn: check },
            },
            FeedEntry {
                line: line("2026-01-10", -85, "VALLEY POWER", "", "A2"),
                action: FeedAction::Add {
                    category: utilities,
                    remember: true,
                },
            },
            FeedEntry {
                line: line("2026-01-12", 500, "Counter sale", "", "A3"),
                action: FeedAction::Add {
                    category: sales,
                    remember: false,
                },
            },
            FeedEntry {
                line: line("2026-01-13", -5, "Fee", "", "A4"),
                action: FeedAction::Skip,
            },
        ];
        let before = c.transactions.len();
        assert_eq!(c.import_feed(checking, entries.clone()).unwrap(), 3);
        assert_eq!(c.transactions.len(), before + 2);
        assert_eq!(c.account_balance(checking, None), dollars(-200 - 85 + 500));
        assert!(c.is_imported(checking, "A2"));
        // The rule remembers Valley Power for next time.
        let next = line("2026-02-10", -90, "Valley Power Co-op", "", "B1");
        assert_eq!(c.rule_for(&next).map(|r| r.category), Some(utilities));
        // The same download again adds nothing.
        assert_eq!(c.import_feed(checking, entries).unwrap(), 0);
        assert_eq!(c.transactions.len(), before + 2);
        c.remove_bank_rule(0).unwrap();
        assert!(c.rule_for(&next).is_none());
    }

    #[test]
    fn a_wrong_match_is_refused() {
        let (mut c, checking, check) = books();
        let entries = vec![FeedEntry {
            line: line("2026-01-07", -250, "", "", "A1"),
            action: FeedAction::Match { txn: check },
        }];
        assert!(c.import_feed(checking, entries).is_err());
    }

    #[test]
    fn credit_card_lines_go_the_right_way() {
        let mut c = Company::with_default_accounts("Card Co");
        let card = c.account_by_name("Business Credit Card").unwrap().id;
        let meals = c.account_by_name("Meals").unwrap().id;
        let checking = c.account_by_name("Checking").unwrap().id;
        let entries = vec![
            // A charge (money out of the card's point of view): balance owed rises.
            FeedEntry {
                line: line("2026-03-01", -40, "Diner", "", "C1"),
                action: FeedAction::Add {
                    category: meals,
                    remember: false,
                },
            },
            // A payment from checking (money in): balance owed falls.
            FeedEntry {
                line: line("2026-03-20", 30, "Payment", "", "C2"),
                action: FeedAction::Add {
                    category: checking,
                    remember: false,
                },
            },
        ];
        assert_eq!(c.import_feed(card, entries).unwrap(), 2);
        assert_eq!(c.account_balance(card, None), dollars(10));
        assert_eq!(c.account_balance(meals, None), dollars(40));
    }
}
