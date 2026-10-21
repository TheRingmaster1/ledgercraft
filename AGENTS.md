# LedgerCraft — instructions for contributors and agents

LedgerCraft is a clean-room, open-source, Rust-native accounting app targeting
the QuickBooks Desktop workflow, and better at it where we can be (speed,
openness, scriptability). It runs natively on Windows, macOS and Linux.

## Start every session here

Read ROADMAP.md (current milestone and next task) and the relevant section of
docs/architecture.md. Accounting behaviour is specified in docs/accounting.md.

## Clean-room — absolute rule

Read docs/clean-room.md. In short:

* Never decompile, disassemble, debug or read the files inside any Intuit
  product, and never reverse-engineer the proprietary `.QBW`/`.QBB` company
  file formats.
* Never copy Intuit's icons, artwork, screen layouts pixel-for-pixel, help text,
  templates, sample companies, chart-of-accounts wording or marketing copy.
  Generic feature names ("Invoice", "Pay Bills", "Undeposited Funds", "Profit &
  Loss") are fine.
* Behaviour comes from public documentation, accounting textbooks, published
  standards and black-box observation of the running product with synthetic
  data. Observation notes and screenshots stay out of the repo (`plan/` is
  gitignored).
* Interoperability is through openly documented formats only: CSV, OFX/QFX/QBO
  (OFX is an open spec), IIF (documented by its vendor for import), and our own
  `.lcbooks`.
* Never copy GPL/AGPL/LGPL code (GnuCash, KMyMoney, LedgerSMB, Odoo…) into this
  MIT/Apache repository. Reading their public docs for accounting concepts is fine.

## Assets

Every asset is original, public domain / CC0, or openly licensed for
redistribution, and has a row in ATTRIBUTION.md. `cargo xtask assets` enforces
it. Prefer art drawn in code.

## Never crash, never lose money

People trust LedgerCraft with their money; a crash or a rounding error loses
their trust for good. This outranks feature work.

* No panics in non-test code: no `unwrap()`, `expect()`, `panic!`,
  `unreachable!`, `todo!`, `unimplemented!` (denied by workspace lints). No
  `unsafe` (`unsafe_code = "forbid"`).
* Money is `Money` (integer cents) and quantities are `Qty` (thousandths).
  Never use `f32`/`f64` for amounts. Use checked or saturating arithmetic.
* Input is hostile (files, CLI, commands): use `get()` instead of indexing, cap
  sizes, return `Result` with a message a bookkeeper can understand.
* Every transaction must post balanced double entries. `Company::validate`
  checks this for every form; keep it that way.
* Last-resort guard: `Session::execute` restores the books if a command fails
  or panics. It is a safety net, not a licence.
* Every bug fix lands with a regression test.

## Everything is a command

User-visible changes to the books are `ledgercraft_engine::Command` variants
with a stable id (`account.create`, `txn.post`, `txn.update`, …) listed in `CATALOG`. The
desktop UI, the CLI and future agent interfaces reach the books only through
`Session::execute`. UI-only state (open page, half-typed forms) lives in
`crates/ui-egui`.

## Layering

`core` (L0) → `ledger` (L1) → `reports`, `file` (L2) → `engine` (L3) →
`ui-egui` (L4) → `apps/*` (L5). Nothing below L4 depends on egui, eframe,
winit or rfd. `cargo xtask layers` enforces this.

## Quality gates before every commit

`cargo xtask ci`: rustfmt check, clippy with `-D warnings`, all tests, layers,
assets. One task per commit, with the milestone id in the message
(`M2.3: bank reconciliation`).

## Roadmap

Update ROADMAP.md when work lands.
