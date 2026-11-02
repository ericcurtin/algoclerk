# Changelog

## Unreleased: ClerkScript and the pixel-art front end

### Changed (breaking)

- **The game now teaches a high-level typed language, ClerkScript**, instead of assembly. Each level is a
  function with a typed signature (`fn two_sum(nums: vec<int>, target: int) -> vec<int>`); the player writes the
  body using strings, vectors, maps, sets, stacks, queues, heaps, linked lists and trees. See `docs/LANGUAGE.md`
  and ADR 0006. The assembler, VM, instruction set and the 48 assembly levels are removed.
- **New campaign of 80 levels in ten wings** (Mail Room, Records Office, Filing Cabinets, Stack Room, Search
  and Sort Annex, Grid Hall, Chain and Tree Gallery, Strategy Suite, Basement Bits, Executive Floor) with
  hidden large tests, a naive solution per level that must time out, and independent Rust oracles.
- **Cost model and complexity estimator**: speed is abstract cost that mirrors real complexity; solved levels
  show a measured time and space class.
- **Pixel-art front end** (ADR 0005) replaces the egui one: 480x270 / 270x480 virtual screen with integer scale,
  tile-built animated office where every variable lives in a tray drawn as a crate, letter tiles, shelf, drawer,
  bin, spring plates, belt, pyramid, tag chain or org chart; card editor and text editor in sync; memo; victory
  with stars and medals; code reveal in C, C++ and Rust; original chiptune music and effects.
- Code generation rewritten for ClerkScript: idiomatic C++17, Rust and C with compile-and-compare tests
  against the interpreter (every reference solution plus random type-correct programs).

### Added

- `algoclerk-cli`: `check`, `run`, `judge`, `validate`, `table`, `tune`, `uses`, `codegen --verify`.
- Level files gained `signature`, `types`, `builtins`, `locked`, `[unlock]`, `[budgets]`, `naive`, `compare`,
  typed seeded `[[generate]]` blocks and `[sweep]`.

## 0.1.0

First release: assembly puzzle game with an egui front end, 48 levels, and code generation to C, C++ and Rust.
