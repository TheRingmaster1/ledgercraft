//! LedgerCraft desktop app.
//!
//! ```text
//! ledgercraft                  # start page: create, open or explore the sample
//! ledgercraft books.lcbooks    # open a company file
//! ledgercraft --sample         # open the sample company
//! ```

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use ledgercraft_engine::{sample_company, EngineError, Session};
use ledgercraft_file::FileError;
use ledgercraft_ui_egui::LedgerApp;

/// The app icon, 256 x 256. Original artwork; see ATTRIBUTION.md.
const APP_ICON: &[u8] = include_bytes!("../../../assets/app-icon/ledgercraft-256.png");

fn main() -> ExitCode {
    let mut path: Option<PathBuf> = None;
    let mut sample = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--sample" => sample = true,
            "--version" | "-V" => {
                println!("ledgercraft {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!("usage: ledgercraft [--sample] [FILE.lcbooks]");
                return ExitCode::SUCCESS;
            }
            other => path = Some(PathBuf::from(other)),
        }
    }

    // A password-protected file opens in the app, which asks for the password.
    let mut unlock = None;
    let session = if let Some(p) = &path {
        match Session::open(p) {
            Ok(s) => Some(s),
            Err(EngineError::File(FileError::PasswordRequired)) => {
                unlock = Some(p.clone());
                None
            }
            Err(e) => {
                eprintln!("ledgercraft: could not open {}: {e}", p.display());
                return ExitCode::FAILURE;
            }
        }
    } else if sample {
        match sample_company() {
            Ok(c) => Some(Session::new(c)),
            Err(e) => {
                eprintln!("ledgercraft: could not build the sample company: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        // No company yet: the app opens on its start page.
        None
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("LedgerCraft")
        // The minimum size is set by `LedgerApp` once the window's
        // monitor scale is known (see `LedgerApp::fit_window`).
        .with_inner_size([1280.0, 800.0]);
    // Title bar and taskbar icon. The .exe's own icon (Explorer, Start menu)
    // is embedded by build.rs from assets/app-icon/ledgercraft.ico.
    match eframe::icon_data::from_png_bytes(APP_ICON) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("ledgercraft: could not load the app icon: {e}"),
    }

    let options = eframe::NativeOptions {
        // wgpu (DirectX 12 / Vulkan / Metal), not OpenGL: glutin's Windows
        // OpenGL path panics in debug builds (see Cargo.toml).
        renderer: eframe::Renderer::Wgpu,
        viewport,
        ..Default::default()
    };
    let result = eframe::run_native(
        "LedgerCraft",
        options,
        Box::new(move |cc| Ok(Box::new(LedgerApp::new(cc, session, unlock)))),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ledgercraft: {e}");
            ExitCode::FAILURE
        }
    }
}
