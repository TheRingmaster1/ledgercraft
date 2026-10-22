use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{
    AccountId, AccountType, Address, Company, Contact, CustomerId, FeedEntry, ItemSpec,
    LedgerResult, TxnBody, TxnId, VendorId,
};
use serde::{Deserialize, Serialize};

/// A change to the books. Serialized as JSON with a `"command"` tag, e.g.
/// `{"command":"account.create","number":"6700","name":"Postage","kind":"expense"}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum Command {
    #[serde(rename = "company.rename")]
    CompanyRename { name: String },
    #[serde(rename = "company.setFiscalYearStart")]
    CompanySetFiscalYearStart { month: u32 },
    /// The name and contact details printed on invoices.
    #[serde(rename = "company.setInfo")]
    CompanySetInfo {
        name: String,
        #[serde(default)]
        legal_name: String,
        #[serde(default)]
        email: String,
        #[serde(default)]
        phone: String,
        #[serde(default)]
        address: Address,
    },
    #[serde(rename = "account.create")]
    AccountCreate {
        #[serde(default)]
        number: String,
        name: String,
        kind: AccountType,
        #[serde(default)]
        description: String,
    },
    #[serde(rename = "account.update")]
    AccountUpdate {
        id: AccountId,
        #[serde(default)]
        number: String,
        name: String,
        #[serde(default)]
        description: String,
    },
    #[serde(rename = "account.setActive")]
    AccountSetActive { id: AccountId, active: bool },
    #[serde(rename = "customer.create")]
    CustomerCreate { contact: Contact },
    #[serde(rename = "customer.update")]
    CustomerUpdate { id: CustomerId, contact: Contact },
    #[serde(rename = "vendor.create")]
    VendorCreate { contact: Contact },
    #[serde(rename = "vendor.update")]
    VendorUpdate { id: VendorId, contact: Contact },
    #[serde(rename = "item.create")]
    ItemCreate { spec: ItemSpec },
    #[serde(rename = "txn.post")]
    TxnPost {
        date: Date,
        #[serde(default)]
        number: String,
        #[serde(default)]
        memo: String,
        body: TxnBody,
    },
    #[serde(rename = "txn.update")]
    TxnUpdate {
        id: TxnId,
        date: Date,
        #[serde(default)]
        number: String,
        #[serde(default)]
        memo: String,
        body: TxnBody,
    },
    #[serde(rename = "txn.void")]
    TxnVoid { id: TxnId },
    #[serde(rename = "account.reconcile")]
    AccountReconcile {
        account: AccountId,
        statement_date: Date,
        ending_balance: Money,
        cleared: Vec<TxnId>,
    },
    #[serde(rename = "account.undoReconciliation")]
    AccountUndoReconciliation { account: AccountId },
    #[serde(rename = "bank.import")]
    BankImport {
        account: AccountId,
        entries: Vec<FeedEntry>,
    },
    #[serde(rename = "bank.removeRule")]
    BankRemoveRule { index: usize },
}

/// One row of the command catalog shown by `ledgercraft-cli commands`.
pub struct CommandInfo {
    pub id: &'static str,
    pub summary: &'static str,
}

