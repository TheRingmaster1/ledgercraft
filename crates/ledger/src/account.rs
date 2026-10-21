use serde::{Deserialize, Serialize};

use crate::AccountId;

/// The five classes every account belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

/// Account types. These are standard bookkeeping categories; their order is
/// the order they appear in the chart of accounts and on reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Bank,
    AccountsReceivable,
    OtherCurrentAsset,
    FixedAsset,
    OtherAsset,
    AccountsPayable,
    CreditCard,
    OtherCurrentLiability,
    LongTermLiability,
    Equity,
    Income,
    CostOfGoodsSold,
    Expense,
    OtherIncome,
    OtherExpense,
}

impl AccountType {
    pub const ALL: [Self; 15] = [
        Self::Bank,
        Self::AccountsReceivable,
        Self::OtherCurrentAsset,
        Self::FixedAsset,
        Self::OtherAsset,
        Self::AccountsPayable,
        Self::CreditCard,
        Self::OtherCurrentLiability,
        Self::LongTermLiability,
        Self::Equity,
        Self::Income,
        Self::CostOfGoodsSold,
        Self::Expense,
        Self::OtherIncome,
        Self::OtherExpense,
    ];

    pub fn class(self) -> Class {
        match self {
            Self::Bank
            | Self::AccountsReceivable
            | Self::OtherCurrentAsset
            | Self::FixedAsset
            | Self::OtherAsset => Class::Asset,
            Self::AccountsPayable
            | Self::CreditCard
            | Self::OtherCurrentLiability
            | Self::LongTermLiability => Class::Liability,
            Self::Equity => Class::Equity,
            Self::Income | Self::OtherIncome => Class::Income,
            Self::CostOfGoodsSold | Self::Expense | Self::OtherExpense => Class::Expense,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bank => "Bank",
            Self::AccountsReceivable => "Accounts Receivable",
            Self::OtherCurrentAsset => "Other Current Asset",
            Self::FixedAsset => "Fixed Asset",
            Self::OtherAsset => "Other Asset",
            Self::AccountsPayable => "Accounts Payable",
            Self::CreditCard => "Credit Card",
            Self::OtherCurrentLiability => "Other Current Liability",
            Self::LongTermLiability => "Long Term Liability",
            Self::Equity => "Equity",
            Self::Income => "Income",
            Self::CostOfGoodsSold => "Cost of Goods Sold",
            Self::Expense => "Expense",
            Self::OtherIncome => "Other Income",
            Self::OtherExpense => "Other Expense",
        }
    }

    /// Assets and expenses grow with debits; everything else with credits.
    pub fn is_debit_normal(self) -> bool {
        matches!(self.class(), Class::Asset | Class::Expense)
    }

    pub fn is_balance_sheet(self) -> bool {
        matches!(
            self.class(),
            Class::Asset | Class::Liability | Class::Equity
        )
    }

    /// AR and AP track who owes what, so every posting to them needs a name.
    pub fn needs_party(self) -> bool {
        matches!(self, Self::AccountsReceivable | Self::AccountsPayable)
    }

    /// Accounts money can be paid from or deposited to.
    pub fn is_cash_like(self) -> bool {
        matches!(
            self,
            Self::Bank | Self::OtherCurrentAsset | Self::CreditCard
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    #[serde(default)]
    pub number: String,
    pub name: String,
    pub kind: AccountType,
    #[serde(default)]
    pub description: String,
    #[serde(default = "yes")]
    pub active: bool,
}

fn yes() -> bool {
    true
}

impl Account {
    /// `1000 · Checking`, or just the name when there is no number.
    pub fn display_name(&self) -> String {
        if self.number.is_empty() {
            self.name.clone()
        } else {
            format!("{} · {}", self.number, self.name)
        }
    }
}
