use ledgercraft_core::{Date, Money};
use ledgercraft_ledger::{
    AccountId, AccountType, Address, Company, Contact, CustomerId, CustomerTax, FeedEntry,
    ImportedTax, ItemId, ItemSpec, LedgerResult, TaxComponentId, TaxGroupId, TaxRate, TxnBody,
    TxnId, VendorId,
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
    /// Close the books through `date` (`None` opens them again). With
    /// `keep_password` the closing date password stays as it is; otherwise
    /// `password` sets it (`None` removes it). Changing the closing date of
    /// books with a password needs that password entered first.
    #[serde(rename = "company.setClosing")]
    CompanySetClosing {
        /// Required, so an empty command never reopens the books: give
        /// `null` to reopen them.
        #[serde(deserialize_with = "Option::deserialize")]
        date: Option<Date>,
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        keep_password: bool,
    },
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
        /// The PDF template and logo; left as they are when not given.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        print: Option<ledgercraft_ledger::PrintStyle>,
    },
    #[serde(rename = "account.create")]
    AccountCreate {
        #[serde(default)]
        number: String,
        name: String,
        kind: AccountType,
        #[serde(default)]
        description: String,
        /// The account this is a sub-account of.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<AccountId>,
    },
    #[serde(rename = "account.update")]
    AccountUpdate {
        id: AccountId,
        #[serde(default)]
        number: String,
        name: String,
        #[serde(default)]
        description: String,
        /// The account this is a sub-account of; none for a top-level account.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<AccountId>,
    },
    /// Moves everything of one account onto another of the same type, then
    /// removes the first.
    #[serde(rename = "account.merge")]
    AccountMerge { from: AccountId, into: AccountId },
    #[serde(rename = "customer.merge")]
    CustomerMerge { from: CustomerId, into: CustomerId },
    #[serde(rename = "vendor.merge")]
    VendorMerge { from: VendorId, into: VendorId },
    #[serde(rename = "item.merge")]
    ItemMerge { from: ItemId, into: ItemId },
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
    #[serde(rename = "customer.setActive")]
    CustomerSetActive { id: CustomerId, active: bool },
    #[serde(rename = "vendor.setActive")]
    VendorSetActive { id: VendorId, active: bool },
    #[serde(rename = "item.create")]
    ItemCreate { spec: ItemSpec },
    #[serde(rename = "item.update")]
    ItemUpdate { id: ItemId, spec: ItemSpec },
    #[serde(rename = "item.setActive")]
    ItemSetActive { id: ItemId, active: bool },
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
    /// A sales tax component with its first rate.
    #[serde(rename = "tax.create")]
    TaxCreate {
        name: String,
        #[serde(default)]
        agency: String,
        #[serde(default)]
        code: String,
        from: Date,
        rate: TaxRate,
    },
    #[serde(rename = "tax.update")]
    TaxUpdate {
        id: TaxComponentId,
        name: String,
        #[serde(default)]
        agency: String,
        #[serde(default)]
        code: String,
    },
    /// Sets the rate that starts on `from` (replacing one starting that day).
    #[serde(rename = "tax.setRate")]
    TaxSetRate {
        id: TaxComponentId,
        from: Date,
        rate: TaxRate,
    },
    #[serde(rename = "tax.removeRate")]
    TaxRemoveRate { id: TaxComponentId, from: Date },
    #[serde(rename = "tax.setActive")]
    TaxSetActive { id: TaxComponentId, active: bool },
    #[serde(rename = "taxGroup.create")]
    TaxGroupCreate {
        name: String,
        components: Vec<TaxComponentId>,
    },
    #[serde(rename = "taxGroup.update")]
    TaxGroupUpdate {
        id: TaxGroupId,
        name: String,
        components: Vec<TaxComponentId>,
    },
    #[serde(rename = "taxGroup.setActive")]
    TaxGroupSetActive { id: TaxGroupId, active: bool },
    /// A customer's tax group and exemption.
    #[serde(rename = "customer.setTax")]
    CustomerSetTax {
        id: CustomerId,
        #[serde(default)]
        tax: CustomerTax,
    },
    #[serde(rename = "item.setTaxable")]
    ItemSetTaxable { id: ItemId, taxable: bool },
    /// The sales tax rates were checked and are current as of `on`.
    #[serde(rename = "tax.markChecked")]
    TaxMarkChecked { on: Date },
    /// Taxes read from a state rate file, added or brought up to date, and
    /// grouped; the rates count as checked on `on`.
    #[serde(rename = "tax.import")]
    TaxImport {
        taxes: Vec<ImportedTax>,
        group: String,
        on: Date,
        /// Also make this the customer's tax group (Find tax group).
        #[serde(default)]
        customer: Option<CustomerId>,
    },
}

