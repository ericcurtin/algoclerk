# Good first issues

Ideas that are well-scoped and friendly to newcomers. Open an issue (or comment on one) to claim an item.
"Level" items touch `levels/` and the oracle in the levels tests (copy a neighbouring file, run
`algoclerk-cli validate`); "Language support" items touch the C++ front end and its tests; "Translation" items
touch `crates/algoclerk-core/lang/` (or, for code generation idioms, the translators); "Art" and "Music" items
only touch `assets/`.

## Levels

Level ideas (write the C++ reference, a naive solution, examples, hints and an oracle; see CONTRIBUTING.md):

1. **Valid sudoku row**: `bool validRow(const std::vector<int>& row)` (`std::unordered_set`).
2. **Roman numerals**: `int fromRoman(const std::string& s)` (a map and a look-ahead).
3. **Pascal's triangle row**: `std::vector<int> pascal(int n)` (nested loops, `vector<vector<int>>`).
4. **Happy numbers**: cycle detection with a set (or Floyd) on digit-square sums.
5. **Remove duplicates from a sorted vector** with two pointers (`std::unique` and `<algorithm>` locked).
6. **Valid mountain array**: one pass with two states.
7. **Search a 2D matrix** (binary search on a flattened index).
8. **Add two numbers as linked lists** (carry handling, `ListNode*`).
9. **Same tree / mirror tree** (recursion with helpers).
10. **Jump game** (greedy reach) and **Gas station** (prefix sums).

Each needs: prelude, signature, story, statement, at least two examples and three hints, a reference
solution, a `naive` solution that times out on the large tests, seeded generators, and an oracle in
`crates/algoclerk-levels/tests/common/mod.rs`. A new **wing** is a new directory under `levels/` with a
`pack.toml` (`title`, `blurb`, `order`) and four to ten levels; tries, union-find, intervals, backtracking and
"office tools" are open topics (see the roadmap).

### Levels where C++ semantics are the lesson

These are good first issues because the cost model already charges for the thing being taught
(`docs/COST_MODEL.md`):

- **Pass by reference versus by value**: the same task twice, or one task with a large input where a by-value
  `std::vector<int>` parameter blows the budget and `const std::vector<int>&` passes.
- **The iterator `erase` idiom**: remove matching elements from a vector or list in place
  (`it = v.erase(it)`), where the naive version invalidates its iterator (the interpreter reports it).
- **Custom comparator sorting**: sort intervals or words with a lambda; lock `std::stable_sort` or require a
  tie-break so the answer does not depend on the order of equal elements.
- **`std::priority_queue` with `std::greater`**: a min-heap (k smallest, merge streams) with the lock on
  `std::sort` explaining why.
- **`std::string_view` versus copying a string**: slicing a long string many times; `substr` of a `std::string`
  costs its length, a `string_view` slice costs 1.

## Language support (the C++ subset)

The subset has documented gaps (`docs/CPP_SUBSET.md`). Every item is a self-contained change with tests, and
none is done until the interpreter and the real compilers provably agree:

- **Support one more `std::` member or function** (for example a `std::string` overload, `std::partial_sum`,
  `std::for_each`; the generator's `KNOWN` list in `crates/algoclerk-core/tests/prop_common/mod.rs` names a few
  that are checked but not interpreted). The recipe, step by step, is "Adding a `std::` member" in
  CONTRIBUTING.md: stdlib table row, checker, interpreter, cost, card flag, documentation table.
- **Shrink the `KNOWN` list**: pick an entry, fix the interpreter or checker, delete the entry, run
  `cpp_prop_diff` with a few hundred cases.
- **A friendlier message** for a construct outside the subset (the checker's wording lives in
  `crates/algoclerk-core/src/cpp/check/`).
- **A new undefined-behaviour check** with a negative case in `cpp_prop_ub.rs`.
- **More corpus programs**: realistic solutions in `crates/algoclerk-core/tests/data/cpp_corpus/` exercise the
  parser, checker and printer.

## Translation

- A new UI language: copy `en.toml`, translate the values (see `docs/TRANSLATING.md`).
- Accented Latin glyphs for the 5x7 font (`assets/art/font.txt`), then a French or German catalogue.
- **Translations and code generation idioms**: the Rust and C translators (`docs/CODEGEN.md`) refuse
  constructs they cannot translate faithfully. Teaching them one more idiom (a library call, a loop shape) so
  the Rust or C tab reads like hand-written code is a well-scoped issue; `cpp_codegen_random` and
  `cpp_codegen_levels` check the result against the interpreter.

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
- Connect the Rust and C translations (`cpp::codegen`) to the tabs of the victory screen (`result.rs`).
- A `cargo xtask screenshots` wrapper around `scripts/screenshots.sh`.
