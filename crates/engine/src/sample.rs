//! A fictional sample company, written for LedgerCraft, used by
//! `--sample`, tests and screenshots.
//!
//! It is meant to show everything LedgerCraft does: January to March is a
//! simple first quarter (the bank download in docs/samples matches it), and
//! April to October adds sales tax, an exempt customer, overdue invoices in
//! every aging column, partial payments, deposits, a financed machine and its
//! loan payments, depreciation, owner's draws, savings with interest, a sales
//! tax payment, reconciled card and savings accounts, bank rules, a voided
//! check and invoice, and inactive customers, vendors and items. Checking is
//! left unreconciled for the Reconcile lesson.
//!
//! The tax rates are illustrative, not anyone's real rates.

use ledgercraft_core::{Date, Money, Qty};
use ledgercraft_ledger::{
    tax_payment_memo, AccountId, AccountLine, AccountType, Address, Applied, BankRule, Bill, Check,
    Company, Contact, CreditMemo, Logo, CustomerId, CustomerTax, Deposit, Invoice, InvoiceTax, ItemId,
    ItemKind, ItemSpec, JournalEntry, JournalLine, LedgerError, LedgerResult, Party, PayBill,
    ReceivePayment, RecurringMode, RecurringSpec, Refund, SalesLine, SalesReceipt, TaxGroupId,
    TaxRate, TxnBody, TxnClassId, TxnId, TxnTemplate, Frequency,
    VendorId,
};

fn d(y: i32, m: u32, day: u32) -> LedgerResult<Date> {
    Ok(Date::from_ymd(y, m, day)?)
}

fn acct(c: &Company, name: &str) -> LedgerResult<AccountId> {
    c.account_by_name(name)
        .map(|a| a.id)
        .ok_or_else(|| LedgerError::NotFound(format!("Account {name:?}")))
}

fn contact(name: &str, email: &str, city: &str, terms_days: u32) -> Contact {
    Contact {
        name: name.into(),
        email: email.into(),
        terms_days,
        address: Address {
            city: city.into(),
            region: "UT".into(),
            ..Address::default()
        },
        ..Contact::default()
    }
}

fn dollars(n: i64) -> Money {
    Money::from_dollars(n)
}

fn cents(n: i64) -> Money {
    Money::from_cents(n)
}

fn rate(percent: &str) -> LedgerResult<TaxRate> {
    TaxRate::parse(percent).map_err(LedgerError::Invalid)
}

/// A customer or vendor with every detail filled in.
struct Who<'a> {
    name: &'a str,
    email: &'a str,
    phone: &'a str,
    street: &'a str,
    city: &'a str,
    zip: &'a str,
    terms: u32,
    notes: &'a str,
}

impl Who<'_> {
    fn contact(&self) -> Contact {
        Contact {
            name: self.name.into(),
            company: String::new(),
            email: self.email.into(),
            phone: self.phone.into(),
            address: Address {
                line1: self.street.into(),
                city: self.city.into(),
                region: "UT".into(),
                postal_code: self.zip.into(),
                ..Address::default()
            },
            terms_days: self.terms,
            notes: self.notes.into(),
            ship_to: Address::default(),
        }
    }
}

/// An invoice line for an item at its own income account and taxability.
fn sale(c: &Company, item: ItemId, desc: &str, qty: i64, rate: Money) -> LedgerResult<SalesLine> {
    let spec = &c
        .item(item)
        .ok_or_else(|| LedgerError::NotFound(format!("Item {item}")))?
        .spec;
    Ok(SalesLine {
        item: Some(item),
        description: desc.into(),
        qty: Qty::from_whole(qty),
        rate,
        taxable: spec.taxable,
        account: spec.income_account,
    })
}

/// Records an invoice, charging `group`'s tax on its taxable lines the way
/// the invoice form does.
#[allow(clippy::too_many_arguments)]
fn invoice(
    c: &mut Company,
    date: Date,
    number: &str,
    memo: &str,
    customer: CustomerId,
    terms: i64,
    group: Option<TaxGroupId>,
    lines: &[(ItemId, &str, i64, Money)],
) -> LedgerResult<TxnId> {
    let (lines, tax) = sale_lines(c, date, group, lines)?;
    let ar_account = acct(c, "Accounts Receivable")?;
    // Shipped where the customer ships to, if not their own address.
    let ship_to = c
        .customer(customer)
        .map(|x| x.contact.ship_to.clone())
        .filter(|a| !a.is_empty());
    c.post(
        date,
        number,
        memo,
        TxnBody::Invoice(Invoice {
            customer,
            due_date: date.add_days(terms),
            ar_account,
            ship_to,
            tax,
            lines,
        }),
    )
}

/// The lines of a sale, and `group`'s tax on the taxable ones (none when
/// nothing is taxable), the way the forms work it out.
fn sale_lines(
    c: &Company,
    date: Date,
    group: Option<TaxGroupId>,
    lines: &[(ItemId, &str, i64, Money)],
) -> LedgerResult<(Vec<SalesLine>, Option<InvoiceTax>)> {
    let lines = lines
        .iter()
        .map(|(item, desc, qty, rate)| sale(c, *item, desc, *qty, *rate))
        .collect::<LedgerResult<Vec<_>>>()?;
    let tax = match group {
        Some(g) => {
            let taxable = lines
                .iter()
                .filter(|l| l.taxable)
                .try_fold(Money::ZERO, |acc, l| acc.checked_add(l.amount()?))
                .ok_or(LedgerError::Overflow)?;
            if taxable.is_zero() {
                None
            } else {
                Some(c.invoice_tax(g, date, taxable)?)
            }
        }
        None => None,
    };
    Ok((lines, tax))
}

/// A sale paid on the spot; `customer` is `None` for a walk-in.
#[allow(clippy::too_many_arguments)]
fn sales_receipt(
    c: &mut Company,
    date: Date,
    number: &str,
    customer: Option<CustomerId>,
    deposit_to: AccountId,
    method: &str,
    group: Option<TaxGroupId>,
    lines: &[(ItemId, &str, i64, Money)],
) -> LedgerResult<TxnId> {
    let (lines, tax) = sale_lines(c, date, group, lines)?;
    c.post(
        date,
        number,
        "",
        TxnBody::SalesReceipt(SalesReceipt {
            customer,
            deposit_to,
            method: method.into(),
            lines,
            tax,
        }),
    )
}

/// A credit memo, all of it applied to `apply_to` if given.
#[allow(clippy::too_many_arguments)]
fn credit_memo(
    c: &mut Company,
    date: Date,
    number: &str,
    memo: &str,
    customer: CustomerId,
    group: Option<TaxGroupId>,
    lines: &[(ItemId, &str, i64, Money)],
    apply_to: Option<TxnId>,
) -> LedgerResult<TxnId> {
    let (lines, tax) = sale_lines(c, date, group, lines)?;
    let ar_account = acct(c, "Accounts Receivable")?;
    let mut body = CreditMemo {
        customer,
        ar_account,
        lines,
        tax,
        applied: Vec::new(),
    };
    if let Some(txn) = apply_to {
        let amount = body.total().ok_or(LedgerError::Overflow)?;
        body.applied.push(Applied { txn, amount });
    }
    c.post(date, number, memo, TxnBody::CreditMemo(body))
}

