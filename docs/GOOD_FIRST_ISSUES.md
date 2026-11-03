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
"office tools" are open topics (see the roadmap). The campaign has 86 levels in 11 wings today.

### Levels where C++ semantics are the lesson

The eleventh wing, The Supply Cupboard (`levels/supply-cupboard`), already has six: pass by reference versus by
value, the erase-remove idiom, a custom comparator, `std::priority_queue` with `std::greater`,
`std::string_view` versus a copy, and the iterator `erase` idiom. More variations are good first issues because
the cost model already charges for the thing being taught (`docs/COST_MODEL.md`); copy one of those files and
change the story and the data. Ideas that are not covered yet:

- **Another comparator**: sort pairs or words by two keys with a lambda, or a `std::set` / `std::map` with a
  comparator type; require a full tie-break so the answer does not depend on the order of equal elements.
- **A second heap level**: k closest points or merging sorted streams with `std::priority_queue` and a lambda
  comparator, with the lock on `std::sort` explaining why.
- **A by-value trap in a recursive helper**: a DFS that takes a `std::vector<std::vector<int>>` by value and
  blows the budget, where `const&` passes.
- **`std::unique` and `erase`** on a sorted vector, or `std::remove_if` with a lambda, as a second erase-remove
  level with a different naive solution.
- **Iterator invalidation in a `std::map` or `std::deque`**: a naive version whose loop erases while iterating
  (the interpreter reports it), and the `it = m.erase(it)` fix.

## Language support (the C++ subset)

The subset has documented gaps (`docs/CPP_SUBSET.md`). Every item is a self-contained change with tests, and
none is done until the interpreter and the real compilers provably agree:

- **Support one more `std::` member or function** (for example `std::gcd`, `std::lcm`, `std::is_sorted` or a
  `std::string` overload that the library table does not list yet). The recipe, step by step, is "Adding a `std::` member" in
  CONTRIBUTING.md: stdlib table row, checker, interpreter, cost, card flag, documentation table.
- **Widen the property generator**: the `KNOWN` list in `crates/algoclerk-core/tests/prop_common/mod.rs` is empty,
  so every construct the generator can produce already agrees with `g++`. Teach it a new construct (a library
  call, a statement shape) and run `cpp_prop_diff` with a few hundred cases; a disagreement is a bug to fix.
- **One of the deliberate gaps** in "Not supported (yet)" in `docs/CPP_SUBSET.md` (for example `short`, or a
  data-only feature of `struct`), provided the interpreter and both compilers provably agree afterwards.
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
- Teach the Rust and C translators the one campaign construct they still refuse (`drop-negatives`: an iterator
  erase loop over a `std::list`).
- A `cargo xtask screenshots` wrapper around `scripts/screenshots.sh`.
