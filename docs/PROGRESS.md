# AlgoClerk progress log

Running log of milestones, decisions and verification status. Updated at every commit.

## Milestones

- [x] M1: workspace, licence, core crate (ISA, assembler, VM, levels, judge, codegen, i18n, progress)
- [x] M2: 48 levels in 7 packs, validator, independent Rust oracles for every level
- [x] M3: codegen compile-tests (gcc, clang, g++, clang++, rustc) for every reference solution in two calling
      conventions, plus proptest corpora of random programs (also forced goto / state-machine forms), robustness fuzz tests
- [x] M4: CLI (list, show, run, judge, validate, tune, codegen incl. `--verify`, export, import), ADR 0001,
      egui game with office scene, editor, text panel, brief, victory/code view, sandbox, settings, themes
- [x] M5: community docs, CI and release workflows, packaging for five platforms, headless software renderer
- [x] M6: cross-compilation checks, real-window smoke test under Xvfb, final report

## Decisions

- Rust 1.95 (installed). Build output outside the repository: `CARGO_TARGET_DIR=/home/ecurtin/git/algoclerk-run/target`
  (side jobs such as the container, Android and release builds used sibling directories under
  `/home/ecurtin/git/algoclerk-run/` so they would not fight over the same lock).
- GUI toolkit: egui/eframe 0.37 + wgpu, see `docs/adr/0001-gui-toolkit.md`. egui/eframe 0.37 need Rust 1.95, so
  no older version had to be pinned. (Bevy's latest release needs 1.97.1 and would not build here.)
- Levels are TOML (`toml` crate, MIT/Apache-2.0), one file per level under `levels/<pack>/`; expected outputs of
  generated tests come from reference solutions and are cross-checked by independent oracles.
- Own SplitMix64 PRNG so generated tests are stable across platforms and crate versions.
- Fonts: egui's default fonts are disabled (one has a non-listed licence); JetBrains Mono (OFL) is bundled.
- `audio` is an opt-in cargo feature (rodio needs ALSA headers on Linux). It type-checks and is clippy clean in the
  `algoclerk-build` container; release workflows enable it where the libraries exist.
- `wayland` / `x11` features of eframe are enabled only on Linux/BSD (they do not compile for Apple targets);
  Android uses `android-native-activity` plus a direct `android-activity` dependency for the entry point type.
- `[profile.dev.package."*"] opt-level = 2` keeps the immediate-mode UI and the software renderer fast in tests.
- The generated-code structurer got labelled blocks for forward jumps (Rust) / `goto` (C, C++) after measuring that
  9 of 48 reference solutions needed the fallback; 41 are now fully structured.

## Bugs found by the tests (worth remembering)

- Codegen compile tests (random corpus) found an undeclared `out_len` in C when a program had no OUTBOX.
- Strict `-Werror` compile of reference solutions found harness warnings (unused buffer) and a C++ `[0]` vs empty
  initialiser bug in the harness.
- The validator found wrong hand-written examples in 5 levels (count-matches, ring-buffer, list-walk, list-reverse).
- A container build with a different canonical path once produced an empty embedded level list; `build.rs` now fails
  loudly if no levels are found.

## Verification matrix (this host: Linux x86_64, immutable Fedora-like, no Java/Xcode)

| Platform | Status | How |
| --- | --- | --- |
| Linux | BUILT AND RUN HERE | `cargo test --workspace`; release build and `linux/package-linux.sh` produced `dist/algoclerk-0.1.0-linux-x86_64.tar.gz`; the real window was run in a container under Xvfb with Mesa lavapipe (software Vulkan) and driven with real mouse and keyboard events (`scripts/gui-smoke.sh`): title, level select, text editing, run, check, victory screen with code. Not run on a physical display or GPU. |
| Windows | TYPE-CHECKED HERE ONLY | `cargo check --workspace --lib --bins --target x86_64-pc-windows-gnu` and `--target x86_64-pc-windows-msvc` |
| macOS | TYPE-CHECKED HERE ONLY | `cargo check --workspace --lib --bins --target aarch64-apple-darwin` and `x86_64-apple-darwin` |
| Android | BUILT HERE (not run) | NDK r27c + cargo-ndk: `cargo ndk -t arm64-v8a build -p algoclerk-app --lib` produced `libalgoclerk_app.so` (aarch64, exports `android_main` and `ANativeActivity_onCreate`); `cargo ndk -t armeabi-v7a|x86_64 check` also pass. The Gradle APK step could not be run (no JDK/SDK installed). |
| iOS | TYPE-CHECKED HERE ONLY | `cargo check --workspace --lib --bins --target aarch64-apple-ios` and `aarch64-apple-ios-sim`. Linking, Xcode project generation and running were not possible here; CI has `ios-check` and the release workflow builds the simulator app. |

Exact commands and results are in `/home/ecurtin/git/algoclerk-run/FINAL_REPORT.md`.
