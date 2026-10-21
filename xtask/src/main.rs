//! `cargo xtask <task>`: the quality gates every commit must pass.
//!
//! * `ci`     – fmt check, clippy (warnings are errors), tests, layers, assets
//! * `layers` – no crate may depend on a crate in the same or a higher layer,
//!   and nothing below the UI layer may depend on a GUI toolkit
//! * `assets` – every file under `assets/` has a row in ATTRIBUTION.md
//!
//! Standard library only, so it builds instantly and offline.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// (crate name, layer). Lower layers never depend on higher ones.
const LAYERS: &[(&str, u8)] = &[
    ("ledgercraft-core", 0),
    ("ledgercraft-ledger", 1),
    ("ledgercraft-reports", 2),
    ("ledgercraft-file", 2),
    ("ledgercraft-engine", 3),
    ("ledgercraft-ui-egui", 4),
    ("ledgercraft", 5),
    ("ledgercraft-cli", 5),
];

/// The first layer allowed to use a GUI toolkit.
const UI_LAYER: u8 = 4;
const GUI_CRATES: &[&str] = &[
    "eframe",
    "egui",
    "egui_extras",
    "winit",
    "rfd",
    "wgpu",
    "glow",
];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let root = workspace_root();
    let result = match task.as_str() {
        "ci" => ci(&root),
        "layers" => layers(&root),
        "assets" => assets(&root),
        _ => {
            eprintln!("usage: cargo xtask <ci|layers|assets>");
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask {task}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.parent().map(Path::to_path_buf).unwrap_or(here)
}

fn cargo(root: &Path, args: &[&str]) -> Result<(), String> {
    println!("$ cargo {}", args.join(" "));
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed", args.join(" ")))
    }
}

fn ci(root: &Path) -> Result<(), String> {
    cargo(root, &["fmt", "--all", "--", "--check"])?;
    cargo(
        root,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    cargo(root, &["test", "--workspace"])?;
    layers(root)?;
    assets(root)?;
    println!("ci: all gates passed");
    Ok(())
}

fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["crates", "apps"] {
        if let Ok(entries) = fs::read_dir(root.join(dir)) {
            for e in entries.flatten() {
                let m = e.path().join("Cargo.toml");
                if m.is_file() {
                    out.push(m);
                }
            }
        }
    }
    out.sort();
    out
}

/// Reads `name = "…"` from [package] and the dependency names from
/// [dependencies]. A tiny line parser is enough for our own manifests.
fn parse_manifest(text: &str) -> (String, Vec<String>) {
    let mut name = String::new();
    let mut deps = Vec::new();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if section == "[package]" && key == "name" {
            name = value.trim().trim_matches('"').to_string();
        } else if section == "[dependencies]" {
            // `foo.workspace = true` or `foo = "1"`
            let dep = key.split('.').next().unwrap_or(key).trim();
            if !dep.is_empty() {
                deps.push(dep.to_string());
            }
        }
    }
    (name, deps)
}

fn layer_of(name: &str) -> Option<u8> {
    LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

fn layers(root: &Path) -> Result<(), String> {
    let mut problems = Vec::new();
    for manifest in manifests(root) {
        let text =
            fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let (name, deps) = parse_manifest(&text);
        let Some(mine) = layer_of(&name) else {
            problems.push(format!(
                "{name}: not in the LAYERS table in xtask/src/main.rs"
            ));
            continue;
        };
        for dep in deps {
            if let Some(theirs) = layer_of(&dep) {
                if theirs >= mine {
                    problems.push(format!("{name} (L{mine}) depends on {dep} (L{theirs})"));
                }
            }
            if mine < UI_LAYER && GUI_CRATES.contains(&dep.as_str()) {
                problems.push(format!("{name} (L{mine}) depends on GUI crate {dep}"));
            }
        }
    }
    if problems.is_empty() {
        println!("layers: ok");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                files_under(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

fn assets(root: &Path) -> Result<(), String> {
    let attribution = fs::read_to_string(root.join("ATTRIBUTION.md"))
        .map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let mut files = Vec::new();
    files_under(&root.join("assets"), &mut files);
    let missing: Vec<String> = files
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .filter(|p| !attribution.contains(p.as_str()))
        .collect();
    if missing.is_empty() {
        println!("assets: ok ({} files)", files.len());
        Ok(())
    } else {
        Err(format!(
            "assets without a row in ATTRIBUTION.md:\n{}",
            missing.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_our_manifest_shape() {
        let (name, deps) = parse_manifest(
            "[package]\nname = \"ledgercraft-engine\"\n\n[dependencies]\nledgercraft-core.workspace = true\nserde = \"1\"\n\n[lints]\nworkspace = true\n",
        );
        assert_eq!(name, "ledgercraft-engine");
        assert_eq!(deps, vec!["ledgercraft-core", "serde"]);
    }
}
