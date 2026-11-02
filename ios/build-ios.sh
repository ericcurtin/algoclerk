#!/usr/bin/env bash
# Build the AlgoClerk iOS app on macOS with Xcode.
#
# Requirements: Xcode 15+, XcodeGen (brew install xcodegen), Rust targets:
#   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
#
# Usage:
#   ios/build-ios.sh simulator      # builds AlgoClerk.app for the iOS simulator (no signing needed)
#   ios/build-ios.sh archive        # builds an .xcarchive for devices (needs a signing team:
#                                   #   export ALGOCLERK_TEAM_ID=XXXXXXXXXX)
set -euo pipefail
cd "$(dirname "$0")/.."
MODE="${1:-simulator}"
export CARGO_TARGET_DIR="$PWD/target-ios"

if [ "$MODE" = "simulator" ]; then
    cargo build --release --target aarch64-apple-ios-sim -p algoclerk-app --lib
else
    cargo build --release --target aarch64-apple-ios -p algoclerk-app --lib
fi

cd ios
xcodegen generate
if [ "$MODE" = "simulator" ]; then
    xcodebuild -project AlgoClerk.xcodeproj -scheme AlgoClerk -sdk iphonesimulator \
        -configuration Release -derivedDataPath build CODE_SIGNING_ALLOWED=NO build
    echo "Simulator app: ios/build/Build/Products/Release-iphonesimulator/AlgoClerk.app"
else
    xcodebuild -project AlgoClerk.xcodeproj -scheme AlgoClerk -sdk iphoneos \
        -configuration Release -archivePath build/AlgoClerk.xcarchive \
        DEVELOPMENT_TEAM="${ALGOCLERK_TEAM_ID:?set ALGOCLERK_TEAM_ID}" archive
    echo "Archive: ios/build/AlgoClerk.xcarchive"
fi
