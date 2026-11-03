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
- **All 80 levels were converted to C++** (ten wings: Mail Room, Records Office, Filing Cabinets, Stack Room,
  Search and Sort Annex, Grid Hall, Chain and Tree Gallery, Strategy Suite, Basement Bits, Executive Floor).
  Signatures, preludes, reference and naive solutions, hint ladders and the lock lists (now real headers,
  `std::` names and members with a reason each) were rewritten. Level files still written for the old language
  are skipped with a warning.
- **The cost model mirrors the STL**: `unordered_map` operations cost 1, `map`, `set` and `priority_queue` log n,
  `std::sort` n log n, copies cost the elements copied, so passing a vector by value or by `const&` matters
  (`docs/COST_MODEL.md`). Judge, hidden large tests, naive-solution timeouts, medals and the empirical time and
  space classes run on top of it.
- **Code reveal**: the C++ tab is the player's own accepted solution, formatted. Rust and C are translations of
  the checked C++ tree with compile-and-compare tests (`docs/CODEGEN.md`). The command line prints all three;
  connecting the Rust and C tabs of the victory screen is still in progress (`docs/PROGRESS.md`).
- **Pixel-art front end** (ADR 0005) replaces the egui one: 480x270 / 270x480 virtual screen with integer scale,
  tile-built animated office where every variable lives in a tray drawn as a crate, letter tiles, shelf,
  drawer, bin, spring plates, belt, pyramid, tag chain or org chart, with C++ type names; card editor and text
  editor in sync with syntax colours, `std::` autocomplete and an on-screen keyboard; memo with the prelude and
  the lock reasons; victory with stars and medals; original chiptune music and effects.

### Added

- Differential tests against real compilers: every reference and naive solution is built with `g++` and
  `clang++` (`-Wall -Wextra`, sanitizers for the reference) and compared with the interpreter and with
  independent Rust oracles; a proptest generator of random UB-free programs; a negative table with one program
  per undefined-behaviour class; parser round trip and fuzz tests. CI installs `g++`, `clang++` and `rustc`.
- `algoclerk-cli`: `list`, `show`, `starter`, `reference`, `check`, `run`, `judge`, `validate`, `table`, `tune`,
  `codegen --lang c|cpp|rust --verify`, `harness`, `diff`, and `--levels DIR` to overlay edited level files.
- Level files gained `prelude`, a C++ `signature`, `unlocked` headers and `locked` names with reasons,
  `[unlock]`, `[budgets]`, `naive`, `compare`, typed seeded `[[generate]]` blocks and `[sweep]`.

### Removed

- ClerkScript (lexer, parser, checker, interpreter, translators and `docs/LANGUAGE.md`), the assembler, VM,
  instruction set and the 48 assembly levels, and the level fields `types` and `builtins`.

## 0.1.0

First release: assembly puzzle game with an egui front end, 48 levels, and code generation to C, C++ and Rust.
