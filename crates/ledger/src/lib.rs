//! L1 of LedgerCraft: the company model.
//!
//! A [`Company`] holds the chart of accounts, customers, vendors, items and
//! transactions. Every transaction form (invoice, bill, check, …) turns into
//! balanced double-entry [`Posting`]s; reports read only those postings, so
//! the books always balance by construction.

mod account;
mod bankfeed;
mod company;
mod error;
mod ids;
mod item;
mod party;
mod reconcile;
mod salestax;
mod template;
mod txn;

pub use account::{Account, AccountType, Class};
pub use bankfeed::{BankRule, FeedAction, FeedEntry, FeedLine, FeedLink, MATCH_DAYS};
pub use company::{Company, CompanyInfo, LedgerEntry};
pub use error::{LedgerError, LedgerResult};
pub use ids::{AccountId, CustomerId, ItemId, TaxComponentId, TaxGroupId, TxnId, VendorId};
pub use item::{Item, ItemKind, ItemSpec};
pub use party::{Address, Contact, Customer, Party, Vendor};
pub use reconcile::{ReconcileItem, Reconciliation};
pub use salestax::{
    rate_publication_date, CustomerTax, DatedRate, ImportedTax, InvoiceTax, TaxComponent, TaxGroup,
    TaxLine, TaxRate, TaxReview, SALES_TAX_ACCOUNT,
};
pub use template::{default_chart_of_accounts, AccountTemplate};
pub use txn::{
    AccountLine, Applied, Bill, Check, Deposit, Invoice, JournalEntry, JournalLine, PayBill,
    Posting, ReceivePayment, SalesLine, Transaction, TxnBody,
};

pub use ledgercraft_core::{Date, Money, Qty};
