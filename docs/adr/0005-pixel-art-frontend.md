# ADR 0005: a pixel-art front end on Bevy

Status: accepted. Supersedes ADR 0001 (egui) for the front end.

## Context

The egui front end was a generic flat dark-mode widget UI with a tiny clerk and an empty floor. The sibling
project Falabichos (same owner, same licence) already has a cohesive pixel-art presentation: a low-resolution
virtual screen with integer scaling, a 5x7 font, cream panels with ink borders, text-defined sprites and
chiptune songs, drawn by an engine-agnostic software renderer and shown through a thin Bevy shell.

## Decision

- All drawing goes through `algoclerk-media`'s software `Canvas` into a 480x270 (landscape) or 270x480
  (portrait) virtual screen; the shell uploads it as one nearest-neighbour texture with an integer scale.
- Art, font, palettes and music are **text definitions** (`.px`, `font.txt`, `palette*.txt`, `.song`) written
  for AlgoClerk, linted by tests (colour limit, frame counts, glyph coverage).
- `algoclerk-game` is engine-agnostic (`Game::tick(&Input)`, `Game::draw(&mut Canvas)`), so every screen is
  tested and screenshotted with no window and no GPU. `algoclerk-app` is a thin Bevy 0.19.1 shell
  (window, input, texture, audio, lifecycle) shared by desktop, Android (GameActivity) and iOS.
- Value widgets replace numbered floor tiles: crates, letter tiles, shelves, drawers, bins, spring plates,
  belts, pyramids, tag chains and org charts.

## Reused from Falabichos and what was learned

Reused (with attribution in CREDITS.md): the palette/sprite/font/canvas/synth engine design, the integer
scaled display and safe-area handling, the Bevy shell structure, the Android and iOS project skeletons and
the headless screenshot approach. New for AlgoClerk: all art, every screen, the value widgets, the card and
text editors, the code reveal.

Learned: keep the game crate free of engine types so the whole UI is testable headless; enforce layout with a
test that every tap rectangle stays inside the screen at many window sizes; keep text in the i18n files and
lint the font against every string and every generated code sample.
