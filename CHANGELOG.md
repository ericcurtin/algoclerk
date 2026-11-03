# Changelog

## Unreleased: real C++ and the pixel-art front end

### Changed (breaking)

- **Players now write real C++17 instead of ClerkScript**, the invented language of the previous development
  builds. Each level is a real function signature (`std::vector<int> twoSum(const std::vector<int>& nums, int
  target)`) with a read-only prelude of `#include` lines and helper structs; the player writes the function
  and, where a level allows, helpers. The accepted subset is documented in `docs/CPP_SUBSET.md`; every accepted
  program is valid C++17 that behaves the same with `g++` and `clang++`. The game still runs no compiler: it
  has its own lexer, parser, type checker and interpreter. See ADR 0007 (ADR 0006, which introduced
  ClerkScript, is kept as history).
- **Undefined behaviour is detected and reported**, not silently accepted: out-of-range `operator[]` and `at()`,
  signed overflow, division by zero, invalid shifts, null, dangling and deleted pointers, uninitialised reads,
  iterator invalidation, unsequenced modification, missing returns, empty-container reads and non-strict
  comparators. The failing line is highlighted with a clerk-style message and the judge rejects the program.
- **Old solutions cannot be migrated.** Drafts and saved solutions written in ClerkScript are in a language that
  no longer exists, so they are cleared. The save file format is now version 2; version 1 files load without
  crashing and keep settings, solved levels, stars and medals. The game shows a note once.
- **All 80 original levels were converted to C++** (ten wings: Mail Room, Records Office, Filing Cabinets,
  Stack Room, Search and Sort Annex, Grid Hall, Chain and Tree Gallery, Strategy Suite, Basement Bits,
  Executive Floor), and an eleventh wing was added (see "Added"), so the campaign has 86 levels. Signatures, preludes, reference and naive solutions, hint ladders and the lock lists (now real headers,
  `std::` names and members with a reason each) were rewritten. Level files still written for the old language
  are skipped with a warning.
- **The cost model mirrors the STL**: `unordered_map` operations cost 1, `map`, `set` and `priority_queue` log n,
  `std::sort` n log n, copies cost the elements copied, so passing a vector by value or by `const&` matters
  (`docs/COST_MODEL.md`). Judge, hidden large tests, naive-solution timeouts, medals and the empirical time and
  space classes run on top of it.
- **Code reveal**: the C++ tab is the player's own accepted solution, formatted, and the victory screen opens on
  it. The Rust and C tabs are connected to `cpp::codegen`: translations of the checked C++ tree that compile with
  `rustc`, `gcc` and `clang` and match the interpreter for every level reference and naive solution
  (`docs/CODEGEN.md`). One level, `drop-negatives` (a `std::list` erase loop), is not translated to Rust and C
  and shows a friendly message. The command line prints all three.
- **Pixel-art front end** (ADR 0005) replaces the egui one: 480x270 / 270x480 virtual screen with integer scale,
  tile-built animated office where every variable lives in a tray drawn as a crate, letter tiles, shelf,
  drawer, bin, spring plates, belt, pyramid, tag chain or org chart, with C++ type names; card editor and text
  editor in sync with syntax colours, `std::` autocomplete and an on-screen keyboard; memo with the prelude and
  the lock reasons; victory with stars and medals; original chiptune music and effects.

### Added

- **The Supply Cupboard**, an eleventh wing (`levels/supply-cupboard`) of six levels in which C++ semantics are
  the lesson: pass by reference versus by value, the erase-remove idiom, a custom comparator,
  `std::priority_queue` with `std::greater`, `std::string_view` versus a copy, and the iterator `erase` idiom.
  53 of the 86 levels now have a naive solution that must time out on the large tests (47 original plus 6 new).
  The measured time and space classes of every level are in the judge table (`algoclerk-cli table`).
- The windowed game was built and run in a Fedora 44 container (Xvfb, Mesa lavapipe) and driven with real input;
  `docs/screenshots/window-*.png` are captures of that run.
- Differential tests against real compilers: every reference and naive solution is built with `g++` and
  `clang++` (`-Wall -Wextra`, sanitizers for the reference) and compared with the interpreter and with
  independent Rust oracles; a proptest generator of random UB-free programs; a negative table with one program
  per undefined-behaviour class; parser round trip and fuzz tests. CI installs `g++`, `clang++` and `rustc`.
  The divergence exclusion lists of the property tests (`KNOWN`, `UB_KNOWN`, `KNOWN_INTERNAL`) are empty: 9000+
  random programs agreed with `g++` (and a `clang++` plus sanitizer sample), about 300000 fuzz mutants ran
  without a panic, and 110 negative programs cover the undefined-behaviour classes.
- `algoclerk-cli`: `list`, `show`, `starter`, `reference`, `check`, `run`, `judge`, `validate`, `table`, `tune`,
  `codegen --lang c|cpp|rust --verify`, `harness`, `diff`, and `--levels DIR` to overlay edited level files.
- Level files gained `prelude`, a C++ `signature`, `unlocked` headers and `locked` names with reasons,
  `[unlock]`, `[budgets]`, `naive`, `compare`, typed seeded `[[generate]]` blocks and `[sweep]`.

### Known limits

- The checker deliberately rejects `std::string += unsigned char` and `+= int`, and erased-iterator detection for
  ordered containers (`std::map`, `std::set`) is key based. `short` and `unsigned short` are refused (use `int`);
  `unsigned char` is supported. `long`, `double` and templates are not in the subset (`docs/CPP_SUBSET.md`).
- Only Linux was built and run on this branch; the Android, iOS, macOS and Windows app builds were not repeated
  (`docs/BUILDING.md`, `docs/PROGRESS.md`).

### Removed

- ClerkScript (lexer, parser, checker, interpreter, translators and `docs/LANGUAGE.md`), the assembler, VM,
  instruction set and the 48 assembly levels, and the level fields `types` and `builtins`.

## 0.1.0

First release: assembly puzzle game with an egui front end, 48 levels, and code generation to C, C++ and Rust.
