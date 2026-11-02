// SPDX-License-Identifier: Apache-2.0
//! AlgoClerk: the game. A thin egui front end over `algoclerk-core`.
//!
//! The same code runs on desktop (`main.rs`), Android (`android_main`) and iOS
//! (`algoclerk_ios_main`). All game logic lives in `algoclerk-core`.
#![allow(missing_docs, missing_debug_implementations, clippy::too_many_lines)]

pub mod app;
mod brief;
mod editor;
pub mod fonts;
mod office;
mod play;
mod play_ui;
pub mod raster;
mod result;
pub mod shot;
mod sound;
pub mod theme;
mod widgets;

pub use app::AlgoClerkApp;

/// Window options shared by all native front ends.
fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("AlgoClerk")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([340.0, 480.0]),
        ..Default::default()
    }
}

fn creation(cc: &eframe::CreationContext<'_>) -> Box<dyn eframe::App> {
    fonts::install(&cc.egui_ctx);
    let saved = cc.storage.and_then(|s| s.get_string("progress"));
    Box::new(AlgoClerkApp::new(saved.as_deref()))
}

/// Run the game in a native window (desktop, and iOS through `algoclerk_ios_main`).
pub fn run_native(options: eframe::NativeOptions) -> eframe::Result {
    eframe::run_native("AlgoClerk", options, Box::new(|cc| Ok(creation(cc))))
}

/// Run the game on a desktop window.
pub fn run_desktop() -> eframe::Result {
    run_native(native_options())
}

/// Android entry point (NativeActivity).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    let _ = run_native(options);
}

/// iOS entry point, called from `main()` of the Xcode wrapper project.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn algoclerk_ios_main() {
    let _ = run_native(native_options());
}
