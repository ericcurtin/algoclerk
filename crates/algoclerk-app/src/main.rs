// SPDX-License-Identifier: Apache-2.0
//! Desktop entry point.
//!
//! `algoclerk` opens the game window. `algoclerk --screenshot <name> --out file.png` renders a
//! screen with the built-in software renderer, without a window or GPU (see `docs/BUILDING.md`).

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| a == "--screenshot" || a == "--icon" || a == "--help" || a == "-h")
    {
        std::process::exit(algoclerk_app::shot::cli(&args));
    }
    if let Err(e) = algoclerk_app::run_desktop() {
        eprintln!("algoclerk: {e}");
        std::process::exit(1);
    }
}