/// Pays back credit memos (`None`: all the credit left on one).
fn refund(
    c: &mut Company,
    date: Date,
    customer: CustomerId,
    pay_from: AccountId,
    method: &str,
    list: &[(TxnId, Option<Money>)],
) -> LedgerResult<TxnId> {
    let ar_account = acct(c, "Accounts Receivable")?;
    let (amount, applied) = applied(c, list);
    c.post(
        date,
        "",
        "",
        TxnBody::Refund(Refund {
            customer,
            pay_from,
            ar_account,
            amount,
            method: method.into(),
            applied,
        }),
    )
}

/// What a sale or payment comes to, tax included.
fn total(c: &Company, id: TxnId) -> LedgerResult<Money> {
    c.txn(id)
        .and_then(|t| t.body.total())
        .ok_or_else(|| LedgerError::NotFound(format!("Transaction {id}")))
}

/// Payments applied to invoices or bills: an amount, or `None` for all that
/// is still open.
fn applied(c: &Company, list: &[(TxnId, Option<Money>)]) -> (Money, Vec<Applied>) {
    let applied: Vec<Applied> = list
        .iter()
        .map(|(txn, amount)| Applied {
            txn: *txn,
            amount: amount.unwrap_or_else(|| c.open_balance(*txn, None)),
        })
        .collect();
    let amount = applied
        .iter()
        .fold(Money::ZERO, |acc, a| acc.saturating_add(a.amount));
    (amount, applied)
}

/// A customer payment, all of it applied to their invoices.
fn receive(
    c: &mut Company,
    date: Date,
    customer: CustomerId,
    deposit_to: AccountId,
    method: &str,
    list: &[(TxnId, Option<Money>)],
) -> LedgerResult<TxnId> {
    let ar_account = acct(c, "Accounts Receivable")?;
    let (amount, applied) = applied(c, list);
    c.post(
        date,
        "",
        "",
        TxnBody::ReceivePayment(ReceivePayment {
            customer,
            deposit_to,
            ar_account,
            amount,
            method: method.into(),
            applied,
        }),
    )
}

fn bill(
    c: &mut Company,
    date: Date,
    number: &str,
    vendor: VendorId,
    terms: i64,
    lines: Vec<AccountLine>,
) -> LedgerResult<TxnId> {
    let ap_account = acct(c, "Accounts Payable")?;
    c.post(
        date,
        number,
        "",
        TxnBody::Bill(Bill {
            vendor,
            due_date: date.add_days(terms),
            ap_account,
            lines,
        }),
    )
}

fn pay_bills(
    c: &mut Company,
    date: Date,
    number: &str,
    vendor: VendorId,
    list: &[(TxnId, Option<Money>)],
) -> LedgerResult<TxnId> {
    let ap_account = acct(c, "Accounts Payable")?;
    let pay_from = acct(c, "Checking")?;
    let (amount, applied) = applied(c, list);
    c.post(
        date,
        number,
        "",
        TxnBody::PayBill(PayBill {
            vendor,
            pay_from,
            ap_account,
            amount,
            applied,
        }),
    )
}

fn check(
    c: &mut Company,
    date: Date,
    number: &str,
    memo: &str,
    payee: Option<Party>,
    pay_from: AccountId,
    lines: Vec<AccountLine>,
) -> LedgerResult<TxnId> {
    c.post(
        date,
        number,
        memo,
        TxnBody::Check(Check {
            payee,
            pay_from,
            lines,
        }),
    )
}

fn deposit(
    c: &mut Company,
    date: Date,
    memo: &str,
    deposit_to: AccountId,
    lines: Vec<AccountLine>,
) -> LedgerResult<TxnId> {
    c.post(
        date,
        "",
        memo,
        TxnBody::Deposit(Deposit { deposit_to, lines }),
    )
}

fn journal(
    c: &mut Company,
    date: Date,
    number: &str,
    memo: &str,
    lines: &[(AccountId, Money, Money, &str)],
) -> LedgerResult<TxnId> {
    let lines = lines
        .iter()
        .map(|(account, debit, credit, memo)| JournalLine {
            account: *account,
            debit: *debit,
            credit: *credit,
            memo: (*memo).into(),
            party: None,
        })
        .collect();
    c.post(date, number, memo, TxnBody::Journal(JournalEntry { lines }))
}

/// Reconciles an account through `through`, clearing everything up to then.
fn reconcile_all(c: &mut Company, account: AccountId, through: Date) -> LedgerResult<()> {
    let items = c.unreconciled(account, through);
    let ending = items
        .iter()
        .fold(c.reconciled_balance(account), |total, i| {
            total.saturating_add(i.amount)
        });
    c.reconcile(
        account,
        through,
        ending,
        items.iter().map(|i| i.txn).collect(),
    )
}

