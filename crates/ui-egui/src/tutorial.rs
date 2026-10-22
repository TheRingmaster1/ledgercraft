//! The Tutorial window: shown at launch until the person ticks "Never show
//! again", and any time from Help > Tutorial.
//!
//! Lessons are plain data (`LESSONS`) so they are easy to edit and to
//! translate later. Every lesson describes the app as it works today; when a
//! feature changes, update its lesson in the same commit.

use eframe::egui::{self, RichText};

use crate::prefs::Prefs;
use crate::{theme, FormKind, Page};

/// Where a lesson's "Try it" button takes the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Page(Page),
    /// File > Open Sample Company.
    Sample,
}

/// One piece of a lesson.
enum Block {
    Text(&'static str),
    Heading(&'static str),
    Steps(&'static [&'static str]),
    Bullets(&'static [&'static str]),
    Tip(&'static str),
}

struct Lesson {
    title: &'static str,
    blocks: &'static [Block],
    try_it: &'static [(&'static str, Target)],
}

use Block::{Bullets, Heading, Steps, Text, Tip};

const fn form(kind: FormKind) -> Target {
    Target::Page(Page::Form(kind))
}

const LESSONS: &[Lesson] = &[
    Lesson {
        title: "Welcome",
        blocks: &[
            Text(
                "LedgerCraft is free, open-source accounting for small businesses. It keeps \
                 real double-entry books: everything you record (invoices, bills, checks, \
                 deposits) updates your accounts, and the reports always balance.",
            ),
            Text(
                "These lessons walk through everything the app can do today. Each takes a \
                 minute or two. Use Next and Back below, or pick a lesson from the list.",
            ),
            Heading("The best way to learn"),
            Text(
                "Open the sample company, Pine Ridge Woodworks, and try things as you read. It \
                 is a small furniture shop with a few months of transactions already entered. \
                 Nothing you do to it matters until you choose to save it.",
            ),
            Tip(
                "LedgerCraft is pre-alpha. Use it to learn and test, and keep your real books \
                 in your current software for now.",
            ),
        ],
        try_it: &[("Open the sample company", Target::Sample)],
    },
    Lesson {
        title: "Getting around",
        blocks: &[
            Heading("Three ways to get anywhere"),
            Bullets(&[
                "The side bar on the left: your lists, reports and the everyday forms.",
                "The menus along the top: File, Edit, View, Lists, Customers, Vendors, \
                 Banking, Reports and Help.",
                "The Home page: buttons laid out in the order work usually happens, and your \
                 key balances under At A Glance.",
            ]),
            Text(
                "Any window size works. Rows of buttons and fields wrap to fit, side-by-side \
                 lists stack in a narrow window, and anything still too wide for the window \
                 scrolls sideways (Shift + mouse wheel, or the scroll bar along the bottom).",
            ),
            Heading("Help is built in"),
            Text(
                "Rest the mouse on almost anything (a side bar item, a button, a field label \
                 or a column heading) and a short explanation appears.",
            ),
            Heading("Messages"),
            Text(
                "The status bar along the bottom confirms what just happened (\"Invoice \
                 recorded\") or explains a problem in red. Messages clear themselves after a \
                 few seconds (problems stay for 20). The right side shows the name of the \
                 company file you have open, whether it is password protected and whether \
                 it has unsaved changes; rest the mouse on it to see the file's full \
                 location. In a narrow window both sides shorten with \"…\"; rest the \
                 mouse on them to read them in full.",
            ),
            Heading("Keyboard shortcuts"),
            Bullets(&[
                "Ctrl+N new company, Ctrl+O open, Ctrl+S save",
                "Ctrl+Z undo, Ctrl+Y redo",
                "Ctrl+I create an invoice, Ctrl+W write a check",
                "Ctrl+B hide or show the side bar (also View > Side Bar, or the < and > \
                 buttons)",
            ]),
        ],
        try_it: &[("Go to Home", Target::Page(Page::Home))],
    },
    Lesson {
        title: "Company files",
        blocks: &[
            Text(
                "All of a company's books live in one file ending in .lcbooks. You can keep \
                 as many companies as you like, one file each.",
            ),
            Heading("Starting out"),
            Text(
                "When no company is open, LedgerCraft shows its start page with three \
                 choices: create a new company, open a company file, or explore the sample \
                 company. The same choices are on the File menu. Below them, Recent \
                 companies lists the files you opened last: click one to open it, or \
                 right-click it to remove it from the list.",
            ),
            Text(
                "LedgerCraft also reopens the company you had open last each time it \
                 starts (turn this off in Help > Preferences), and remembers the window \
                 size and whether the side bar was showing.",
            ),
            Bullets(&[
                "File > New Company opens Create a Company: enter the name (the only \
                 required field), when your fiscal year starts, and contact details, and \
                 choose a standard chart of accounts or an empty one. You can also bring in \
                 your lists (accounts, customers, vendors, items) from IIF or CSV files \
                 exported by other software. LedgerCraft then asks where to save the file.",
                "File > Open Company opens a .lcbooks file you saved earlier. A \
                 password-protected file asks for its password first. File > Open \
                 Previous Company lists the recent ones.",
                "Double-clicking a .lcbooks file opens it in LedgerCraft once it is set \
                 up: on Windows, Help > Preferences > Files; on Linux, the install.sh \
                 script in the release package does it.",
                "File > Password sets, changes or removes the password of the open \
                 company. New companies always get one.",
                "File > Open Sample Company loads Pine Ridge Woodworks to practise on.",
                "File > Close Company puts the books away and returns to the start page. \
                 If there are unsaved changes, it asks first.",
            ]),
            Heading("Saving"),
            Text(
                "Use File > Save (Ctrl+S) or Save As. Until you save, the title bar shows \
                 \"Unsaved\" and the status bar says \"unsaved changes\". If you close the \
                 window or open another company with changes pending, LedgerCraft asks \
                 whether to save first.",
            ),
            Heading("Passwords and encryption"),
            Text(
                "Every new company is protected by a password you choose when you create \
                 it. The file is encrypted with it, so a copy on a lost laptop, a USB drive \
                 or in cloud storage cannot be read without the password. Keep the \
                 password in a password manager: if it is forgotten, the books cannot be \
                 recovered by anyone.",
            ),
            Text(
                "A password needs at least 12 characters and must be hard to guess: the \
                 meter under the password field shows how strong it is, and common \
                 passwords, names, dates, keyboard runs and the company's own name are \
                 refused. Four or more unrelated words make a strong password that is easy \
                 to remember. Files with older, shorter passwords still open.",
            ),
            Bullets(&[
                "Show, beside the password box, reveals what you typed (Hide covers it \
                 again). Check nobody can see your screen first. Every password window \
                 opens with it hidden.",
                "Next to Confirm, \"Doesn't match\" (red) or \"Matches\" (green) tells you \
                 straight away whether the two passwords agree.",
            ]),
            Text(
                "Company files made before passwords existed still open. LedgerCraft \
                 offers to protect them when you open one; say yes, or use File > \
                 Password… later, and the file is upgraded to an encrypted one.",
            ),
            Heading("Backups"),
            Text(
                "Back up by copying the .lcbooks file somewhere safe, such as a USB drive or \
                 cloud storage. Saving is crash-safe: LedgerCraft writes a new copy and swaps \
                 it in, so a power cut never leaves half a file.",
            ),
            Tip(
                "The file format is open and documented (see docs/file-format.md): with \
                 your password, your books are never locked inside LedgerCraft.",
            ),
        ],
        try_it: &[],
    },
    Lesson {
        title: "The chart of accounts",
        blocks: &[
            Text(
                "Accounts are the buckets your money is sorted into. Every amount you record \
                 lands in at least two of them, which is what keeps the books balanced.",
            ),
            Heading("The kinds of account"),
            Bullets(&[
                "Assets: what the business has. Bank accounts, Accounts Receivable (what \
                 customers owe you), equipment.",
                "Liabilities: what it owes. Accounts Payable (unpaid bills), credit cards, \
                 loans.",
                "Equity: the owner's stake, including profit kept from earlier years.",
                "Income: money earned from sales and services.",
                "Expenses and Cost of Goods Sold: money spent to run the business.",
            ]),
            Heading("Working with accounts"),
            Steps(&[
                "Open Chart of Accounts from the side bar.",
                "Click any account's name to see its register: every transaction in that \
                 account with a running balance. Click a transaction's date (or \
                 double-click its row) to open it and change it.",
                "To add one, open \"New account\", enter a number (optional), a name and \
                 the type, then click Add.",
                "Make inactive hides an account you no longer use. It is only allowed once \
                 the balance is zero, and its history is kept. Tick \"Show inactive \
                 accounts\" to see them again.",
            ]),
            Tip(
                "An account's type cannot be changed after it is created, so posted history \
                 always keeps its meaning. Choose carefully.",
            ),
        ],
        try_it: &[("Open the Chart of Accounts", Target::Page(Page::Accounts))],
    },
    Lesson {
        title: "Customers, vendors and items",
        blocks: &[
            Heading("Customers and vendors"),
            Text(
                "Customers are who pays you; vendors are who you pay. Add them on the \
                 Customers and Vendors pages under \"New customer\" or \"New vendor\".",
            ),
            Bullets(&[
                "Name is required and must be unique. Email, phone and address are optional.",
                "Terms (days) sets when their invoices or bills fall due: 30 means Net 30, \
                 0 means due on receipt. Due dates are filled in from it.",
                "Each list shows what every customer owes you, or what you owe every vendor.",
                "The Invoice / Receive payment and Enter bill / Pay buttons beside each name \
                 open the right form with that name already chosen.",
            ]),
            Heading("Products and services"),
            Text(
                "Items are the things you sell. Give each one a name, a type, a usual rate \
                 and the income account its sales count toward. Choosing an item on an \
                 invoice fills in its description, price and income account for you.",
            ),
        ],
        try_it: &[
            ("Customers", Target::Page(Page::Customers)),
            ("Vendors", Target::Page(Page::Vendors)),
            ("Products & Services", Target::Page(Page::Items)),
        ],
    },
    Lesson {
        title: "Invoicing a customer",
        blocks: &[
            Text(
                "An invoice records a sale the customer will pay for later. The total is \
                 added to what they owe (Accounts Receivable) and counted as income right \
                 away.",
            ),
            Steps(&[
                "Open Create Invoice (side bar, Customers menu or Ctrl+I).",
                "Choose the customer. The due date fills in from their terms.",
                "Leave Invoice # empty to use the next number shown in grey, or type your own.",
                "On each line, choose an item (or type a description), then the quantity and \
                 rate. An empty quantity means 1. The amount and total update as you type.",
                "Check the income account on each line, then click Save & New.",
            ]),
            Heading("Sending it as a PDF"),
            Text(
                "Click Save as PDF… instead of Save & New to record the invoice and save a \
                 PDF of it, ready to attach to an email or print. It shows your company's \
                 name, address, phone and email (change them in File > Company Information…), \
                 who it is billed to, \
                 the lines, payments received so far and the balance due.",
            ),
            Bullets(&[
                "For an invoice already recorded, click PDF on its row in Transactions, or \
                 open it and click Save as PDF…; changes you made are saved first.",
                "A new empty line appears as soon as you type in the last one, so there is \
                 always room for the next; the x button removes a line.",
                "Dates can be typed as 2026-03-31 or 3/31/2026.",
                "Clear empties the form without saving anything.",
            ]),
            Tip("Practise in the sample company: invoice Maple Street Cafe for one Chair."),
        ],
        try_it: &[("Create an invoice", form(FormKind::Invoice))],
    },
    Lesson {
        title: "Charging sales tax",
        blocks: &[
            Text(
                "A sales tax is one agency's rate (state, county, city or district) with the \
                 date each rate starts. A tax group is the taxes charged together where you \
                 sell, such as state + county + city. Invoices charge a group; the tax goes \
                 to Sales Tax Payable, what you owe the state.",
            ),
            Heading("Setting it up"),
            Steps(&[
                "Open Lists > Sales Tax.",
                "In most states, click Import Rates from State Files…: download your state's \
                 rate file and boundary file (Rate and Boundary Files at \
                 streamlinedsalestax.org), choose them, and type your ZIP+4. LedgerCraft adds \
                 each tax with its rate history and a group for your location.",
                "Or add each tax by hand under New sales tax, then put them together under \
                 New tax group.",
                "In Products and Services, untick Taxable for anything not taxed (many \
                 services). In Customers, choose each customer's group, or Exempt with their \
                 certificate number.",
            ]),
            Heading("On an invoice"),
            Bullets(&[
                "Sales tax starts with the customer's group (or your only one). Change it, \
                 or choose No sales tax.",
                "The Tax box on each line says whether it is taxed. The subtotal, each tax \
                 and the total show under the lines, and on the PDF.",
                "The tax uses the rates in force on the invoice's date, and is kept on the \
                 invoice: changing a rate later never changes an invoice already recorded.",
            ]),
            Heading("Keeping rates current"),
            Text(
                "States change rates at the start of a quarter and publish them the month \
                 before. From March, June, September and December 1, LedgerCraft reminds you \
                 to check: import the new files, or click Rates Are Up to Date.",
            ),
            Heading("Filing and paying"),
            Bullets(&[
                "Reports > Sales Tax Liability, for your filing period, shows your sales, \
                 the taxable part, the tax charged by location (tax group) and by agency, \
                 and what is owed.",
                "Pay sales tax, at the bottom of Lists > Sales Tax, shows what each agency is \
                 owed and records your payment (Amount owed fills it in).",
            ]),
            Tip(
                "Rate files name taxes by code (UT City 58730). Click Edit beside a tax to give \
                 it a real name, such as Payson City.",
            ),
        ],
        try_it: &[("Open Sales Tax", Target::Page(Page::SalesTax))],
    },
    Lesson {
        title: "Receiving payments and deposits",
        blocks: &[
            Text(
                "When a customer pays, record it with Receive Payment. It reduces what they \
                 owe and puts the money in the account you choose.",
            ),
            Steps(&[
                "Open Receive Payment and choose the customer. Their open invoices are listed.",
                "Type the amount paid against each invoice, or click Pay all open.",
                "Enter how they paid (check number, ACH, card) if you like.",
                "Choose where the money goes, then Save & New.",
            ]),
            Heading("Undeposited Funds"),
            Text(
                "Payments go to Undeposited Funds by default: a holding account for money \
                 you have received but not yet taken to the bank. When you make the bank \
                 deposit, record it with Make Deposit:",
            ),
            Steps(&[
                "Open Make Deposit and choose the bank account.",
                "Click \"Add it to this deposit\" to bring in everything waiting in \
                 Undeposited Funds.",
                "Add any other lines (for example the owner putting money in), then Save & New.",
            ]),
            Tip(
                "Grouping payments this way makes each deposit match one line on your bank \
                 statement. To skip it, choose a bank account under Deposit to when you \
                 receive the payment.",
            ),
        ],
        try_it: &[
            ("Receive a payment", form(FormKind::ReceivePayment)),
            ("Make a deposit", form(FormKind::Deposit)),
        ],
    },
    Lesson {
        title: "Bills and paying vendors",
        blocks: &[
            Text(
                "Use Enter Bill when a bill arrives that you will pay later, and Pay Bills \
                 when you pay it. Entering the bill counts the expense now and tracks what \
                 you owe; no money leaves the bank until you pay.",
            ),
            Heading("Enter Bill"),
            Steps(&[
                "Choose the vendor. The due date fills in from their terms.",
                "Ref no. is the vendor's own invoice number, so you can match it later.",
                "On each line choose the expense account (Rent, Utilities, Office Supplies...) \
                 and the amount, then Save & New.",
            ]),
            Heading("Pay Bills"),
            Steps(&[
                "Choose the vendor. Their unpaid bills are listed.",
                "Choose the account to pay from: a bank account, or a credit card.",
                "Type the amount for each bill, or click Pay all open, then Save & New. The \
                 check number fills in automatically.",
            ]),
            Tip(
                "The Vendors page and the A/P Aging report show what you owe and how \
                 overdue it is.",
            ),
        ],
        try_it: &[
            ("Enter a bill", form(FormKind::Bill)),
            ("Pay bills", form(FormKind::PayBills)),
        ],
    },
    Lesson {
        title: "Checks, card purchases and expenses",
        blocks: &[
            Text(
                "Write Check / Expense records something paid for on the spot when there is \
                 no bill to track: a check, a debit card payment or a credit card purchase.",
            ),
            Steps(&[
                "Open Write Check / Expense (Ctrl+W).",
                "Pay from: a bank account for checks and debit cards, or a credit card \
                 account for card purchases.",
                "Payee is optional; choose a vendor or customer if you want it on record.",
                "On each line choose what it was for (an expense account) and the amount.",
                "Save & New. Checks from a bank account are numbered automatically.",
            ]),
            Tip(
                "Purchases on a credit card show as \"Credit Card Charge\" in the \
                 transaction list. Pay the card off later with a check to the card account.",
            ),
        ],
        try_it: &[("Write a check", form(FormKind::Check))],
    },
    Lesson {
        title: "Journal entries",
        blocks: &[
            Text(
                "A journal entry moves amounts between any accounts directly. Use it for \
                 opening balances, corrections and adjustments that no other form covers.",
            ),
            Bullets(&[
                "Every line has either a debit or a credit, not both.",
                "Total debits must equal total credits. The line under the form shows the \
                 totals and how far out of balance you are.",
                "Lines on Accounts Receivable need a customer in the Name column, and lines \
                 on Accounts Payable need a vendor.",
                "Debits increase assets and expenses; credits increase liabilities, equity \
                 and income. Hover the Debit and Credit headings for a reminder.",
            ]),
            Tip(
                "Example: the owner puts 1,000 into the business. Debit Checking 1,000, \
                 credit Owner's Equity 1,000.",
            ),
        ],
        try_it: &[("Open Journal Entry", form(FormKind::Journal))],
    },
    Lesson {
        title: "Fixing mistakes",
        blocks: &[
            Heading("Undo and redo"),
            Text(
                "Edit > Undo (Ctrl+Z) reverses your last change and Redo (Ctrl+Y) puts it \
                 back. The Edit menu names what it will undo. While you are typing in a \
                 field, Ctrl+Z undoes your typing instead.",
            ),
            Heading("Changing a transaction"),
            Text(
                "To fix a transaction that is already recorded, open it: click Edit beside it \
                 on the Transactions page, double-click its row, or click its date in an \
                 account register or the Transaction List report. It opens in its own form, \
                 titled Edit, with what was entered filled in.",
            ),
            Bullets(&[
                "Change what you need and click Save Changes. The transaction keeps its \
                 place, and payments applied to it stay applied. Cancel leaves it as it was.",
                "Edits are checked like new entries. An invoice or bill cannot drop below \
                 what has already been paid on it, or change customer or vendor while \
                 payments are applied.",
                "Changed your mind? Edit > Undo puts the original back.",
            ]),
            Heading("Voiding a transaction"),
            Text(
                "On the Transactions page, Void cancels a transaction: it stays on file, \
                 struck through and marked VOID, but no longer counts anywhere. This keeps \
                 an honest record of what happened.",
            ),
            Bullets(&[
                "An invoice or bill that has payments applied cannot be voided until those \
                 payments are voided first.",
                "A void transaction cannot be edited.",
            ]),
            Heading("When something is refused"),
            Text(
                "LedgerCraft checks every form before it is recorded: a missing customer, an \
                 unbalanced journal entry, paying more than is owed. The reason appears in \
                 red in the status bar and nothing is saved, so you can correct it and try \
                 again.",
            ),
        ],
        try_it: &[("Open Transactions", Target::Page(Page::Transactions))],
    },
    Lesson {
        title: "Importing from your bank",
        blocks: &[
            Text(
                "Instead of typing every bank or card transaction, download them from your \
                 bank's website and import the file. Most banks offer OFX, QFX (Quicken), \
                 QBO (QuickBooks Web Connect) or CSV downloads.",
            ),
            Steps(&[
                "Open Banking > Import Bank Transactions.",
                "Choose the bank or credit card account, then Choose File… and pick the \
                 download.",
                "Each line shows what LedgerCraft suggests. Match means it is a transaction \
                 you already recorded (same amount, within 10 days, same check number if \
                 there is one). Add records it as a new transaction: choose its category, \
                 such as Utilities or Sales. Skip leaves it out.",
                "Click Import. Everything comes in at once, and Edit > Undo takes it all \
                 back out; the download's lines then show again, ready to import.",
                "Then click Reconcile with This Statement: Reconcile opens with the \
                 statement date and ending balance from the download filled in.",
            ]),
            Bullets(&[
                "Tick Remember when adding a line to make a rule: next time, lines from \
                 that payee get the same category. Rules are listed at the bottom of the \
                 page, with Delete.",
                "Importing the same download twice is safe: lines brought in before show \
                 as Already imported and are passed over.",
                "Money in becomes a deposit (or, on a credit card, a payment lowering what \
                 you owe); money out becomes a check or card charge.",
            ]),
            Tip(
                "To try it, open the sample company and import docs/samples/\
                 pine-ridge-checking.ofx from the LedgerCraft source folder into Checking.",
            ),
        ],
        try_it: &[("Open Import Bank Transactions", Target::Page(Page::BankImport))],
    },
    Lesson {
        title: "Reconciling with your statement",
        blocks: &[
            Text(
                "Reconciling checks your books against a bank or credit card statement, \
                 item by item. When they agree, you know every deposit, check and charge \
                 is recorded correctly, and nothing is missing or doubled.",
            ),
            Steps(&[
                "Open Banking > Reconcile (or Reconcile in the side bar, or the Reconcile \
                 link at the top of a bank or credit card register).",
                "Choose the account, then enter the statement date and the ending balance \
                 printed on the statement.",
                "Tick each item that appears on the statement, in the two lists: money in, \
                 and money out. Mark All and Unmark All help with long lists.",
                "Watch the Difference (the line under the summary always says what to \
                 do next). When it reaches $0.00, click Reconcile Now. The \
                 reconciliation report opens in its own window, with Export to Excel and \
                 Export CSV.",
            ]),
            Bullets(&[
                "Items that are not on the statement yet (a check not cashed) stay \
                 unticked; they carry over to next month.",
                "Registers mark reconciled items with R. A reconciled transaction cannot \
                 be voided or have its amount changed, so last month's agreed balance \
                 stays agreed. Its memo can still change.",
                "Made a mistake? Undo Last Reconciliation (click it twice) un-reconciles \
                 the last statement's items so you can fix them and reconcile again.",
                "Previous reconciliations are listed under the lists; click one to open \
                 its report window again.",
            ]),
            Tip(
                "If the difference will not reach $0.00, look for an amount typed wrongly, \
                 a transaction entered twice, or a bank fee or interest not entered yet.",
            ),
        ],
        try_it: &[("Open Reconcile", Target::Page(Page::Reconcile))],
    },
    Lesson {
        title: "Reports",
        blocks: &[
            Text("Open Reports from the side bar, or pick one straight from the Reports menu."),
            Bullets(&[
                "Profit & Loss: income minus expenses for a date range. Is the business \
                 making money?",
                "Balance Sheet: what you have, what you owe and the owner's equity on one \
                 date. Total assets always equal total liabilities and equity.",
                "Trial Balance: every account's debit or credit balance. The two totals \
                 always match.",
                "A/R Aging Summary: what each customer owes, grouped by how overdue it is.",
                "A/P Aging Summary: what you owe each vendor, grouped the same way.",
                "Transaction List: everything recorded in a date range.",
            ]),
            Heading("Using reports"),
            Bullets(&[
                "Set From and To (or As of), or click This year or This month.",
                "Click an account name in a report to open its register.",
                "In the Transaction List, click a date (or double-click a row) to open that \
                 transaction and change it.",
                "Export to Excel saves a formatted workbook; Export CSV saves plain values \
                 for other programs.",
            ]),
        ],
        try_it: &[("Open Reports", Target::Page(Page::Reports))],
    },
    Lesson {
        title: "Preferences, and what's next",
        blocks: &[
            Heading("Preferences"),
            Text(
                "Help > Preferences holds settings that apply to every company. Pick a page \
                 from the list on the left; its settings appear on the right and take \
                 effect straight away.",
            ),
            Bullets(&[
                "General: Row Shading (how strongly alternate rows in lists and reports are \
                 shaded), Open Menus On Hover (open the top menus by resting the mouse on \
                 them) and Open The Last Company At Startup. Reset To Default puts these \
                 back.",
                "Currency: the symbol shown with every amount ($, €, £ and others), in the \
                 app and in Excel exports. It changes how amounts look, not their value.",
                "Text: the font and size (Standard, Larger or Largest) for the whole app, \
                 with a preview. Each font's name in the list is shown in that font. \
                 Darkness softens the text colour, which makes heavy fonts look lighter, \
                 and Hinting sharpens letters (turn it off for smoother text). Inter Light, Inter and Classic come with LedgerCraft; \
                 fonts already on your computer (such as Segoe UI or Calibri on Windows, \
                 DejaVu Sans or Noto Sans on Linux) are listed under \"On this computer\".",
                "Files (Windows): make double-clicking a .lcbooks file open it in \
                 LedgerCraft.",
            ]),
            Heading("Seeing this again"),
            Text(
                "Help > Tutorial > Show Tutorial opens these lessons any time. Tick \"Never \
                 show again\" below to stop them appearing at launch.",
            ),
            Heading("Coming later"),
            Text(
                "Live bank connections, a sales tax liability report, customer statements and more. See ROADMAP.md in the \
                 project for the plan. Help > Licenses and Help > About explain the open- \
                 source licence (and the licence of Inter, the font LedgerCraft's text is \
                 set in).",
            ),
        ],
        try_it: &[],
    },
];

