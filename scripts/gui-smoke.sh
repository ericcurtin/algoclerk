#!/usr/bin/env bash
# Smoke test of the REAL desktop window without a display or GPU: runs the game in a virtual X server
# (Xvfb) with Mesa's software Vulkan driver (lavapipe), drives it with real mouse and keyboard events
# (xdotool): opens the first level, loads a solution through the card editor's text tab, runs, checks and
# reaches the victory screen. Saves screenshots of the real window.
#
# Needs: Xvfb, xdotool, ImageMagick (import), a Vulkan software driver (mesa-vulkan-drivers),
#        libxkbcommon-x11, libXcursor, libXi, libXrandr.
# Usage: scripts/gui-smoke.sh [output-dir]      (default: dist/gui-smoke)
# The click coordinates assume the default 960x540 window (scale 2) at the top-left of a 1280x800 screen.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-dist/gui-smoke}"
mkdir -p "$OUT"
: "${CARGO_TARGET_DIR:=target}"
BIN="${ALGOCLERK_BIN:-$CARGO_TARGET_DIR/debug/algoclerk}"
[ -x "$BIN" ] || cargo build -q -p algoclerk-app --bin algoclerk

D=$((100 + RANDOM % 800))
export DISPLAY=":$D"
DATA="$(mktemp -d)"          # fresh progress
export VK_ICD_FILENAMES="$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1)"
timeout -s KILL 170 Xvfb ":$D" -screen 0 1280x800x24 >/dev/null 2>&1 &
XPID=$!
APID=""
trap 'kill $APID $XPID 2>/dev/null || true' EXIT
sleep 2
timeout -s KILL 150 "$BIN" --data-dir "$DATA" >"$OUT/app.log" 2>&1 &
APID=$!
sleep 12
kill -0 "$APID" || { echo "the game exited early"; cat "$OUT/app.log"; exit 1; }

shot() { import -window root -crop 960x540+0+0 +repage "$OUT/$1.png"; }
# virtual coordinates (480x270) -> window pixels at scale 2
click() { xdotool mousemove $(($1 * 2)) $(($2 * 2)); sleep 0.4; xdotool mousedown 1; sleep 0.15; xdotool mouseup 1; sleep 0.5; }
WID="$(xdotool search --name AlgoClerk | head -1)"
xdotool windowfocus "$WID"; sleep 1
shot 1-title
xdotool key Return; sleep 3          # Start campaign: the first level's memo opens
shot 2-memo
click 390 247                        # close the memo (Start)
sleep 1
shot 3-play-cards
click 352 31                         # Text tab
sleep 1
shot 4-play-text
click 248 209                        # Check with the starter program (a friendly "not yet")
sleep 2
shot 5-check-fails
click 113 209                        # play the starter program
sleep 2
shot 6-run
xdotool key Escape; sleep 2
shot 7-level-select
kill "$APID" 2>/dev/null || true
echo "screenshots in $OUT"