/// Pine Ridge Woodworks: a small custom-furniture shop's first quarter of 2026.
pub fn sample_company() -> LedgerResult<Company> {
    let mut c = Company::with_default_accounts("Pine Ridge Woodworks");
    c.info.legal_name = "Pine Ridge Woodworks LLC".into();
    c.info.email = "hello@pineridge.example".into();
    c.info.phone = "(801) 555-0142".into();
    // Printed at the top of its invoices and statements (drawn for the sample).
    c.info.print.logo = Some(Logo(
        include_bytes!("../../../docs/samples/pine-ridge-logo.png").to_vec(),
    ));
    c.info.address = Address {
        line1: "100 Example Way".into(),
        city: "Springville".into(),
        region: "UT".into(),
        postal_code: "84000".into(),
        country: "USA".into(),
        ..Address::default()
    };

    let checking = acct(&c, "Checking")?;
    let undeposited = acct(&c, "Undeposited Funds")?;
    let ar = acct(&c, "Accounts Receivable")?;
    let ap = acct(&c, "Accounts Payable")?;
    let card = acct(&c, "Business Credit Card")?;
    let equity = acct(&c, "Owner's Equity")?;
    let equipment = acct(&c, "Equipment")?;
    let sales = acct(&c, "Sales")?;
    let service = acct(&c, "Service Income")?;
    let cogs = acct(&c, "Cost of Goods Sold")?;
    let rent = acct(&c, "Rent")?;
    let utilities = acct(&c, "Utilities")?;
    let supplies = acct(&c, "Office Supplies")?;
    let software = acct(&c, "Software and Subscriptions")?;
    let advertising = acct(&c, "Advertising and Marketing")?;

    let maple = c.add_customer(contact(
        "Maple Street Cafe",
        "orders@maple.example",
        "Provo",
        30,
    ))?;
    let harbor = c.add_customer(contact(
        "Harbor Design Studio",
        "ap@harbor.example",
        "Orem",
        15,
    ))?;
    let lin = c.add_customer(contact("Dana Lin", "dana@lin.example", "Lehi", 0))?;
    let lumber = c.add_vendor(contact(
        "Canyon Lumber Supply",
        "billing@canyon.example",
        "Spanish Fork",
        30,
    ))?;
    let power = c.add_vendor(contact("Valley Power & Light", "", "Provo", 20))?;
    let landlord = c.add_vendor(contact("Riverside Properties", "", "Springville", 0))?;

    let table = c.add_item(ItemSpec {
        name: "Dining Table".into(),
        description: "Solid hardwood dining table, made to order".into(),
        kind: ItemKind::NonInventory,
        rate: dollars(2400),
        income_account: sales,
        taxable: true,
    })?;
    let chair = c.add_item(ItemSpec {
        name: "Chair".into(),
        description: "Matching hardwood chair".into(),
        kind: ItemKind::NonInventory,
        rate: dollars(325),
        income_account: sales,
        taxable: true,
    })?;
    let design = c.add_item(ItemSpec {
        name: "Design Hours".into(),
        description: "Custom design and drafting".into(),
        kind: ItemKind::Service,
        rate: dollars(85),
        income_account: service,
        // Design work is a service: not taxed.
        taxable: false,
    })?;

    let line = |item, desc: &str, qty: i64, rate: Money, account| SalesLine {
        item: Some(item),
        description: desc.into(),
        qty: Qty::from_whole(qty),
        rate,
        taxable: true,
        account,
    };
    let exp = |account, description: &str, amount: Money| AccountLine {
        account,
        description: description.into(),
        amount,
    };

    // Owner invests cash and an existing table saw.
    c.post(
        d(2026, 1, 2)?,
        "",
        "Opening investment",
        TxnBody::Journal(JournalEntry {
            lines: vec![
                JournalLine {
                    account: checking,
                    debit: dollars(25_000),
                    credit: Money::ZERO,
                    memo: String::new(),
                    party: None,
                },
                JournalLine {
                    account: equipment,
                    debit: dollars(6_500),
                    credit: Money::ZERO,
                    memo: "Table saw".into(),
                    party: None,
                },
                JournalLine {
                    account: equity,
                    debit: Money::ZERO,
                    credit: dollars(31_500),
                    memo: String::new(),
                    party: None,
                },
            ],
        }),
    )?;

    c.post(
        d(2026, 1, 3)?,
        "1001",
        "January rent",
        TxnBody::Check(Check {
            payee: Some(Party::Vendor(landlord)),
            pay_from: checking,
            lines: vec![exp(rent, "Shop rent", dollars(1_800))],
        }),
    )?;

    let lumber_bill = c.post(
        d(2026, 1, 6)?,
        "CL-5521",
        "",
        TxnBody::Bill(Bill {
            vendor: lumber,
            due_date: d(2026, 2, 5)?,
            ap_account: ap,
            lines: vec![exp(cogs, "Walnut and white oak", dollars(3_150))],
        }),
    )?;

    let inv1 = c.post(
        d(2026, 1, 12)?,
        "1001",
        "",
        TxnBody::Invoice(Invoice {
            customer: maple,
            due_date: d(2026, 2, 11)?,
            ar_account: ar,
            ship_to: None,
            tax: None,
            lines: vec![
                line(table, "Communal table, 10 ft", 2, dollars(2_400), sales),
                line(chair, "Bench-height chairs", 12, dollars(325), sales),
            ],
        }),
    )?;

    let inv2 = c.post(
        d(2026, 1, 20)?,
        "1002",
        "",
        TxnBody::Invoice(Invoice {
            customer: harbor,
            due_date: d(2026, 2, 4)?,
            ar_account: ar,
            ship_to: None,
            tax: None,
            lines: vec![line(
                design,
                "Showroom fixture design",
                18,
                dollars(85),
                service,
            )],
        }),
    )?;

    c.post(
        d(2026, 1, 22)?,
        "",
        "",
        TxnBody::Check(Check {
            payee: None,
            pay_from: card,
            lines: vec![
                exp(
                    software,
                    "Design software subscription",
                    Money::from_cents(5_999),
                ),
                exp(
                    supplies,
                    "Sandpaper, glue, finish",
                    Money::from_cents(21_437),
                ),
            ],
        }),
    )?;

    c.post(
        d(2026, 2, 2)?,
        "",
        "",
        TxnBody::ReceivePayment(ReceivePayment {
            customer: harbor,
            deposit_to: undeposited,
            ar_account: ar,
            amount: dollars(1_530),
            method: "Check #4410".into(),
            applied: vec![Applied {
                txn: inv2,
                amount: dollars(1_530),
            }],
        }),
    )?;

    c.post(
        d(2026, 2, 3)?,
        "1002",
        "February rent",
        TxnBody::Check(Check {
            payee: Some(Party::Vendor(landlord)),
            pay_from: checking,
            lines: vec![exp(rent, "Shop rent", dollars(1_800))],
        }),
    )?;

    c.post(
        d(2026, 2, 4)?,
        "",
        "",
        TxnBody::Deposit(Deposit {
            deposit_to: checking,
            lines: vec![exp(
                undeposited,
                "Harbor Design Studio check",
                dollars(1_530),
            )],
        }),
    )?;

    c.post(
        d(2026, 2, 5)?,
        "1003",
        "",
        TxnBody::PayBill(PayBill {
            vendor: lumber,
            pay_from: checking,
            ap_account: ap,
            amount: dollars(3_150),
            applied: vec![Applied {
                txn: lumber_bill,
                amount: dollars(3_150),
            }],
        }),
    )?;

    c.post(
        d(2026, 2, 10)?,
        "",
        "",
        TxnBody::ReceivePayment(ReceivePayment {
            customer: maple,
            deposit_to: checking,
            ar_account: ar,
            amount: dollars(4_000),
            method: "ACH".into(),
            applied: vec![Applied {
                txn: inv1,
                amount: dollars(4_000),
            }],
        }),
    )?;

    c.post(
        d(2026, 2, 14)?,
        "VPL-0214",
        "",
        TxnBody::Bill(Bill {
            vendor: power,
            due_date: d(2026, 3, 6)?,
            ap_account: ap,
            lines: vec![exp(
                utilities,
                "Shop power, January",
                Money::from_cents(41_276),
            )],
        }),
    )?;

    c.post(
        d(2026, 2, 20)?,
        "1003",
        "",
        TxnBody::Invoice(Invoice {
            customer: lin,
            due_date: d(2026, 2, 20)?,
            ar_account: ar,
            ship_to: None,
            tax: None,
            lines: vec![
                line(table, "Round kitchen table", 1, dollars(2_150), sales),
                line(design, "Design consult", 3, dollars(85), service),
            ],
        }),
    )?;

    c.post(
        d(2026, 3, 1)?,
        "",
        "",
        TxnBody::Check(Check {
            payee: None,
            pay_from: card,
            lines: vec![exp(advertising, "Craft fair booth", dollars(450))],
        }),
    )?;

    c.post(
        d(2026, 3, 3)?,
        "1004",
        "March rent",
        TxnBody::Check(Check {
            payee: Some(Party::Vendor(landlord)),
            pay_from: checking,
            lines: vec![exp(rent, "Shop rent", dollars(1_800))],
        }),
    )?;

    c.post(
        d(2026, 3, 9)?,
        "1004",
        "",
        TxnBody::Invoice(Invoice {
            customer: harbor,
            due_date: d(2026, 3, 24)?,
            ar_account: ar,
            ship_to: None,
            tax: None,
            lines: vec![
                line(design, "Reception desk design", 10, dollars(85), service),
                line(chair, "Lounge chairs", 4, dollars(325), sales),
            ],
        }),
    )?;

    rest_of_year(&mut c, [maple, harbor, lin], [lumber, power, landlord], [table, chair, design])?;
    classes(&mut c)?;
    recurring_transactions(&mut c)?;
    Ok(c)
}

