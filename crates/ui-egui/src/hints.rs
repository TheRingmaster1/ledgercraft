//! Hover help. All the plain-English explanations live here, so they read
//! consistently and can be translated in one place later.

use crate::{FormKind, Page};

impl FormKind {
    /// One or two sentences on what the form is for and when to use it.
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Self::Invoice => {
                "Bill a customer for goods or services. They owe you the total until you \
                 record their payment with Receive Payment."
            }
            Self::ReceivePayment => {
                "Record money a customer paid you and apply it to their open invoices."
            }
            Self::Bill => {
                "Record a bill you will pay later. The expense is counted now, but no money \
                 leaves the bank until you use Pay Bills."
            }
            Self::PayBills => {
                "Pay bills you have already entered with Enter Bill. Money leaves the bank \
                 (or goes on a credit card) now."
            }
            Self::Check => {
                "Record something paid for on the spot (check, debit card or credit card) \
                 when there is no bill to track."
            }
            Self::Deposit => {
                "Record money going into a bank account, usually customer payments waiting \
                 in Undeposited Funds."
            }
            Self::Journal => {
                "Advanced: move amounts between any accounts directly, for corrections and \
                 adjustments. Debits must equal credits."
            }
            Self::SalesReceipt => {
                "Record a sale paid for on the spot (cash, card or check): no invoice, and \
                 nothing owed."
            }
            Self::CreditMemo => {
                "Credit a customer for a return, a discount or a mistake. It lowers what \
                 they owe and can pay down their open invoices."
            }
            Self::Refund => {
                "Pay a customer back the credit left on their credit memos."
            }
        }
    }
}

impl Page {
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Self::Home => "Your daily workflow and key balances at a glance.",
            Self::Accounts => {
                "Every account your money is tracked in: bank accounts, what you own and owe, \
                 income and expenses. Click a name to see its register."
            }
            Self::Register(_) => "Every transaction in one account, with a running balance.",
            Self::Customers => "People and businesses who pay you, and what each one owes.",
            Self::Vendors => "People and businesses you pay, and what you owe each one.",
            Self::Items => {
                "What you sell. Choosing an item on an invoice fills in its description, \
                 price and income account."
            }
            Self::Transactions => {
                "Every transaction, newest first. Void one to cancel it while keeping a record."
            }
            Self::Reports => "Profit & Loss, Balance Sheet, who owes what, and more.",
            Self::BankImport => {
                "Bring in a download from your bank or card website and match it to the \
                 books, adding what is missing."
            }
            Self::SalesTax => {
                "The sales taxes you charge, their rates and when each starts, and the tax \
                 groups invoices use."
            }
            Self::Reconcile => {
                "Check a bank or credit card account against its statement, item by item, \
                 so you know the books match the bank."
            }
            Self::Form(kind) => kind.hint(),
        }
    }
}

/// Hover help for a field label, by its text. `None` when it needs no help.
pub(crate) fn field(label: &str) -> Option<&'static str> {
    Some(match label {
        "Deposit to" => {
            "Undeposited Funds holds payments until you record a deposit, so several \
             checks can become one deposit that matches your bank statement. Choose a bank \
             account to skip that step."
        }
        "Method / ref" => "How they paid, such as a check number or ACH. For your reference.",
        "Received from" => "The customer who paid you. Their open invoices appear below.",
        "Sales tax" => {
            "The tax group this invoice charges, at the rates in force on its date. It \
             starts with the customer's group."
        }
        "Agency" => "Who this tax is paid to, such as the state tax commission.",
        "Rate %" => "A percentage, such as 4.85 for 4.85%.",
        "Starts" => {
            "The first day the rate applies. Invoices dated before it use the earlier rate."
        }
        "Taxable" => {
            "Charge sales tax on it. Many services are not taxed; check your state's rules."
        }
        "Ref no." => "The vendor's invoice or account number, so you can match it later.",
        "Due date" => "Filled in from the payment terms. You can change it.",
        "Pay from" => {
            "A bank account for checks and debit cards; a credit card account for card \
             purchases."
        }
        "Payee" => "Optional: who you paid. Leave empty for small purchases.",
        "Terms (days)" => "Days until payment is due: 30 means Net 30, 0 means due on receipt.",
        "Income account" => "The income account this counts toward on the Profit & Loss.",
        "Customers owe you" => "Accounts Receivable: the total of all unpaid invoices.",
        "You owe vendors" => "Accounts Payable: the total of all unpaid bills.",
        "Credit cards" => "What you currently owe on credit card accounts.",
        "As of" => "The report shows balances on this date.",
        _ => return None,
    })
}

/// Hover help for a table column header, by its text.
pub(crate) fn column(header: &str) -> Option<&'static str> {
    Some(match header {
        "Income account" => "The income account this line counts toward on the Profit & Loss.",
        "Name" => {
            "Required on Accounts Receivable and Accounts Payable lines, so the amount is \
             tied to a customer or vendor."
        }
        "Debit" => "Increases assets and expenses; decreases what you owe, equity and income.",
        "Credit" => "Increases what you owe, equity and income; decreases assets and expenses.",
        "Open" => "What is still unpaid.",
        "Payment" => "How much of this payment to apply to the document.",
        _ => return None,
    })
}