/// One row of the command catalog shown by `ledgercraft-cli commands`.
pub struct CommandInfo {
    pub id: &'static str,
    pub summary: &'static str,
}

pub const CATALOG: &[CommandInfo] = &[
    CommandInfo { id: "company.rename", summary: "Rename the company. {name}" },
    CommandInfo { id: "company.setFiscalYearStart", summary: "First month of the fiscal year, 1-12. {month}" },
    CommandInfo { id: "company.setClosing", summary: "Close the books through a date; transactions then or before are locked. {date: YYYY-MM-DD, or null to reopen the books, password?, keep_password?}" },
    CommandInfo { id: "company.setInfo", summary: "Name and contact details printed on invoices. {name, legal_name?, email?, phone?, address?:{line1, line2, city, region, postal_code, country}, print?:{template: classic|modern|simple, logo?: base64 PNG or JPEG}}" },
    CommandInfo { id: "account.create", summary: "Add an account. {number?, name, kind, description?, parent?: account id}" },
    CommandInfo { id: "account.update", summary: "Rename or renumber an account, or make it a sub-account. {id, number?, name, description?, parent?: account id (omit for top level)}" },
    CommandInfo { id: "account.merge", summary: "Move everything of one account onto another of the same type, then remove the first. {from, into}" },
    CommandInfo { id: "customer.merge", summary: "Move a customer's transactions onto another customer, then remove the first. {from, into}" },
    CommandInfo { id: "vendor.merge", summary: "Move a vendor's transactions onto another vendor, then remove the first. {from, into}" },
    CommandInfo { id: "item.merge", summary: "Move an item's lines onto another item, then remove the first. {from, into}" },
    CommandInfo { id: "account.setActive", summary: "Make an account active or inactive. {id, active}" },
    CommandInfo { id: "customer.create", summary: "Add a customer. {contact:{name, email?, phone?, terms_days?, …}}" },
    CommandInfo { id: "customer.update", summary: "Edit a customer. {id, contact}" },
    CommandInfo { id: "vendor.create", summary: "Add a vendor. {contact}" },
    CommandInfo { id: "vendor.update", summary: "Edit a vendor. {id, contact}" },
    CommandInfo { id: "customer.setActive", summary: "Make a customer active or inactive (inactive ones are left off forms). {id, active}" },
    CommandInfo { id: "vendor.setActive", summary: "Make a vendor active or inactive (inactive ones are left off forms). {id, active}" },
    CommandInfo { id: "item.create", summary: "Add a product or service. {spec:{name, rate, income_account, kind?, taxable?}}" },
    CommandInfo { id: "item.update", summary: "Change a product or service; recorded invoices keep their lines. {id, spec}" },
    CommandInfo { id: "item.setActive", summary: "Make an item active or inactive (inactive ones are left off forms). {id, active}" },
    CommandInfo { id: "txn.post", summary: "Record a transaction. {date, number?, memo?, body:{type: invoice|sales_receipt|credit_memo|refund|receive_payment|bill|pay_bill|check|deposit|journal, …}}" },
    CommandInfo { id: "txn.update", summary: "Edit a recorded transaction; it keeps its id and type. {id, date, number?, memo?, body}" },
    CommandInfo { id: "txn.void", summary: "Void a transaction (it stays on file). {id}" },
    CommandInfo { id: "account.reconcile", summary: "Reconcile a bank or credit card account against a statement; the cleared items must match the ending balance exactly. {account, statement_date, ending_balance, cleared:[txn ids]}" },
    CommandInfo { id: "account.undoReconciliation", summary: "Undo an account's last reconciliation. {account}" },
    CommandInfo { id: "bank.import", summary: "Bring downloaded bank lines into an account: match, add in a category, or skip each; lines imported before are passed over. {account, entries:[{line:{date, amount, payee?, memo?, number?, fit_id?}, action:{action: match|add|skip, txn? | category?, remember?}}]}" },
    CommandInfo { id: "bank.removeRule", summary: "Delete a bank feed rule by its position, from 0. {index}" },
    CommandInfo { id: "tax.create", summary: "Add a sales tax (one agency's rate) with its first rate, in percent. {name, agency?, code?, from, rate}" },
    CommandInfo { id: "tax.update", summary: "Rename a sales tax or change its agency or code. {id, name, agency?, code?}" },
    CommandInfo { id: "tax.setRate", summary: "Set the rate starting on a date; recorded invoices keep their rate. {id, from, rate}" },
    CommandInfo { id: "tax.removeRate", summary: "Remove the rate starting on a date (one must remain). {id, from}" },
    CommandInfo { id: "tax.setActive", summary: "Make a sales tax active or inactive. {id, active}" },
    CommandInfo { id: "taxGroup.create", summary: "Add a tax group: the sales taxes charged together at a location. {name, components:[ids]}" },
    CommandInfo { id: "taxGroup.update", summary: "Rename a tax group or change its sales taxes. {id, name, components:[ids]}" },
    CommandInfo { id: "taxGroup.setActive", summary: "Make a tax group active or inactive. {id, active}" },
    CommandInfo { id: "customer.setTax", summary: "A customer's tax group and exemption. {id, tax:{group?, exempt?, exemption?}}" },
    CommandInfo { id: "item.setTaxable", summary: "Mark an item taxable or not. {id, taxable}" },
    CommandInfo { id: "tax.import", summary: "Add or update sales taxes read from a state rate file (matched by code) and group them. {taxes:[{code, name, agency?, rates:[{from, rate}]}], group, on, customer?}" },
    CommandInfo { id: "tax.markChecked", summary: "Record that the sales tax rates were checked on a date (stops the quarterly reminder). {on}" },
];

