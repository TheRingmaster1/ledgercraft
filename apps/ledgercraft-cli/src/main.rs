//! `ledgercraft-cli`: drive a company file without the GUI. Every change goes
//! through the same engine commands the desktop app uses.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ledgercraft_core::Date;
use ledgercraft_engine::{new_company, sample_company, Session, CATALOG};
use ledgercraft_reports::ReportKind;

const USAGE: &str = "\
usage:
  ledgercraft-cli new FILE [--name NAME] [--sample]
  ledgercraft-cli run FILE --cmd JSON [--cmd JSON ...]
  ledgercraft-cli report FILE KIND [--from DATE] [--to DATE] [--csv]
  ledgercraft-cli accounts FILE
  ledgercraft-cli invoice-pdf FILE NUMBER [--out PDF]
  ledgercraft-cli tax-import FILE --rates CSV --boundary CSV --zip ZIP[-PLUS4] [--location CODE] [--date DATE]
  ledgercraft-cli protect FILE         (password from LEDGERCRAFT_NEW_PASSWORD)
  ledgercraft-cli commands
  ledgercraft-cli --version

report KIND is one of: pl, balance-sheet, trial-balance, ar-aging, ap-aging, transactions
dates are YYYY-MM-DD; --to is also the \"as of\" date
invoice-pdf saves invoice NUMBER as a PDF (default: \"Invoice NUMBER.pdf\")
tax-import reads a Streamlined Sales Tax rate and boundary file (streamlinedsalestax.org)
  and adds or updates the sales taxes for a ZIP+4, grouped; a bare ZIP lists its locations

example:
  ledgercraft-cli run books.lcbooks --cmd '{\"command\":\"account.create\",\"number\":\"6700\",\"name\":\"Postage\",\"kind\":\"expense\"}'";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ledgercraft-cli: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The symbol amounts are printed with, from the company's currency code.
fn currency_symbol(code: &str) -> &'static str {
    match code.trim().to_ascii_uppercase().as_str() {
        "USD" | "CAD" | "AUD" | "NZD" | "MXN" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "CHF" => "CHF ",
        "ZAR" => "R ",
        "SEK" => "SEK ",
        _ => "",
    }
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(sub) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    let rest = args.get(1..).unwrap_or_default();
    match sub.as_str() {
        "--version" | "-V" => {
            println!("ledgercraft-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        "commands" => {
            for c in CATALOG {
                println!("{:<28} {}", c.id, c.summary);
            }
            Ok(())
        }
        "new" => {
            let file = rest.first().ok_or("new: which file?")?;
            let path = ledgercraft_file::with_extension(Path::new(file));
            if path.exists() {
                return Err(format!(
                    "{} already exists; refusing to overwrite it",
                    path.display()
                ));
            }
            let company = if rest.iter().any(|a| a == "--sample") {
                sample_company().map_err(|e| e.to_string())?
            } else {
                let name = flag_value(rest, "--name").unwrap_or_else(|| "My Company".into());
                new_company(&name)
            };
            // A new file is protected when LEDGERCRAFT_PASSWORD is set.
            let protection = match password_from_env() {
                Some(p) => Some(ledgercraft_file::Protection::new(&p).map_err(|e| e.to_string())?),
                None => None,
            };
            ledgercraft_file::save(&path, &company, protection.as_ref())
                .map_err(|e| e.to_string())?;
            println!("created {}", path.display());
            Ok(())
        }
        "run" => {
            let file = rest.first().ok_or("run: which file?")?;
            let mut session = open(file)?;
            let mut ran = 0;
            let mut i = 1;
            while let Some(arg) = rest.get(i) {
                if arg != "--cmd" {
                    return Err(format!("unexpected argument {arg:?}"));
                }
                let json = rest.get(i + 1).ok_or("--cmd needs a JSON command")?;
                let outcome = session.execute_json(json).map_err(|e| e.to_string())?;
                match outcome.created {
                    Some(id) => println!("{}: created {id}", outcome.command),
                    None => println!("{}: done", outcome.command),
                }
                ran += 1;
                i += 2;
            }
            if ran == 0 {
                return Err("run: give at least one --cmd".into());
            }
            // All-or-nothing: we only get here if every command succeeded,
            // so a failing command never leaves the file half-changed.
            session.save().map_err(|e| e.to_string())
        }
        "report" => {
            let file = rest.first().ok_or("report: which file?")?;
            let kind_id = rest.get(1).ok_or("report: which report?")?;
            let kind = ReportKind::from_cli_id(kind_id)
                .ok_or_else(|| format!("unknown report {kind_id:?}"))?;
            let session = open(file)?;
            let to = match flag_value(rest, "--to") {
                Some(s) => Date::parse(&s).map_err(|e| e.to_string())?,
                None => Date::today(),
            };
            let from = match flag_value(rest, "--from") {
                Some(s) => Date::parse(&s).map_err(|e| e.to_string())?,
                None => to.fiscal_year_start(session.company().info.fiscal_year_start_month),
            };
            let report = kind.build(session.company(), from, to);
            if rest.iter().any(|a| a == "--csv") {
                print!("{}", report.to_csv());
            } else {
                print!("{}", report.to_text());
            }
            Ok(())
        }
        "protect" => {
            // Encrypts a company file with a new password, in place. Upgrades
            // files made before encryption; also changes the password of a
            // protected file (opened with LEDGERCRAFT_PASSWORD).
            let file = rest.first().ok_or("protect: which file?")?;
            let new = std::env::var("LEDGERCRAFT_NEW_PASSWORD")
                .ok()
                .filter(|p| !p.is_empty())
                .ok_or("protect: set LEDGERCRAFT_NEW_PASSWORD to the new password")?;
            let mut session = open(file)?;
            session
                .set_password(Some(&new))
                .map_err(|e| e.to_string())?;
            println!("{file}: protected and encrypted");
            Ok(())
        }
        "accounts" => {
            let file = rest.first().ok_or("accounts: which file?")?;
            let session = open(file)?;
            let c = session.company();
            for a in c.sorted_accounts() {
                println!(
                    "{:<6} {:<6} {:<32} {:<24} {:>16}",
                    a.id.to_string(),
                    a.number,
                    a.name,
                    a.kind.label(),
                    c.account_balance(a.id, None).to_string()
                );
            }
            Ok(())
        }
        "invoice-pdf" => {
            let file = rest.first().ok_or("invoice-pdf: which file?")?;
            let number = rest.get(1).ok_or("invoice-pdf: which invoice number?")?;
            let session = open(file)?;
            let c = session.company();
            let invoice = c
                .transactions
                .iter()
                .find(|t| {
                    matches!(t.body, ledgercraft_ledger::TxnBody::Invoice(_))
                        && t.number.trim() == number.trim()
                })
                .ok_or_else(|| format!("invoice-pdf: no invoice numbered {number:?}"))?;
            let bytes =
                ledgercraft_reports::invoice_pdf(c, invoice.id, currency_symbol(&c.info.currency))?;
            let out = flag_value(rest, "--out").map_or_else(
                || PathBuf::from(ledgercraft_reports::invoice_pdf_name(c, invoice.id)),
                PathBuf::from,
            );
            std::fs::write(&out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
            println!("{}", out.display());
            Ok(())
        }
        "tax-import" => {
            use ledgercraft_file::sst;
            let file = rest.first().ok_or("tax-import: which company file?")?;
            let need = |flag: &str| {
                flag_value(rest, flag).ok_or_else(|| format!("tax-import: give {flag}"))
            };
            let rates_path = need("--rates")?;
            let boundary_path = need("--boundary")?;
            let zip = need("--zip")?;
            let date = match flag_value(rest, "--date") {
                Some(s) => Date::parse(&s).map_err(|e| e.to_string())?,
                None => Date::today(),
            };
            let text =
                std::fs::read_to_string(&rates_path).map_err(|e| format!("{rates_path}: {e}"))?;
            let rates = sst::read_rates(&text)?;
            let boundary =
                std::fs::File::open(&boundary_path).map_err(|e| format!("{boundary_path}: {e}"))?;
            let found = sst::locations(std::io::BufReader::new(boundary), &zip, date)?;
            let chosen = match flag_value(rest, "--location") {
                Some(code) => found
                    .iter()
                    .find(|l| l.location_code == code.replace('-', ""))
                    .ok_or_else(|| format!("tax-import: {zip} has no location {code}"))?,
                None if found.len() == 1 => &found[0],
                None => {
                    println!("{zip} has more than one tax location on {date}:");
                    for l in &found {
                        let rate = sst::plan(&rates, l, date)
                            .map_or_else(|e| e, |p| p.total_rate().to_string());
                        println!(
                            "  location {:<8} {rate:>8}   ({} address ranges)",
                            l.location_code, l.rows
                        );
                    }
                    return Err("tax-import: give a ZIP+4, or --location CODE".into());
                }
            };
            let plan = sst::plan(&rates, chosen, date)?;
            println!("{}: {} on {date}", plan.group_name, plan.total_rate());
            for t in &plan.taxes {
                println!("  {:<24} {:>8}   {}", t.name, t.rate.to_string(), t.code);
            }
            let taxes = plan
                .taxes
                .iter()
                .map(|t| ledgercraft_ledger::ImportedTax {
                    code: t.code.clone(),
                    name: t.name.clone(),
                    agency: t.agency.clone(),
                    rates: t.rates.clone(),
                })
                .collect();
            let mut session = open(file)?;
            session
                .execute(ledgercraft_engine::Command::TaxImport {
                    taxes,
                    group: plan.group_name.clone(),
                    on: Date::today(),
                })
                .map_err(|e| e.to_string())?;
            session.save().map_err(|e| e.to_string())
        }
        other => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    }
}

/// Password-protected files are opened with the password in the
/// LEDGERCRAFT_PASSWORD environment variable (never a command-line argument,
/// which other users and shell history can see).
fn open(file: &str) -> Result<Session, String> {
    let path = PathBuf::from(file);
    let opened = match password_from_env() {
        Some(p) => Session::open_with_password(&path, &p),
        None => Session::open(&path),
    };
    opened.map_err(|e| match e {
        ledgercraft_engine::EngineError::File(ledgercraft_file::FileError::PasswordRequired) => {
            format!(
                "{}: this company file is password protected; set LEDGERCRAFT_PASSWORD",
                path.display()
            )
        }
        e => format!("{}: {e}", path.display()),
    })
}

fn password_from_env() -> Option<String> {
    std::env::var("LEDGERCRAFT_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty())
}
