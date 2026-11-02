#!/usr/bin/env bash
# Smoke test of the REAL desktop window without a display or GPU: runs the game in a virtual X
# server (Xvfb) with Mesa's software Vulkan driver (lavapipe), drives it with real mouse and
# keyboard events (xdotool), solves the first level by typing a program, and saves screenshots.
#
# Needs: Xvfb, xdotool, ImageMagick (import), a Vulkan software driver (mesa-vulkan-drivers),
#        libxkbcommon-x11, libXcursor, libXi, libXrandr.
# Usage: scripts/gui-smoke.sh [output-dir]      (default: dist/gui-smoke)
# The click coordinates assume the default 1180x760 window at the top-left of a 1280x800 screen.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-dist/gui-smoke}"
mkdir -p "$OUT"
: "${CARGO_TARGET_DIR:=target}"
cargo build -q -p algoclerk-app --bin algoclerk
BIN="$CARGO_TARGET_DIR/debug/algoclerk"

D=$((100 + RANDOM % 800))
export DISPLAY=":$D"
export HOME="$(mktemp -d)"          # fresh progress
export VK_ICD_FILENAMES="$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1)"
timeout -s KILL 170 Xvfb ":$D" -screen 0 1280x800x24 >/dev/null 2>&1 &
XPID=$!
APID=""
trap 'kill $APID $XPID 2>/dev/null || true' EXIT
sleep 2
timeout -s KILL 150 "$BIN" >"$OUT/app.log" 2>&1 &
APID=$!
sleep 10
kill -0 "$APID" || { echo "the game exited early"; cat "$OUT/app.log"; exit 1; }

click() { xdotool mousemove "$1" "$2"; sleep 0.5; xdotool mousedown 1; sleep 0.2; xdotool mouseup 1; sleep 0.4; }
WID="$(xdotool search --name AlgoClerk | head -1)"
xdotool windowfocus "$WID"; sleep 1
import -window root "$OUT/1-title.png"
click 560 468            # Start the campaign
import -window root "$OUT/2-levels.png"
click 110 150            # first level
click 1144 55            # open the text panel
sleep 1
click 1000 250           # focus the text editor
xdotool key --delay 100 a g a i n colon Return i n b o x Return o u t b o x Return j u m p space a g a i n
sleep 1
import -window root "$OUT/3-typed.png"
click 98 627             # Run
sleep 5
import -window root "$OUT/4-running.png"
click 180 663            # Check
sleep 2
import -window root "$OUT/5-victory.png"
kill -0 "$APID" || { echo "the game crashed"; cat "$OUT/app.log"; exit 1; }
if [ "$(sha256sum < "$OUT/1-title.png")" = "$(sha256sum < "$OUT/5-victory.png")" ]; then echo "screen did not change"; exit 1; fi
echo "ok: screenshots in $OUT"
