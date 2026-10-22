# Attribution

Every asset file in this repository has a row here: its path, author, source
and licence. `cargo xtask assets` fails the build when a file under `assets/`
has no row. Adding an asset without a row is a bug.

| Path | Author | Source | Licence |
|------|--------|--------|---------|
| assets/app-icon/ledgercraft-1024.png | LedgerCraft contributors | Original artwork made for LedgerCraft (drafted with an AI image tool, then edited) | MIT OR Apache-2.0 |
| assets/app-icon/ledgercraft-256.png | LedgerCraft contributors | Resized from ledgercraft-1024.png | MIT OR Apache-2.0 |
| assets/app-icon/ledgercraft.ico | LedgerCraft contributors | Windows icon (16-256 px) built from ledgercraft-1024.png | MIT OR Apache-2.0 |
| assets/fonts/Inter-Regular.ttf | Rasmus Andersson and the Inter project authors | Inter 4.1 (font version 4.001), https://github.com/rsms/inter. The Regular face (index 0 of Inter.ttc in the release package), saved on its own as a .ttf with fontTools; outlines and tables unchanged | SIL Open Font License 1.1 |
| assets/fonts/Inter-Light.ttf | Rasmus Andersson and the Inter project authors | Inter 4.1 (font version 4.001), https://github.com/rsms/inter. The Light face (index 6 of Inter.ttc in the release package), saved on its own as a .ttf with fontTools; outlines and tables unchanged | SIL Open Font License 1.1 |
| assets/fonts/OFL.txt | The Inter project authors | LICENSE.txt from the same Inter release package, unchanged: the licence that must accompany the font | SIL Open Font License 1.1 (licence text) |

Notes

* The bundled fonts are Inter Regular and Inter Light (above), unmodified,
  offered in Help > Preferences > Text (Inter Light is the default). Fonts
  listed there under "On this computer" (Segoe UI, Calibri, DejaVu Sans and
  others) are read at run time from where the operating system keeps them;
  LedgerCraft never includes, copies or redistributes them. The SIL
  Open Font License allows bundling it with software under any licence, as
  long as the licence text travels with it (assets/fonts/OFL.txt, also shown
  in Help > Licenses) and the font is not sold on its own. egui's built-in
  fonts remain as a fallback and for monospace.
* PDF invoices name the standard Helvetica fonts that PDF readers supply;
  no font is embedded. The character widths used to lay out the text
  (`crates/reports/src/pdf.rs`) come from Adobe's Core 14 AFM font metrics,
  which Adobe allows to be used, copied and distributed for any purpose.
  The invoice layout is original.
* UI icons, colours and layouts are defined in code (`crates/ui-egui/src/theme.rs`)
  and are original.
* The sample company and default chart of accounts are original text written
  for LedgerCraft (`crates/engine/src/sample.rs`, `crates/ledger/src/template.rs`).
