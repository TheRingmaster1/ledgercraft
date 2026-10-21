# Changelog

All notable changes to LedgerCraft are listed here. Versions follow
`0.MINOR.PATCH` until 1.0: each release the maintainer calls is the next
minor version (0.1, 0.2, 0.3, …); patch versions (0.1.1) are only for urgent
fixes to a release. See docs/releasing.md.

## [Unreleased]

### Added
- Bank feed import (Banking > Import Bank Transactions). Read a download from
  the bank or card website: OFX (SGML and XML), QFX, QBO or CSV (columns found
  by name: Date, Description/Payee, Amount or Debit/Credit, Check number).
  Each line is matched to a transaction already recorded (same amount within
  10 days, an equal check number first), added as a new transaction in a
  chosen category (a deposit, a check or card charge, or a card payment), or
  skipped. One Import brings everything in and one Undo takes it out. Lines
  imported before (by the bank's id) are never imported twice. Remember
  creates a rule (payee text to category), listed with Delete. Afterwards,
  Reconcile with This Statement opens Reconcile with the download's date and
  ending balance. New engine commands `bank.import` and `bank.removeRule`; a
  sample download is in docs/samples; a new tutorial lesson explains it.
  Undoing (or redoing) an import puts the page back in step: the download's
  lines show again, ready to import.
- Bank and credit card reconciliation (Banking > Reconcile, the side bar, or
  the Reconcile link on a bank or credit card register). Enter the statement
  date and ending balance, tick the items on the statement in two lists
  (money in, money out; Mark All / Unmark All), and Reconcile Now becomes
  available when the Difference is $0.00. A reconciliation report (beginning
  balance, cleared items, cleared balance, items still uncleared) opens in
  its own window afterwards, with Export to Excel and Export CSV; earlier ones
  reopen in the same window from Previous reconciliations. A line under the
  summary says what to do next at each step (enter the statement's ending
  balance, tick the items on the statement, then Reconcile Now).
  Registers show R beside reconciled items. A reconciled transaction cannot be
  voided or have its amount in that account changed until Undo Last
  Reconciliation. New engine commands `account.reconcile` and
  `account.undoReconciliation`; company files keep their reconciliations
  (older files open with none). A new tutorial lesson explains it.

### Changed
- Nothing is left off screen in a small window. Every page scrolls sideways
  as well as down when it has to; toolbars, button rows and memo and total
  lines wrap onto the next line; Reconcile stacks its two lists when the
  window is narrow; and Import Bank Transactions keeps each line on one row,
  with narrower Action and Category boxes, moving the category under Add
  only when the window is too narrow for it.
- The user interface library is upgraded from egui/eframe 0.29 to 0.37. Text
  is now drawn with font hinting (letters fitted to the screen's pixels), so
  it is noticeably sharper in every font, and many egui fixes come with it.
  Building now needs Rust 1.95 or newer (was 1.88). Menus close when an item
  is chosen; Open Menus On Hover opens the hovered menu and closes it like
  any menu, by choosing an item or clicking elsewhere. The side bar can also
  be dragged open or shut.

### Added
- Help > Preferences > Text > Hinting: sharpen letters (on, the default) or
  turn it off for smoother, more print-like text.
- Edit recorded transactions. Open an invoice, payment, bill, bill payment,
  check, deposit or journal entry from the Transactions page (Edit, or
  double-click the row), an account register or the Transaction List report
  (click the date, or double-click the row). It opens in its form titled
  "Edit …"; Save Changes replaces it in place and returns to where it was
  opened from, Cancel leaves it unchanged, and Edit > Undo puts the original
  back. The transaction keeps its id, so payments stay applied. Edits are
  checked like new entries, an invoice or bill cannot drop below what has
  been paid on it or change customer or vendor while payments are applied,
  the type cannot change, and void transactions cannot be edited. The CLI and
  agents get the same as the `txn.update` command.
- Recent companies: the start page lists the company files opened last
  (click to open, right-click to remove one), and File > Open Previous
  Company lists them too. A file that has been moved or deleted is reported
  and dropped from the list.
- LedgerCraft reopens the company that was open last when it starts (asking
  for its password if it has one). Help > Preferences > Open The Last Company
  At Startup turns this off.
- The window size, maximized state and side bar are remembered between
  launches.
- Show / Hide button beside the password boxes (Create a Company, File >
  Password… and the password prompt when opening a protected company), to
  check what was typed. Every window opens with passwords hidden.
- The Confirm password box says "Doesn't match" (red) or "Matches" (green)
  to its right as you type, in Create a Company and File > Password….
- Password strength meter on every new password (Create a Company and File >
  Password…), using zxcvbn. Passwords that are easy to guess (common
  passwords, names, dates, keyboard runs, repeated characters, or the
  company's own name) are refused, with a passphrase tip.
- Double-clicking a .lcbooks file opens it in LedgerCraft. Windows: Help >
  Preferences > Files registers the file type for your account (no
  administrator rights needed). Linux: install.sh registers it, with the
  LedgerCraft icon on company files.
- Linux is tested and supported (x86_64; first run on Arch Linux with XFCE).
  CI now runs every gate on Linux as well as Windows, and releases include
  `ledgercraft-<version>-x86_64-unknown-linux-gnu.tar.gz` with an `install.sh`
  that adds LedgerCraft to the Applications menu with its icon for the
  current user.
- The window identifies itself as `ledgercraft` to Linux desktops, so the
  taskbar and dock match it to the menu entry and icon (including Wayland).

### Fixed
- The app no longer stops at start-up on Windows with "wgpu-hal WGL Instance
  Thread has overflowed its stack": wgpu's OpenGL fallback is not started;
  LedgerCraft draws with DirectX 12, Vulkan or Metal (WGPU_BACKEND=gl still
  selects OpenGL where needed).
- Entry boxes are easy to see: white with a thin grey outline (the accent
  outline when focused), instead of near-white boxes with no edge that all
  but vanished on unshaded rows. Buttons and drop-downs get the same outline.
- Form lines grow as you type: Create Invoice, Enter Bill, Write Check,
  Make Deposit and Journal Entry start with one empty line (two for a journal
  entry) and add the next as soon as you type in the last, always keeping one
  empty line ready. The "+ Add line" buttons are gone.
- Create Invoice: each line's Amount (Qty x Rate) shows from the start
  ($0.00 on a blank line) instead of staying empty until a rate was typed,
  which made the column look missing. Text that is not a number shows
  "check qty / rate".
- Form lines you type into (invoice, bill, check, deposit and journal entry
  lines) are no longer shaded in alternating grey bars; Row Shading still
  applies to lists, registers and reports, where it helps reading.
- The status bar no longer overlaps itself in a narrow window. Messages on
  the left clear themselves after a few seconds (errors after 20), the right
  side shows the company's file name rather than its whole path (hover for
  the full path), and both shorten with "…" instead of running into each
  other.
- The password meter applies the 12-character minimum first: a short
  password shows "Too short: 9 of 12 characters" instead of "Good" (it
  showed "Good" for "beautiful", which Create then refused).
- Create a Company no longer runs off the bottom of a small LedgerCraft
  window. The company details are on the left and the password and imports on
  the right; if the window is still too short, the fields scroll while the
  Create Company and Cancel buttons stay in view. (A window too narrow for two
  columns gets one scrolling column.)

### Changed
- Help > Preferences is a fixed-size window with its pages listed on the left
  (General, Currency, Text, Files, styled like the side bar) and the chosen
  page's settings on the right, instead of one column that grew with every
  new setting. "Global" is now called General. Its contents fit the window
  exactly, so the title bar spans the whole window.
- Help > Preferences > Text: choose the font and the size (Standard, Larger,
  Largest) for the whole app, with a preview. Included with LedgerCraft:
  Classic (the egui font 0.2 used), Inter and Inter Light, the new default.
  Also offered when installed on the computer, read from where the system
  keeps them and never bundled: Segoe UI, Segoe UI Semilight, Segoe UI Light,
  Calibri, Calibri Light, Verdana, Tahoma and Arial on Windows; DejaVu Sans,
  Noto Sans, Liberation Sans, Ubuntu and Cantarell on Linux. A font that
  cannot be found or read falls back to Classic instead of stopping the app.
  Fonts with embedded bitmap or colour glyphs (such as Windows' Calibri) have
  those glyphs removed when loaded, so egui draws their outlines in the text
  colour: egui 0.37 drew the embedded glyphs in their own colour, so letters
  came out white or missing.
  Each font's name in the list is shown in that font. A Darkness slider
  (Darker to Softer, with Default) softens ordinary text from black towards
  dark grey, which makes heavy fonts such as Verdana, Tahoma and Arial look
  lighter; egui draws text without hinting, so this is the adjustment it
  offers. The default stays black. Inter is bundled under
  the SIL Open Font License; Help > Licenses has a new Inter Font tab with its
  licence.
- Tutorial brought up to date: opening and changing recorded transactions
  (Fixing mistakes, Working with accounts, Reports), status bar messages that
  clear themselves and the file name on the right (Getting around), and the
  Show / Hide and Matches / Doesn't match password helpers (Company files).
  docs/accounting.md explains editing and its rules.
- New passwords need at least 12 characters (was 8). Files protected with an
  older, shorter password still open; only new passwords are checked.
- README: requirements for Linux (Arch, Debian/Ubuntu and Fedora packages)
  and macOS (in testing), how to install a release, and an updated "What works
  today" table. ROADMAP updated for 0.2 and the new platforms, with a
  security hardening list.

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
