# LedgerCraft

Small-business accounting: an open-source, clean-room take on the QuickBooks
Desktop workflow, written from scratch in pure Rust.

Invoices, payments, bills, checks, deposits and journal entries post to a real
double-entry ledger; the Profit & Loss, Balance Sheet, Trial Balance and aging
reports read straight from it. Your books live in one open, documented file you
own. No subscription, no licence server, no telemetry.

> **Status: pre-alpha (0.2).** Tested and supported on **Windows 10/11 (x64)**
> and **Linux (x86_64)**. **macOS** is being tested; it should build from
> source, but there is no macOS release package yet.
>
> **Pre-alpha:** Use it to learn, test and contribute. Keep your
> real books in your existing software until LedgerCraft reaches beta.

## Why LedgerCraft

* **Familiar.** The things you do every day work the way you expect:
  the Home workflow (Enter Bills → Pay Bills, Create Invoices → Receive
  Payments → Record Deposits), Write Checks, Make Journal Entry, the Chart of
  Accounts, account registers, Undeposited Funds, and the reports you hand to
  your accountant.
* **Correct by construction.** Money is exact integer cents, never floating
  point. Every form becomes balanced double-entry postings and is validated
  before it touches the books. A failed command changes nothing.
* **Yours.** Company files use an open, documented format
  ([docs/file-format.md](docs/file-format.md)): JSON inside password-based
  encryption, saved atomically so a crash or power cut can never leave half a
  file.
* **Scriptable.** Every change is a command. The desktop app, the CLI and
  (soon) an MCP server for agents all run the same commands.
* **Native and fast.** One Rust codebase for Windows, macOS and Linux. No
  Electron, no web view.

## Quick start

### Requirements

#### Windows 10/11 (x64)

