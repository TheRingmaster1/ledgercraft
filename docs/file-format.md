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
    "next_id": 60
  }
}
```

* Amounts (`rate`, `amount`, `debit`, `credit`) are integers in **cents**.
* Quantities (`qty`) are integers in **thousandths** (`1000` = 1).
* Dates are `YYYY-MM-DD`.
* Ids are unique across the whole file.
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
