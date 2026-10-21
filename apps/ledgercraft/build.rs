//! Embeds the app icon in ledgercraft.exe on Windows, so Explorer, the Start
//! menu and shortcuts show it. Other platforms take their icon from the
//! packaging instead, so this does nothing there.
//!
//! A failure here never breaks the build: without the Windows resource
//! compiler the .exe just keeps the default icon (the window still shows the
//! LedgerCraft icon, which main.rs sets at run time).

fn main() {
    println!("cargo:rerun-if-changed=../../assets/app-icon/ledgercraft.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/ledgercraft.ico");
    res.set("ProductName", "LedgerCraft");
    res.set("FileDescription", "LedgerCraft - open-source small-business accounting");
    res.set("LegalCopyright", "Copyright (c) 2026 the LedgerCraft contributors. MIT OR Apache-2.0.");
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not embed the app icon in the .exe: {e}");
    }
}
