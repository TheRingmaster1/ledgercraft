//! L4 of LedgerCraft: the desktop interface.
//!
//! The UI is thin: pages read the [`ledgercraft_engine::Session`]'s company
//! and push [`Action`]s; the app turns those into engine commands after the
//! frame is drawn. Nothing below this crate knows egui exists.

mod app;
mod forms;
mod hints;
mod licenses;
mod pages;
mod prefs;
mod start;
mod theme;
mod tutorial;
mod widgets;

pub use app::LedgerApp;

use ledgercraft_engine::Command;
use ledgercraft_ledger::{AccountId, CustomerId, VendorId};

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
}

// `Run` carries a whole `Command` (contacts are large); a handful per frame is fine.
/// Something a page asks the app to do once the frame is drawn.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Action {
    Run { cmd: Command, reset: Reset },
    Go(Page),
    InvoiceFor(CustomerId),
    PaymentFrom(CustomerId),
    BillFrom(VendorId),
    PayVendor(VendorId),
        Error(String),
    /// A form was cleared: drop any message about what was in it.
    ClearStatus,
    ExportCsv { name: String, csv: String },
        /// A finished .xlsx file, or why it could not be built.
    ExportXlsx { name: String, data: Result<Vec<u8>, String> },

}