pub const CATALOG: &[CommandInfo] = &[
    CommandInfo { id: "company.rename", summary: "Rename the company. {name}" },
    CommandInfo { id: "company.setFiscalYearStart", summary: "First month of the fiscal year, 1-12. {month}" },
    CommandInfo { id: "company.setInfo", summary: "Name and contact details printed on invoices. {name, legal_name?, email?, phone?, address?:{line1, line2, city, region, postal_code, country}}" },
    CommandInfo { id: "account.create", summary: "Add an account. {number?, name, kind, description?}" },
    CommandInfo { id: "account.update", summary: "Rename or renumber an account. {id, number?, name, description?}" },
    CommandInfo { id: "account.setActive", summary: "Make an account active or inactive. {id, active}" },
    CommandInfo { id: "customer.create", summary: "Add a customer. {contact:{name, email?, phone?, terms_days?, …}}" },
    CommandInfo { id: "customer.update", summary: "Edit a customer. {id, contact}" },
    CommandInfo { id: "vendor.create", summary: "Add a vendor. {contact}" },
    CommandInfo { id: "vendor.update", summary: "Edit a vendor. {id, contact}" },
    CommandInfo { id: "item.create", summary: "Add a product or service. {spec:{name, rate, income_account, kind?}}" },
    CommandInfo { id: "txn.post", summary: "Record a transaction. {date, number?, memo?, body:{type: invoice|receive_payment|bill|pay_bill|check|deposit|journal, …}}" },
    CommandInfo { id: "txn.update", summary: "Edit a recorded transaction; it keeps its id and type. {id, date, number?, memo?, body}" },
    CommandInfo { id: "txn.void", summary: "Void a transaction (it stays on file). {id}" },
    CommandInfo { id: "account.reconcile", summary: "Reconcile a bank or credit card account against a statement; the cleared items must match the ending balance exactly. {account, statement_date, ending_balance, cleared:[txn ids]}" },
    CommandInfo { id: "account.undoReconciliation", summary: "Undo an account's last reconciliation. {account}" },
    CommandInfo { id: "bank.import", summary: "Bring downloaded bank lines into an account: match, add in a category, or skip each; lines imported before are passed over. {account, entries:[{line:{date, amount, payee?, memo?, number?, fit_id?}, action:{action: match|add|skip, txn? | category?, remember?}}]}" },
    CommandInfo { id: "bank.removeRule", summary: "Delete a bank feed rule by its position, from 0. {index}" },
];

