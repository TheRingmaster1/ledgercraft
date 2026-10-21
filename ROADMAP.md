# LedgerCraft roadmap

Stage: **pre-alpha 0.2** (version 0.2.0, released 2026-10-21; releases go
0.1 → 0.2 → 0.3 …, see docs/releasing.md) — the double-entry core, the everyday
customer, vendor and banking forms, the core reports, encrypted company files
and a first-run experience. Last updated 2026-10-21.

Platforms: **Windows 10/11 x64 and Linux x86_64** are tested and supported
(Linux first tested on Arch, XFCE/X11, in a virtual machine). **macOS** is in
testing.

Estimates are for what a typical small business uses QuickBooks Desktop for
day to day; weighted by depth, LedgerCraft 0.2 covers roughly 25% of it.

## Done

### M0–M1 (0.1)

* M0 Foundation: workspace, layering, lints, xtask gates, CI, licences, clean-room rules
* M0 Core types: exact money and quantities, calendar dates
* M1 Ledger: 15 account types, chart of accounts, customers, vendors, items,
  invoices, payments, bills, bill payments, checks, credit card charges,
  deposits, journal entries, void, validation, open balances
* M1 Reports: P&L, Balance Sheet, Trial Balance, A/R and A/P Aging, Transaction
  List, registers, CSV export (UTF-8 with BOM for Excel), Export to Excel (.xlsx)
* M1 Engine: commands, undo/redo, atomic `.lcbooks` files, sample company
* M1 Apps: desktop app (egui, wgpu renderer), CLI
* Hover help on fields and buttons, hideable side bar (Ctrl+B), Help > Licenses

### M1.5 so far (0.2)

* Start page when no company is open: Create a New Company, Open a Company
  File, Explore the Sample Company, and a tutorial link
* Create a Company window (name, legal name, fiscal year start, contact
  details, standard or empty chart of accounts) and File > Close Company
* Import lists from IIF and CSV when creating a company (accounts, customers,
  vendors, products and services) with an Import Results window
* Password-protected, encrypted company files (Argon2id + XChaCha20-Poly1305,
  file format version 2); unlock prompt; File > Password…; older files
  upgrade in place; `ledgercraft-cli protect`
* Currency symbol on every amount, Currency section in Preferences, symbols in
  Excel exports
* Thirteen-lesson in-app tutorial with "Try it" buttons
* Help > Preferences: Row Shading, Open Menus On Hover, Currency
* App icon (title bar, taskbar, .exe)
* Windows fixes: wgpu/DX12 renderer, mixed-DPI resizing, OneDrive save retry,
  Home layout, Clear on Create Invoice

### After 0.2 (unreleased)

* Linux tested and supported: CI runs on Linux, releases include a Linux
  package with a desktop entry and icon
* Recent companies (start page and File > Open Previous Company), reopen the
  last company at startup, remembered window size and side bar
* Password strength meter (zxcvbn) and a 12-character minimum
* Double-click a `.lcbooks` file to open it (Windows: Preferences > Files;
  Linux: install.sh)
* Edit recorded transactions in their forms (M2.1)
* Bank and credit card reconciliation with reports (M2.2)

## Now (M1.5: solid on Windows, Linux and macOS)

1. ~~M1.5.1 Debug and release builds start cleanly on Windows (wgpu/DX12 renderer)~~ (done)
2. M1.5.2 Walk every page and form with the sample company; fix crashes and layout problems
   (most done in 0.2; keep going for each new form)
3. M1.5.3 ~~`cargo xtask ci` green on Windows and Linux runners~~ (done); macOS runner after testing
4. M1.5.4 Desktop niceties
   - ~~App icon~~ (done: open-ledger icon in the title bar, taskbar and .exe; Linux desktop entry)
   - ~~High-DPI check~~ (done: window fits and resizes on mixed-DPI monitors)
   - ~~Start page, Create a Company window and Close Company~~ (done)
   - ~~Import lists (IIF, CSV) when creating a company~~ (done); next: File > Import
     for an open company, opening balances, transactions
   - ~~Password protection and encryption at rest~~ (done, with upgrade of older files)
   - ~~File association for `.lcbooks`~~ (done: Windows from Preferences, Linux from install.sh)
   - ~~Remembered window size, side bar and last company~~ (done)
   - ~~Recent companies on the start page~~ (done, also File > Open Previous Company)
   - A clear message when the system file dialog cannot open (Linux without an
     xdg-desktop-portal backend)
5. M1.5.5 Packaging: Windows installer (MSI) and portable zip; Linux AppImage and
   an AUR PKGBUILD (deb/rpm later); code signing later
6. M1.5.6 macOS: test results from a Mac, then CI, a universal `LedgerCraft.app`
   in a `.dmg`, and Cmd shortcuts; notarization later

## Security hardening (from the 0.2 review)

1. ~~Password strength meter on every password field; refuse common and easily
   guessed passwords, not only short ones (`zxcvbn`, MIT)~~ (done)
2. ~~Raise the minimum password length from 8 to 12 and suggest passphrases in the hints~~ (done)
3. Stronger Argon2id settings for new and re-saved files (e.g. 64 MiB, 3 passes);
   existing files keep the settings stored in their header and still open
4. Lock the open company after a period of inactivity (re-enter the password)
5. File names are not encrypted: say so in the docs and Create a Company, and
   offer a neutral file name when the company name is sensitive

## Next (M2: usable for a real sole proprietor)

1. ~~M2.1 Edit a posted transaction; open a transaction from any list or report~~ (done: Transactions page, registers and the Transaction List report; `txn.update`)
2. ~~M2.2 Bank reconciliation with cleared status and reconciliation reports~~ (done: Banking > Reconcile, `account.reconcile`)
3. M2.3 Bank feed import: OFX/QFX/QBO and CSV, with matching and rules
4. M2.4 Sales tax: tax items, rates, groups, Sales Tax Payable and liability report
5. M2.5 Printable and PDF invoices and statements (original templates)
6. M2.6 Credit memos, refunds, customer credits; sales receipts
7. M2.7 Closing date and password; audit trail report
8. M2.8 Edit and merge list entries; sub-accounts; classes
9. M2.9 Native file dialogs that remember locations, recent files, auto-backup

## Later (M3: small business)

* Inventory (average cost), purchase orders, estimates → invoices
* Recurring (memorized) transactions and reminders
* Budgets; P&L by class, by customer, by month
* IIF export; QuickBooks Online CSV export import (IIF/CSV list import done in 0.2)
* Multi-currency transactions and exchange rates (display currency done in 0.2;
  decide whether the currency is stored per company)
* Agent interfaces: JSON control channel and MCP server over the command catalog
* ~~Encryption at rest~~ (done in 0.2); next: multi-user access through a local server
* Performance: copy-on-write company state, cached balances
* Dark theme: a Light / Dark / Follow the system choice in Preferences > General,
  with dark versions of the palette in theme.rs (the side bar, lists, row
  shading and amounts)

## Out of scope for now

* Payroll and tax filing (jurisdiction-specific and regulated; may become a
  separate plug-in project)
* Reading `.QBW` files (proprietary format; see docs/clean-room.md). Migration
  will go through IIF, CSV and report exports.