/// Which lesson the window is on. Kept for the session, not saved.
#[derive(Default)]
pub(crate) struct Tutorial {
    lesson: usize,
}

/// Draws the Tutorial window. The "Never show again" box edits `prefs` (the
/// app saves them). Returns where a "Try it" button asks to go, if one was
/// clicked.
/// Share of the app window the tutorial takes, and its size limits in points.
const WIDTH_SHARE: f32 = 0.70;
const HEIGHT_SHARE: f32 = 0.75;
const MIN_SIZE: egui::Vec2 = egui::vec2(560.0, 340.0);
const MAX_SIZE: egui::Vec2 = egui::vec2(960.0, 680.0);
/// Height of the bottom row (checkbox, Back, Next) and the line above it.
const FOOTER_HEIGHT: f32 = 40.0;
const LIST_WIDTH: f32 = 190.0;

/// The tutorial's size: a share of the app window, within limits, and never
/// bigger than the window itself, so it always fits with its buttons showing.
fn window_size(screen: egui::Vec2) -> egui::Vec2 {
    let wanted =
        egui::vec2(screen.x * WIDTH_SHARE, screen.y * HEIGHT_SHARE).clamp(MIN_SIZE, MAX_SIZE);
    // Leave room for the title bar and a margin on every side.
    wanted
        .min(screen - egui::vec2(40.0, 80.0))
        .max(egui::vec2(200.0, 160.0))
}

