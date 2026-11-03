# Building AlgoClerk

All platforms build from the same Rust code. You need **Rust 1.95 or newer** (`rustup update`). Set
`CARGO_TARGET_DIR` if you want build output outside the repository
(`export CARGO_TARGET_DIR=$HOME/algoclerk-target`).

| Platform | Build / package |
| --- | --- |
| Linux | `cargo build --release -p algoclerk-app --bin algoclerk --features audio` or `linux/package-linux.sh --features audio` |
| macOS | `macos/package-macos.sh [target]` |
| Windows | `windows/package-windows.ps1` |
| Android | `android/build-android.sh release --aab` |
| iOS | `ios/build-ios.sh simulator` or `archive` |

## Verification status (be skeptical, read this before relying on a platform)

The project was developed on a Linux x86_64 host with no GPU, no physical display, no JDK, no Xcode and no
Windows. Each platform has exactly one of the labels BUILT AND RUN / BUILT (not run) / TYPE-CHECKED ONLY / NOT
RE-VERIFIED.

**Two code bases are described here.** The "Earlier (`main`)" column is evidence produced on `main`, the code
from before the move to real C++ (the invented-language era, ADR 0006). The column "On `cpp-lang`" is what was
run on this branch. The branch changed `algoclerk-core`, `algoclerk-levels`, `algoclerk-game` and
`algoclerk-cli` heavily and the app shell and front end too; the packaging scripts and the platform projects
are unchanged.

