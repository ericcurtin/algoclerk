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

The project was developed on a Linux x86_64 host with no GPU or display, no JDK, no Xcode and no Windows. Each
platform has exactly one of the labels BUILT AND RUN HERE / BUILT HERE (not run) / TYPE-CHECKED HERE ONLY / NOT
VERIFIED, with the commands that produced it.

| Platform | Status | Evidence |
| --- | --- | --- |
| Linux x86_64 | **BUILT AND RUN HERE** | `cargo test --workspace` (all unit, integration, oracle, fuzz and compile-and-compare tests); the real windowed game (debug build in a Fedora 44 container with Bevy 0.19.1) was run under Xvfb with Mesa lavapipe (software Vulkan) and driven with real mouse and keyboard events by `scripts/gui-smoke.sh`: title, memo, play, text tab, a failing Check, run. Not run on a physical display or GPU. The `audio` feature type-checks; no sound device exists here, so sound output was not heard. |
| Windows | **TYPE-CHECKED HERE ONLY** | `cargo check --workspace --lib --bins --target x86_64-pc-windows-gnu` and `--target x86_64-pc-windows-msvc` (both pass) |
| macOS | **TYPE-CHECKED HERE ONLY** | `cargo check --workspace --lib --bins --target aarch64-apple-darwin` and `x86_64-apple-darwin` (both pass) |
| Android | **BUILT HERE (not run)** | `cargo ndk -t arm64-v8a build -p algoclerk-app --lib --features android-game-activity` with NDK r27c produced `libalgoclerk_app.so` (aarch64, exports `android_main` and `GameActivity_onCreate`). The Gradle APK step was not run (no JDK or Android SDK here). The pure crates also type-check for `armv7-linux-androideabi` and `x86_64-linux-android`. |
| iOS | **TYPE-CHECKED HERE ONLY (pure crates); the Bevy app NOT VERIFIED** | `cargo check -p algoclerk-core -p algoclerk-levels -p algoclerk-media -p algoclerk-game --lib --target aarch64-apple-ios` (and `-sim`) pass. The `algoclerk-app` crate cannot be checked from Linux because Bevy's iOS logging dependency (`tracing-oslog`) has a build script that needs the Apple SDK (`xcrun`); CI has an `ios-check` job on a macOS runner. |

CI (`.github/workflows/ci.yml`, `release.yml`) contains the jobs that verify the rest on real runners.

## Everyday development

```sh
cargo test --workspace                                  # all tests (compile tests use gcc, g++, clang, rustc if present)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check licenses bans                           # cargo install cargo-deny
cargo run -p algoclerk-cli -- validate                   # prove every shipped level is sound
cargo run -p algoclerk-cli -- table                      # judge table of every reference solution
cargo run --release -p algoclerk-app --bin algoclerk     # the game (needs a display and a Vulkan/Metal/DX12 driver)
```

Useful environment variables for tests: `ALGOCLERK_LEVEL=<id>` limits the compile and oracle tests to one level,
`ALGOCLERK_RANDOM_SEEDS=1,2,3` chooses the random-program seeds, `ALGOCLERK_FUZZ_SEED=n` the UI fuzz seed,
`UPDATE_GOLDEN=1` rewrites the coarse golden maps in `crates/algoclerk-game/tests/golden`.

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
podman run -d --name algoclerk-build -v "$PWD":"$PWD":Z -w "$PWD" fedora:latest sleep infinity
podman exec --user root algoclerk-build dnf install -y gcc gcc-c++ clang pkgconf-pkg-config alsa-lib-devel \
  systemd-devel wayland-devel libxkbcommon-devel mesa-vulkan-drivers xorg-x11-server-Xvfb xdotool ImageMagick
podman exec -e CARGO_TARGET_DIR=/path/to/target -w "$PWD" algoclerk-build cargo build -p algoclerk-app
```

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

`.github/workflows/ci.yml`: rustfmt; clippy and tests on Linux, macOS and Windows; level validation and oracles;
`cargo deny`; the real-window smoke test under Xvfb; Android type-check with the NDK; iOS type-check on a macOS
runner; Windows cross-check from Linux. `.github/workflows/release.yml` (on `v*` tags): Linux tar.gz, macOS
zips, Windows zip, Android APK and AAB, iOS simulator zip, attached to a GitHub release.

## Dependency versions

Bevy `=0.19.1` (the newest release that builds with Rust 1.95; same pin as Falabichos) with default features off:
2D sprite rendering, windowing, input and gamepads; audio only with the `audio` feature. No web or wasm features.
