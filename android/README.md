# AlgoClerk for Android

A Gradle project around **GameActivity** (the activity flavour Bevy supports through its
`android-game-activity` feature). The whole game is the Rust library `libalgoclerk_app.so`; the Java
`MainActivity` only loads it and keeps the game fullscreen. The layout follows the sibling project
Falabichos (Apache-2.0, same owner).

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked
export ANDROID_HOME=$HOME/Android/Sdk ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018
android/build-android.sh debug             # APK in android/app/build/outputs/
android/build-android.sh release --aab     # unsigned unless ALGOCLERK_KEYSTORE* are set
```

- Touch input, the on-screen keyboard of the text view, the portrait layout and the integer-scaled
  low-resolution screen are handled by `algoclerk-game`; the shell queries no permissions.
- Progress is stored in the app's private data directory (`FileStorage`).
- Safe-area insets use conservative defaults (winit 0.30 has no query); override with the environment
  variable `ALGOCLERK_SAFE_INSETS=l,t,r,b` when testing.
- See `docs/BUILDING.md` for what was and was not verified on the development host (the native library is
  built there; the Gradle/APK step needs a JDK and the Android SDK and is run by CI).
