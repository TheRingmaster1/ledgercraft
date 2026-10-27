//! L4 of LedgerCraft: the desktop interface.
//!
//! The UI is thin: pages read the [`ledgercraft_engine::Session`]'s company
//! and push [`Action`]s; the app turns those into engine commands after the
//! frame is drawn. Nothing below this crate knows egui exists.

mod app;
mod assoc;
mod bankimport;
mod closing;
mod company_info;
mod currency;
mod dialogs;
mod forms;
mod hints;
mod licenses;
mod pages;
mod prefs;
mod recent;
mod reconcile;
mod recurring;
mod salestax;
mod security;
mod start;
mod statement;
mod theme;
mod tutorial;
mod widgets;

pub use app::LedgerApp;

use ledgercraft_engine::Command;
use ledgercraft_ledger::{AccountId, CustomerId, TxnId, VendorId};

/// Where the main area is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Home,
    Accounts,
    Register(AccountId),
    Customers,
    Vendors,
    Items,
    Transactions,
    Reports,
    Reconcile,
    BankImport,
    SalesTax,
    Classes,
    Recurring,
    Form(FormKind),
}

/// The transaction entry forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FormKind {
    Invoice,
    ReceivePayment,
    Bill,
    PayBills,
    Check,
    Deposit,
    Journal,
    SalesReceipt,
    CreditMemo,
    Refund,
}

impl FormKind {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Invoice => "Create Invoice",
            Self::ReceivePayment => "Receive Payment",
            Self::Bill => "Enter Bill",
            Self::PayBills => "Pay Bills",
            Self::Check => "Write Check / Expense",
            Self::Deposit => "Make Deposit",
            Self::Journal => "Journal Entry",
            Self::SalesReceipt => "Enter Sales Receipt",
            Self::CreditMemo => "Create Credit Memo",
            Self::Refund => "Refund a Credit",
        }
    }
}

/// Which input to clear after a command succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reset {
    Nothing,
    Form(FormKind),
    NewAccount,
    NewCustomer,
    NewVendor,
    NewItem,
    NewTax,
    NewTaxGroup,
    NewTaxRate,
    PayTax,
    EditedCustomer,
    EditedVendor,
    EditedItem,
    EditedAccount,
    NewClass,
    EditedClass,
    EditedRecurring,
}

// `Run` carries a whole `Command` (contacts are large); a handful per frame is fine.
/// Something a page asks the app to do once the frame is drawn.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Action {
    Run {
        cmd: Command,
        reset: Reset,
    },
    Go(Page),
    /// Open Reconcile for this account.
    Reconcile(AccountId),
    /// Open Reconcile for this account with a downloaded statement's date and
    /// ending balance filled in.
    ReconcileStatement {
        account: AccountId,
        date: ledgercraft_core::Date,
        balance: ledgercraft_core::Money,
    },
    InvoiceFor(CustomerId),
    PaymentFrom(CustomerId),
    BillFrom(VendorId),
    PayVendor(VendorId),
    Error(String),
    /// A form was cleared: drop any message about what was in it.
    ClearStatus,
    /// Open a recorded transaction in its form, to change it.
    Open(TxnId),
    /// Memorize… on a form: what it would record (a `txn.post`).
    Memorize(FormKind, Box<Command>),
    /// Open a recurring transaction's next occurrence in its form, to check
    /// and enter it.
    EnterRecurring(ledgercraft_ledger::RecurringId),
    /// Cancel on a form that was editing a recorded transaction.
    EditCancelled,
    ExportCsv {
        name: String,
        csv: String,
    },
    /// A finished .xlsx file, or why it could not be built.
    ExportXlsx {
        name: String,
        data: Result<Vec<u8>, String>,
    },
    /// Save a recorded invoice as a PDF, asking where.
    InvoicePdf(TxnId),
    /// Customers > Create Statement…, for this customer if one is given.
    Statement(Option<CustomerId>),
    /// Customers > Sales tax > Find from address.
    FindTaxGroup(CustomerId),
    /// A state's rate and boundary files were imported from: remember them.
    SstFilesUsed {
        state: String,
        rates: std::path::PathBuf,
        boundary: std::path::PathBuf,
    },
    /// Save the invoice, sales receipt or credit memo on the form (new, or
    /// changes to a recorded one), then save it as a PDF.
    SaveInvoicePdf(FormKind, Command),
}
