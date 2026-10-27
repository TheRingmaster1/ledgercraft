use std::collections::BTreeSet;

use ledgercraft_core::{Date, Money};
use serde::{Deserialize, Serialize};

use crate::{
    default_chart_of_accounts, Account, AccountId, AccountLine, AccountType, Address, Applied,
    BankRule, Contact, Customer, CustomerId, CustomerTax, FeedLink, Item, ItemId, ItemSpec,
    LedgerError, LedgerResult, Party, Posting, Reconciliation, Sale, TaxComponent, TaxGroup,
    Transaction, TxnBody, TxnId, Vendor, VendorId,
};

/// Company-wide settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanyInfo {
    pub name: String,
    #[serde(default)]
    pub legal_name: String,
    #[serde(default)]
    pub address: Address,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub phone: String,
    /// First month of the fiscal year, 1–12.
    #[serde(default = "january")]
    pub fiscal_year_start_month: u32,
    #[serde(default = "usd")]
    pub currency: String,
    /// The look of printed documents, and the logo on them.
    #[serde(default)]
    pub print: crate::PrintStyle,
}

fn january() -> u32 {
    1
}

fn usd() -> String {
    "USD".to_string()
}

/// One posting with the transaction it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerEntry {
    pub txn: TxnId,
    pub date: Date,
    pub posting: Posting,
}

/// A company file: everything LedgerCraft knows about one set of books.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    pub info: CompanyInfo,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub customers: Vec<Customer>,
    #[serde(default)]
    pub vendors: Vec<Vendor>,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    /// Completed bank and credit card reconciliations, oldest first.
    #[serde(default)]
    pub reconciliations: Vec<Reconciliation>,
    /// Downloaded bank lines already imported (so none comes in twice).
    #[serde(default)]
    pub feed_links: Vec<FeedLink>,
    /// Bank feed rules: payee text to category.
    #[serde(default)]
    pub bank_rules: Vec<BankRule>,
    /// Sales tax components (one rate for one agency, with its history).
    #[serde(default)]
    pub tax_components: Vec<TaxComponent>,
    /// Sales tax groups: the components charged together at a location.
    #[serde(default)]
    pub tax_groups: Vec<TaxGroup>,
    /// When the sales tax rates were last checked or changed.
    #[serde(default)]
    pub tax_rates_checked: Option<Date>,
    /// The books are closed through this date, if they have been closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closing: Option<crate::Closing>,
    /// Every change made to the books, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audit: Vec<crate::AuditEntry>,
    #[serde(default)]
    next_id: u64,
}

impl Company {
    /// An empty company with no accounts.
    pub fn new(name: &str) -> Self {
        Self {
            info: CompanyInfo {
                name: name.to_string(),
                legal_name: String::new(),
                address: Address::default(),
                email: String::new(),
                phone: String::new(),
                fiscal_year_start_month: 1,
                currency: usd(),
                print: crate::PrintStyle::default(),
            },
            accounts: Vec::new(),
            customers: Vec::new(),
            vendors: Vec::new(),
            items: Vec::new(),
            transactions: Vec::new(),
            reconciliations: Vec::new(),
            feed_links: Vec::new(),
            bank_rules: Vec::new(),
            tax_components: Vec::new(),
            tax_groups: Vec::new(),
            tax_rates_checked: None,
            closing: None,
            audit: Vec::new(),
            next_id: 0,
        }
    }

    /// A new company with the default chart of accounts.
    pub fn with_default_accounts(name: &str) -> Self {
        let mut c = Self::new(name);
        for row in default_chart_of_accounts() {
            // The template has unique names and numbers, so this cannot fail;
            // ignoring the result keeps construction infallible.
            let _ = c.add_account(row.number, row.name, row.kind, row.description);
        }
        c
    }

    pub(crate) fn alloc(&mut self) -> u64 {
        self.next_id = self.next_id.saturating_add(1);
        self.next_id
    }

    /// After loading a file, make sure new ids never collide with existing ones.
    pub fn repair_next_id(&mut self) {
        let max = self
            .accounts
            .iter()
            .map(|a| a.id.0)
            .chain(self.customers.iter().map(|c| c.id.0))
            .chain(self.vendors.iter().map(|v| v.id.0))
            .chain(self.items.iter().map(|i| i.id.0))
            .chain(self.transactions.iter().map(|t| t.id.0))
            .chain(self.tax_components.iter().map(|t| t.id.0))
            .chain(self.tax_groups.iter().map(|g| g.id.0))
            .max()
            .unwrap_or(0);
        self.next_id = self.next_id.max(max);
    }

    // ----- lookups ------------------------------------------------------

    pub fn account(&self, id: AccountId) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn account_by_name(&self, name: &str) -> Option<&Account> {
        let name = name.trim();
        self.accounts.iter().find(|a| {
            a.name.eq_ignore_ascii_case(name) || (!a.number.is_empty() && a.number == name)
        })
    }

    /// The first active account of a type, e.g. the default AR account.
    pub fn first_account_of(&self, kind: AccountType) -> Option<AccountId> {
        self.accounts
            .iter()
            .find(|a| a.kind == kind && a.active)
            .map(|a| a.id)
    }

    /// Accounts sorted the way the chart of accounts shows them.
    pub fn sorted_accounts(&self) -> Vec<&Account> {
        let mut v: Vec<&Account> = self.accounts.iter().collect();
        v.sort_by(|a, b| (a.kind, &a.number, &a.name).cmp(&(b.kind, &b.number, &b.name)));
        v
    }

    pub fn customer(&self, id: CustomerId) -> Option<&Customer> {
        self.customers.iter().find(|c| c.id == id)
    }

    pub fn vendor(&self, id: VendorId) -> Option<&Vendor> {
        self.vendors.iter().find(|v| v.id == id)
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn txn(&self, id: TxnId) -> Option<&Transaction> {
        self.transactions.iter().find(|t| t.id == id)
    }

    pub fn party_name(&self, party: Party) -> String {
        match party {
            Party::Customer(id) => self
                .customer(id)
                .map(|c| c.contact.name.clone())
                .unwrap_or_else(|| id.to_string()),
            Party::Vendor(id) => self
                .vendor(id)
                .map(|v| v.contact.name.clone())
                .unwrap_or_else(|| id.to_string()),
        }
    }

    /// What a transaction is called in lists and registers. A "check" paid
    /// from a credit card is really a card charge, so it is named that way.
    pub fn txn_type_label(&self, t: &Transaction) -> &'static str {
        match &t.body {
            TxnBody::Check(c)
                if self
                    .account(c.pay_from)
                    .is_some_and(|a| a.kind == AccountType::CreditCard) =>
            {
                "Credit Card Charge"
            }
            body => body.label(),
        }
    }

    pub fn account_name(&self, id: AccountId) -> String {
        self.account(id)
            .map(Account::display_name)
            .unwrap_or_else(|| id.to_string())
    }