/// Recurring transactions: the rent check entered automatically on the
/// first, the loan payment as a reminder (October's is due), and the
/// quarterly depreciation entry.
fn recurring_transactions(c: &mut Company) -> LedgerResult<()> {
    let latest = |c: &Company, memo: &str| {
        c.transactions
            .iter()
            .filter(|t| t.memo == memo && !t.voided)
            .max_by_key(|t| t.date)
            .map(|t| TxnTemplate {
                date: t.date,
                memo: t.memo.clone(),
                body: t.body.clone(),
                class: t.class,
            })
            .ok_or_else(|| LedgerError::NotFound(format!("The {memo} transaction")))
    };
    let mut rent = latest(c, "October rent")?;
    rent.memo = "Shop rent".into();
    let loan = latest(c, "Loan payment")?;
    let mut depreciation = latest(c, "Third-quarter depreciation")?;
    depreciation.memo = "Quarterly depreciation".into();
    for (name, frequency, next, mode, template) in [
        (
            "Shop rent",
            Frequency::Monthly,
            d(2026, 11, 1)?,
            RecurringMode::Automatic,
            rent,
        ),
        (
            "Loan payment",
            Frequency::Monthly,
            d(2026, 10, 4)?,
            RecurringMode::Remind,
            loan,
        ),
        (
            "Quarterly depreciation",
            Frequency::Quarterly,
            d(2026, 12, 31)?,
            RecurringMode::Remind,
            depreciation,
        ),
    ] {
        let spec = RecurringSpec {
            name: name.into(),
            frequency,
            every: 1,
            next,
            end: None,
            mode,
        };
        c.add_recurring(spec, template)?;
    }
    Ok(())
}

/// Classes split the shop's own sales from the craft fairs for Profit &
/// Loss by Class. Expenses are left unclassified, as many owners start.
fn classes(c: &mut Company) -> LedgerResult<()> {
    let shop = c.add_class("Shop")?;
    let fairs = c.add_class("Craft fairs")?;
    let online = c.add_class("Online")?;
    // Tried, then dropped: inactive, so forms leave it off.
    c.set_class_active(online, false)?;
    let tags: Vec<(TxnId, TxnClassId)> = c
        .transactions
        .iter()
        .filter_map(|t| match &t.body {
            TxnBody::Invoice(_) => Some((t.id, shop)),
            TxnBody::SalesReceipt(r) if r.customer.is_some() => Some((t.id, shop)),
            // Walk-in sales at the fair, with no customer.
            TxnBody::SalesReceipt(_) => Some((t.id, fairs)),
            TxnBody::Check(k) if k.lines.iter().any(|l| l.description.contains("Craft fair")) => {
                Some((t.id, fairs))
            }
            _ => None,
        })
        .collect();
    for (id, class) in tags {
        c.set_txn_class(id, Some(class))?;
    }
    Ok(())
}