| Requirement | Version | Notes |
|---|---|---|
| [Rust](https://rustup.rs) (stable, MSVC toolchain) | **1.95 or newer** | `rustup update stable` to upgrade; `rustc --version` to check |
| Microsoft C++ Build Tools | 2019 or newer | The "Desktop development with C++" workload; provides the linker and the resource compiler that embeds the app icon |
| Graphics | DirectX 12 (or Vulkan) | Drawn with wgpu. If the window is blank or slow, run `$env:WGPU_BACKEND="vulkan"` first |

#### Linux (x86_64)

| Requirement | Version | Notes |
|---|---|---|
| [Rust](https://rustup.rs) (stable) | **1.95 or newer** | From `rustup`, or your distribution's `rustup` package |
| C compiler and linker | any | `base-devel` (Arch), `build-essential` (Debian/Ubuntu), `gcc` (Fedora) |
| Window system libraries | — | xkbcommon, Wayland and X11 (Xcursor, Xrandr, Xi); see the commands below |
| Graphics | Vulkan | The driver for your graphics card; in a virtual machine, the Mesa software driver (lavapipe). If the window does not appear, try `WGPU_BACKEND=gl` |
| File dialogs | xdg-desktop-portal | Plus the backend for your desktop (GNOME, KDE, GTK for XFCE/Sway/i3, Hyprland). Without it, Open and Save dialogs do not appear |

```sh
# Arch
sudo pacman -S --needed base-devel git rustup libxkbcommon wayland libx11 libxcursor \
  libxrandr libxi vulkan-icd-loader xdg-desktop-portal xdg-desktop-portal-gtk
# plus one of: vulkan-radeon, vulkan-intel, nvidia-utils, vulkan-swrast (virtual machine)

# Debian / Ubuntu
sudo apt install build-essential git curl libxkbcommon-dev libwayland-dev libx11-dev \
  libxcursor-dev libxrandr-dev libxi-dev mesa-vulkan-drivers xdg-desktop-portal xdg-desktop-portal-gtk

# Fedora
sudo dnf install gcc git libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel \
  libXrandr-devel libXi-devel mesa-vulkan-drivers xdg-desktop-portal xdg-desktop-portal-gtk
```

Then install Rust with `rustup default stable` (Arch) or from
[rustup.rs](https://rustup.rs). Preferences are saved in
`~/.config/ledgercraft/`.

#### macOS (in testing)

Rust 1.95 or newer and the Xcode command-line tools (`xcode-select --install`).
Drawn with Metal. Reports from Mac users are welcome.

#### All platforms

Rust 1.95 is the minimum because of the user interface library (egui and
eframe 0.37, which bring font hinting and sharper text); it is set as
`rust-version` in `Cargo.toml`, so an older compiler reports the problem
clearly. The first build downloads and compiles the dependencies and takes a
few minutes; later builds take seconds.

### Build and run

The same commands work in PowerShell (Windows) and a terminal (Linux, macOS):

```sh
git clone https://github.com/TheRingmaster1/ledgercraft
cd ledgercraft
cargo run --release -p ledgercraft -- --sample          # the desktop app with the sample company
cargo run --release -p ledgercraft -- books.lcbooks     # open a company file
cargo test --workspace                                  # the test suite
cargo xtask ci                                          # fmt, clippy, tests, layering, assets
```

Keep the source folder outside OneDrive and other synced folders: syncing the
`target` build folder slows builds and can lock files.

### Installing a release

Download the package for your system from
[Releases](https://github.com/TheRingmaster1/ledgercraft/releases):

* **Windows:** `ledgercraft-<version>-x86_64-pc-windows-msvc.zip`. Unzip it and
  run `ledgercraft.exe`. The app is not code-signed yet, so SmartScreen may ask
  you to confirm (More info → Run anyway). To open company files by
  double-clicking them, use Help > Preferences > Files.
* **Linux:** `ledgercraft-<version>-x86_64-unknown-linux-gnu.tar.gz`. Unpack it
  and run `./install.sh` to add LedgerCraft to your Applications menu for your
  user, and makes double-clicking a `.lcbooks` file open it (no root
  needed; `./install.sh --uninstall` removes it), or run
  `./ledgercraft` directly. It needs the graphics and file dialog packages
  listed above.

Command line:

```sh
ledgercraft-cli new books.lcbooks --name "Acme Repair"
ledgercraft-cli run books.lcbooks --cmd '{"command":"customer.create","contact":{"name":"Dana Lin","terms_days":30}}'
ledgercraft-cli report books.lcbooks pl --from 2026-01-01 --to 2026-12-31
ledgercraft-cli report books.lcbooks balance-sheet --to 2026-12-31 --csv > bs.csv
ledgercraft-cli invoice-pdf books.lcbooks 1001                # writes "Invoice 1001.pdf"
ledgercraft-cli tax-import books.lcbooks --rates UTR2026Q4AUG11.csv \
    --boundary UTB2026Q4AUG11.csv --zip 84601-1100            # Provo: 7.45%
ledgercraft-cli commands                                # every command and its params
```

## What works today

| Area | Highlights |
|------|-----------|
| Company | Start page with recent companies, Create a Company window (standard or empty chart of accounts), sample company, fiscal year start, open/save/save as/close, reopens the last company, double-click a .lcbooks file to open it, unsaved-changes guard, undo/redo (200 steps) |
| Security | Password-protected, encrypted company files with a password strength meter; older files upgrade in place |
| Import | Accounts, customers, vendors and products/services from IIF or CSV when creating a company |
| Lists | Chart of accounts (15 account types, active/inactive), customers, vendors (contact, address, terms, notes) and products and services, all editable and with active/inactive |
| Customers | Create invoices (items, qty × rate, per-line income account, due date from terms, auto-numbering), sales tax (taxes with dated rates, groups, taxable items, exempt customers, rates imported from Streamlined Sales Tax state files, a quarterly reminder to check rates, a Sales Tax Liability report and Pay Sales Tax), save invoices as PDFs to email or print, receive payments applied to open invoices, Undeposited Funds, make deposits |
| Vendors | Enter bills, pay bills applied to open bills |
| Banking | Write checks / expenses (bank or credit card), deposits, journal entries with names on AR/AP lines |
| Reports | Profit & Loss, Balance Sheet (retained earnings rolled forward by fiscal year), Trial Balance, A/R and A/P Aging, Transaction List, account registers; CSV and Excel export, currency symbol of your choice |
| Help | Sixteen-lesson tutorial with "Try it" buttons, hover help, Preferences (row shading, hover menus, currency, text font and size, reopen last company) |
| Banking | Import bank and card downloads (OFX, QFX, QBO, CSV) with matching, categories and rules; reconcile bank and credit card accounts against statements, with a reconciliation report; reconciled items are marked R and protected |
| Editing | Open any recorded transaction from the Transactions page, a register or the Transaction List and change it in its form; payments stay applied, Undo restores the original |
| Safety | Void instead of delete (audit trail), payments block voiding the invoices they pay, all-or-nothing commands, atomic saves |

The honest picture, and what comes next, is in [ROADMAP.md](ROADMAP.md).
Release history is in [CHANGELOG.md](CHANGELOG.md); releases are numbered
0.1, 0.2, 0.3 … and made as described in [docs/releasing.md](docs/releasing.md).

## Security

Company files hold sensitive financial data, so every new company is
protected by a password and **encrypted on disk** with Argon2id and
XChaCha20-Poly1305 (details in [docs/file-format.md](docs/file-format.md)).
Copies and backups stay encrypted; a wrong password or a modified file is
detected. New passwords need at least 12 characters and must pass a strength
check (zxcvbn) that refuses common passwords, names, dates and keyboard runs;
a passphrase of four or more unrelated words is the easy way to a strong one. While a company is open its books are decrypted in the app's memory
only. There is no password recovery, so keep the password in a password
manager.

## Architecture

LedgerCraft is an engine-first Cargo workspace with enforced layering
(`cargo xtask layers`). The egui front end is a separate crate, so the UI can be
swapped without touching the books.

| Layer | Crate | Job |
|------|-------|-----|
| L0 | `ledgercraft-core` | exact money, quantities, dates, errors |
| L1 | `ledgercraft-ledger` | company model, accounts, names, items, transactions, double-entry posting, validation |
| L2 | `ledgercraft-reports`, `ledgercraft-file` | financial statements; the `.lcbooks` file format |
| L3 | `ledgercraft-engine` | sessions, commands, undo/redo, sample company |
| L4 | `ledgercraft-ui-egui` | the desktop interface |
| L5 | `apps/ledgercraft`, `apps/ledgercraft-cli` | desktop app, command line |

Sales tax rate import covers the 24 Streamlined Sales Tax states only, until a
comparable free source for the others exists; see
[docs/sales-tax.md](docs/sales-tax.md).

More in [docs/architecture.md](docs/architecture.md) and
[docs/accounting.md](docs/accounting.md). Contributor and agent rules
(clean-room, never-crash, quality gates) are in [AGENTS.md](AGENTS.md).

## Contributing

Pull requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/clean-room.md](docs/clean-room.md) first: LedgerCraft must stay a clean-room
project, so there are firm rules about what you may look at and copy.

## License and credits

LedgerCraft is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option. Unless you explicitly state
otherwise, any contribution you intentionally submit for inclusion in the work,
as defined in the Apache-2.0 license, shall be dual-licensed as above, without
any additional terms or conditions.

Copyright (c) 2026 Jeffrey Parke and the LedgerCraft contributors. Required
notices are in [NOTICE](NOTICE); bundled assets (none yet) are listed in
[ATTRIBUTION.md](ATTRIBUTION.md).

### Contributors

Thanks to everyone who has contributed to LedgerCraft:

* [@Mr-Sidez](https://github.com/Mr-Sidez)

The project structure and contributor conventions are inspired by the
[ArtCraft "Crafting Apps"](https://github.com/storytold) (clean-room Rust
reimplementations of creative and office tools). LedgerCraft is not part of
ArtCraft and shares no code with it.

QuickBooks and Intuit are trademarks or registered trademarks of Intuit Inc.
LedgerCraft is an independent, open-source project and is not affiliated with,
sponsored by or endorsed by Intuit Inc.; these names are used only to describe
the workflows LedgerCraft is compatible with.

**LedgerCraft is not tax, legal or accounting advice.** Check your books and
filings with a qualified professional.
