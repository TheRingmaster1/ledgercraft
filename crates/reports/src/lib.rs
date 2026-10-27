//! L2 of LedgerCraft: reports.
//!
//! Every report is built from [`Company::ledger_entries`], never from the
//! forms directly, and returned as a toolkit-free [`Report`] the desktop UI,
//! the CLI and CSV export all render the same way.

mod aging;
mod audit;
mod invoice_pdf;
mod logo;
mod look;
mod pdf;
mod report;
mod salestax;
mod statement_pdf;
mod statements;
mod xlsx;

pub use aging::{ap_aging, ar_aging};
pub use audit::{audit_trail, sales_by_state};
pub use invoice_pdf::{has_pdf, invoice_pdf, invoice_pdf_name};
pub use logo::logo_size;
pub use report::{Cell, Report, Row, RowKind};
pub use salestax::sales_tax_liability;
pub use statement_pdf::{statement_pdf, statement_pdf_name};
pub use statements::{
    account_register, balance_sheet, profit_and_loss, profit_and_loss_by_class,
    reconciliation_report, transaction_list,
    trial_balance,
};

/// The reports the app offers, for menus and the CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReportKind {
    ProfitAndLoss,
    ProfitAndLossByClass,
    BalanceSheet,
    TrialBalance,
    ArAging,
    ApAging,
    TransactionList,
    SalesTaxLiability,
    SalesByState,
    AuditTrail,
}

impl ReportKind {
    pub const ALL: [Self; 10] = [
        Self::ProfitAndLoss,
        Self::ProfitAndLossByClass,
        Self::BalanceSheet,
        Self::TrialBalance,
        Self::ArAging,
        Self::ApAging,
        Self::TransactionList,
        Self::SalesTaxLiability,
        Self::SalesByState,
        Self::AuditTrail,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ProfitAndLoss => "Profit & Loss",
            Self::ProfitAndLossByClass => "Profit & Loss by Class",
            Self::BalanceSheet => "Balance Sheet",
            Self::TrialBalance => "Trial Balance",
            Self::ArAging => "A/R Aging Summary",
            Self::ApAging => "A/P Aging Summary",
            Self::TransactionList => "Transaction List",
            Self::SalesTaxLiability => "Sales Tax Liability",
            Self::SalesByState => "Sales by State",
            Self::AuditTrail => "Audit Trail",
        }
    }

    /// The id used on the command line: `pl`, `balance-sheet`, …
    pub fn cli_id(self) -> &'static str {
        match self {
            Self::ProfitAndLoss => "pl",
            Self::ProfitAndLossByClass => "pl-by-class",
            Self::BalanceSheet => "balance-sheet",
            Self::TrialBalance => "trial-balance",
            Self::ArAging => "ar-aging",
            Self::ApAging => "ap-aging",
            Self::TransactionList => "transactions",
            Self::SalesTaxLiability => "sales-tax",
            Self::SalesByState => "sales-by-state",
            Self::AuditTrail => "audit",
        }
    }

    pub fn from_cli_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.cli_id() == id)
    }

    /// Period reports use a from/to range; the rest are "as of" one date.
    pub fn uses_period(self) -> bool {
        matches!(
            self,
            Self::ProfitAndLoss
                | Self::ProfitAndLossByClass
                | Self::TransactionList
                | Self::SalesTaxLiability
                | Self::SalesByState
                | Self::AuditTrail
        )
    }

    pub fn build(
        self,
        company: &ledgercraft_ledger::Company,
        from: ledgercraft_core::Date,
        to: ledgercraft_core::Date,
    ) -> Report {
        match self {
            Self::ProfitAndLoss => profit_and_loss(company, from, to),
            Self::ProfitAndLossByClass => profit_and_loss_by_class(company, from, to),
            Self::BalanceSheet => balance_sheet(company, to),
            Self::TrialBalance => trial_balance(company, to),
            Self::ArAging => ar_aging(company, to),
            Self::ApAging => ap_aging(company, to),
            Self::TransactionList => transaction_list(company, from, to),
            Self::SalesTaxLiability => sales_tax_liability(company, from, to),
            Self::SalesByState => sales_by_state(company, from, to),
            Self::AuditTrail => audit_trail(company, from, to),
        }
    }
}
