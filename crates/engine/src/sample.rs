//! A fictional sample company, written for LedgerCraft, used by
//! `--sample`, tests and screenshots.

use ledgercraft_core::{Date, Money, Qty};
use ledgercraft_ledger::{
    AccountId, AccountLine, Address, Applied, Bill, Check, Company, Contact, Deposit, Invoice,
    ItemKind, ItemSpec, JournalEntry, JournalLine, LedgerError, LedgerResult, Party, PayBill,
    ReceivePayment, SalesLine, TxnBody,
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

/// Pine Ridge Woodworks: a small custom-furniture shop's first quarter of 2026.
pub fn sample_company() -> LedgerResult<Company> {
    let mut c = Company::with_default_accounts("Pine Ridge Woodworks");
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
            tax: None,
            lines: vec![
                line(design, "Reception desk design", 10, dollars(85), service),
                line(chair, "Lounge chairs", 4, dollars(325), sales),
            ],
        }),
    )?;

    Ok(c)
}
