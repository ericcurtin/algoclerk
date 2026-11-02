#!/usr/bin/env bash
# Regenerate docs/screenshots with the headless software renderer (no window, display or GPU).
# Every scenario is rendered in landscape; the main screens also in portrait (phone layout).
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CARGO_TARGET_DIR:=target}"
cargo build -q -p algoclerk-tools --bin algoclerk-shots
OUT=docs/screenshots
mkdir -p "$OUT"
rm -f "$OUT"/*.png
"$CARGO_TARGET_DIR/debug/algoclerk-shots" "$OUT" --scale 3
echo "wrote screenshots to $OUT"
