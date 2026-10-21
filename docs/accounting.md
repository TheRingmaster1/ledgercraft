# Accounting model

LedgerCraft keeps standard double-entry books. Each transaction produces
postings; a positive amount is a debit, a negative amount a credit, and the
postings of every transaction sum to zero.

| Form | Debit | Credit |
|------|-------|--------|
| Invoice | Accounts Receivable (total), name = customer | each line's income account |
| Receive Payment | Deposit-to account (bank or Undeposited Funds) | Accounts Receivable |
| Make Deposit | Bank (total) | each line's account (often Undeposited Funds) |
| Enter Bill | each line's expense/asset account | Accounts Payable (total), name = vendor |
| Pay Bills | Accounts Payable | Bank or credit card |
| Write Check / Expense | each line's account | Bank or credit card (total) |
| Journal Entry | as entered | as entered (must balance; AR/AP lines need a name) |

## Balances

Assets and expenses are debit-normal; liabilities, equity and income are
credit-normal. Balances are shown in the account's normal sign, so a positive
bank balance is money you have and a positive Accounts Payable is money you owe.

## Open balances and payments

An invoice's open balance is its total less amounts applied to it by
non-voided payments dated on or before the report date. A payment cannot apply
more than is open, and cannot apply more than its own amount; any unapplied
remainder stays on the customer's A/R as a credit.

## Voiding

Voiding keeps the transaction on file for the audit trail and removes all of
its postings. An invoice or bill with payments applied cannot be voided until
those payments are voided.

## Editing

A recorded transaction can be edited (`txn.update`; in the app, open it from
the Transactions page, a register or the Transaction List). The edit replaces
the transaction's date, number, memo and form contents in place and keeps its
id, so payments applied to an invoice or bill stay applied; every report reads
the new postings straight away. An edit is validated exactly like a new
transaction, with these extra rules:

* It stays the same type of transaction (an invoice stays an invoice).
* A void transaction cannot be edited.
* An invoice or bill cannot drop below the total that live payments apply to
  it, and cannot change customer or vendor while any payment is applied.
* When a payment is edited, its own earlier applications are left out of the
  open balances it is checked against, so it can apply the same amounts again.

Undo restores the transaction as it was. There is no change history in the
file yet; an audit trail report is planned (ROADMAP M2.7).

## Reconciliation

A reconciliation compares a bank or credit card account with its statement.
Amounts use the account's own sign: for a bank, money in is positive and
money out negative; for a credit card, charges are positive and payments
negative, so the ending balance is what the statement says you owe.

* The beginning balance is the previous reconciliation's ending balance
  (zero for the first).
* Cleared balance = beginning balance + the ticked items. A reconciliation is
  recorded only when it equals the statement's ending balance exactly.
* Ticked items must be live, touch the account, not be reconciled already, and
  be dated on or before the statement date, which cannot be earlier than the
  last reconciliation's.
* Reconciled transactions are protected: they cannot be voided, and an edit
  cannot change what they move the reconciled account by (other details,
  such as the memo, can change). Undo Last Reconciliation removes the most
  recent reconciliation of an account and frees its items.

## Balance Sheet equity

Income and expense from before the start of the current fiscal year are rolled
into Retained Earnings; activity since then is shown as Net Income.
