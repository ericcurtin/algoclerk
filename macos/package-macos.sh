#!/usr/bin/env bash
# Build AlgoClerk.app and a zip for macOS (run on macOS).
#
# Usage: macos/package-macos.sh [target-triple]     (default: the host, e.g. aarch64-apple-darwin)
# Optional signing/notarisation (never required to run the app locally):
#   export ALGOCLERK_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET="${1:-$(rustc -vV | sed -n 's/host: //p')}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
cargo build --release --target "$TARGET" -p algoclerk-app --bin algoclerk
OUT="dist/AlgoClerk.app"
rm -rf "$OUT" && mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"
cp "${CARGO_TARGET_DIR:-target}/$TARGET/release/algoclerk" "$OUT/Contents/MacOS/algoclerk"
sed "s/@VERSION@/$VERSION/" macos/Info.plist.in > "$OUT/Contents/Info.plist"

# Build an .icns from the generated PNG icons.
ICONSET="$(mktemp -d)/AlgoClerk.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    cp "assets/icons/algoclerk-$s.png" "$ICONSET/icon_${s}x${s}.png"
done
cp assets/icons/algoclerk-32.png "$ICONSET/icon_16x16@2x.png"
cp assets/icons/algoclerk-64.png "$ICONSET/icon_32x32@2x.png"
cp assets/icons/algoclerk-256.png "$ICONSET/icon_128x128@2x.png"
cp assets/icons/algoclerk-512.png "$ICONSET/icon_256x256@2x.png"
cp assets/icons/algoclerk-1024.png "$ICONSET/icon_512x512@2x.png"
iconutil -c icns "$ICONSET" -o "$OUT/Contents/Resources/AlgoClerk.icns"

if [ -n "${ALGOCLERK_SIGN_IDENTITY:-}" ]; then
    codesign --force --options runtime --deep --sign "$ALGOCLERK_SIGN_IDENTITY" "$OUT"
fi
(cd dist && zip -qr "AlgoClerk-$VERSION-macos-${TARGET%%-*}.zip" AlgoClerk.app)
echo "Built dist/AlgoClerk-$VERSION-macos-${TARGET%%-*}.zip"
