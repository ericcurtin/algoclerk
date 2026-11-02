# Building AlgoClerk

All platforms build from the same Rust code. You need **Rust 1.95 or newer** (`rustup update`).
Set `CARGO_TARGET_DIR` if you want build output outside the repository
(`export CARGO_TARGET_DIR=$HOME/algoclerk-target`).

| Platform | Build / package | Verified how |
| --- | --- | --- |
| Linux | `cargo build --release -p algoclerk-app --bin algoclerk` or `linux/package-linux.sh` | see "Verification status" |
| macOS | `macos/package-macos.sh [target]` | see "Verification status" |
| Windows | `windows/package-windows.ps1` | see "Verification status" |
| Android | `android/build-android.sh release --aab` | see "Verification status" |
| iOS | `ios/build-ios.sh simulator` or `archive` | see "Verification status" |

## Verification status (be skeptical, read this before relying on a platform)

The project was developed on a Linux x86_64 host without Xcode, Windows or an Android SDK/JDK. What was
actually verified, with the commands:

| Platform | Status | Evidence |
| --- | --- | --- |
| Linux | built and run (software rendering) | `cargo test --workspace`; `linux/package-linux.sh`; `scripts/gui-smoke.sh` runs the real window under Xvfb with Mesa's software Vulkan driver and real input events |
| Windows | type-checked only | `cargo check --workspace --lib --bins --target x86_64-pc-windows-gnu` (and `-msvc`) |
| macOS | type-checked only | `cargo check --workspace --lib --bins --target aarch64-apple-darwin` (and `x86_64-apple-darwin`) |
| Android | native library built, not run | `cargo ndk -t arm64-v8a build -p algoclerk-app --lib` with NDK r27c; Gradle packaging not run |
| iOS | type-checked only | `cargo check --workspace --lib --bins --target aarch64-apple-ios` (and `-sim`) |

CI (`.github/workflows/ci.yml`, `release.yml`) contains the jobs that verify the rest on real runners.

## Everyday development

```sh
cargo test --workspace                                   # all tests (compile tests use gcc/clang/rustc if present)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check                                         # cargo install cargo-deny
cargo run -p algoclerk-cli -- judge --all --reference    # whole campaign through the judge
cargo run --release -p algoclerk-app --bin algoclerk     # the game (needs a display and a GPU/driver)
```

### Headless rendering (no window, display or GPU)

```sh
cargo run -p algoclerk-app --bin algoclerk -- --screenshot play --level max-of --out play.png
cargo run -p algoclerk-app --bin algoclerk -- --screenshot victory --level fibonacci --lang c --out win.png
scripts/screenshots.sh        # regenerates docs/screenshots/*.png
```

The screenshots use a CPU rasteriser for egui meshes (`crates/algoclerk-app/src/raster.rs`). They show the
real UI code, but not the GPU renderer, so they are **not** proof that a window opens on a given
machine.

## Linux

Building needs only a Rust toolchain: the window code loads Wayland, X11 and Vulkan/OpenGL libraries at
run time (`dlopen`), so no `-devel` packages are needed. At run time you need a Wayland or X11
session and a Vulkan or OpenGL driver (Mesa works).

Optional sound (`--features audio`) needs ALSA development files:
`sudo apt install libasound2-dev` or `sudo dnf install alsa-lib-devel`.

On an immutable Fedora-like host, use a container instead of installing packages:

```sh
podman run -d --name algoclerk-build -v "$PWD":"$PWD":Z -w "$PWD" fedora:latest sleep infinity
podman exec --user root algoclerk-build dnf install -y gcc gcc-c++ clang pkgconf-pkg-config alsa-lib-devel
podman exec algoclerk-build cargo build --release -p algoclerk-app --features audio   # mount your ~/.cargo and ~/.rustup, or install rust in the container
```

Remember that `CARGO_TARGET_DIR` is not inherited by `podman exec` unless you pass `-e`.

## macOS

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
macos/package-macos.sh aarch64-apple-darwin      # AlgoClerk.app zipped into dist/
```

The script assembles the bundle, builds the `.icns` icon with `iconutil` and can sign with
`ALGOCLERK_SIGN_IDENTITY="Developer ID Application: ..."`. For distribution outside the Mac App Store
notarise with `xcrun notarytool submit dist/AlgoClerk-*.zip --keychain-profile ... --wait` followed by
`xcrun stapler staple dist/AlgoClerk.app`. Secrets (certificates, passwords) are never part of the
repository.

## Windows

```powershell
rustup target add x86_64-pc-windows-msvc
windows/package-windows.ps1
```

Cross-checking from Linux without Windows: `rustup target add x86_64-pc-windows-gnu` and
`cargo check --workspace --target x86_64-pc-windows-gnu --lib --bins` (this is a type-check; linking
needs `mingw-w64`). `cargo xwin` can build MSVC binaries on Linux.

## Android

Requirements: Android SDK (platform 35, build-tools), **Android NDK r27** (or newer), JDK 17, Gradle
8.x, `cargo-ndk`.

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018
cargo ndk -t arm64-v8a check -p algoclerk-app --lib          # quick type-check
android/build-android.sh debug                                # APK
android/build-android.sh release --aab                        # APK + AAB
```

The app is a `NativeActivity` (no Java/Kotlin). Release signing is optional through the environment
variables `ALGOCLERK_KEYSTORE`, `ALGOCLERK_KEYSTORE_PASSWORD`, `ALGOCLERK_KEY_ALIAS`,
`ALGOCLERK_KEY_PASSWORD`; without them `assembleRelease` produces an unsigned APK. In CI the keystore
comes from the secret `ANDROID_KEYSTORE_BASE64` plus `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`
and `ANDROID_KEY_PASSWORD` (all optional).

## iOS

Requirements: macOS, Xcode 15+, [XcodeGen](https://github.com/yonaskolb/XcodeGen), Rust targets.

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo check -p algoclerk-app --lib --target aarch64-apple-ios       # type-check works on any host
ios/build-ios.sh simulator      # unsigned simulator app
ALGOCLERK_TEAM_ID=XXXXXXXXXX ios/build-ios.sh archive               # .xcarchive for devices
```

Device builds need an Apple Developer team. In CI the optional secret `ALGOCLERK_TEAM_ID` enables the
archive step; exporting an IPA additionally needs a provisioning profile and distribution certificate
(`xcodebuild -exportArchive` with an `ExportOptions.plist`), which the maintainers must configure.

## Continuous integration

`.github/workflows/ci.yml`: rustfmt; clippy and tests on Linux, macOS and Windows; level validation;
`cargo deny`; Android type-check with the NDK (`cargo ndk ... check`); iOS type-check for the device
and simulator targets; a Windows cross-check from Linux.
`.github/workflows/release.yml` (on `v*` tags): Linux tar.gz, macOS zips (arm64 and x86_64), Windows
zip, Android APK and AAB, iOS simulator zip (plus an `.xcarchive` when signing secrets exist), attached
to a GitHub release.

## Dependency versions

Versions are pinned in `Cargo.toml`/`Cargo.lock`. egui/eframe `0.37.0` require Rust 1.95, exactly the
version this project was developed with, so no older version had to be pinned. Bevy's latest release
(0.20.1, Rust 1.97.1) would not build on 1.95, one of the reasons it was not chosen (ADR 0001).
