# ADR 0001: GUI toolkit

- Status: accepted
- Date: 2026-11

## Context

AlgoClerk must run from one Rust code base on macOS, Linux, Windows, Android and iOS, with mouse,
keyboard and touch, scale from phones to desktops, draw custom graphics (an animated office scene,
belts, a clerk) and support drag and drop, text editing and clipboard. Hard constraints:

- Rust-native, permissively licensed (MIT / Apache-2.0), actively maintained.
- **No WebAssembly** anywhere (so no web target; this removes a major selling point of several toolkits).
- **Not Slint** (its licence is incompatible with the goal of this project).
- It must build with the installed `rustc` 1.95.

Facts below were checked on crates.io (licence, latest release, minimum Rust version) at the time of
writing; claims about mobile support come from the crates' documentation and my reading of their
sources and are marked where they could not be tested on this Linux machine.

## Candidates

| Toolkit | Licence | Latest release (checked) | Notes |
| --- | --- | --- | --- |
| **egui + eframe 0.37** | MIT OR Apache-2.0 | 0.37.0 (Oct 2026), MSRV 1.95 | Immediate mode. `Painter` for custom drawing, built-in drag-and-drop helpers (`dnd_drag_source`, `dnd_drop_zone`), touch events, text editing, clipboard, safe-area insets (`InputState::safe_area_insets`), multi-viewport. Backed by winit (macOS, Windows, Linux X11/Wayland, **Android** through `android-activity`, **iOS**) and wgpu (Vulkan, Metal, D3D12, GLES). |
| macroquad 0.4 / miniquad | MIT OR Apache-2.0 | 0.4.16 (Jul 2026) | Very small, good game-loop ergonomics, explicit Android and iOS support in miniquad. But no widgets: text input, IME, clipboard, scrolling lists and drag and drop would all be hand-written, and a program editor is mostly widgets. Android packaging relies on older tooling (`cargo-quad-apk`). |
| Bevy 0.20 (+ bevy_ui) | MIT OR Apache-2.0 | 0.20.1 (Oct 2026), **MSRV 1.97.1** | Strong, documented mobile story (official iOS and Android examples) and an excellent renderer, but heavy for a 2D puzzle UI, `bevy_ui` is weak for text editing and lists, and the newest release needs a newer `rustc` than installed here (an older Bevy would have to be pinned). |
| Iced 0.14 | MIT | 0.14.0 (Dec 2025) | Elm-style retained UI on winit + wgpu. Nice widgets, no first-class drag and drop, mobile support is experimental and undocumented. |
| Floem 0.2 | MIT | 0.2.0 (Nov 2024) | Reactive UI on winit; no release for almost two years, mobile is not a goal. |
| Makepad 1.0 | MIT OR Apache-2.0 | 1.0.0 (May 2025) | Interesting GPU UI with Android and iOS targets, but its own DSL and build tooling (`cargo-makepad`), a small ecosystem and a steep contribution barrier for a community project. |
| Slint | GPL / commercial / Slint licence | not considered | Ruled out by the licence requirement. |

## Decision

**egui with eframe 0.37 and the wgpu renderer**, with the bundled font JetBrains Mono.

Why:

1. **Interaction primitives we need exist**: free-form painting for the office scene, text editing for
   the assembly panel (with a code-editor mode), drag and drop with payloads for reordering
   instructions, touch input, clipboard, scrolling, combo boxes, sliders.
2. **Immediate mode keeps game state simple.** The UI is a pure function of the core's state, which
   keeps the app crate thin and lets us render it **headlessly**: `egui::Context::run_ui` plus a
   small CPU rasteriser (`crates/algoclerk-app/src/raster.rs`) gives real screenshots and smoke tests
   without a window or GPU.
3. **Credible platform story without WebAssembly**: eframe is built on winit and wgpu, which target all
   five platforms natively. egui also exposes safe-area insets, which we use for notches and rotation.
4. **Community**: by far the largest Rust GUI contributor pool and ecosystem, a good fit for the
   project goal of attracting many contributors.
5. **It builds on the installed compiler** (`rustc` 1.95 is exactly the MSRV of egui 0.37). Bevy's
   latest release would not.

## Honest assessment of mobile maturity

- **Android**: eframe documents the `android-native-activity` and `android-game-activity` features and
  the egui repository contains an Android example. We use NativeActivity (no Java code) with a
  `cdylib` and `cargo-ndk` + Gradle. *Verified here:* the library cross-compiles for `aarch64-linux-android`
  with the NDK on Linux (see PROGRESS.md and BUILDING.md). *Not verified:* running on a device or emulator.
- **iOS**: winit and wgpu support iOS (UIKit, Metal), but eframe has **no official iOS example or
  packaging guide**; it works by exposing a `staticlib` whose entry point is called from `main()` of
  an Xcode wrapper. This is the least mature part of the choice. *Verified here:* the crate type-checks for
  `aarch64-apple-ios` and `aarch64-apple-ios-sim`. *Not verified:* linking, running in a simulator or
  on a device (needs macOS and Xcode; the CI jobs `ios-check` and the release workflow do this).
- **Desktop**: eframe on X11/Wayland, Windows and macOS is the toolkit's primary target.
- **Text input on touch** (soft keyboard) is the area where egui is weakest compared with native
  toolkits; the game minimises free text input (instructions are tapped, operands use buttons) and the
  text panel is optional.
- **Accessibility**: egui's AccessKit integration exists but is not enabled in v0.1 (it pulls a large
  dependency tree on Linux); it is on the roadmap.

## Consequences

- Versions are pinned (`eframe = "=0.37.0"`); upgrades are deliberate pull requests.
- The default `default_fonts` feature is disabled, because egui's bundled fonts include one under a
  licence outside the accepted list; we bundle our own OFL font instead.
- Features `wayland` and `x11` are enabled only on Linux and the BSDs (they do not build on Apple
  targets); `android-native-activity` only on Android.
- If iOS proves too fragile, the `algoclerk-app` crate is a thin layer (UI only), and replacing the shell
  would not touch the core, levels or CLI.

## Alternatives rejected

macroquad (no widgets, so the text editor and program list would be a project of their own), Bevy
(heavyweight and the current release needs a newer compiler), Iced and Floem (no drag and drop, mobile
immature or stale), Makepad (custom DSL and tooling, small contributor pool), Slint (licence).