/// April to October: the shop grows, registers for sales tax, finances a
/// machine and starts using everything LedgerCraft offers.
fn rest_of_year(
    c: &mut Company,
    [maple, harbor, lin]: [CustomerId; 3],
    [lumber, power, landlord]: [VendorId; 3],
    [table, chair, design]: [ItemId; 3],
) -> LedgerResult<()> {
    let checking = acct(c, "Checking")?;
    let savings = acct(c, "Savings")?;
    let undeposited = acct(c, "Undeposited Funds")?;
    let card = acct(c, "Business Credit Card")?;
    let equipment = acct(c, "Equipment")?;
    let accumulated = acct(c, "Accumulated Depreciation")?;
    let loan = acct(c, "Loan Payable")?;
    let draw = acct(c, "Owner's Draw")?;
    let sales = acct(c, "Sales")?;
    let service = acct(c, "Service Income")?;
    let cogs = acct(c, "Cost of Goods Sold")?;
    let rent = acct(c, "Rent")?;
    let utilities = acct(c, "Utilities")?;
    let supplies = acct(c, "Office Supplies")?;
    let software = acct(c, "Software and Subscriptions")?;
    let insurance = acct(c, "Insurance")?;
    let repairs = acct(c, "Repairs and Maintenance")?;
    let bank_charges = acct(c, "Bank Service Charges")?;
    let interest_income = acct(c, "Interest Income")?;
    let interest_expense = acct(c, "Interest Expense")?;
    // An account of the shop's own, beside the standard chart.
    let depreciation = c.add_account(
        "6700",
        "Depreciation Expense",
        AccountType::Expense,
        "Wear on equipment, spread over its useful life",
    )?;
    let line = |account, description: &str, amount: Money| AccountLine {
        account,
        description: description.into(),
        amount,
    };

    // ----- people and products ---------------------------------------------

    // The first three customers get their full details.
    let full = [
        (
            maple,
            Who {
                name: "Maple Street Cafe",
                email: "orders@maple.example",
                phone: "(801) 555-0175",
                street: "12 Maple Street",
                city: "Provo",
                zip: "84601",
                terms: 30,
                notes: "Ask for Rosa Alvarez. Deliver before 7 a.m. or after 2 p.m., when the cafe is quiet.",
            },
        ),
        (
            harbor,
            Who {
                name: "Harbor Design Studio",
                email: "ap@harbor.example",
                phone: "(801) 555-0123",
                street: "480 Center Street, Suite 2",
                city: "Orem",
                zip: "84057",
                terms: 15,
                notes: "Marcus Webb buys for the showroom; they resell our chairs.",
            },
        ),
        (
            lin,
            Who {
                name: "Dana Lin",
                email: "dana@lin.example",
                phone: "(801) 555-0168",
                street: "77 Willow Lane",
                city: "Lehi",
                zip: "84043",
                terms: 0,
                notes: "",
            },
        ),
    ];
    for (id, who) in &full {
        c.update_customer(*id, who.contact())?;
    }
    let summit = c.add_customer(
        Who {
            name: "Summit Ridge Academy",
            email: "bursar@summitridge.example",
            phone: "(801) 555-0190",
            street: "900 School House Road",
            city: "Springville",
            zip: "84663",
            terms: 30,
            notes: "Needs a purchase order number on every invoice (put it in the memo).",
        }
        .contact(),
    )?;
    let alpine = c.add_customer(
        Who {
            name: "Alpine Lodge Rentals",
            email: "kyle@alpinelodge.example",
            phone: "(801) 555-0131",
            street: "3 Canyon View Drive",
            city: "Provo",
            zip: "84604",
            terms: 15,
            notes: "Slow to pay. Call Kyle Brennan before the due date.",
        }
        .contact(),
    )?;
    let whitfield = c.add_customer(
        Who {
            name: "Jordan Whitfield",
            email: "jordan@whitfield.example",
            phone: "",
            street: "15 Orchard Way",
            city: "Springville",
            zip: "84663",
            terms: 0,
            notes: "Moved out of state in May.",
        }
        .contact(),
    )?;

    // Summit Ridge's furniture goes to its west campus; Harbor's to its
    // showroom across the state line in Idaho (see Sales by State).
    for (id, ship_to) in [
        (
            summit,
            Address {
                line1: "West Campus, 450 Mountain View Road".into(),
                city: "Mapleton".into(),
                region: "UT".into(),
                postal_code: "84664".into(),
                ..Address::default()
            },
        ),
        (
            harbor,
            Address {
                line1: "Harbor Showroom, 210 Main Street".into(),
                city: "Boise".into(),
                region: "ID".into(),
                postal_code: "83702".into(),
                ..Address::default()
            },
        ),
    ] {
        let mut contact = c
            .customer(id)
            .map(|x| x.contact.clone())
            .ok_or_else(|| LedgerError::NotFound(format!("Customer {id}")))?;
        contact.ship_to = ship_to;
        c.update_customer(id, contact)?;
    }

    let vendor = |name: &str, email: &str, city: &str, terms: u32, notes: &str| {
        Who {
            name,
            email,
            phone: "",
            street: "",
            city,
            zip: "",
            terms,
            notes,
        }
        .contact()
    };
    let hardware = c.add_vendor(vendor(
        "Wasatch Hardware",
        "",
        "Springville",
        0,
        "Paid with the business credit card.",
    ))?;
    let tax_office = c.add_vendor(vendor(
        "Utah State Tax Commission",
        "",
        "Salt Lake City",
        0,
        "Sales tax is filed and paid quarterly, by the end of the following month.",
    ))?;
    let insurer = c.add_vendor(vendor(
        "Mountain States Insurance",
        "billing@msi.example",
        "Provo",
        30,
        "",
    ))?;
    let bank = c.add_vendor(vendor(
        "First Valley Bank",
        "",
        "Springville",
        0,
        "Equipment loan for the CNC router: $12,000 over 40 months.",
    ))?;
    let old_mill = c.add_vendor(vendor(
        "Old Mill Finishes",
        "",
        "Payson",
        30,
        "Closed in March; finishes now come from Wasatch Hardware.",
    ))?;
    c.set_vendor_active(old_mill, false)?;

    let bookshelf = c.add_item(ItemSpec {
        name: "Bookshelf".into(),
        description: "Five-shelf bookcase, solid oak".into(),
        kind: ItemKind::NonInventory,
        rate: dollars(890),
        income_account: sales,
        taxable: true,
    })?;
    let delivery = c.add_item(ItemSpec {
        name: "Delivery".into(),
        description: "Delivery and set-up".into(),
        kind: ItemKind::OtherCharge,
        rate: dollars(75),
        income_account: sales,
        taxable: false,
    })?;
    let refinish = c.add_item(ItemSpec {
        name: "Refinishing".into(),
        description: "Strip, repair and refinish, per hour".into(),
        kind: ItemKind::Service,
        rate: dollars(65),
        income_account: service,
        taxable: false,
    })?;
    let rocker = c.add_item(ItemSpec {
        name: "Rocking Chair".into(),
        description: "Bent-oak rocking chair".into(),
        kind: ItemKind::NonInventory,
        rate: dollars(640),
        income_account: sales,
        taxable: true,
    })?;

    // ----- sales tax ---------------------------------------------------------

    let agency = "Utah State Tax Commission";
    let from = d(2026, 1, 1)?;
    let state = c.add_tax_component("Utah State", agency, "", from, rate("4.85")?)?;
    let county = c.add_tax_component("Utah County", agency, "", from, rate("0.95")?)?;
    // The county's rate went up in July: invoices before then keep the old one.
    c.set_tax_rate(county, d(2026, 7, 1)?, rate("1.05")?)?;
    let springville_city = c.add_tax_component("Springville City", agency, "", from, rate("1.00")?)?;
    let provo_city = c.add_tax_component("Provo City", agency, "", from, rate("1.10")?)?;
    let lehi_city = c.add_tax_component("Lehi City", agency, "", from, rate("1.00")?)?;
    let springville = c.add_tax_group("Springville", vec![state, county, springville_city])?;
    let provo = c.add_tax_group("Provo", vec![state, county, provo_city])?;
    let lehi = c.add_tax_group("Lehi", vec![state, county, lehi_city])?;
    for (customer, group) in [
        (maple, provo),
        (lin, lehi),
        (summit, springville),
        (alpine, provo),
        (whitfield, springville),
    ] {
        c.set_customer_tax(
            customer,
            CustomerTax {
                group: Some(group),
                ..CustomerTax::default()
            },
        )?;
    }
    c.set_customer_tax(
        harbor,
        CustomerTax {
            group: Some(provo),
            exempt: true,
            exemption: "Resale certificate R-20418".into(),
        },
    )?;
    // Checked at the last quarterly review, so no reminder until December.
    c.mark_tax_rates_checked(d(2026, 9, 2)?);

    // Bank feed rules, for the bank download in docs/samples.
    c.bank_rules.push(BankRule {
        contains: "VALLEY POWER".into(),
        category: utilities,
    });
    c.bank_rules.push(BankRule {
        contains: "SERVICE CHARGE".into(),
        category: bank_charges,
    });

    // ----- April to October --------------------------------------------------

    // Old business first: the February power bill and the open invoices.
    let feb_power = c
        .transactions
        .iter()
        .find(|t| t.number == "VPL-0214")
        .map(|t| t.id)
        .ok_or_else(|| LedgerError::NotFound("Bill VPL-0214".into()))?;
    let open_q1: Vec<(TxnId, Money)> = c
        .transactions
        .iter()
        .filter(|t| matches!(t.body, TxnBody::Invoice(_)))
        .map(|t| (t.id, c.open_balance(t.id, None)))
        .collect();
    let open = |n: usize| {
        open_q1
            .get(n)
            .copied()
            .ok_or_else(|| LedgerError::NotFound(format!("Invoice {n}")))
    };
    // 1001 (Maple, $4,700 left), 1002 (paid), 1003 (Dana Lin), 1004 (Harbor).
    let (inv1001, maple_left) = open(0)?;
    let (inv1003, _) = open(2)?;
    let (inv1004, _) = open(3)?;

    pay_bills(c, d(2026, 4, 2)?, "1005", power, &[(feb_power, None)])?;
    check(
        c,
        d(2026, 4, 3)?,
        "1006",
        "April rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    receive(c, d(2026, 4, 3)?, harbor, checking, "ACH", &[(inv1004, None)])?;
    let jw = invoice(
        c,
        d(2026, 4, 6)?,
        "1005",
        "",
        whitfield,
        0,
        Some(springville),
        &[
            (rocker, "Bent-oak rocking chair", 1, dollars(640)),
            (delivery, "Delivery and set-up", 1, dollars(75)),
        ],
    )?;
    let lumber_apr = bill(
        c,
        d(2026, 4, 8)?,
        "CL-5602",
        lumber,
        30,
        vec![line(cogs, "Red oak and maple", dollars(2_840))],
    )?;
    receive(
        c,
        d(2026, 4, 10)?,
        maple,
        undeposited,
        "Check #2231",
        &[(inv1001, None)],
    )?;
    let jw_total = total(c, jw)?;
    receive(c, d(2026, 4, 10)?, whitfield, undeposited, "Cash", &[(jw, None)])?;
    deposit(
        c,
        d(2026, 4, 13)?,
        "",
        checking,
        vec![
            line(undeposited, "Maple Street Cafe check #2231", maple_left),
            line(undeposited, "Jordan Whitfield cash", jw_total),
        ],
    )?;
    let inv1006 = invoice(
        c,
        d(2026, 4, 14)?,
        "1006",
        "",
        maple,
        30,
        Some(provo),
        &[
            (bookshelf, "Bookshelves for the reading nook", 2, dollars(890)),
            (delivery, "Delivery and set-up", 1, dollars(75)),
        ],
    )?;
    // The CNC router, financed by the bank.
    journal(
        c,
        d(2026, 4, 15)?,
        "JE-1",
        "CNC router financed by First Valley Bank",
        &[
            (equipment, dollars(12_000), Money::ZERO, "CNC router"),
            (loan, Money::ZERO, dollars(12_000), "First Valley Bank loan"),
        ],
    )?;
    check(
        c,
        d(2026, 4, 18)?,
        "",
        "",
        Some(Party::Vendor(hardware)),
        card,
        vec![line(supplies, "Router bits and clamps", cents(38_642))],
    )?;
    let power_apr = bill(
        c,
        d(2026, 4, 20)?,
        "VPL-0420",
        power,
        20,
        vec![line(utilities, "Shop power, March", cents(38_815))],
    )?;
    let inv1007 = invoice(
        c,
        d(2026, 4, 28)?,
        "1007",
        "PO 4471",
        summit,
        30,
        Some(springville),
        &[
            (bookshelf, "Library bookshelves", 6, dollars(890)),
            (delivery, "Delivery and set-up", 2, dollars(75)),
        ],
    )?;
    // The card bill, paid in full.
    check(
        c,
        d(2026, 4, 28)?,
        "1007",
        "Card payment",
        None,
        checking,
        vec![line(card, "Business credit card", cents(110_078))],
    )?;
    c.set_customer_active(whitfield, false)?;
    c.set_item_active(rocker, false)?;

    // May
    check(
        c,
        d(2026, 5, 1)?,
        "1008",
        "May rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    deposit(
        c,
        d(2026, 5, 1)?,
        "Move to savings",
        savings,
        vec![line(checking, "Transfer from checking", dollars(5_000))],
    )?;
    check(
        c,
        d(2026, 5, 4)?,
        "1009",
        "Loan payment",
        Some(Party::Vendor(bank)),
        checking,
        vec![
            line(loan, "Principal", dollars(300)),
            line(interest_expense, "Interest", dollars(75)),
        ],
    )?;
    pay_bills(
        c,
        d(2026, 5, 6)?,
        "1010",
        lumber,
        &[(lumber_apr, None)],
    )?;
    pay_bills(c, d(2026, 5, 8)?, "1011", power, &[(power_apr, None)])?;
    receive(c, d(2026, 5, 12)?, maple, checking, "ACH", &[(inv1006, None)])?;
    let inv1008 = invoice(
        c,
        d(2026, 5, 18)?,
        "1008",
        "Showroom order",
        harbor,
        15,
        None,
        &[
            (chair, "Matching hardwood chair", 8, dollars(325)),
            (design, "Showroom layout", 6, dollars(85)),
        ],
    )?;
    check(
        c,
        d(2026, 5, 22)?,
        "",
        "",
        Some(Party::Vendor(hardware)),
        card,
        vec![
            line(supplies, "Finish, glue and sandpaper", cents(24_318)),
            line(repairs, "Dust collector filter", cents(8_950)),
        ],
    )?;
    check(
        c,
        d(2026, 5, 22)?,
        "",
        "",
        None,
        card,
        vec![line(software, "Design software subscription", cents(5_999))],
    )?;

    // June
    check(
        c,
        d(2026, 6, 1)?,
        "1012",
        "June rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    let summit_pay = total(c, inv1007)?;
    receive(c, d(2026, 6, 2)?, summit, undeposited, "Check #10552", &[(inv1007, None)])?;
    check(
        c,
        d(2026, 6, 4)?,
        "1013",
        "Loan payment",
        Some(Party::Vendor(bank)),
        checking,
        vec![
            line(loan, "Principal", dollars(300)),
            line(interest_expense, "Interest", dollars(73)),
        ],
    )?;
    let harbor_pay = total(c, inv1008)?;
    receive(c, d(2026, 6, 5)?, harbor, undeposited, "Check #5120", &[(inv1008, None)])?;
    deposit(
        c,
        d(2026, 6, 8)?,
        "",
        checking,
        vec![
            line(undeposited, "Summit Ridge Academy check #10552", summit_pay),
            line(undeposited, "Harbor Design Studio check #5120", harbor_pay),
        ],
    )?;
    let inv1009 = invoice(
        c,
        d(2026, 6, 10)?,
        "1009",
        "Lodge great room",
        alpine,
        15,
        Some(provo),
        &[
            (table, "Live-edge dining table", 1, dollars(2_400)),
            (chair, "Matching hardwood chair", 6, dollars(325)),
        ],
    )?;
    let lumber_jun = bill(
        c,
        d(2026, 6, 10)?,
        "CL-5688",
        lumber,
        30,
        vec![line(cogs, "Walnut slabs", dollars(3_420))],
    )?;
    let power_jun = bill(
        c,
        d(2026, 6, 12)?,
        "VPL-0612",
        power,
        20,
        vec![line(utilities, "Shop power, May", cents(36_204))],
    )?;
    // A weekend craft fair: walk-in sales, paid on the spot.
    let fair_cash = sales_receipt(
        c,
        d(2026, 6, 13)?,
        "1001",
        None,
        undeposited,
        "Cash",
        Some(springville),
        &[(chair, "Matching hardwood chair", 2, dollars(325))],
    )?;
    let fair_card = sales_receipt(
        c,
        d(2026, 6, 13)?,
        "1002",
        None,
        undeposited,
        "Visa",
        Some(springville),
        &[(bookshelf, "Five-shelf bookcase, solid oak", 1, dollars(890))],
    )?;
    let (cash, card_sale) = (total(c, fair_cash)?, total(c, fair_card)?);
    deposit(
        c,
        d(2026, 6, 15)?,
        "Craft fair takings",
        checking,
        vec![
            line(undeposited, "Craft fair cash", cash),
            line(undeposited, "Craft fair card sales", card_sale),
        ],
    )?;
    let inv1010 = invoice(
        c,
        d(2026, 6, 22)?,
        "1010",
        "",
        lin,
        0,
        Some(lehi),
        &[
            (chair, "Matching hardwood chair", 2, dollars(325)),
            (refinish, "Refinish the old table", 4, dollars(65)),
        ],
    )?;
    receive(
        c,
        d(2026, 6, 22)?,
        lin,
        checking,
        "Visa",
        &[(inv1003, None), (inv1010, None)],
    )?;
    check(
        c,
        d(2026, 6, 25)?,
        "1014",
        "Card payment",
        None,
        checking,
        vec![line(card, "Business credit card", cents(39_267))],
    )?;
    deposit(
        c,
        d(2026, 6, 30)?,
        "",
        savings,
        vec![line(interest_income, "Interest", cents(412))],
    )?;
    journal(
        c,
        d(2026, 6, 30)?,
        "JE-2",
        "Second-quarter depreciation",
        &[
            (depreciation, dollars(625), Money::ZERO, "Table saw and CNC router"),
            (accumulated, Money::ZERO, dollars(625), ""),
        ],
    )?;

    // July
    let rent_jul = check(
        c,
        d(2026, 7, 1)?,
        "1015",
        "July rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    // Printed with the wrong amount: voided and written again.
    c.void(rent_jul)?;
    check(
        c,
        d(2026, 7, 1)?,
        "1016",
        "July rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    pay_bills(c, d(2026, 7, 2)?, "1017", power, &[(power_jun, None)])?;
    // One of Alpine Lodge's chairs arrived scratched: a credit against 1009.
    credit_memo(
        c,
        d(2026, 7, 2)?,
        "1001",
        "Scratched chair",
        alpine,
        Some(provo),
        &[(chair, "Credit for one scratched chair", 1, dollars(325))],
        Some(inv1009),
    )?;
    // Dana Lin returned a chair she had paid for, and was refunded.
    let returned = credit_memo(
        c,
        d(2026, 7, 8)?,
        "1002",
        "Returned chair",
        lin,
        Some(lehi),
        &[(chair, "Returned chair", 1, dollars(325))],
        None,
    )?;
    refund(c, d(2026, 7, 10)?, lin, checking, "Refund to Visa", &[(returned, None)])?;
    check(
        c,
        d(2026, 7, 6)?,
        "1018",
        "Loan payment",
        Some(Party::Vendor(bank)),
        checking,
        vec![
            line(loan, "Principal", dollars(300)),
            line(interest_expense, "Interest", dollars(71)),
        ],
    )?;
    let inv1011 = invoice(
        c,
        d(2026, 7, 9)?,
        "1011",
        "",
        maple,
        30,
        Some(provo),
        &[
            (table, "Second communal table, 8 ft", 1, dollars(2_400)),
            (chair, "Bench-height chairs", 4, dollars(325)),
        ],
    )?;
    pay_bills(
        c,
        d(2026, 7, 10)?,
        "1019",
        lumber,
        &[(lumber_jun, Some(dollars(2_000)))],
    )?;
    check(
        c,
        d(2026, 7, 15)?,
        "1020",
        "Owner's draw",
        None,
        checking,
        vec![line(draw, "Owner's draw", dollars(2_000))],
    )?;
    check(
        c,
        d(2026, 7, 21)?,
        "",
        "",
        Some(Party::Vendor(hardware)),
        card,
        vec![line(supplies, "Hinges, pulls and finish", cents(31_877))],
    )?;
    check(
        c,
        d(2026, 7, 22)?,
        "",
        "",
        None,
        card,
        vec![line(software, "Design software subscription", cents(5_999))],
    )?;
    let cabins = invoice(
        c,
        d(2026, 7, 20)?,
        "1012",
        "Guest cabins",
        alpine,
        15,
        Some(provo),
        &[
            (bookshelf, "Bookshelves for the cabins", 3, dollars(890)),
            (delivery, "Delivery and set-up", 1, dollars(75)),
        ],
    )?;
    let summit_jul = invoice(
        c,
        d(2026, 7, 27)?,
        "1013",
        "PO 4529",
        summit,
        30,
        Some(springville),
        &[
            (refinish, "Refinish auditorium benches", 20, dollars(65)),
            (bookshelf, "Classroom bookshelves", 2, dollars(890)),
        ],
    )?;
    // Second-quarter sales tax, paid by the end of July.
    let q2_owed = c
        .sales_tax_owed(d(2026, 6, 30)?)
        .into_iter()
        .find(|o| o.agency == agency)
        .map_or(Money::ZERO, |o| o.owed);
    let tax_account = c
        .sales_tax_account()
        .ok_or_else(|| LedgerError::NotFound("Sales Tax Payable".into()))?;
    check(
        c,
        d(2026, 7, 28)?,
        "1021",
        "Sales tax payment",
        Some(Party::Vendor(tax_office)),
        checking,
        vec![line(tax_account, &tax_payment_memo(agency), q2_owed)],
    )?;
    deposit(
        c,
        d(2026, 7, 31)?,
        "",
        savings,
        vec![line(interest_income, "Interest", cents(415))],
    )?;

    // August
    check(
        c,
        d(2026, 8, 3)?,
        "1022",
        "August rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    check(
        c,
        d(2026, 8, 4)?,
        "1023",
        "Loan payment",
        Some(Party::Vendor(bank)),
        checking,
        vec![
            line(loan, "Principal", dollars(300)),
            line(interest_expense, "Interest", dollars(69)),
        ],
    )?;
    pay_bills(
        c,
        d(2026, 8, 5)?,
        "1024",
        lumber,
        &[(lumber_jun, None)],
    )?;
    // A partial payment: the rest came in September.
    receive(c, d(2026, 8, 7)?, maple, checking, "ACH", &[(inv1011, Some(dollars(2_000)))])?;
    let lumber_aug = bill(
        c,
        d(2026, 8, 12)?,
        "CL-5790",
        lumber,
        30,
        vec![line(cogs, "White oak and walnut", dollars(2_960))],
    )?;
    let power_aug = bill(
        c,
        d(2026, 8, 14)?,
        "VPL-0814",
        power,
        20,
        vec![line(utilities, "Shop power, July", cents(44_190))],
    )?;
    // Paid on the spot when the work was picked up.
    sales_receipt(
        c,
        d(2026, 8, 20)?,
        "1003",
        Some(maple),
        checking,
        "Visa",
        Some(provo),
        &[
            (refinish, "Refinish two cafe tables", 3, dollars(65)),
            (delivery, "Delivery and set-up", 1, dollars(75)),
        ],
    )?;
    check(
        c,
        d(2026, 8, 21)?,
        "",
        "",
        Some(Party::Vendor(hardware)),
        card,
        vec![line(supplies, "Sanding discs and finish", cents(19_455))],
    )?;
    check(
        c,
        d(2026, 8, 22)?,
        "",
        "",
        None,
        card,
        vec![line(software, "Design software subscription", cents(5_999))],
    )?;
    check(
        c,
        d(2026, 8, 25)?,
        "1025",
        "Card payment",
        None,
        checking,
        vec![line(card, "Business credit card", cents(37_876))],
    )?;
    receive(
        c,
        d(2026, 8, 26)?,
        summit,
        checking,
        "Check #10618",
        &[(summit_jul, None)],
    )?;
    deposit(
        c,
        d(2026, 8, 31)?,
        "",
        savings,
        vec![line(interest_income, "Interest", cents(418))],
    )?;

    // September
    check(
        c,
        d(2026, 9, 1)?,
        "1026",
        "September rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    // Not paid yet, and overdue: it shows on the A/P Aging report.
    bill(
        c,
        d(2026, 9, 1)?,
        "MSI-26-09",
        insurer,
        30,
        vec![line(insurance, "Liability insurance, next six months", dollars(1_140))],
    )?;
    // Not paid yet: a month late by October.
    invoice(
        c,
        d(2026, 9, 3)?,
        "1014",
        "",
        harbor,
        15,
        None,
        &[
            (design, "Fall showroom refresh", 12, dollars(85)),
            (chair, "Matching hardwood chair", 6, dollars(325)),
        ],
    )?;
    check(
        c,
        d(2026, 9, 4)?,
        "1027",
        "Loan payment",
        Some(Party::Vendor(bank)),
        checking,
        vec![
            line(loan, "Principal", dollars(300)),
            line(interest_expense, "Interest", dollars(67)),
        ],
    )?;
    receive(c, d(2026, 9, 8)?, maple, checking, "ACH", &[(inv1011, None)])?;
    pay_bills(
        c,
        d(2026, 9, 10)?,
        "1028",
        lumber,
        &[(lumber_aug, None)],
    )?;
    pay_bills(c, d(2026, 9, 10)?, "1029", power, &[(power_aug, None)])?;
    invoice(
        c,
        d(2026, 9, 15)?,
        "1015",
        "PO 4602",
        summit,
        30,
        Some(springville),
        &[
            (bookshelf, "Office bookshelves", 4, dollars(890)),
            (delivery, "Delivery and set-up", 1, dollars(75)),
        ],
    )?;
    // Alpine Lodge pays a little of what they owe.
    receive(c, d(2026, 9, 20)?, alpine, checking, "Check #388", &[(cabins, Some(dollars(1_000)))])?;
    // An allowance for a late delivery, not used yet.
    credit_memo(
        c,
        d(2026, 9, 25)?,
        "1003",
        "Allowance for a late delivery",
        harbor,
        None,
        &[(design, "Allowance for a late delivery", 2, dollars(85))],
        None,
    )?;
    check(
        c,
        d(2026, 9, 21)?,
        "",
        "",
        Some(Party::Vendor(hardware)),
        card,
        vec![line(supplies, "Clamps and glue", cents(14_220))],
    )?;
    check(
        c,
        d(2026, 9, 22)?,
        "",
        "",
        None,
        card,
        vec![line(software, "Design software subscription", cents(5_999))],
    )?;
    deposit(
        c,
        d(2026, 9, 30)?,
        "",
        savings,
        vec![line(interest_income, "Interest", cents(421))],
    )?;
    journal(
        c,
        d(2026, 9, 30)?,
        "JE-3",
        "Third-quarter depreciation",
        &[
            (depreciation, dollars(825), Money::ZERO, "Table saw and CNC router"),
            (accumulated, Money::ZERO, dollars(825), ""),
        ],
    )?;

    // October, so far
    check(
        c,
        d(2026, 10, 1)?,
        "1030",
        "October rent",
        Some(Party::Vendor(landlord)),
        checking,
        vec![line(rent, "Shop rent", dollars(1_800))],
    )?;
    check(
        c,
        d(2026, 10, 5)?,
        "1031",
        "Loan payment",
        Some(Party::Vendor(bank)),
        checking,
        vec![
            line(loan, "Principal", dollars(300)),
            line(interest_expense, "Interest", dollars(65)),
        ],
    )?;
    let inv1016 = invoice(
        c,
        d(2026, 10, 6)?,
        "1016",
        "",
        maple,
        30,
        Some(provo),
        &[
            (chair, "Patio chairs", 6, dollars(325)),
            (delivery, "Delivery and set-up", 1, dollars(75)),
        ],
    )?;
    // Not yet due: it shows in Accounts Payable as current.
    bill(
        c,
        d(2026, 10, 9)?,
        "CL-5873",
        lumber,
        30,
        vec![line(cogs, "Cherry and maple", dollars(3_180))],
    )?;
    bill(
        c,
        d(2026, 10, 12)?,
        "VPL-1012",
        power,
        20,
        vec![line(utilities, "Shop power, September", cents(40_533))],
    )?;
    invoice(
        c,
        d(2026, 10, 13)?,
        "1017",
        "",
        lin,
        0,
        Some(lehi),
        &[
            (table, "Round kitchen table", 1, dollars(2_150)),
            (delivery, "Delivery and set-up", 1, dollars(75)),
        ],
    )?;
    // Waiting in Undeposited Funds for the next trip to the bank.
    receive(
        c,
        d(2026, 10, 15)?,
        maple,
        undeposited,
        "Check #2318",
        &[(inv1016, Some(dollars(1_000)))],
    )?;
    // The same cafe entered again under a slightly different name, with one
    // sale: merge it into Maple Street Cafe to practise merging.
    let duplicate_cafe = c.add_customer(
        Who {
            name: "Maple St. Cafe",
            email: "",
            phone: "",
            street: "12 Maple Street",
            city: "Provo",
            zip: "84601",
            terms: 30,
            notes: "Entered twice by mistake: the same as Maple Street Cafe.",
        }
        .contact(),
    )?;
    sales_receipt(
        c,
        d(2026, 10, 14)?,
        "1004",
        Some(duplicate_cafe),
        checking,
        "Visa",
        None,
        &[(refinish, "Refinish a cafe table", 1, dollars(65))],
    )?;
    // Accumulated depreciation as a sub-account of the equipment it wears.
    c.set_account_parent(accumulated, Some(equipment))?;

    // Entered twice by mistake; the copy was voided.
    let duplicate = invoice(
        c,
        d(2026, 10, 16)?,
        "1018",
        "Duplicate of 1012",
        alpine,
        15,
        Some(provo),
        &[(bookshelf, "Bookshelves for the cabins", 3, dollars(890))],
    )?;
    c.void(duplicate)?;
    check(
        c,
        d(2026, 10, 19)?,
        "",
        "",
        Some(Party::Vendor(hardware)),
        card,
        vec![line(supplies, "Finish and sandpaper", cents(22_764))],
    )?;

    // January and February are closed (no password, so moving the date
    // reopens them). March stays open for the bank download lesson.
    c.closing = Some(ledgercraft_ledger::Closing {
        date: d(2026, 2, 28)?,
        password: None,
    });

    // The card statement through August and the savings statement through
    // September are reconciled; Checking is left for the Reconcile lesson.
    reconcile_all(c, card, d(2026, 8, 31)?)?;
    reconcile_all(c, savings, d(2026, 9, 30)?)?;
    Ok(())
}
