#!/usr/bin/env bash
# Build the AlgoClerk Android app (debug or release APK, optional AAB).
#
# Requirements: Android SDK + NDK (ANDROID_HOME, ANDROID_NDK_HOME), JDK 17+, Gradle 8.x,
# Rust targets and cargo-ndk:
#   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
#   cargo install cargo-ndk
#
# Usage: android/build-android.sh [debug|release] [--aab]
set -euo pipefail
cd "$(dirname "$0")/.."
MODE="${1:-debug}"
PROFILE_FLAG=""
[ "$MODE" = "release" ] && PROFILE_FLAG="--release"

cargo ndk \
    -t arm64-v8a -t armeabi-v7a -t x86_64 \
    -o android/app/src/main/jniLibs \
    build $PROFILE_FLAG -p algoclerk-app --lib

cd android
if [ "$MODE" = "release" ]; then
    gradle --no-daemon :app:assembleRelease
    [ "${2:-}" = "--aab" ] && gradle --no-daemon :app:bundleRelease
else
    gradle --no-daemon :app:assembleDebug
fi
echo "APK/AAB files are in android/app/build/outputs/"