| Platform | Earlier (`main`) | On `cpp-lang` |
| --- | --- | --- |
| Linux x86_64 | **BUILT AND RUN**: the windowed game in a Fedora 44 container (Bevy 0.19.1) under Xvfb with Mesa lavapipe, driven with real mouse and keyboard events by `scripts/gui-smoke.sh`. | **BUILT AND RUN** in the same container setup (Fedora 44, Xvfb, lavapipe): typing and card editing, a runtime error, level select, settings, the watch board, the victory screen and its code tabs. `docs/screenshots/window-*.png` are real captures. `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` (host for every crate except `algoclerk-app`, which is checked and built in the container), `cargo test --workspace` excluding the app (host, 440+ tests, the levels crate takes about 5 minutes) and `cargo deny check licenses bans` all pass. Not run on a physical display or a real GPU; the `audio` feature type-checks but no sound device exists, so sound was not heard. **`scripts/gui-smoke.sh` was not re-run** (its click coordinates depend on the layout). |
| Windows | **TYPE-CHECKED ONLY**: `cargo check --workspace --lib --bins --target x86_64-pc-windows-gnu` and `-msvc`. | **TYPE-CHECKED ONLY (engine-free crates)**: `cargo check` of `algoclerk-core`, `-levels`, `-media`, `-game` for `x86_64-pc-windows-gnu` passes. **The Windows app was NOT re-built.** The compile-and-compare suites need GNU `timeout`; CI runs the interpreter-only tests on Windows. |
| macOS | **TYPE-CHECKED ONLY**: `cargo check --workspace --lib --bins --target aarch64-apple-darwin` and `x86_64-apple-darwin`. | **TYPE-CHECKED ONLY (engine-free crates)** for `aarch64-apple-darwin`. **The macOS app was NOT re-built or run.** CI installs coreutils so that `timeout` exists; that leg has never been seen green with the compile suites. |
| Android | **BUILT (not run)**: `cargo ndk -t arm64-v8a build -p algoclerk-app --lib --features android-game-activity` with NDK r27c produced `libalgoclerk_app.so` (exports `android_main` and `GameActivity_onCreate`). The Gradle APK step was not run (no JDK or Android SDK). | **BUILT (not run)**: `cargo ndk -t arm64-v8a check` and `build -p algoclerk-app --lib --features android-game-activity` with NDK r27c both pass on this branch and produce `libalgoclerk_app.so`. The Gradle APK step was not run (no JDK or Android SDK) and nothing was run on a device. |
| iOS | **TYPE-CHECKED ONLY (pure crates)**: `cargo check -p algoclerk-core -p algoclerk-levels -p algoclerk-media -p algoclerk-game --lib --target aarch64-apple-ios` (and `-sim`). The `algoclerk-app` crate cannot be checked from Linux (Bevy's `tracing-oslog` build script needs `xcrun`); CI has an `ios-check` job on a macOS runner. | **TYPE-CHECKED ONLY (engine-free crates)** for `aarch64-apple-ios`. **The iOS app was NOT re-built.** |

Known limits that apply to every platform: no touch-device, notch or soft-keyboard testing (touch drag is
unverified on a device); no real GPU; the wide editor mode in landscape is not implemented; the interpreter's
claim to be C++ is tested against `g++` and `clang++` on Linux only, because that is where the differential
suites run (see below). The remaining gaps are listed in [PROGRESS.md](PROGRESS.md).

CI (`.github/workflows/ci.yml`, `release.yml`) contains the jobs that verify the rest on real runners. None of
them has run yet for `cpp-lang`.

## Everyday development

```sh
cargo test --workspace                                  # all tests (compiler suites skip a missing compiler)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check licenses bans                           # cargo install cargo-deny
cargo run -p algoclerk-cli -- validate                   # prove every shipped level is sound
cargo run -p algoclerk-cli -- table                      # judge table of every reference solution
cargo run --release -p algoclerk-app --bin algoclerk     # the game (needs a display and a Vulkan/Metal/DX12 driver)
```

### Compilers the test suite needs

Players write real C++17 and the repository checks the game's interpreter against real compilers, so the full
suite needs these on `PATH`:

| Tool | Used by |
| --- | --- |
| `g++` (C++17) | every differential test: snippets, random programs, level solutions, negative UB table (sanitizers `-fsanitize=undefined,address` when the runtime is installed) |
| `clang++` (C++17) | the same tests as a second opinion; always for the level tests, for a sample of the random programs unless `ALGOCLERK_DIFF_CLANG` is set |
| `rustc` | the Rust translations (`cpp_codegen_random`, `cpp_codegen_levels`, `codegen --verify`) |
| `gcc` or `clang` (C) | the C translations |
| GNU `timeout` | wraps every compiler and test binary run (`timeout -s KILL`); not available on Windows |

A suite whose compiler is missing prints a message and passes without testing, so **check the output for
"skipping"** before believing a green run. CI installs all of them (Linux, plus coreutils on macOS).

### Differential and property tests

```sh
# every level: reference and naive built with g++ and clang++, results compared (large tests capped)
cargo test -p algoclerk-levels --test cpp_diff_levels
ALGOCLERK_LEVEL=two-sum cargo test -p algoclerk-levels --test cpp_diff_levels -- --nocapture   # one level
ALGOCLERK_DIFF_FULL=1 cargo test -p algoclerk-levels --test cpp_diff_levels                    # large tests at full size

# random UB-free programs: interpreter versus g++ (and clang++)
ALGOCLERK_PROP_CASES=1000 ALGOCLERK_PROP_SEED=7 ALGOCLERK_DIFF_CLANG=1 \
  cargo test -p algoclerk-core --test cpp_prop_diff -- --nocapture

# snippets, round trip, negative table, fuzz, translations
cargo test -p algoclerk-core --test cpp_interp_diff --test cpp_parse_roundtrip --test cpp_prop_ub --test cpp_prop_fuzz
cargo test -p algoclerk-core --test cpp_codegen_random
cargo test -p algoclerk-levels --test cpp_codegen_levels
```

| Variable | Effect |
| --- | --- |
| `ALGOCLERK_DIFF_CLANG` | any value: compile every random program and snippet with `clang++` too (the default samples some batches) |
| `ALGOCLERK_PROP_CASES` | number of random programs (default 40) or fuzz mutants (default 300) |
| `ALGOCLERK_PROP_SEED` | seed of the random programs; every run prints its seed |
| `ALGOCLERK_PROP_REPLAY` | hex choice vector printed by a failing run: replay that one program |
| `ALGOCLERK_PROP_NOSHRINK`, `ALGOCLERK_PROP_THREADS` | disable shrinking of a failure; number of worker threads |
| `ALGOCLERK_PROP_FILE`, `ALGOCLERK_PROP_ARGS` | run one C++ file through the property comparison |
| `ALGOCLERK_LEVEL` | restrict the level tests (oracles, differential, translation) to one level id |
| `ALGOCLERK_LANG` | `rust` or `c`: restrict `cpp_codegen_levels` to one translation language |
| `ALGOCLERK_DIFF_FULL` | large level tests at full size instead of capped |
| `ALGOCLERK_DIFF_KEEP`, `ALGOCLERK_CODEGEN_KEEP` | keep the generated sources and binaries for inspection |
| `ALGOCLERK_CORPUS` | restrict `cpp_codegen_random` to one corpus file |
| `ALGOCLERK_LEVELS` | directory of extra level packs read by the game and the CLI |
| `ALGOCLERK_FUZZ_SEED` | seed of the UI fuzz test |
| `UPDATE_GOLDEN=1` | rewrite the coarse golden maps in `crates/algoclerk-game/tests/golden` |

The same check for one level from the command line: `algoclerk-cli diff <id> [--full] [--no-sanitize]`, and
`algoclerk-cli harness <id>` prints the generated test program.

### In the container (immutable hosts)

On an immutable host without the Bevy `-devel` packages, build and test in the `algoclerk-build` container
(Fedora 44, with the compilers installed, see "Linux" below). The wrapper exports the target directory, which
`podman exec` does not inherit:

```sh
#!/bin/bash
# container-cargo.sh: run cargo inside the algoclerk-build container
exec podman exec -e CARGO_TARGET_DIR=/path/to/target-container -w "$PWD" algoclerk-build cargo "$@"
```

```sh
./container-cargo.sh test --workspace
./container-cargo.sh test -p algoclerk-levels --test cpp_diff_levels
```

Compile suites are CPU heavy: run at most two cargo processes at once, and wrap long runs in
`timeout -s KILL <secs>`.

### Headless rendering (no window, display or GPU)

```sh
scripts/screenshots.sh                    # regenerates docs/screenshots/*.png with the software renderer
cargo run -p algoclerk-tools --bin algoclerk-shots -- out --only play,victory --scale 3
```

These screenshots show the real game code drawing into the software canvas; they are not proof that a window
opens on a given machine. `scripts/gui-smoke.sh` does that for Linux (Xvfb + lavapipe + xdotool).

## Linux

Bevy needs a few system libraries at build time: `libudev-dev` (gamepads), `libwayland-dev`,
`libxkbcommon-dev`, `libx11-dev`, `libxcursor-dev`, `libxi-dev`, `libxrandr-dev`, `pkg-config`; the `audio`
feature additionally needs ALSA (`libasound2-dev` / `alsa-lib-devel`). Without `--features audio` everything
else builds and the game is fully playable muted; release builds enable it. At run time you need a Wayland or
X11 session and a Vulkan driver (Mesa works, including software `lavapipe`).

On an immutable Fedora-like host use a container instead of installing packages:

```sh
podman run -d --name algoclerk-build -v "$PWD":"$PWD":Z -w "$PWD" fedora:44 sleep infinity
podman exec --user root algoclerk-build dnf install -y gcc gcc-c++ clang libasan libubsan pkgconf-pkg-config alsa-lib-devel \
  systemd-devel wayland-devel libxkbcommon-devel mesa-vulkan-drivers xorg-x11-server-Xvfb xdotool ImageMagick \
  libXcursor libXi libXrandr libXinerama libxkbcommon-x11 libXrender libXext mesa-libGL
podman exec -e CARGO_TARGET_DIR=/path/to/target -w "$PWD" algoclerk-build cargo build -p algoclerk-app
```

The last line of packages (`libXcursor` to `mesa-libGL`) is what the windowed game needs inside the container
(winit's X11 backend and the software Vulkan/GL stack); the headless screenshots and the tests do not need it.
`CARGO_TARGET_DIR` is not inherited by `podman exec` unless you pass `-e`.

## macOS

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
macos/package-macos.sh aarch64-apple-darwin      # AlgoClerk.app zipped into dist/
```

The script assembles the bundle, builds the `.icns` icon with `iconutil` and can sign with
`ALGOCLERK_SIGN_IDENTITY="Developer ID Application: ..."`. For distribution outside the Mac App Store notarise
with `xcrun notarytool submit dist/AlgoClerk-*.zip --keychain-profile ... --wait` followed by
`xcrun stapler staple dist/AlgoClerk.app`. Secrets are never part of the repository.

## Windows

```powershell
rustup target add x86_64-pc-windows-msvc
windows/package-windows.ps1
```

Cross-checking from Linux: `cargo check --workspace --target x86_64-pc-windows-gnu --lib --bins` (and
`x86_64-pc-windows-msvc`; both type-check without extra toolchains because `algoclerk-app` enables blake3's
`pure` feature). Linking needs `mingw-w64` or `cargo xwin`.

## Android

Requirements: Android SDK (platform 35, build-tools), **Android NDK r27** (or newer), JDK 17, Gradle 8.x,
`cargo-ndk`.

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked
export ANDROID_HOME=$HOME/Android/Sdk ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018
cargo ndk -t arm64-v8a check -p algoclerk-app --lib --features android-game-activity   # quick type-check
android/build-android.sh debug                                # APK
android/build-android.sh release --aab                        # APK + AAB
```

The app is a GameActivity project ported from the sibling project Falabichos (`android/`); the Java
`MainActivity` loads `libalgoclerk_app.so`, whose `android_main` is generated by Bevy's `#[bevy_main]`. Release
signing is optional through `ALGOCLERK_KEYSTORE`, `ALGOCLERK_KEYSTORE_PASSWORD`, `ALGOCLERK_KEY_ALIAS`,
`ALGOCLERK_KEY_PASSWORD`; in CI the keystore comes from the optional secrets `ANDROID_KEYSTORE_BASE64`,
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`.

## iOS

Requirements: macOS, Xcode 15+, [XcodeGen](https://github.com/yonaskolb/XcodeGen), Rust targets.

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
ios/build-ios.sh simulator      # unsigned simulator app
ALGOCLERK_TEAM_ID=XXXXXXXXXX ios/build-ios.sh archive               # .xcarchive for devices
```

`ios/AlgoClerk/main.m` calls `main_rs()` in the Rust static library (Bevy/winit runs `UIApplicationMain`). Device
builds need an Apple Developer team; exporting an IPA additionally needs a provisioning profile and distribution
certificate that the maintainers must configure.

## Continuous integration

`.github/workflows/ci.yml`:

- `fmt`: rustfmt.
- `test` (Linux, macOS, Windows): clippy and tests. Linux installs `gcc`, `g++` and `clang` so every compiler suite
  runs; macOS installs coreutils for GNU `timeout`; Windows has no GNU `timeout`, so it runs the unit tests and
  the integration tests that need only the interpreter (not the compile-and-compare suites). The macOS and
  Windows legs are **unverified**: they are the most likely to need a fix on their first run.
- `differential` (Linux): installs `g++`, `clang++` and relies on the `rustc` of the toolchain action; runs the
  snippet, round-trip, random-program (400 cases, both compilers, a new seed per run), UB-table and fuzz suites,
  the level differential tests, and the Rust and C translation tests. The seed is printed; reproduce a failure
  with `ALGOCLERK_PROP_SEED=<n>`.
- `levels`: `algoclerk-cli validate`, `table` and the oracle and shipped-campaign tests.
- `deny`: `cargo deny`.
- `gui-smoke`: the real-window smoke test under Xvfb (hard-coded click coordinates; may need updating).
- `android-check`, `ios-check`, `windows-cross`: type-checks for the other platforms.

`.github/workflows/release.yml` (on `v*` tags): Linux tar.gz, macOS zips, Windows zip, Android APK and AAB, iOS
simulator zip, attached to a GitHub release. It builds the game only and needs no C++ compiler.

## Dependency versions

Bevy `=0.19.1` (the newest release that builds with Rust 1.95; same pin as Falabichos) with default features off:
2D sprite rendering, windowing, input and gamepads; audio only with the `audio` feature. No web or wasm features.
