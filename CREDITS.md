# Credits and asset licences

## Code

AlgoClerk is licensed under the Apache License 2.0 (see `LICENSE` and `NOTICE`). Every Rust file starts with
an SPDX header. Third-party crates are all permissively licensed; `cargo deny check licenses bans` verifies
this in CI and the accepted licence list is in `deny.toml`.

### Reused from Falabichos

The sibling project [Falabichos](https://github.com/ericcurtin/falabichos) (same owner, Apache-2.0) is the
origin of the pixel-art engine design that was adapted here: the palette / `.px` sprite / bitmap font /
canvas / text-song synthesiser crate (`algoclerk-media`), the integer-scaled virtual screen and safe-area
handling (`algoclerk-game/src/layout.rs`), the Bevy shell structure (`algoclerk-app`), the Android
GameActivity and iOS project skeletons, and the headless screenshot approach. **No art, music or text was
copied from Falabichos**: all sprites, the font glyph set, the palettes, the songs and every screen of
AlgoClerk are new.

### Dual-licensed dependencies

Some crates in the Bevy dependency tree are dual licensed (`MIT OR Apache-2.0`, and in a few cases
`Apache-2.0 OR GPL-2.0-only` such as `self_cell`). AlgoClerk uses them under the permissive alternative,
which `cargo deny` accepts because `deny.toml` does not allow the copyleft alternative.

## Art, font, music, levels and text

- All sprites (`assets/art/**/*.px`), the 5x7 and 3x5 bitmap fonts, the three palettes and the clerk
  templates were drawn for this project as text definitions. No external image files, no scanned or traced
  art, nothing from Human Resource Machine or any other game.
- All music and sound effects (`assets/music/*.song`) are original text-defined chiptune scores rendered by
  the built-in synthesiser at run time; there are no audio recordings.
- The application icon PNGs in `assets/icons/` were rendered from the game's own art.
- All 80 levels, stories, statements, hints, oracles and UI strings were written for this project. Problems
  are described in the project's own words.

## Inspiration

AlgoClerk is inspired by the genre of programming puzzle games and by classic algorithm interview topics.
No assets, text, names, characters, level designs or problem statements were copied from any game or website.