impl Command {
    pub fn id(&self) -> &'static str {
        match self {
            Self::CompanyRename { .. } => "company.rename",
            Self::CompanySetFiscalYearStart { .. } => "company.setFiscalYearStart",
            Self::CompanySetClosing { .. } => "company.setClosing",
            Self::CompanySetInfo { .. } => "company.setInfo",
            Self::AccountCreate { .. } => "account.create",
            Self::AccountUpdate { .. } => "account.update",
            Self::AccountMerge { .. } => "account.merge",
            Self::CustomerMerge { .. } => "customer.merge",
            Self::VendorMerge { .. } => "vendor.merge",
            Self::ItemMerge { .. } => "item.merge",
            Self::AccountSetActive { .. } => "account.setActive",
            Self::CustomerCreate { .. } => "customer.create",
            Self::CustomerUpdate { .. } => "customer.update",
            Self::VendorCreate { .. } => "vendor.create",
            Self::VendorUpdate { .. } => "vendor.update",
            Self::CustomerSetActive { .. } => "customer.setActive",
            Self::VendorSetActive { .. } => "vendor.setActive",
            Self::ItemCreate { .. } => "item.create",
            Self::ItemUpdate { .. } => "item.update",
            Self::ItemSetActive { .. } => "item.setActive",
            Self::TxnPost { .. } => "txn.post",
            Self::TxnUpdate { .. } => "txn.update",
            Self::TxnVoid { .. } => "txn.void",
            Self::AccountReconcile { .. } => "account.reconcile",
            Self::AccountUndoReconciliation { .. } => "account.undoReconciliation",
            Self::BankImport { .. } => "bank.import",
            Self::BankRemoveRule { .. } => "bank.removeRule",
            Self::TaxCreate { .. } => "tax.create",
            Self::TaxUpdate { .. } => "tax.update",
            Self::TaxSetRate { .. } => "tax.setRate",
            Self::TaxRemoveRate { .. } => "tax.removeRate",
            Self::TaxSetActive { .. } => "tax.setActive",
            Self::TaxGroupCreate { .. } => "taxGroup.create",
            Self::TaxGroupUpdate { .. } => "taxGroup.update",
            Self::TaxGroupSetActive { .. } => "taxGroup.setActive",
            Self::CustomerSetTax { .. } => "customer.setTax",
            Self::ItemSetTaxable { .. } => "item.setTaxable",
            Self::TaxMarkChecked { .. } => "tax.markChecked",
            Self::TaxImport { .. } => "tax.import",
        }
    }

    /// What Undo says it will undo.
    pub fn undo_label(&self) -> String {
        match self {
            Self::CompanyRename { .. } | Self::CompanySetFiscalYearStart { .. } => {
                "Company Settings".into()
            }
            Self::CompanySetInfo { .. } => "Company Information".into(),
            Self::CompanySetClosing {
                date: Some(date), ..
            } => format!("Closing Date {date}"),
            Self::CompanySetClosing { date: None, .. } => "Remove Closing Date".into(),
            Self::AccountCreate { name, .. } => format!("New Account {name}"),
            Self::AccountUpdate { name, .. } => format!("Edit Account {name}"),
            Self::AccountMerge { .. } => "Merge Accounts".into(),
            Self::CustomerMerge { .. } => "Merge Customers".into(),
            Self::VendorMerge { .. } => "Merge Vendors".into(),
            Self::ItemMerge { .. } => "Merge Items".into(),
            Self::AccountSetActive { .. } => "Account Status".into(),
            Self::CustomerCreate { contact } => format!("New Customer {}", contact.name),
            Self::CustomerUpdate { contact, .. } => format!("Edit Customer {}", contact.name),
            Self::VendorCreate { contact } => format!("New Vendor {}", contact.name),
            Self::VendorUpdate { contact, .. } => format!("Edit Vendor {}", contact.name),
            Self::CustomerSetActive { .. } => "Customer Status".into(),
            Self::VendorSetActive { .. } => "Vendor Status".into(),
            Self::ItemCreate { spec } => format!("New Item {}", spec.name),
            Self::ItemUpdate { spec, .. } => format!("Edit Item {}", spec.name),
            Self::ItemSetActive { .. } => "Item Status".into(),
            Self::TxnPost { body, .. } => body.label().to_string(),
            Self::TxnUpdate { body, .. } => format!("Edit {}", body.label()),
            Self::TxnVoid { id } => format!("Void {id}"),
            Self::AccountReconcile { .. } => "Reconcile".into(),
            Self::AccountUndoReconciliation { .. } => "Undo Reconciliation".into(),
            Self::BankImport { .. } => "Import Bank Transactions".into(),
            Self::BankRemoveRule { .. } => "Delete Bank Rule".into(),
            Self::TaxCreate { name, .. } => format!("New Sales Tax {name}"),
            Self::TaxUpdate { name, .. } => format!("Edit Sales Tax {name}"),
            Self::TaxSetRate { .. } | Self::TaxRemoveRate { .. } => "Sales Tax Rate".into(),
            Self::TaxSetActive { .. } => "Sales Tax Status".into(),
            Self::TaxGroupCreate { name, .. } => format!("New Tax Group {name}"),
            Self::TaxGroupUpdate { name, .. } => format!("Edit Tax Group {name}"),
            Self::TaxGroupSetActive { .. } => "Tax Group Status".into(),
            Self::CustomerSetTax { .. } => "Customer Sales Tax".into(),
            Self::ItemSetTaxable { .. } => "Item Taxable".into(),
            Self::TaxMarkChecked { .. } => "Sales Tax Rates Checked".into(),
            Self::TaxImport { customer: None, .. } => "Import Sales Tax Rates".into(),
            Self::TaxImport { .. } => "Find Tax Group".into(),
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
                print,
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
                if let Some(print) = print {
                    if print
                        .logo
                        .as_ref()
                        .is_some_and(|l| l.0.len() > ledgercraft_ledger::LOGO_MAX_BYTES)
                    {
                        return Err(ledgercraft_ledger::LedgerError::Invalid(
                            "The logo file is too large (1 MB at most)".into(),
                        ));
                    }
                    c.info.print = print;
                }
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
            Self::CompanySetClosing {
                date,
                password,
                keep_password,
            } => {
                let kept = c.closing.as_ref().and_then(|x| x.password.clone());
                let password = match (keep_password, password) {
                    (true, _) => kept,
                    (false, Some(p)) => Some(
                        ledgercraft_file::password_check(&p)
                            .map_err(|e| ledgercraft_ledger::LedgerError::Invalid(e.to_string()))?,
                    ),
                    (false, None) => None,
                };
                c.closing = date.map(|date| ledgercraft_ledger::Closing { date, password });
                None
            }
            Self::AccountCreate {
                number,
                name,
                kind,
                description,
                parent,
            } => {
                let id = c.add_account(&number, &name, kind, &description)?;
                if parent.is_some() {
                    c.set_account_parent(id, parent)?;
                }
                Some(id.to_string())
            }
            Self::AccountUpdate {
                id,
                number,
                name,
                description,
                parent,
            } => {
                c.update_account(id, &number, &name, &description)?;
                c.set_account_parent(id, parent)?;
                None
            }
            Self::AccountMerge { from, into } => {
                c.merge_accounts(from, into)?;
                None
            }
            Self::CustomerMerge { from, into } => {
                c.merge_customers(from, into)?;
                None
            }
            Self::VendorMerge { from, into } => {
                c.merge_vendors(from, into)?;
                None
            }
            Self::ItemMerge { from, into } => {
                c.merge_items(from, into)?;
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
            Self::CustomerSetActive { id, active } => {
                c.set_customer_active(id, active)?;
                None
            }
            Self::VendorSetActive { id, active } => {
                c.set_vendor_active(id, active)?;
                None
            }
            Self::ItemCreate { spec } => Some(c.add_item(spec)?.to_string()),
            Self::ItemUpdate { id, spec } => {
                c.update_item(id, spec)?;
                None
            }
            Self::ItemSetActive { id, active } => {
                c.set_item_active(id, active)?;
                None
            }
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
            Self::TaxCreate {
                name,
                agency,
                code,
                from,
                rate,
            } => {
                let id = c.add_tax_component(&name, &agency, &code, from, rate)?;
                // Adding a rate is checking it.
                c.mark_tax_rates_checked(Date::today());
                Some(id.to_string())
            }
            Self::TaxUpdate {
                id,
                name,
                agency,
                code,
            } => {
                c.update_tax_component(id, &name, &agency, &code)?;
                None
            }
            Self::TaxSetRate { id, from, rate } => {
                c.set_tax_rate(id, from, rate)?;
                c.mark_tax_rates_checked(Date::today());
                None
            }
            Self::TaxRemoveRate { id, from } => {
                c.remove_tax_rate(id, from)?;
                None
            }
            Self::TaxSetActive { id, active } => {
                c.set_tax_component_active(id, active)?;
                None
            }
            Self::TaxGroupCreate { name, components } => {
                Some(c.add_tax_group(&name, components)?.to_string())
            }
            Self::TaxGroupUpdate {
                id,
                name,
                components,
            } => {
                c.update_tax_group(id, &name, components)?;
                None
            }
            Self::TaxGroupSetActive { id, active } => {
                c.set_tax_group_active(id, active)?;
                None
            }
            Self::CustomerSetTax { id, tax } => {
                c.set_customer_tax(id, tax)?;
                None
            }
            Self::ItemSetTaxable { id, taxable } => {
                c.set_item_taxable(id, taxable)?;
                None
            }
            Self::TaxMarkChecked { on } => {
                c.mark_tax_rates_checked(on);
                None
            }
            Self::TaxImport {
                taxes,
                group,
                on,
                customer,
            } => {
                let group = c.import_taxes(taxes, &group, on)?;
                if let Some(id) = customer {
                    let mut tax = c.customer(id).map(|x| x.tax.clone()).ok_or_else(|| {
                        ledgercraft_ledger::LedgerError::NotFound(format!("Customer {id}"))
                    })?;
                    tax.group = Some(group);
                    c.set_customer_tax(id, tax)?;
                }
                Some(group.to_string())
            }
        })
    }
}