    // ----- lists --------------------------------------------------------

    pub fn add_account(
        &mut self,
        number: &str,
        name: &str,
        kind: AccountType,
        description: &str,
    ) -> LedgerResult<AccountId> {
        self.check_account_names(None, number, name)?;
        let id = AccountId(self.alloc());
        self.accounts.push(Account {
            id,
            number: number.trim().to_string(),
            name: name.trim().to_string(),
            kind,
            description: description.to_string(),
            active: true,
            parent: None,
        });
        Ok(id)
    }

    /// Renames or renumbers an account. Its type cannot change once created,
    /// so posted history keeps its meaning.
    pub fn update_account(
        &mut self,
        id: AccountId,
        number: &str,
        name: &str,
        description: &str,
    ) -> LedgerResult<()> {
        self.check_account_names(Some(id), number, name)?;
        let acct = self
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Account {id}")))?;
        acct.number = number.trim().to_string();
        acct.name = name.trim().to_string();
        acct.description = description.to_string();
        Ok(())
    }

    fn check_account_names(
        &self,
        me: Option<AccountId>,
        number: &str,
        name: &str,
    ) -> LedgerResult<()> {
        let (number, name) = (number.trim(), name.trim());
        if name.is_empty() {
            return Err(LedgerError::Required("An account name"));
        }
        for a in self.accounts.iter().filter(|a| Some(a.id) != me) {
            if a.name.eq_ignore_ascii_case(name) {
                return Err(LedgerError::Duplicate(format!("An account named {name:?}")));
            }
            if !number.is_empty() && a.number == number {
                return Err(LedgerError::Duplicate(format!("Account number {number}")));
            }
        }
        Ok(())
    }

    pub fn set_account_active(&mut self, id: AccountId, active: bool) -> LedgerResult<()> {
        if !active && !self.account_balance(id, None).is_zero() {
            return Err(LedgerError::InUse(
                "An account with a balance cannot be made inactive; move its balance first".into(),
            ));
        }
        let acct = self
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Account {id}")))?;
        acct.active = active;
        Ok(())
    }

    fn check_contact(contact: &Contact) -> LedgerResult<()> {
        if contact.name.trim().is_empty() {
            return Err(LedgerError::Required("A name"));
        }
        if contact.terms_days > 3650 {
            return Err(LedgerError::Invalid(
                "Payment terms must be 3650 days or fewer".into(),
            ));
        }
        Ok(())
    }

