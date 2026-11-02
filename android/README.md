# AlgoClerk for Android

A NativeActivity app: the whole game is the Rust library `libalgoclerk_app.so`
(crate `algoclerk-app`, entry point `android_main`). There is no Java or Kotlin code.

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>
android/build-android.sh debug          # -> android/app/build/outputs/apk/debug/app-debug.apk
android/build-android.sh release --aab  # unsigned unless the variables below are set
```

Signing (optional, release builds only): set `ALGOCLERK_KEYSTORE`, `ALGOCLERK_KEYSTORE_PASSWORD`,
`ALGOCLERK_KEY_ALIAS` and `ALGOCLERK_KEY_PASSWORD`. Never commit keystores (they are git-ignored).

Notes:

- `minSdk` 26 (Android 8), rendering through wgpu (Vulkan, with a GLES fallback).
- Touch input, on-screen keyboard text entry, rotation and display cut-outs (safe-area insets) are
  handled by egui/winit; see `docs/BUILDING.md` for the verification status.
- Progress is stored in the app's private data directory by eframe's persistence.
