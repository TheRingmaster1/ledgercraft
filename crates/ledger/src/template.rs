use crate::AccountType;

/// One row of a starting chart of accounts.
#[derive(Clone, Copy, Debug)]
pub struct AccountTemplate {
    pub number: &'static str,
    pub name: &'static str,
    pub kind: AccountType,
    pub description: &'static str,
}

const fn row(
    number: &'static str,
    name: &'static str,
    kind: AccountType,
    description: &'static str,
) -> AccountTemplate {
    AccountTemplate {
        number,
        name,
        kind,
        description,
    }
}

/// A general-purpose small-business chart of accounts, written for LedgerCraft.
pub fn default_chart_of_accounts() -> &'static [AccountTemplate] {
    use AccountType::*;
    const ROWS: &[AccountTemplate] = &[
        row("1000", "Checking", Bank, "Main operating bank account"),
        row("1050", "Savings", Bank, "Reserve savings account"),
        row(
            "1200",
            "Accounts Receivable",
            AccountsReceivable,
            "Money customers owe you",
        ),
        row(
            "1300",
            "Undeposited Funds",
            OtherCurrentAsset,
            "Payments received but not yet deposited",
        ),
        row(
            "1400",
            "Prepaid Expenses",
            OtherCurrentAsset,
            "Expenses paid in advance",
        ),
        row(
            "1500",
            "Equipment",
            FixedAsset,
            "Tools, machinery and computers",
        ),
        row(
            "1590",
            "Accumulated Depreciation",
            FixedAsset,
            "Depreciation to date",
        ),
        row(
            "2000",
            "Accounts Payable",
            AccountsPayable,
            "Money you owe vendors",
        ),
        row(
            "2100",
            "Business Credit Card",
            CreditCard,
            "Company credit card",
        ),
        row(
            "2200",
            "Sales Tax Payable",
            OtherCurrentLiability,
            "Sales tax collected, not yet remitted",
        ),
        row(
            "2300",
            "Payroll Liabilities",
            OtherCurrentLiability,
            "Withholdings and payroll taxes owed",
        ),
        row("2700", "Loan Payable", LongTermLiability, "Long-term loans"),
        row("3000", "Owner's Equity", Equity, "Owner investment"),
        row("3100", "Owner's Draw", Equity, "Owner withdrawals"),
        row(
            "3900",
            "Retained Earnings",
            Equity,
            "Profit kept from earlier years",
        ),
        row("4000", "Sales", Income, "Product sales"),
        row("4100", "Service Income", Income, "Fees for services"),
        row("4900", "Discounts Given", Income, "Discounts on sales"),
        row(
            "5000",
            "Cost of Goods Sold",
            CostOfGoodsSold,
            "Direct cost of products sold",
        ),
        row("6000", "Advertising and Marketing", Expense, ""),
        row("6050", "Bank Service Charges", Expense, ""),
        row("6100", "Insurance", Expense, ""),
        row("6150", "Meals", Expense, ""),
        row("6200", "Office Supplies", Expense, ""),
        row(
            "6250",
            "Professional Fees",
            Expense,
            "Accountants, lawyers and consultants",
        ),
        row("6300", "Rent", Expense, ""),
        row("6350", "Repairs and Maintenance", Expense, ""),
        row("6400", "Software and Subscriptions", Expense, ""),
        row("6450", "Travel", Expense, ""),
        row("6500", "Utilities", Expense, ""),
        row("6600", "Wages", Expense, ""),
        row("7000", "Interest Income", OtherIncome, ""),
        row("8000", "Interest Expense", OtherExpense, ""),
    ];
    ROWS
}
