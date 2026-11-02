# Good first issues

Ideas that are well-scoped and friendly to newcomers. Open an issue (or comment on one) to claim an item.
"Level" items only touch `levels/` (copy a neighbouring file, run `algoclerk-cli validate`); "Translation"
items only touch `crates/algoclerk-core/lang/`; "Art" and "Music" items only touch `assets/`.

## Levels (data only)

1. **Valid sudoku row**: `fn valid_row(row: vec<int>) -> bool` (sets).
2. **Roman numerals**: `fn from_roman(s: string) -> int` (map and a look-ahead).
3. **Pascal's triangle row**: `fn pascal(n: int) -> vec<int>` (nested loops, vec of vec).
4. **Happy numbers**: cycle detection with a set (or Floyd) on digit-square sums.
5. **Remove duplicates from a sorted vec** with two pointers (push is locked).
6. **Valid mountain array**: one pass with two states.
7. **Search a 2D matrix** (binary search on a flattened index).
8. **Add two numbers as linked lists** (carry handling, `lnode`).
9. **Same tree / mirror tree** (recursion with helpers).
10. **Jump game** (greedy reach) and **Gas station** (prefix sums).

Each needs: signature, story, statement, at least two examples and three hints, a reference solution,
a `naive` solution that times out on the large tests, seeded generators, and an oracle in
`crates/algoclerk-levels/tests/oracles.rs`.

## Translation

- A new UI language: copy `en.toml`, translate the values (see `docs/TRANSLATING.md`).
- Accented Latin glyphs for the 5x7 font (`assets/art/font.txt`), then a French or German catalogue.

## Art

- **Clerk outfits**: new hair / shirt / pants colour presets in `assets/art/outfits.txt`, or a third body
  template `_body_c.px` (see `docs/ART_PIPELINE.md`).
- **Props**: a coat rack, a printer, a fish tank, a second desk plant (16x16 `.px` with 2 to 4 frames).
- **Value widgets**: nicer pixel art for the stack plates, the heap pyramid or the tag chain
  (`crates/algoclerk-game/src/value_view.rs` draws them with primitives today; a sprite version is welcome).
- **Palette**: tune `palette_cb.txt` against a colour-blindness simulator; keep the letters.

## Music

- A calmer second office loop, a "boss floor" track for the Executive Floor wing, a short jingle per wing.
  Songs are plain text (`assets/music/*.song`); keep them original.

## Engineering

- Explain a runtime error with the exact card it happened on (highlight the card and the tray).
- Put the "step into helper function" view on a second tray row.
- A `cargo xtask screenshots` wrapper around `scripts/screenshots.sh`.