/// Draws the Tutorial window. The "Never show again" box edits `prefs` (the
/// app saves them). Returns where a "Try it" button asks to go, if one was
/// clicked.
pub(crate) fn window(
    ctx: &egui::Context,
    open: &mut bool,
    prefs: &mut Prefs,
    state: &mut Tutorial,
) -> Option<Target> {
    let mut go = None;
    let last = LESSONS.len().saturating_sub(1);
    state.lesson = state.lesson.min(last);
    let screen = ctx.content_rect();
    let size = window_size(screen.size());
    let body_height = (size.y - FOOTER_HEIGHT).max(80.0);
    egui::Window::new("Tutorial")
        .open(open)
        .collapsible(false)
        // A fixed size that follows the app window: everything inside is laid
        // out to it, so nothing can push the window past the screen.
        .fixed_size(size)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(screen.center())
        .constrain(true)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.set_height(body_height);
                lesson_list(ui, state, body_height);
                ui.separator();
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("tutorial_lesson")
                        .max_height(body_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(lesson) = LESSONS.get(state.lesson) {
                                go = lesson_body(ui, lesson);
                            }
                        });
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                let mut never = !prefs.show_tutorial;
                if ui.checkbox(&mut never, "Never show again").changed() {
                    prefs.show_tutorial = !never;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(state.lesson < last, egui::Button::new("Next"))
                        .clicked()
                    {
                        state.lesson += 1;
                    }
                    ui.label(format!("{} of {}", state.lesson + 1, LESSONS.len()));
                    if ui
                        .add_enabled(state.lesson > 0, egui::Button::new("Back"))
                        .clicked()
                    {
                        state.lesson -= 1;
                    }
                });
            });
        });
    go
}

