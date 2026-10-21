# Changelog

All notable changes to LedgerCraft are listed here. Versions follow
`0.MINOR.PATCH` until 1.0: each release the maintainer calls is the next
minor version (0.1, 0.2, 0.3, …); patch versions (0.1.1) are only for urgent
fixes to a release. See docs/releasing.md.

## [Unreleased]

## [0.2.0] - 2026-10-21

Highlights: password-protected, encrypted company files (with an upgrade path
for older files); a start page and Create a Company window with list import
from IIF and CSV; Close Company; a full in-app tutorial; currency symbols and
a Currency preference; Export to Excel; the app icon; Preferences; and many
Windows fixes. Building now needs Rust 1.88.

### Changed
- Building LedgerCraft now needs Rust 1.88 or newer (was 1.82). README lists
  the Windows requirements.

### Fixed
- Tutorial: the window no longer runs off the bottom of the screen and hides
  its Back and Next buttons. It is now 70% of the app window's width and 75%
  of its height (within sensible limits) and follows the window when resized.
- The register's "back to Chart of Accounts" link showed an empty box instead
  of an arrow (egui's font has no arrow glyph).
- Clear on the Create Invoice form no longer closes the app.
- The window can be resized on setups that mix Retina and standard-resolution
  monitors; it no longer opens larger than the screen.
- Home: At A Glance shows its amounts, and no longer overlaps the workflow
  buttons in a narrow window (it moves below them).
- The desktop app builds on Linux (a source file name differed only in case).
- Preferences: Reset To Default resets only the Global settings (row shading
  and Open Menus On Hover); it no longer turns the tutorial back on.

### Added
- Password protection and encryption: every new company file is encrypted
  with a password chosen in Create a Company (Argon2id key derivation and
  XChaCha20-Poly1305 authenticated encryption, file format version 2). The
  file is never written unencrypted, including temporary files during a save.
  Opening a protected file asks for its password; File > Password… sets,
  changes or removes it for the open company. A wrong password or a tampered
  file is detected. Plain (version 1) files still open. The CLI reads the
  password from LEDGERCRAFT_PASSWORD.
- Upgrading older files: opening a company file made before encryption (or
  with its password removed) offers to protect it; File > Password… does the
  same any time. The file is rewritten encrypted in place with its books
  unchanged. `ledgercraft-cli protect FILE` does it from the command line.
- Amounts are shown with a currency symbol everywhere in the app ($1,234.56),
  and Excel exports format them the same way. Help > Preferences has a new
  Currency section (US, Canadian, Australian, New Zealand and Mexican dollars,
  euro, pound, Swiss franc, rand, krona) with a preview. It changes display
  only; nothing is converted. Amount fields also accept a typed symbol or
  code, such as €12.50 or CHF 12.
- Start page: with no company open, LedgerCraft shows its icon and three
  choices (Create a New Company, Open a Company File, Explore the Sample
  Company) plus a link to the tutorial. The app now starts here instead of
  creating an unnamed "My Company".
- Create a Company window: company name (required), legal name, fiscal year
  start month, email, phone and address, and a standard or empty chart of
  accounts; then asks where to save the file. File > New Company opens it.
- Import existing lists when creating a company: IIF list exports (for
  example from QuickBooks Desktop) and CSV files with a header row, for the
  chart of accounts, customers, vendors and products and services. Records
  that cannot be added are skipped and listed in an Import Results window.
  Opening balances and transactions are not imported yet.
- File > Close Company returns to the start page, asking first if there are
  unsaved changes. Menus and shortcuts that need a company are greyed out
  until one is open.
- App icon: an open ledger book on navy, shown in the title bar and taskbar,
  and embedded in ledgercraft.exe for Explorer, the Start menu and shortcuts.
- Tutorial: thirteen lessons covering the whole app as it stands (getting
  around, company files, the chart of accounts, customers, vendors and items,
  invoicing, payments and deposits, bills, checks and card purchases, journal
  entries, fixing mistakes, reports and preferences). A lesson list, Back and
  Next, and "Try it" buttons that open the form or page being taught, or the
  sample company. A test fails if a form has no lesson.
- Help > Preferences: choose the grey used to shade At A Glance rows. Saved
  per user and kept between launches.
- Preferences are grouped under a selectable Global section (styled like the
  selected side bar item) that shows or hides Row Shading; the shading preview
  is boxed.
- Row Shading applies to every striped list in the app (Chart of Accounts,
  customers, vendors, transactions, form lines, registers and reports), not only
  At A Glance.
- All text uses one dark colour across the app (text with its own colour, such
  as the side bar, links, hints and negative amounts, keeps it).
- Preferences: the Row Shading bar no longer shows a number, and its default
  is the middle of the bar (grey 205).
- A Tutorial window (blank for now) opens at launch until "Never show again"
  is ticked; the choice is saved with the preferences. Help > Tutorial opens it
  on demand (Show Tutorial) or turns it back on for the next launch (Show at
  Next Launch).
- Preferences: Open Menus On Hover (off by default) opens File, Edit, View and
  the other top menus when the pointer rests on them instead of on a click.

### Changed
- Home: "At A Glance" heading, with a divider in the side bar colour between
  the workflow and At A Glance.
- Home: every other At A Glance row is shaded grey, and the Workflow and
  At A Glance headings are underlined.
- Home: all At A Glance labels and amounts use the same dark text colour.

## [0.1.0] - unreleased

First public pre-alpha.

### Added
- Exact money (integer cents), quantities and dates.
- Company file model: chart of accounts with 15 account types, customers,
  vendors, products and services.
- Transactions: invoices, receive payments, Undeposited Funds and deposits,
  bills, pay bills, checks and card expenses, journal entries, void.
- Reports: Profit & Loss, Balance Sheet, Trial Balance, A/R and A/P Aging,
  Transaction List, account registers, CSV export.
- Open `.lcbooks` file format with atomic saves; sample company.
- Engine with commands, undo/redo and all-or-nothing execution.
- Desktop app (egui) and `ledgercraft-cli`.
