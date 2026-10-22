//! L2 of LedgerCraft: reports.
//!
//! Every report is built from [`Company::ledger_entries`], never from the
//! forms directly, and returned as a toolkit-free [`Report`] the desktop UI,
//! the CLI and CSV export all render the same way.

mod aging;
mod invoice_pdf;
mod pdf;
mod report;
mod statements;
mod xlsx;

pub use aging::{ap_aging, ar_aging};
pub use invoice_pdf::{invoice_pdf, invoice_pdf_name};
pub use report::{Cell, Report, Row, RowKind};
pub use statements::{
    account_register, balance_sheet, profit_and_loss, reconciliation_report, transaction_list,
    trial_balance,
};

/// The reports the app offers, for menus and the CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReportKind {
    ProfitAndLoss,
    BalanceSheet,
    TrialBalance,
    ArAging,
    ApAging,
    TransactionList,
}

impl ReportKind {
    pub const ALL: [Self; 6] = [
        Self::ProfitAndLoss,
        Self::BalanceSheet,
        Self::TrialBalance,
        Self::ArAging,
        Self::ApAging,
        Self::TransactionList,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ProfitAndLoss => "Profit & Loss",
            Self::BalanceSheet => "Balance Sheet",
            Self::TrialBalance => "Trial Balance",
            Self::ArAging => "A/R Aging Summary",
            Self::ApAging => "A/P Aging Summary",
            Self::TransactionList => "Transaction List",
        }
    }

    /// The id used on the command line: `pl`, `balance-sheet`, …
    pub fn cli_id(self) -> &'static str {
        match self {
            Self::ProfitAndLoss => "pl",
            Self::BalanceSheet => "balance-sheet",
            Self::TrialBalance => "trial-balance",
            Self::ArAging => "ar-aging",
            Self::ApAging => "ap-aging",
            Self::TransactionList => "transactions",
        }
    }

    pub fn from_cli_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.cli_id() == id)
    }

    /// Period reports use a from/to range; the rest are "as of" one date.
    pub fn uses_period(self) -> bool {
        matches!(self, Self::ProfitAndLoss | Self::TransactionList)
    }

    pub fn build(
        self,
        company: &ledgercraft_ledger::Company,
        from: ledgercraft_core::Date,
        to: ledgercraft_core::Date,
    ) -> Report {
        match self {
            Self::ProfitAndLoss => profit_and_loss(company, from, to),
            Self::BalanceSheet => balance_sheet(company, to),
            Self::TrialBalance => trial_balance(company, to),
            Self::ArAging => ar_aging(company, to),
            Self::ApAging => ap_aging(company, to),
            Self::TransactionList => transaction_list(company, from, to),
        }
    }
}