fn lesson_list(ui: &mut egui::Ui, state: &mut Tutorial, height: f32) {
    ui.vertical(|ui| {
        ui.set_width(LIST_WIDTH);
        egui::ScrollArea::vertical()
            .id_salt("tutorial_list")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, lesson) in LESSONS.iter().enumerate() {
                    let label = format!("{}. {}", i + 1, lesson.title);
                    if ui.selectable_label(state.lesson == i, label).clicked() {
                        state.lesson = i;
                    }
                }
            });
    });
}

fn lesson_body(ui: &mut egui::Ui, lesson: &Lesson) -> Option<Target> {
    ui.label(RichText::new(lesson.title).heading().color(theme::ACCENT));
    ui.add_space(6.0);
    for block in lesson.blocks {
        match block {
            Text(t) => {
                ui.label(*t);
            }
            Heading(h) => {
                ui.add_space(4.0);
                ui.label(RichText::new(*h).strong().size(15.0).color(theme::ACCENT));
            }
            Steps(steps) => {
                for (i, step) in steps.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        ui.label(RichText::new(format!("{}.", i + 1)).strong());
                        ui.add(egui::Label::new(*step).wrap());
                    });
                }
            }
            Bullets(items) => {
                for item in items.iter() {
                    ui.horizontal_top(|ui| {
                        ui.label("•");
                        ui.add(egui::Label::new(*item).wrap());
                    });
                }
            }
            Tip(t) => {
                ui.add_space(2.0);
                egui::Frame::NONE
                    .fill(theme::ACCENT_SOFT)
                    .corner_radius(4.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(*t).italics());
                    });
            }
        }
        ui.add_space(4.0);
    }
    let mut go = None;
    if !lesson.try_it.is_empty() {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Try it:").strong());
            for (label, target) in lesson.try_it {
                if ui.button(*label).clicked() {
                    go = Some(*target);
                }
            }
        });
    }
    go
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lesson_has_a_title_and_content() {
        assert!(LESSONS.len() >= 10);
        for lesson in LESSONS {
            assert!(!lesson.title.is_empty());
            assert!(!lesson.blocks.is_empty(), "{} is empty", lesson.title);
        }
    }

    #[test]
    fn the_window_always_fits() {
        let screens = [
            egui::vec2(1280.0, 800.0),
            egui::vec2(900.0, 560.0),
            egui::vec2(3840.0, 2160.0),
            egui::vec2(400.0, 300.0),
        ];
        for screen in screens {
            let size = window_size(screen);
            assert!(
                size.x <= screen.x && size.y <= screen.y,
                "{size:?} does not fit {screen:?}"
            );
            assert!(size.x <= MAX_SIZE.x && size.y <= MAX_SIZE.y);
        }
        // A typical laptop window gets 70% by 75%.
        let laptop = window_size(egui::vec2(1280.0, 800.0));
        assert!(
            (laptop.x - 896.0).abs() < 0.5 && (laptop.y - 600.0).abs() < 0.5,
            "{laptop:?}"
        );
    }

    /// Adding a form without teaching it fails here, as a reminder.
    #[test]
    fn every_form_has_a_try_it_button() {
        let all = [
            FormKind::Invoice,
            FormKind::ReceivePayment,
            FormKind::Bill,
            FormKind::PayBills,
            FormKind::Check,
            FormKind::Deposit,
            FormKind::Journal,
        ];
        for kind in all {
            let taught = LESSONS
                .iter()
                .flat_map(|l| l.try_it.iter())
                .any(|(_, t)| *t == Target::Page(Page::Form(kind)));
            assert!(taught, "{kind:?} has no Try it button in the tutorial");
        }
    }
}