impl Command {
    pub fn id(&self) -> &'static str {
        match self {
            Self::CompanyRename { .. } => "company.rename",
            Self::CompanySetFiscalYearStart { .. } => "company.setFiscalYearStart",
            Self::CompanySetInfo { .. } => "company.setInfo",
            Self::AccountCreate { .. } => "account.create",
            Self::AccountUpdate { .. } => "account.update",
            Self::AccountSetActive { .. } => "account.setActive",
            Self::CustomerCreate { .. } => "customer.create",
            Self::CustomerUpdate { .. } => "customer.update",
            Self::VendorCreate { .. } => "vendor.create",
            Self::VendorUpdate { .. } => "vendor.update",
            Self::ItemCreate { .. } => "item.create",
            Self::TxnPost { .. } => "txn.post",
            Self::TxnUpdate { .. } => "txn.update",
            Self::TxnVoid { .. } => "txn.void",
            Self::AccountReconcile { .. } => "account.reconcile",
            Self::AccountUndoReconciliation { .. } => "account.undoReconciliation",
            Self::BankImport { .. } => "bank.import",
            Self::BankRemoveRule { .. } => "bank.removeRule",
        }
    }

    /// What Undo says it will undo.
    pub fn undo_label(&self) -> String {
        match self {
            Self::CompanyRename { .. } | Self::CompanySetFiscalYearStart { .. } => {
                "Company Settings".into()
            }
            Self::CompanySetInfo { .. } => "Company Information".into(),
            Self::AccountCreate { name, .. } => format!("New Account {name}"),
            Self::AccountUpdate { name, .. } => format!("Edit Account {name}"),
            Self::AccountSetActive { .. } => "Account Status".into(),
            Self::CustomerCreate { contact } => format!("New Customer {}", contact.name),
            Self::CustomerUpdate { contact, .. } => format!("Edit Customer {}", contact.name),
            Self::VendorCreate { contact } => format!("New Vendor {}", contact.name),
            Self::VendorUpdate { contact, .. } => format!("Edit Vendor {}", contact.name),
            Self::ItemCreate { spec } => format!("New Item {}", spec.name),
            Self::TxnPost { body, .. } => body.label().to_string(),
            Self::TxnUpdate { body, .. } => format!("Edit {}", body.label()),
            Self::TxnVoid { id } => format!("Void {id}"),
            Self::AccountReconcile { .. } => "Reconcile".into(),
            Self::AccountUndoReconciliation { .. } => "Undo Reconciliation".into(),
            Self::BankImport { .. } => "Import Bank Transactions".into(),
            Self::BankRemoveRule { .. } => "Delete Bank Rule".into(),
        }
    }

    /// Applies the command. Returns a short description of what was created.
    pub(crate) fn apply(self, c: &mut Company) -> LedgerResult<Option<String>> {
        Ok(match self {
            Self::CompanyRename { name } => {
                let name = name.trim();
                if name.is_empty() {
                    return Err(ledgercraft_ledger::LedgerError::Required("A company name"));
                }
                c.info.name = name.to_string();
                None
            }
            Self::CompanySetInfo {
                name,
                legal_name,
                email,
                phone,
                address,
            } => {
                let name = name.trim();
                if name.is_empty() {
                    return Err(ledgercraft_ledger::LedgerError::Required("A company name"));
                }
                let tidy = |s: &str| s.trim().to_string();
                c.info.name = name.to_string();
                c.info.legal_name = tidy(&legal_name);
                c.info.email = tidy(&email);
                c.info.phone = tidy(&phone);
                c.info.address = Address {
                    line1: tidy(&address.line1),
                    line2: tidy(&address.line2),
                    city: tidy(&address.city),
                    region: tidy(&address.region),
                    postal_code: tidy(&address.postal_code),
                    country: tidy(&address.country),
                };
                None
            }
            Self::CompanySetFiscalYearStart { month } => {
                if !(1..=12).contains(&month) {
                    return Err(ledgercraft_ledger::LedgerError::Invalid(
                        "The fiscal year must start in month 1 to 12".into(),
                    ));
                }
                c.info.fiscal_year_start_month = month;
                None
            }
            Self::AccountCreate {
                number,
                name,
                kind,
                description,
            } => Some(
                c.add_account(&number, &name, kind, &description)?
                    .to_string(),
            ),
            Self::AccountUpdate {
                id,
                number,
                name,
                description,
            } => {
                c.update_account(id, &number, &name, &description)?;
                None
            }
            Self::AccountSetActive { id, active } => {
                c.set_account_active(id, active)?;
                None
            }
            Self::CustomerCreate { contact } => Some(c.add_customer(contact)?.to_string()),
            Self::CustomerUpdate { id, contact } => {
                c.update_customer(id, contact)?;
                None
            }
            Self::VendorCreate { contact } => Some(c.add_vendor(contact)?.to_string()),
            Self::VendorUpdate { id, contact } => {
                c.update_vendor(id, contact)?;
                None
            }
            Self::ItemCreate { spec } => Some(c.add_item(spec)?.to_string()),
            Self::TxnPost {
                date,
                number,
                memo,
                body,
            } => Some(c.post(date, &number, &memo, body)?.to_string()),
            Self::TxnUpdate {
                id,
                date,
                number,
                memo,
                body,
            } => {
                c.update(id, date, &number, &memo, body)?;
                None
            }
            Self::TxnVoid { id } => {
                c.void(id)?;
                None
            }
            Self::AccountReconcile {
                account,
                statement_date,
                ending_balance,
                cleared,
            } => {
                c.reconcile(account, statement_date, ending_balance, cleared)?;
                None
            }
            Self::AccountUndoReconciliation { account } => {
                c.undo_last_reconciliation(account)?;
                None
            }
            Self::BankImport { account, entries } => {
                let done = c.import_feed(account, entries)?;
                Some(format!("{done} lines"))
            }
            Self::BankRemoveRule { index } => {
                c.remove_bank_rule(index)?;
                None
            }
        })
    }
}
