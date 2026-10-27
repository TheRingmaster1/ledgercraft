# The `.lcbooks` company file

A company file is UTF-8 JSON:

```json
{
  "format": "ledgercraft",
  "version": 1,
  "company": {
    "info": { "name": "Pine Ridge Woodworks", "fiscal_year_start_month": 1, "currency": "USD" },
    "accounts": [ { "id": 1, "number": "1000", "name": "Checking", "kind": "bank", "active": true } ],
    "customers": [ { "id": 34, "contact": { "name": "Dana Lin", "terms_days": 0 }, "active": true } ],
    "vendors": [],
    "items": [],
    "transactions": [
      {
        "id": 50, "date": "2026-01-12", "number": "1001", "memo": "", "voided": false,
        "body": {
          "type": "invoice", "customer": 34, "due_date": "2026-02-11", "ar_account": 3,
          "lines": [ { "item": null, "description": "Table", "qty": 1000, "rate": 240000, "account": 16 } ]
        }
      }
    ],
    "reconciliations": [
      { "account": 1, "statement_date": "2026-01-31", "beginning_balance": 0,
        "ending_balance": 240000, "cleared": [50] }
    ],
    "next_id": 60
  }
}
```

* Amounts (`rate`, `amount`, `debit`, `credit`) are integers in **cents**.
* Quantities (`qty`) are integers in **thousandths** (`1000` = 1).
* Dates are `YYYY-MM-DD`.
* Daily backups are byte-for-byte copies of the file, in a `LedgerCraft
  Backups` folder beside it, named `<company file name> YYYY-MM-DD.lcbooks`.
* An account's `parent` is the account it is a sub-account of (same type);
  left out for a top-level account.
* `recurring` lists recurring transactions: id, `name`, `frequency`
  (`daily`, `weekly`, `monthly`, `quarterly` or `yearly`), `every` (every N
  of those), `start` (the first occurrence), `done` (occurrences entered or
  skipped; the next is counted from `start`), `end` (optional), `mode`
  (`remind` or `automatic`), `active`, and `template`: the transaction's
  `date`, `memo`, `body` and `class`. Left out when there are none.
* `classes` are the transaction classes (departments, locations, lines of
  business): id, `name` and `active`. A transaction's `class` is the id of
  the class it belongs to. Both are left out when there are none, so older
  files open unchanged.
* `closing`, when the books are closed: `date` (transactions dated then or
  before are locked) and, if set, `password`: a one-way Argon2id check value
  (`argon2id$memory$passes$lanes$salt$hash`), never the password itself.
* `audit` is the audit trail, oldest first: `at` (`YYYY-MM-DD HH:MM UTC`),
  `action` and `detail` for every change, undo and redo.
* A customer's `contact.ship_to` and an invoice's `ship_to` are an address
  goods go to when not the customer's own; both are left out when empty.
* `info.print` is how printed documents look: `template` (`classic`,
  `modern` or `simple`) and, if one was chosen, `logo`: the PNG or JPEG file
  as base64 text (1 MB at most). Older files without it print the classic
  look with no logo.
* Ids are unique across the whole file.
* `tax_components` are the sales taxes: id, `name`, `agency`, `code` (a
  state rate file's jurisdiction code, when imported), `rates` (each
  `{from, rate}`, oldest first; a rate is a percentage string such as
  `"7.125"`) and `active`. `tax_groups` are the taxes charged together at a
  location: id, `name`, `components` (ids) and `active`. A customer's `tax`
  holds their `group`, `exempt` and `exemption` note; an item spec's
  `taxable` and an invoice line's `taxable` default to true. An invoice's
  `tax`, when it charges tax, stores what was charged: `group`, the
  liability `account`, and `lines` of `{component, name, rate, taxable,
  amount}`. `tax_rates_checked` is the date the rates were last checked or
  changed (for the quarterly reminder). All are absent in older files.
* `feed_links` records downloaded bank lines already imported (account, the
  bank's id, and the transaction it became or matched); `bank_rules` holds the
  import rules (payee text and category). Both are absent in older files.
* `reconciliations` lists completed bank and credit card reconciliations,
  oldest first, with the transactions each one cleared. Files written before
  reconciliation existed have no such field and open with none; adding it did
  not change the version.
* `version` increases only on incompatible changes; LedgerCraft refuses files
  from a newer version rather than misreading them.
* Saves are atomic: the file is written to `NAME.lcbooks.tmp`, flushed, then
  renamed over the original.

## Password-protected files (version 2)

A company with a password is saved as:

```json
{
  "format": "ledgercraft",
  "version": 2,
  "encryption": {
    "cipher": "xchacha20poly1305",
    "kdf": "argon2id",
    "params": { "memory_kib": 19456, "iterations": 2, "parallelism": 1 },
    "salt": "<16 bytes, base64>",
    "nonce": "<24 bytes, base64>"
  },
  "ciphertext": "<base64>"
}
```

* The key is 32 bytes from **Argon2id** (version 0x13) of the UTF-8 password
  with the salt and the stored parameters.
* The ciphertext is **XChaCha20-Poly1305** of the complete version 1 file
  text (the plain JSON above), with this associated data binding the header:
  `ledgercraft:2:<cipher>:<kdf>:<memory_kib>:<iterations>:<parallelism>:<salt>`.
* A fresh random nonce is used for every save; the salt changes when the
  password does.
* Readers must refuse parameters outside memory 8 KiB to 1 GiB, iterations
  1 to 16 and parallelism 1 to 16, so a damaged or hostile file cannot make
  them hang.
* Only the encrypted form is ever written to disk, including the temporary
  file used for atomic saves. There is no password recovery.

### Upgrading a version 1 file

Version 1 files keep opening. Setting a password (File > Password…, the offer
shown when an unprotected file is opened, or `ledgercraft-cli protect`)
rewrites the same file as version 2 straight away. The books inside are
unchanged: the encrypted payload is the version 1 JSON.

Note that on most drives, especially SSDs, the old unencrypted bytes may
remain in free space until overwritten. For data that must not be recovered,
also delete older plain copies and backups, and rely on full-disk encryption
(BitLocker on Windows) for the free space.
