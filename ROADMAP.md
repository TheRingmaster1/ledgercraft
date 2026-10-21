# LedgerCraft roadmap

Stage: **pre-alpha 0.1** (version 0.1.0; releases go 0.1 → 0.2 → 0.3 …, see docs/releasing.md) — the double-entry core, the everyday customer,
vendor and banking forms, and the core reports. Last updated 2026-10-21.

Estimates are for what a typical small business uses QuickBooks Desktop for
day to day; weighted by depth, LedgerCraft 0.1 covers roughly 20% of it.

## Done (M0–M1)

* M0 Foundation: workspace, layering, lints, xtask gates, CI, licences, clean-room rules
* M0 Core types: exact money and quantities, calendar dates
* M1 Ledger: 15 account types, chart of accounts, customers, vendors, items,
  invoices, payments, bills, bill payments, checks, deposits, journal entries,
  void, validation, open balances
* M1 Reports: P&L, Balance Sheet, Trial Balance, A/R and A/P Aging, Transaction
  List, registers, CSV
* M1 Engine: commands, undo/redo, atomic `.lcbooks` files, sample company
* M1 Apps: desktop app (egui), CLI

## Now (M1.5 — solid on Windows)

Platform focus: **Windows 10/11 x64 only** until this list is done.

1. ~~M1.5.1 Debug and release builds start cleanly on Windows (wgpu/DX12 renderer)~~ (done)
2. M1.5.2 Walk every page and form with the sample company; fix crashes and layout problems
3. M1.5.3 `cargo xtask ci` green on the Windows GitHub runner
4. M1.5.4 Windows niceties
   - ~~App icon~~ (done: open-ledger icon in the title bar, taskbar and .exe)
   - ~~High-DPI check~~ (done: window fits and resizes on mixed-DPI monitors)
   - File association for `.lcbooks`
   - Remembered window size, side bar and last company
   - ~~Start page, Create a Company window and Close Company~~ (done)
   - Recent companies on the start page
   - ~~Import lists (IIF, CSV) when creating a company~~ (done); next: File > Import for an open company, opening balances, transactions
5. M1.5.5 Windows installer (MSI) and portable zip; code signing later

## Next (M2 — usable for a real sole proprietor)

1. M2.1 Edit a posted transaction (today: void and re-enter); open a transaction from any list or report
2. M2.2 Bank reconciliation with cleared status and reconciliation reports
3. M2.3 Bank feed import: OFX/QFX/QBO and CSV, with matching and rules
4. M2.4 Sales tax: tax items, rates, groups, Sales Tax Payable and liability report
5. M2.5 Printable and PDF invoices and statements (original templates)
6. M2.6 Credit memos, refunds, customer credits; sales receipts
7. M2.7 Closing date and password; audit trail report
8. M2.8 Edit and merge list entries; sub-accounts; classes
9. M2.9 Native file dialogs remembered locations, recent files, auto-backup

## Later (M3 — small business)

* Inventory (average cost), purchase orders, estimates → invoices
* Recurring (memorized) transactions and reminders
* Budgets, P&L by class, by customer, by month
* IIF import/export, CSV list import, QuickBooks Online CSV export import
* Multi-currency
* Agent interfaces: JSON control channel and MCP server over the command catalog
* ~~Encryption at rest~~ (done in 0.1: password-protected, encrypted company files); next: lock after inactivity, multi-user via a local server
* Performance: copy-on-write company state, cached balances

## Out of scope for now

* Payroll and tax filing (jurisdiction-specific and regulated; may become a
  separate plug-in project)
* Reading `.QBW` files (proprietary format; see docs/clean-room.md). Migration
  will go through IIF, CSV and report exports.
