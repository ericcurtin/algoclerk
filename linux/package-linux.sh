#!/usr/bin/env bash
# Build a tar.gz of AlgoClerk for Linux.
# Usage: linux/package-linux.sh [--features audio]   (audio needs libasound2-dev / alsa-lib-devel)
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(uname -m)"
cargo build --release -p algoclerk-app --bin algoclerk "$@"
STAGE="dist/algoclerk-$VERSION-linux-$ARCH"
rm -rf "$STAGE" && mkdir -p "$STAGE/share/applications" "$STAGE/share/icons/hicolor/256x256/apps" "$STAGE/share/metainfo"
cp "${CARGO_TARGET_DIR:-target}/release/algoclerk" "$STAGE/algoclerk"
cp linux/algoclerk.desktop "$STAGE/share/applications/"
cp linux/algoclerk.png "$STAGE/share/icons/hicolor/256x256/apps/algoclerk.png"
cp linux/org.algoclerk.AlgoClerk.metainfo.xml "$STAGE/share/metainfo/"
cp LICENSE NOTICE README.md CREDITS.md "$STAGE/"
tar -C dist -czf "$STAGE.tar.gz" "$(basename "$STAGE")"
echo "Built $STAGE.tar.gz"