    pub fn add_customer(&mut self, contact: Contact) -> LedgerResult<CustomerId> {
        Self::check_contact(&contact)?;
        let name = contact.name.trim();
        if self
            .customers
            .iter()
            .any(|c| c.contact.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!("A customer named {name:?}")));
        }
        let id = CustomerId(self.alloc());
        self.customers.push(Customer {
            id,
            contact,
            active: true,
            tax: CustomerTax::default(),
        });
        Ok(id)
    }

    pub fn update_customer(&mut self, id: CustomerId, contact: Contact) -> LedgerResult<()> {
        Self::check_contact(&contact)?;
        let name = contact.name.trim().to_string();
        if self
            .customers
            .iter()
            .any(|c| c.id != id && c.contact.name.eq_ignore_ascii_case(&name))
        {
            return Err(LedgerError::Duplicate(format!("A customer named {name:?}")));
        }
        let c = self
            .customers
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Customer {id}")))?;
        c.contact = contact;
        Ok(())
    }

    pub fn add_vendor(&mut self, contact: Contact) -> LedgerResult<VendorId> {
        Self::check_contact(&contact)?;
        let name = contact.name.trim();
        if self
            .vendors
            .iter()
            .any(|v| v.contact.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!("A vendor named {name:?}")));
        }
        let id = VendorId(self.alloc());
        self.vendors.push(Vendor {
            id,
            contact,
            active: true,
        });
        Ok(id)
    }

    pub fn update_vendor(&mut self, id: VendorId, contact: Contact) -> LedgerResult<()> {
        Self::check_contact(&contact)?;
        let name = contact.name.trim().to_string();
        if self
            .vendors
            .iter()
            .any(|v| v.id != id && v.contact.name.eq_ignore_ascii_case(&name))
        {
            return Err(LedgerError::Duplicate(format!("A vendor named {name:?}")));
        }
        let v = self
            .vendors
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Vendor {id}")))?;
        v.contact = contact;
        Ok(())
    }

    pub fn add_item(&mut self, spec: ItemSpec) -> LedgerResult<ItemId> {
        let name = spec.name.trim();
        if name.is_empty() {
            return Err(LedgerError::Required("An item name"));
        }
        if self
            .items
            .iter()
            .any(|i| i.spec.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!("An item named {name:?}")));
        }
        let acct = self.require_account(spec.income_account)?;
        if acct.kind.needs_party() {
            return Err(LedgerError::WrongAccountType {
                account: acct.display_name(),
                expected: "an income or other non-AR/AP account",
            });
        }
        let id = ItemId(self.alloc());
        self.items.push(Item {
            id,
            spec,
            active: true,
        });
        Ok(id)
    }

    /// Changes an item. Invoices already recorded keep their own lines.
    pub fn update_item(&mut self, id: ItemId, spec: ItemSpec) -> LedgerResult<()> {
        let name = spec.name.trim();
        if name.is_empty() {
            return Err(LedgerError::Required("An item name"));
        }
        if self
            .items
            .iter()
            .any(|i| i.id != id && i.spec.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!("An item named {name:?}")));
        }
        let acct = self.require_account(spec.income_account)?;
        if acct.kind.needs_party() {
            return Err(LedgerError::WrongAccountType {
                account: acct.display_name(),
                expected: "an income or other non-AR/AP account",
            });
        }
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Item {id}")))?;
        item.spec = spec;
        Ok(())
    }

    /// An inactive item is left off the item lists on forms; invoices
    /// already recorded keep it.
    pub fn set_item_active(&mut self, id: ItemId, active: bool) -> LedgerResult<()> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Item {id}")))?;
        item.active = active;
        Ok(())
    }

    /// An inactive customer is left off the customer lists on forms; their
    /// transactions and balance stay.
    pub fn set_customer_active(&mut self, id: CustomerId, active: bool) -> LedgerResult<()> {
        let c = self
            .customers
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Customer {id}")))?;
        c.active = active;
        Ok(())
    }

    /// An inactive vendor is left off the vendor lists on forms; their
    /// transactions and balance stay.
    pub fn set_vendor_active(&mut self, id: VendorId, active: bool) -> LedgerResult<()> {
        let v = self
            .vendors
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Vendor {id}")))?;
        v.active = active;
        Ok(())
    }

    // ----- transactions ---------------------------------------------------

    fn require_account(&self, id: AccountId) -> LedgerResult<&Account> {
        let a = self
            .account(id)
            .ok_or_else(|| LedgerError::NotFound(format!("Account {id}")))?;
        if !a.active {
            return Err(LedgerError::Inactive(a.display_name()));
        }
        Ok(a)
    }

    pub(crate) fn require_kind(
        &self,
        id: AccountId,
        ok: impl Fn(AccountType) -> bool,
        expected: &'static str,
    ) -> LedgerResult<&Account> {
        let a = self.require_account(id)?;
        if !ok(a.kind) {
            return Err(LedgerError::WrongAccountType {
                account: a.display_name(),
                expected,
            });
        }
        Ok(a)
    }

    fn require_customer(&self, id: CustomerId) -> LedgerResult<&Customer> {
        let c = self
            .customer(id)
            .ok_or_else(|| LedgerError::NotFound(format!("Customer {id}")))?;
        if !c.active {
            return Err(LedgerError::Inactive(c.contact.name.clone()));
        }
        Ok(c)
    }

    fn require_vendor(&self, id: VendorId) -> LedgerResult<&Vendor> {
        let v = self
            .vendor(id)
            .ok_or_else(|| LedgerError::NotFound(format!("Vendor {id}")))?;
        if !v.active {
            return Err(LedgerError::Inactive(v.contact.name.clone()));
        }
        Ok(v)
    }

    fn require_party(&self, party: Party) -> LedgerResult<()> {
        match party {
            Party::Customer(id) => self.require_customer(id).map(|_| ()),
            Party::Vendor(id) => self.require_vendor(id).map(|_| ()),
        }
    }

    fn check_expense_lines(&self, lines: &[AccountLine]) -> LedgerResult<Money> {
        if lines.is_empty() {
            return Err(LedgerError::Required("At least one line"));
        }
        for l in lines {
            self.require_kind(
                l.account,
                |k| !k.needs_party(),
                "an account other than AR or AP",
            )?;
        }
        let total =
            Money::checked_sum(lines.iter().map(|l| l.amount)).ok_or(LedgerError::Overflow)?;
        if !total.is_positive() {
            return Err(LedgerError::NotPositive("The total"));
        }
        Ok(total)
    }

    /// Checks a payment's applications against the open documents.
    fn check_applications(
        &self,
        applied: &[Applied],
        amount: Money,
        is_target: impl Fn(&TxnBody) -> bool,
        ignore: Option<TxnId>,
    ) -> LedgerResult<()> {
        let mut seen = BTreeSet::new();
        let mut sum = Money::ZERO;
        for a in applied {
            if !a.amount.is_positive() {
                return Err(LedgerError::NotPositive("Each applied amount"));
            }
            if !seen.insert(a.txn) {
                return Err(LedgerError::Invalid(format!("{} is applied twice", a.txn)));
            }
            let doc = self
                .txn(a.txn)
                .filter(|t| !t.voided && is_target(&t.body))
                .ok_or_else(|| LedgerError::NotFound(format!("Open document {}", a.txn)))?;
            let open = self.open_balance_excluding(doc.id, None, ignore);
            if a.amount > open {
                return Err(LedgerError::OverApplied {
                    txn: doc.id,
                    open,
                    applied: a.amount,
                });
            }
            sum = sum.checked_add(a.amount).ok_or(LedgerError::Overflow)?;
        }
        if sum > amount {
            return Err(LedgerError::Invalid(format!(
                "applied amounts ({sum}) exceed the payment ({amount})"
            )));
        }
        Ok(())
    }

    /// Checks the lines and tax of an invoice, sales receipt or credit memo;
    /// returns its total. `what` names the total in the error.
    fn check_sale(&self, sale: &Sale<'_>, what: &'static str) -> LedgerResult<Money> {
        if sale.lines.is_empty() {
            return Err(LedgerError::Required("At least one line"));
        }
        for l in sale.lines {
            self.require_kind(
                l.account,
                |k| !k.needs_party(),
                "an account other than AR or AP",
            )?;
            if let Some(item) = l.item {
                self.item(item)
                    .ok_or_else(|| LedgerError::NotFound(format!("Item {item}")))?;
            }
        }
        let total = sale.total().ok_or(LedgerError::Overflow)?;
        if !total.is_positive() {
            return Err(LedgerError::NotPositive(what));
        }
        self.validate_sale_tax(sale)?;
        Ok(total)
    }

    /// Validates a transaction against the books without changing anything.
    pub fn validate(&self, body: &TxnBody) -> LedgerResult<()> {
        self.validate_for(body, None)
    }

    /// Validates `body`. `editing` is the transaction it would replace, if
    /// any: that transaction's own payment amounts are left out when checking
    /// what is still open, so an edited payment can apply them again.
    fn validate_for(&self, body: &TxnBody, editing: Option<TxnId>) -> LedgerResult<()> {
        match body {
            TxnBody::Journal(j) => {
                if j.lines.len() < 2 {
                    return Err(LedgerError::Required("At least two lines"));
                }
                let mut debits = Money::ZERO;
                let mut credits = Money::ZERO;
                for l in &j.lines {
                    let acct = self.require_account(l.account)?;
                    if l.debit.is_negative() || l.credit.is_negative() {
                        return Err(LedgerError::Invalid(
                            "Debits and credits cannot be negative".into(),
                        ));
                    }
                    if l.debit.is_zero() == l.credit.is_zero() {
                        return Err(LedgerError::Invalid(
                            "Each line needs either a debit or a credit".into(),
                        ));
                    }
                    if let Some(p) = l.party {
                        self.require_party(p)?;
                    }
                    match (acct.kind, l.party) {
                        (AccountType::AccountsReceivable, Some(Party::Customer(_)))
                        | (AccountType::AccountsPayable, Some(Party::Vendor(_))) => {}
                        (AccountType::AccountsReceivable, _) => {
                            return Err(LedgerError::Required("A customer on each AR line"))
                        }
                        (AccountType::AccountsPayable, _) => {
                            return Err(LedgerError::Required("A vendor on each AP line"))
                        }
                        _ => {}
                    }
                    debits = debits.checked_add(l.debit).ok_or(LedgerError::Overflow)?;
                    credits = credits.checked_add(l.credit).ok_or(LedgerError::Overflow)?;
                }
                if debits != credits {
                    return Err(LedgerError::Unbalanced { debits, credits });
                }
            }
            TxnBody::Invoice(inv) => {
                self.require_customer(inv.customer)?;
                self.require_kind(
                    inv.ar_account,
                    |k| k == AccountType::AccountsReceivable,
                    "an Accounts Receivable account",
                )?;
                if let Some(sale) = body.sale() {
                    self.check_sale(&sale, "The invoice total")?;
                }
            }
            TxnBody::SalesReceipt(r) => {
                if let Some(customer) = r.customer {
                    self.require_customer(customer)?;
                }
                self.require_kind(
                    r.deposit_to,
                    |k| matches!(k, AccountType::Bank | AccountType::OtherCurrentAsset),
                    "a bank or Undeposited Funds account",
                )?;
                if let Some(sale) = body.sale() {
                    self.check_sale(&sale, "The sales receipt total")?;
                }
            }
            TxnBody::CreditMemo(m) => {
                self.require_customer(m.customer)?;
                self.require_kind(
                    m.ar_account,
                    |k| k == AccountType::AccountsReceivable,
                    "an Accounts Receivable account",
                )?;
                let total = match body.sale() {
                    Some(sale) => self.check_sale(&sale, "The credit memo total")?,
                    None => Money::ZERO,
                };
                let customer = m.customer;
                self.check_applications(
                    &m.applied,
                    total,
                    |b| matches!(b, TxnBody::Invoice(i) if i.customer == customer),
                    editing,
                )?;
                // An edited credit cannot shrink below what it has paid out.
                let refunded = editing.map_or(Money::ZERO, |id| self.paid_against(id));
                let used = Money::checked_sum(m.applied.iter().map(|a| a.amount))
                    .and_then(|s| s.checked_add(refunded))
                    .ok_or(LedgerError::Overflow)?;
                if used > total {
                    return Err(LedgerError::Invalid(format!(
                        "the credit of {total} is less than the {used} it applies and refunds"
                    )));
                }
            }
            TxnBody::Refund(r) => {
                self.require_customer(r.customer)?;
                self.require_kind(
                    r.pay_from,
                    |k| matches!(k, AccountType::Bank | AccountType::CreditCard),
                    "a bank or credit card account",
                )?;
                self.require_kind(
                    r.ar_account,
                    |k| k == AccountType::AccountsReceivable,
                    "an Accounts Receivable account",
                )?;
                if !r.amount.is_positive() {
                    return Err(LedgerError::NotPositive("The refund amount"));
                }
                let customer = r.customer;
                self.check_applications(
                    &r.applied,
                    r.amount,
                    |b| matches!(b, TxnBody::CreditMemo(m) if m.customer == customer),
                    editing,
                )?;
                let applied = Money::checked_sum(r.applied.iter().map(|a| a.amount))
                    .ok_or(LedgerError::Overflow)?;
                if applied != r.amount {
                    return Err(LedgerError::Invalid(format!(
                        "a refund pays back credit memos: {applied} of the {} is applied to them",
                        r.amount
                    )));
                }
            }
            TxnBody::ReceivePayment(p) => {
                self.require_customer(p.customer)?;
                self.require_kind(
                    p.deposit_to,
                    |k| matches!(k, AccountType::Bank | AccountType::OtherCurrentAsset),
                    "a bank or Undeposited Funds account",
                )?;
                self.require_kind(
                    p.ar_account,
                    |k| k == AccountType::AccountsReceivable,
                    "an Accounts Receivable account",
                )?;
                if !p.amount.is_positive() {
                    return Err(LedgerError::NotPositive("The payment amount"));
                }
                let customer = p.customer;
                self.check_applications(
                    &p.applied,
                    p.amount,
                    |b| matches!(b, TxnBody::Invoice(i) if i.customer == customer),
                    editing,
                )?;
            }
            TxnBody::Bill(b) => {
                self.require_vendor(b.vendor)?;
                self.require_kind(
                    b.ap_account,
                    |k| k == AccountType::AccountsPayable,
                    "an Accounts Payable account",
                )?;
                self.check_expense_lines(&b.lines)?;
            }
            TxnBody::PayBill(p) => {
                self.require_vendor(p.vendor)?;
                self.require_kind(
                    p.pay_from,
                    |k| matches!(k, AccountType::Bank | AccountType::CreditCard),
                    "a bank or credit card account",
                )?;
                self.require_kind(
                    p.ap_account,
                    |k| k == AccountType::AccountsPayable,
                    "an Accounts Payable account",
                )?;
                if !p.amount.is_positive() {
                    return Err(LedgerError::NotPositive("The payment amount"));
                }
                let vendor = p.vendor;
                self.check_applications(
                    &p.applied,
                    p.amount,
                    |b| matches!(b, TxnBody::Bill(bill) if bill.vendor == vendor),
                    editing,
                )?;
            }
            TxnBody::Check(c) => {
                if let Some(p) = c.payee {
                    self.require_party(p)?;
                }
                self.require_kind(
                    c.pay_from,
                    |k| matches!(k, AccountType::Bank | AccountType::CreditCard),
                    "a bank or credit card account",
                )?;
                self.check_expense_lines(&c.lines)?;
            }
            TxnBody::Deposit(d) => {
                self.require_kind(d.deposit_to, |k| k == AccountType::Bank, "a bank account")?;
                self.check_expense_lines(&d.lines)?;
            }
        }
        // Belt and braces: whatever the form, the postings must balance.
        let postings = body.postings()?;
        let sum =
            Money::checked_sum(postings.iter().map(|p| p.amount)).ok_or(LedgerError::Overflow)?;
        if !sum.is_zero() {
            return Err(LedgerError::Invalid(format!(
                "internal: postings are off by {sum}"
            )));
        }
        Ok(())
    }

    /// Validates and records a transaction.
    pub fn post(
        &mut self,
        date: Date,
        number: &str,
        memo: &str,
        body: TxnBody,
    ) -> LedgerResult<TxnId> {
        self.validate(&body)?;
        let id = TxnId(self.alloc());
        self.transactions.push(Transaction {
            id,
            date,
            number: number.trim().to_string(),
            memo: memo.to_string(),
            body,
            voided: false,
        });
        Ok(id)
    }

    /// Replaces a posted transaction with an edited version. It keeps its id,
    /// so payments applied to an invoice or bill stay attached, and it stays
    /// the same type of transaction. The edit is checked like a new
    /// transaction, and an invoice or bill cannot drop below what has already
    /// been paid on it, or change customer or vendor while payments are applied.
    pub fn update(
        &mut self,
        id: TxnId,
        date: Date,
        number: &str,
        memo: &str,
        body: TxnBody,
    ) -> LedgerResult<()> {
        let old = self
            .txn(id)
            .ok_or_else(|| LedgerError::NotFound(format!("Transaction {id}")))?;
        if old.voided {
            return Err(LedgerError::Invalid(format!(
                "{id} is void; a void transaction cannot be edited"
            )));
        }
        if std::mem::discriminant(&old.body) != std::mem::discriminant(&body) {
            return Err(LedgerError::Invalid(format!(
                "{id} is a {}; it cannot be changed into a {}",
                old.body.label(),
                body.label()
            )));
        }
        let old_party = old.body.party();
        self.validate_for(&body, Some(id))?;
        self.check_reconciled_change(id, Some(&body))?;
        let paid = self.paid_against(id);
        if paid.is_positive() {
            if body.party() != old_party {
                return Err(LedgerError::InUse(format!(
                    "{id} has payments applied to it; void those payments before changing \
                     the customer or vendor"
                )));
            }
            let total = match &body {
                TxnBody::Invoice(i) => i.total(),
                TxnBody::Bill(b) => b.total(),
                TxnBody::CreditMemo(m) => m.total(),
                _ => None,
            }
            .unwrap_or(Money::ZERO);
            if total < paid {
                return Err(LedgerError::Invalid(format!(
                    "{id} already has {paid} paid against it, more than its new total of {total}"
                )));
            }
        }
        let t = self
            .transactions
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Transaction {id}")))?;
        t.date = date;
        t.number = number.trim().to_string();
        t.memo = memo.to_string();
        t.body = body;
        Ok(())
    }

    /// The total that live payments apply to an invoice or bill.
    fn paid_against(&self, doc: TxnId) -> Money {
        self.transactions
            .iter()
            .filter(|p| !p.voided && p.id != doc)
            .flat_map(|p| p.body.applications().iter())
            .filter(|a| a.txn == doc)
            .fold(Money::ZERO, |acc, a| acc.saturating_add(a.amount))
    }

    /// Voids a transaction: it stays on file but no longer posts.
    pub fn void(&mut self, id: TxnId) -> LedgerResult<()> {
        self.check_reconciled_change(id, None)?;
        let in_use = self
            .transactions
            .iter()
            .any(|t| !t.voided && t.body.applications().iter().any(|a| a.txn == id));
        if in_use {
            return Err(LedgerError::InUse(format!(
                "{id} has payments applied to it; void those payments first"
            )));
        }
        let t = self
            .transactions
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Transaction {id}")))?;
        if t.voided {
            return Err(LedgerError::Invalid(format!("{id} is already void")));
        }
        t.voided = true;
        Ok(())
    }

    // ----- balances -------------------------------------------------------

    /// Every posting from every live transaction, in date order.
    pub fn ledger_entries(&self) -> Vec<LedgerEntry> {
        let mut txns: Vec<&Transaction> = self.transactions.iter().filter(|t| !t.voided).collect();
        txns.sort_by_key(|t| (t.date, t.id));
        let mut out = Vec::new();
        for t in txns {
            if let Ok(postings) = t.body.postings() {
                for posting in postings {
                    out.push(LedgerEntry {
                        txn: t.id,
                        date: t.date,
                        posting,
                    });
                }
            }
        }
        out
    }

    /// An account's balance as a person reads it: positive means the account
    /// holds its normal kind of balance (an asset you have, a debt you owe).
    pub fn account_balance(&self, id: AccountId, as_of: Option<Date>) -> Money {
        let Some(acct) = self.account(id) else {
            return Money::ZERO;
        };
        let raw = self
            .ledger_entries()
            .into_iter()
            .filter(|e| e.posting.account == id && as_of.is_none_or(|d| e.date <= d))
            .fold(Money::ZERO, |acc, e| acc.saturating_add(e.posting.amount));
        if acct.kind.is_debit_normal() {
            raw
        } else {
            raw.saturating_neg()
        }
    }

    /// What is still owed on an invoice or bill.
    pub fn open_balance(&self, doc: TxnId, as_of: Option<Date>) -> Money {
        self.open_balance_excluding(doc, as_of, None)
    }

    fn open_balance_excluding(
        &self,
        doc: TxnId,
        as_of: Option<Date>,
        ignore: Option<TxnId>,
    ) -> Money {
        let Some(t) = self.txn(doc) else {
            return Money::ZERO;
        };
        if t.voided {
            return Money::ZERO;
        }
        let total = match &t.body {
            TxnBody::Invoice(i) => i.total(),
            TxnBody::Bill(b) => b.total(),
            TxnBody::CreditMemo(m) => m.total(),
            _ => None,
        }
        .unwrap_or(Money::ZERO);
        // A credit memo's own credit is also used up by the invoices it pays.
        let used_here = match &t.body {
            TxnBody::CreditMemo(m) if ignore != Some(doc) => m
                .applied
                .iter()
                .fold(Money::ZERO, |acc, a| acc.saturating_add(a.amount)),
            _ => Money::ZERO,
        };
        let paid = self
            .transactions
            .iter()
            .filter(|p| !p.voided && Some(p.id) != ignore && as_of.is_none_or(|d| p.date <= d))
            .flat_map(|p| p.body.applications().iter())
            .filter(|a| a.txn == doc)
            .fold(Money::ZERO, |acc, a| acc.saturating_add(a.amount));
        total.saturating_sub(paid).saturating_sub(used_here)
    }

    /// Open invoices for a customer, oldest first: (invoice, open amount).
    pub fn open_invoices(&self, customer: CustomerId) -> Vec<(&Transaction, Money)> {
        self.open_invoices_for(customer, None)
    }

    /// Open bills for a vendor, oldest first: (bill, open amount).
    pub fn open_bills(&self, vendor: VendorId) -> Vec<(&Transaction, Money)> {
        self.open_bills_for(vendor, None)
    }

    /// Open invoices as the payment being edited (`editing`) sees them: its
    /// own amounts are left out, so the invoices it paid show as open again.
    pub fn open_invoices_for(
        &self,
        customer: CustomerId,
        editing: Option<TxnId>,
    ) -> Vec<(&Transaction, Money)> {
        self.open_docs(
            |b| matches!(b, TxnBody::Invoice(i) if i.customer == customer),
            editing,
        )
    }

    /// A customer's credit memos with credit left, as the refund being edited
    /// (`editing`) sees them: (credit memo, credit left).
    pub fn open_credits_for(
        &self,
        customer: CustomerId,
        editing: Option<TxnId>,
    ) -> Vec<(&Transaction, Money)> {
        self.open_docs(
            |b| matches!(b, TxnBody::CreditMemo(m) if m.customer == customer),
            editing,
        )
    }

    /// Open bills as the bill payment being edited (`editing`) sees them.
    pub fn open_bills_for(
        &self,
        vendor: VendorId,
        editing: Option<TxnId>,
    ) -> Vec<(&Transaction, Money)> {
        self.open_docs(
            |b| matches!(b, TxnBody::Bill(bill) if bill.vendor == vendor),
            editing,
        )
    }

    fn open_docs(
        &self,
        pick: impl Fn(&TxnBody) -> bool,
        ignore: Option<TxnId>,
    ) -> Vec<(&Transaction, Money)> {
        let mut v: Vec<(&Transaction, Money)> = self
            .transactions
            .iter()
            .filter(|t| !t.voided && pick(&t.body))
            .map(|t| (t, self.open_balance_excluding(t.id, None, ignore)))
            .filter(|(_, open)| open.is_positive())
            .collect();
        v.sort_by_key(|(t, _)| (t.date, t.id));
        v
    }

    /// What a customer owes in total (open invoices less unapplied payments).
    pub fn customer_balance(&self, id: CustomerId) -> Money {
        self.party_balance(Party::Customer(id), AccountType::AccountsReceivable)
    }

    /// What you owe a vendor in total.
    pub fn vendor_balance(&self, id: VendorId) -> Money {
        self.party_balance(Party::Vendor(id), AccountType::AccountsPayable)
            .saturating_neg()
    }

    fn party_balance(&self, party: Party, kind: AccountType) -> Money {
        self.ledger_entries()
            .into_iter()
            .filter(|e| {
                e.posting.party == Some(party)
                    && self
                        .account(e.posting.account)
                        .is_some_and(|a| a.kind == kind)
            })
            .fold(Money::ZERO, |acc, e| acc.saturating_add(e.posting.amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bill, Invoice, JournalEntry, JournalLine, PayBill, ReceivePayment, SalesLine};
    use ledgercraft_core::Qty;

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn acct(c: &Company, name: &str) -> AccountId {
        c.account_by_name(name).unwrap().id
    }

    fn setup() -> (Company, CustomerId, VendorId) {
        let mut c = Company::with_default_accounts("Test Co");
        let cust = c
            .add_customer(Contact {
                name: "Acme".into(),
                terms_days: 30,
                ..Contact::default()
            })
            .unwrap();
        let vend = c
            .add_vendor(Contact {
                name: "Paper Mill".into(),
                ..Contact::default()
            })
            .unwrap();
        (c, cust, vend)
    }

    fn invoice(c: &Company, cust: CustomerId, cents: i64) -> TxnBody {
        TxnBody::Invoice(Invoice {
            customer: cust,
            due_date: d("2026-02-01"),
            ar_account: acct(c, "Accounts Receivable"),
            ship_to: None,
            tax: None,
            lines: vec![SalesLine {
                item: None,
                description: "Work".into(),
                qty: Qty::from_whole(2),
                rate: Money::from_cents(cents),
                taxable: true,
                account: acct(c, "Service Income"),
            }],
        })
    }

    #[test]
    fn items_can_be_changed_and_made_inactive() {
        let mut c = Company::with_default_accounts("Item Co");
        let sales = acct(&c, "Sales");
        let spec = |name: &str| ItemSpec {
            name: name.into(),
            description: String::new(),
            kind: crate::ItemKind::Service,
            rate: Money::from_dollars(50),
            income_account: sales,
            taxable: true,
        };
        let a = c.add_item(spec("Repair")).unwrap();
        c.add_item(spec("Install")).unwrap();
        let mut changed = spec("Repair, hourly");
        changed.rate = Money::from_dollars(65);
        changed.taxable = false;
        c.update_item(a, changed).unwrap();
        let item = c.item(a).unwrap();
        assert_eq!(item.spec.name, "Repair, hourly");
        assert_eq!(item.spec.rate, Money::from_dollars(65));
        assert!(!item.spec.taxable);
        // Names stay unique; the item keeps its own name.
        assert!(c.update_item(a, spec("install")).is_err());
        assert!(c.update_item(a, spec("Repair, hourly")).is_ok());
        let ar = acct(&c, "Accounts Receivable");
        let mut bad = spec("Repair");
        bad.income_account = ar;
        assert!(c.update_item(a, bad).is_err());
        c.set_item_active(a, false).unwrap();
        assert!(!c.item(a).unwrap().active);
    }

    #[test]
    fn customers_and_vendors_can_be_made_inactive() {
        let (mut c, cust, vend) = setup();
        c.set_customer_active(cust, false).unwrap();
        c.set_vendor_active(vend, false).unwrap();
        assert!(!c.customer(cust).unwrap().active);
        assert!(c.vendors.iter().all(|v| !v.active));
        c.set_customer_active(cust, true).unwrap();
        assert!(c.customer(cust).unwrap().active);
        assert!(c.set_customer_active(CustomerId(999_999), false).is_err());
    }

    #[test]
    fn invoice_and_payment_flow() {
        let (mut c, cust, _) = setup();
        let inv = c
            .post(d("2026-01-02"), "1001", "", invoice(&c, cust, 50_000))
            .unwrap();
        assert_eq!(
            c.account_balance(acct(&c, "Accounts Receivable"), None),
            Money::from_cents(100_000)
        );
        assert_eq!(
            c.account_balance(acct(&c, "Service Income"), None),
            Money::from_cents(100_000)
        );
        assert_eq!(c.open_balance(inv, None), Money::from_cents(100_000));

        let pay = TxnBody::ReceivePayment(ReceivePayment {
            customer: cust,
            deposit_to: acct(&c, "Checking"),
            ar_account: acct(&c, "Accounts Receivable"),
            amount: Money::from_cents(40_000),
            method: "Check".into(),
            applied: vec![Applied {
                txn: inv,
                amount: Money::from_cents(40_000),
            }],
        });
        let pay_id = c.post(d("2026-01-10"), "", "", pay).unwrap();
        assert_eq!(c.open_balance(inv, None), Money::from_cents(60_000));
        assert_eq!(c.customer_balance(cust), Money::from_cents(60_000));
        assert_eq!(
            c.account_balance(acct(&c, "Checking"), None),
            Money::from_cents(40_000)
        );

        assert!(matches!(c.void(inv), Err(LedgerError::InUse(_))));
        c.void(pay_id).unwrap();
        assert_eq!(c.open_balance(inv, None), Money::from_cents(100_000));
        c.void(inv).unwrap();
        assert_eq!(c.customer_balance(cust), Money::ZERO);
    }

    fn payment(c: &Company, cust: CustomerId, inv: TxnId, cents: i64) -> TxnBody {
        TxnBody::ReceivePayment(ReceivePayment {
            customer: cust,
            deposit_to: acct(c, "Checking"),
            ar_account: acct(c, "Accounts Receivable"),
            amount: Money::from_cents(cents),
            method: String::new(),
            applied: vec![Applied {
                txn: inv,
                amount: Money::from_cents(cents),
            }],
        })
    }

    #[test]
    fn editing_an_invoice_keeps_its_id_and_reposts() {
        let (mut c, cust, _) = setup();
        let inv = c
            .post(d("2026-01-02"), "1001", "", invoice(&c, cust, 50_000))
            .unwrap();
        c.update(
            inv,
            d("2026-01-05"),
            "1001A",
            "fixed",
            invoice(&c, cust, 30_000),
        )
        .unwrap();
        let t = c.txn(inv).unwrap();
        assert_eq!(
            (t.date, t.number.as_str(), t.memo.as_str()),
            (d("2026-01-05"), "1001A", "fixed")
        );
        assert_eq!(c.transactions.len(), 1);
        assert_eq!(
            c.account_balance(acct(&c, "Accounts Receivable"), None),
            Money::from_cents(60_000)
        );
    }

    #[test]
    fn edits_are_checked_like_new_transactions() {
        let (mut c, cust, vend) = setup();
        let inv = c
            .post(d("2026-01-02"), "", "", invoice(&c, cust, 100))
            .unwrap();
        // Same type only.
        let bill = TxnBody::Bill(Bill {
            vendor: vend,
            due_date: d("2026-02-01"),
            ap_account: acct(&c, "Accounts Payable"),
            lines: vec![AccountLine {
                account: acct(&c, "Office Supplies"),
                description: String::new(),
                amount: Money::from_cents(100),
            }],
        });
        assert!(c.update(inv, d("2026-01-02"), "", "", bill).is_err());
        // Invalid content is refused and changes nothing.
        let empty = TxnBody::Invoice(Invoice {
            customer: cust,
            due_date: d("2026-02-01"),
            ar_account: acct(&c, "Accounts Receivable"),
            ship_to: None,
            tax: None,
            lines: Vec::new(),
        });
        assert!(c.update(inv, d("2026-01-02"), "", "", empty).is_err());
        assert_eq!(c.open_balance(inv, None), Money::from_cents(200));
        // Void transactions stay as they are.
        c.void(inv).unwrap();
        assert!(c
            .update(inv, d("2026-01-02"), "", "", invoice(&c, cust, 100))
            .is_err());
        assert!(c
            .update(TxnId(9999), d("2026-01-02"), "", "", invoice(&c, cust, 100))
            .is_err());
    }

    #[test]
    fn a_paid_invoice_cannot_drop_below_its_payments_or_change_customer() {
        let (mut c, cust, _) = setup();
        let other = c
            .add_customer(Contact {
                name: "Other".into(),
                ..Contact::default()
            })
            .unwrap();
        let inv = c
            .post(d("2026-01-02"), "", "", invoice(&c, cust, 50_000))
            .unwrap();
        c.post(d("2026-01-10"), "", "", payment(&c, cust, inv, 60_000))
            .unwrap();
        // Total 2 x 250.00 = 500.00 is below the 600.00 paid.
        assert!(c
            .update(inv, d("2026-01-02"), "", "", invoice(&c, cust, 25_000))
            .is_err());
        assert!(matches!(
            c.update(inv, d("2026-01-02"), "", "", invoice(&c, other, 50_000)),
            Err(LedgerError::InUse(_))
        ));
        // Raising it is fine, and the payment stays attached.
        c.update(inv, d("2026-01-02"), "", "", invoice(&c, cust, 40_000))
            .unwrap();
        assert_eq!(c.open_balance(inv, None), Money::from_cents(20_000));
    }

    #[test]
    fn an_edited_payment_can_reapply_its_own_amounts() {
        let (mut c, cust, _) = setup();
        let inv = c
            .post(d("2026-01-02"), "", "", invoice(&c, cust, 50_000))
            .unwrap();
        // Pays the invoice in full.
        let pay = c
            .post(d("2026-01-10"), "", "", payment(&c, cust, inv, 100_000))
            .unwrap();
        assert!(c.open_invoices(cust).is_empty());
        // As the payment sees it, the invoice is fully open again.
        let open = c.open_invoices_for(cust, Some(pay));
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].1, Money::from_cents(100_000));
        // Changing only the date re-applies the same amount without tripping
        // the overpayment check.
        c.update(
            pay,
            d("2026-01-11"),
            "",
            "",
            payment(&c, cust, inv, 100_000),
        )
        .unwrap();
        // Lowering it leaves the rest open.
        c.update(pay, d("2026-01-11"), "", "", payment(&c, cust, inv, 70_000))
            .unwrap();
        assert_eq!(c.open_balance(inv, None), Money::from_cents(30_000));
        // But it still cannot overpay.
        assert!(c
            .update(
                pay,
                d("2026-01-11"),
                "",
                "",
                payment(&c, cust, inv, 100_001)
            )
            .is_err());
    }

    #[test]
    fn overpaying_an_invoice_is_refused() {
        let (mut c, cust, _) = setup();
        let inv = c
            .post(d("2026-01-02"), "", "", invoice(&c, cust, 100))
            .unwrap();
        let pay = TxnBody::ReceivePayment(ReceivePayment {
            customer: cust,
            deposit_to: acct(&c, "Checking"),
            ar_account: acct(&c, "Accounts Receivable"),
            amount: Money::from_cents(500),
            method: String::new(),
            applied: vec![Applied {
                txn: inv,
                amount: Money::from_cents(500),
            }],
        });
        assert!(matches!(
            c.post(d("2026-01-03"), "", "", pay),
            Err(LedgerError::OverApplied { .. })
        ));
    }

    #[test]
    fn bills_and_bill_payments() {
        let (mut c, _, vend) = setup();
        let bill = c
            .post(
                d("2026-01-05"),
                "B-7",
                "",
                TxnBody::Bill(Bill {
                    vendor: vend,
                    due_date: d("2026-02-04"),
                    ap_account: acct(&c, "Accounts Payable"),
                    lines: vec![AccountLine {
                        account: acct(&c, "Office Supplies"),
                        description: "Paper".into(),
                        amount: Money::from_cents(12_345),
                    }],
                }),
            )
            .unwrap();
        assert_eq!(c.vendor_balance(vend), Money::from_cents(12_345));
        c.post(
            d("2026-01-20"),
            "2001",
            "",
            TxnBody::PayBill(PayBill {
                vendor: vend,
                pay_from: acct(&c, "Checking"),
                ap_account: acct(&c, "Accounts Payable"),
                amount: Money::from_cents(12_345),
                applied: vec![Applied {
                    txn: bill,
                    amount: Money::from_cents(12_345),
                }],
            }),
        )
        .unwrap();
        assert_eq!(c.vendor_balance(vend), Money::ZERO);
        assert_eq!(
            c.account_balance(acct(&c, "Checking"), None),
            Money::from_cents(-12_345)
        );
        assert!(c.open_bills(vend).is_empty());
    }

    #[test]
    fn journal_must_balance_and_name_ar_lines() {
        let (mut c, cust, _) = setup();
        let line = |a, dr: i64, cr: i64, party| JournalLine {
            account: a,
            debit: Money::from_cents(dr),
            credit: Money::from_cents(cr),
            memo: String::new(),
            party,
        };
        let checking = acct(&c, "Checking");
        let equity = acct(&c, "Owner's Equity");
        let ar = acct(&c, "Accounts Receivable");
        let bad = TxnBody::Journal(JournalEntry {
            lines: vec![line(checking, 100, 0, None), line(equity, 0, 99, None)],
        });
        assert!(matches!(
            c.post(d("2026-01-01"), "", "", bad),
            Err(LedgerError::Unbalanced { .. })
        ));
        let no_name = TxnBody::Journal(JournalEntry {
            lines: vec![line(ar, 100, 0, None), line(equity, 0, 100, None)],
        });
        assert!(c.post(d("2026-01-01"), "", "", no_name).is_err());
        let ok = TxnBody::Journal(JournalEntry {
            lines: vec![
                line(ar, 100, 0, Some(Party::Customer(cust))),
                line(equity, 0, 100, None),
            ],
        });
        assert!(c.post(d("2026-01-01"), "", "", ok).is_ok());
    }

    #[test]
    fn accounts_are_unique_and_balances_block_deactivation() {
        let (mut c, cust, _) = setup();
        assert!(c
            .add_account("1000", "Another", AccountType::Bank, "")
            .is_err());
        assert!(c
            .add_account("", "checking", AccountType::Bank, "")
            .is_err());
        c.post(d("2026-01-02"), "", "", invoice(&c, cust, 100))
            .unwrap();
        assert!(c
            .set_account_active(acct(&c, "Service Income"), false)
            .is_err());
        assert!(c.set_account_active(acct(&c, "Rent"), false).is_ok());
    }

    fn lines(c: &Company, cents: i64) -> Vec<SalesLine> {
        vec![SalesLine {
            item: None,
            description: "Returned work".into(),
            qty: Qty::ONE,
            rate: Money::from_cents(cents),
            taxable: true,
            account: acct(c, "Service Income"),
        }]
    }

    fn credit(c: &Company, cust: CustomerId, cents: i64, applied: Vec<Applied>) -> TxnBody {
        TxnBody::CreditMemo(crate::CreditMemo {
            customer: cust,
            ar_account: acct(c, "Accounts Receivable"),
            lines: lines(c, cents),
            tax: None,
            applied,
        })
    }

    fn refund(c: &Company, cust: CustomerId, memo: TxnId, cents: i64) -> TxnBody {
        TxnBody::Refund(crate::Refund {
            customer: cust,
            pay_from: acct(c, "Checking"),
            ar_account: acct(c, "Accounts Receivable"),
            amount: Money::from_cents(cents),
            method: "Check 1050".into(),
            applied: vec![Applied {
                txn: memo,
                amount: Money::from_cents(cents),
            }],
        })
    }

    #[test]
    fn a_sales_receipt_is_paid_on_the_spot() {
        let (mut c, cust, _) = setup();
        let receipt = TxnBody::SalesReceipt(crate::SalesReceipt {
            customer: Some(cust),
            deposit_to: acct(&c, "Undeposited Funds"),
            method: "Cash".into(),
            lines: lines(&c, 12_500),
            tax: None,
        });
        c.post(d("2026-01-05"), "1", "", receipt).unwrap();
        assert_eq!(
            c.account_balance(acct(&c, "Undeposited Funds"), None),
            Money::from_cents(12_500)
        );
        assert_eq!(
            c.account_balance(acct(&c, "Service Income"), None),
            Money::from_cents(12_500)
        );
        // Nothing is owed.
        assert_eq!(c.customer_balance(cust), Money::ZERO);
        // A walk-in sale needs no customer, but must go to a bank or
        // Undeposited Funds.
        let walk_in = TxnBody::SalesReceipt(crate::SalesReceipt {
            customer: None,
            deposit_to: acct(&c, "Rent"),
            method: String::new(),
            lines: lines(&c, 500),
            tax: None,
        });
        assert!(c.validate(&walk_in).is_err());
    }

    #[test]
    fn credit_memos_pay_invoices_and_the_rest_is_refunded() {
        let (mut c, cust, _) = setup();
        // $1,000 invoiced, $300 credited: $200 against the invoice, $100 left.
        let inv = c
            .post(d("2026-01-02"), "1001", "", invoice(&c, cust, 50_000))
            .unwrap();
        let applied = vec![Applied {
            txn: inv,
            amount: Money::from_cents(20_000),
        }];
        let memo = c
            .post(
                d("2026-01-10"),
                "CM-1",
                "",
                credit(&c, cust, 30_000, applied),
            )
            .unwrap();
        assert_eq!(c.customer_balance(cust), Money::from_cents(70_000));
        assert_eq!(c.open_balance(inv, None), Money::from_cents(80_000));
        assert_eq!(c.open_balance(memo, None), Money::from_cents(10_000));
        assert_eq!(
            c.account_balance(acct(&c, "Service Income"), None),
            Money::from_cents(70_000)
        );
        assert_eq!(c.open_credits_for(cust, None).len(), 1);

        // More than is left cannot be refunded, or applied.
        assert!(c.validate(&refund(&c, cust, memo, 10_001)).is_err());
        let too_much = vec![Applied {
            txn: inv,
            amount: Money::from_cents(40_000),
        }];
        assert!(c.validate(&credit(&c, cust, 30_000, too_much)).is_err());

        let r = c
            .post(d("2026-01-12"), "1050", "", refund(&c, cust, memo, 10_000))
            .unwrap();
        assert_eq!(c.open_balance(memo, None), Money::ZERO);
        assert!(c.open_credits_for(cust, None).is_empty());
        // The refund took the credit off their account: they owe the invoice.
        assert_eq!(c.customer_balance(cust), Money::from_cents(80_000));
        assert_eq!(
            c.account_balance(acct(&c, "Checking"), None),
            Money::from_cents(-10_000)
        );

        // A refunded credit cannot be voided, or shrink below its refund.
        assert!(c.void(memo).is_err());
        let smaller = credit(
            &c,
            cust,
            25_000,
            vec![Applied {
                txn: inv,
                amount: Money::from_cents(20_000),
            }],
        );
        assert!(c
            .update(memo, d("2026-01-10"), "CM-1", "", smaller)
            .is_err());
        // Undo the refund, then voiding the credit reopens the invoice in full.
        c.void(r).unwrap();
        c.void(memo).unwrap();
        assert_eq!(c.open_balance(inv, None), Money::from_cents(100_000));
        assert_eq!(c.customer_balance(cust), Money::from_cents(100_000));
    }

    #[test]
    fn a_refund_must_pay_back_credit_memos() {
        let (mut c, cust, _) = setup();
        let memo = c
            .post(
                d("2026-01-10"),
                "CM-1",
                "",
                credit(&c, cust, 30_000, Vec::new()),
            )
            .unwrap();
        let mut unapplied = refund(&c, cust, memo, 5_000);
        if let TxnBody::Refund(r) = &mut unapplied {
            r.applied.clear();
        }
        assert!(c.validate(&unapplied).is_err());
        assert!(c.validate(&refund(&c, cust, memo, 5_000)).is_ok());
    }

    #[test]
    fn json_round_trip() {
        let (mut c, cust, _) = setup();
        c.post(d("2026-01-02"), "1", "", invoice(&c, cust, 100))
            .unwrap();
        let json = serde_json::to_string(&c).unwrap();
        let mut back: Company = serde_json::from_str(&json).unwrap();
        back.repair_next_id();
        assert_eq!(back, c);
    }
}
