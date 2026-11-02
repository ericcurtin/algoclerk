#!/usr/bin/env bash
# Regenerate the README screenshots with the headless software renderer (no window, display or GPU).
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CARGO_TARGET_DIR:=target}"
cargo build -q -p algoclerk-app --bin algoclerk
BIN="$CARGO_TARGET_DIR/debug/algoclerk"
OUT=docs/screenshots
mkdir -p "$OUT"
shot() { "$BIN" --screenshot "$@" 2>/dev/null; }
shot title   --out "$OUT/title.png"      --width 1000 --height 640
shot levels  --out "$OUT/levels.png"     --width 1000 --height 720
shot play    --out "$OUT/play.png"       --width 1280 --height 800 --level max-of --steps 30
shot play    --out "$OUT/play-phone.png" --width 780 --height 1688 --scale 2 --level max-of --steps 30 --tab office
shot victory --out "$OUT/victory-rust.png" --width 1000 --height 1000 --level max-of --lang rust
shot victory --out "$OUT/victory-c.png"    --width 1000 --height 1000 --level fibonacci --lang c
shot sandbox --out "$OUT/sandbox.png"    --width 1280 --height 800
echo "wrote screenshots to $OUT"
