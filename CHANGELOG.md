# Changelog

All notable changes to LedgerCraft are listed here. Versions follow
`0.MINOR.PATCH` until 1.0: each release the maintainer calls is the next
minor version (0.1, 0.2, 0.3, …); patch versions (0.1.1) are only for urgent
fixes to a release. See docs/releasing.md.

## [Unreleased]

### Fixed
- Clear on the Create Invoice form no longer closes the app.
- The window can be resized on setups that mix Retina and standard-resolution
  monitors; it no longer opens larger than the screen.
- Home: At A Glance shows its amounts, and no longer overlaps the workflow
  buttons in a narrow window (it moves below them).
- The desktop app builds on Linux (a source file name differed only in case).
- Preferences: Reset To Default under Row Shading resets only the shading; it
  no longer turns the tutorial back on.

### Added
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
