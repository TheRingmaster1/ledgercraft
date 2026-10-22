# Architecture

```
apps/ledgercraft  apps/ledgercraft-cli      L5  binaries
        │                 │
crates/ui-egui            │                 L4  desktop UI (swappable)
        └────────┬────────┘
          crates/engine                     L3  Session, Command, undo/redo, sample
          ┌──────┴──────┐
   crates/reports   crates/file             L2  statements; .lcbooks I/O
          └──────┬──────┘
          crates/ledger                     L1  Company, accounts, names, items,
                 │                              transactions, posting, validation
          crates/core                       L0  Money, Qty, Date
```

## Data flow

1. A page or form in `ui-egui` collects text input and, on Save, builds a
   `Command` (e.g. `TxnPost { body: TxnBody::Invoice(..) }`).
2. The app calls `Session::execute`. The session clones the company (undo
   snapshot), applies the command and keeps the result only if it succeeded.
3. `Company::post` validates the form (names exist and are active, account
   types fit, amounts positive, payments do not exceed open balances) and
   checks that its postings sum to zero before recording it.
4. Reports call `Company::ledger_entries()` — the postings of every
   non-voided transaction — and never look at forms directly. Because every
   form becomes balanced postings, the Trial Balance always balances and the
   Balance Sheet always ties out.

## Key types

* `Money` — `i64` cents with checked/saturating arithmetic and a parser for
  `$1,234.50`, `(12.00)` etc.
* `TxnBody` — the forms: `Journal`, `Invoice`, `SalesReceipt`,
  `ReceivePayment`, `CreditMemo`, `Refund`, `Bill`, `PayBill`, `Check`,
  `Deposit`. `TxnBody::postings()` is the single source of truth for what
  each form does to the books; `TxnBody::sale()` gives the lines and tax that
  invoices, sales receipts and credit memos share.
* `Report` — a toolkit-free table (label column + typed cells) rendered by the
  UI, the CLI text printer and CSV export alike.

## Performance notes

Version 0.1 favours obviously-correct code: it recomputes postings and clones
the company for undo. Both are fine for tens of thousands of transactions; the
roadmap has copy-on-write state and cached balances for larger files.
