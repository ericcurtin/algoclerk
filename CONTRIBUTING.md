# Contributing to AlgoClerk

Thank you for helping! AlgoClerk is meant to grow into a community project: levels, translations,
themes, fixes and ideas are all welcome. Please read the [Code of Conduct](CODE_OF_CONDUCT.md) first.

## Quick start for developers

```sh
git clone <your fork>
cd algoclerk
export CARGO_TARGET_DIR=$HOME/algoclerk-target     # optional: keep build output out of the tree
cargo test --workspace                              # runs everything (see below for the compilers)
cargo run --release -p algoclerk-app --bin algoclerk
```

You need Rust 1.95+. Players write real C++17 and the repository proves that the game's interpreter agrees
with real compilers, so the full test suite needs **`g++`, `clang++` and `rustc`** on `PATH` (and GNU
`timeout`, which every compiler run is wrapped in). A test whose compiler is missing skips itself with a
message; CI has all of them. On Linux the window code needs no `-devel` packages; Wayland, X11 and Vulkan/OpenGL
libraries are loaded at run time. The optional `audio` feature needs ALSA headers (`libasound2-dev`).

### Checks that CI runs (please run them before pushing)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check                       # licences, bans, advisories (cargo install cargo-deny)
cargo run -p algoclerk-cli -- validate
```

CI additionally runs more random programs against both compilers (`ALGOCLERK_DIFF_CLANG=1`,
`ALGOCLERK_PROP_CASES=...`, see [docs/BUILDING.md](docs/BUILDING.md)). If you touch the parser, checker,
interpreter or code generators, run the differential tests with a few hundred cases yourself.

Take care with long-running commands: tests run the interpreter with cost limits and compile with
compiler time limits, but a runaway process of your own is your responsibility.

### Where things live

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). In short: all logic is in `crates/algoclerk-core`
(no GUI dependency, `#![forbid(unsafe_code)]`), levels are data in `levels/`, the game is a thin
Bevy shell in `crates/algoclerk-app` around the engine-free game in `crates/algoclerk-game`. The C++ front
end and interpreter are `crates/algoclerk-core/src/cpp/`.

## Writing levels (the easiest way to contribute)

Levels are TOML files; you write C++. Read [docs/LEVEL_FORMAT.md](docs/LEVEL_FORMAT.md) (fields, value
literals, generators, the rules for code that is valid on every compiler) and
[docs/CPP_SUBSET.md](docs/CPP_SUBSET.md) (what players can write), and [docs/COST_MODEL.md](docs/COST_MODEL.md)
(what a solution costs).

1. Pick a classic algorithm topic and write the story, statement, hints and names **yourself**.
   Do not copy text, puzzles, level designs or art from other games or from problem websites.
   Topics and ideas are fine; sentences are not.
2. Copy a neighbouring file in `levels/<wing>/`. A level has a read-only `prelude` (`#include` lines and
   helper structs such as `ListNode`), a real C++ `signature` with parameter names (`std::vector<int>
   twoSum(const std::vector<int>& nums, int target)`, camelCase), `unlocked` headers, `locked` names with a
   reason each, and examples written in C++ brace notation (`{2, 7, 11, 15}`).
3. Write the **reference** as idiomatic modern C++ (range-for, `auto`, `const&`, structured bindings, standard
   algorithms where the level allows them), not a transliteration from another language. It must compile
   warning-free with `g++ -std=c++17 -Wall -Wextra` and `clang++`, and have no undefined behaviour.
4. Write a **naive** solution when the topic has one: correct, but slow enough that it runs out of budget on
   the large test. The validator requires that it passes the standard tests and times out on the large one.
5. Add hand-written edge cases with explicit outputs, seeded generated cases including a large one (and a
   `[sweep]` generator that forces the worst case, so the complexity estimator measures it), and a three-step
   hint ladder. Add a `technique` / `big_o` note naming the algorithm.
6. Run the flow:

   ```sh
   cargo run -p algoclerk-cli -- check my.cpp --level <id>        # does the player's text pass the checker?
   cargo run -p algoclerk-cli -- judge <id> my.cpp                # tests, cost, medals, estimated class
   cargo run -p algoclerk-cli -- tune <id> --write                # medal targets from the reference
   cargo run -p algoclerk-cli -- validate <id>                    # every guarantee the campaign makes
   cargo run -p algoclerk-cli -- diff <id>                        # build reference and naive with g++ and clang++
   ```

   `--levels DIR` makes the CLI read edited level files from a directory instead of the embedded ones, so you
   do not need to rebuild while iterating.
7. Add an independent **oracle**: plain Rust that computes the answer without looking at your reference. It
   goes into `oracle()` in `crates/algoclerk-levels/tests/common/mod.rs` (a `match` on the level id; the
   oracle test fails with "no oracle" for a level without one). The oracles are compared with the expected
   value of every test, and again with the output of the compiled reference.
8. `cargo test -p algoclerk-levels` (add `ALGOCLERK_LEVEL=<id>` to run one level) and open a pull request using
   the template.

Traps to know about (the full list is in LEVEL_FORMAT.md): `int` is 32 bits and overflow is undefined
behaviour, so use `long long` where a value or an intermediate can pass two billion; never rely on
`unordered_map` iteration order or on the order of equal elements after `std::sort`; every header a name needs
must be named in the prelude or in `unlocked`.

C++ semantics make good levels in their own right: pass by reference versus by value (a by-value
`std::vector<int>` costs its size on every call), the iterator `erase` idiom, sorting with a custom
comparator, `std::priority_queue` with `std::greater`, `std::string_view` versus copying a string. If a
level needs a construct the subset lacks, see the next section.

## Language support: subset gaps and the differential tests

The subset is a **contract**: everything accepted must be valid C++17 with the same meaning under `g++` and
`clang++`. Gaps are documented in [docs/CPP_SUBSET.md](docs/CPP_SUBSET.md); closing one is welcome, but it is
not done until the tests prove the game agrees with the compilers.

- Corpus and unit tests: `crates/algoclerk-core/tests/data/cpp_interp/*.cpp` (snippets that define `test()`)
  and `tests/data/cpp_corpus/*.cpp` (realistic solutions) are compiled by real compilers and run by the
  interpreter, the parser round trip and the checker. Add a snippet for every behaviour you add.
- `cpp_interp_diff` compares the interpreter with `g++` (and `clang++` with `ALGOCLERK_DIFF_CLANG=1`) on the
  snippets.
- `cpp_prop_diff` generates random UB-free programs (proptest, shrinking, `ALGOCLERK_PROP_CASES`,
  `ALGOCLERK_PROP_SEED`) and compares with the compilers. The generator lives in
  `crates/algoclerk-core/tests/prop_common/`; add your construct to it. Constructs on which the interpreter
  is known to disagree are listed by name in `prop_common::KNOWN` with a one-line reason; that list is empty today
  and must stay empty or shrink.
- `cpp_prop_ub` and `cpp_interp_ub` hold one negative program per undefined-behaviour class; a new UB check
  needs a case there, and a case that the sanitizers also reject is flagged `san`.
- `cpp_prop_fuzz` and `cpp_parse_fuzz` mutate programs and require "no panic, bounded time".
- `cpp_diff_levels` (every level, both compilers, sanitizers) and `cpp_codegen_levels` (Rust and C translations
  compiled and compared) cover the campaign.

A divergence found by these tests is a bug in the game. Fix it, or exclude the construct from the generator
with a documented `KNOWN` entry and open an issue; do not weaken a test to make it pass.

### Adding a `std::` member

Say you want `std::vector::resize` with a fill value, or `std::string::find_last_of`. A member is supported
only when **all** of these exist, and the tests fail loudly when one is missing:

1. **Stdlib table**: a row in `MEMBERS` in `crates/algoclerk-core/src/cpp/stdlib.rs` with the receiver pattern,
   name, the strict standard header that declares it (it must be valid with both libstdc++ and libc++), the
   display signature, the cost class, the `card` flag and a compilable `example`. Every `example` is a real C++
   statement list: `cpp_check_stdlib` runs each one through the checker, and you must make sure it compiles
   with `g++` and `clang++` (`stdlib::example_program` builds the translation unit; step 8 gives the
   compiler-backed test).
2. **Checker**: overload resolution and const-correctness are driven by the signature in the table
   (`cpp/check/libres.rs`, `libcall.rs`, `sigpat.rs`). Most rows need no code; unusual typing (a result type that
   depends on the arguments) needs a rule there. Add a negative case to `cpp_check_negative.rs` for the
   misuse you expect.
3. **Interpreter**: the behaviour, including every undefined-behaviour check that applies (bounds,
   invalidation, empty containers): `cpp/interp/seqs.rs`, `assoc.rs`, `methods.rs` (members) or `algo.rs` (free
   functions).
4. **Cost**: charge what the real operation does (`self.charge(...)` in the same place) and add the row to the
   matching table in [docs/COST_MODEL.md](docs/COST_MODEL.md). The `cost` field of the stdlib row and the
   charge must agree.
5. **Card flag**: set `card: true` when the card editor can express the call as a typed chip (a plain member
   call or free function); `false` makes statements using it appear as raw C++ cards, which is fine.
6. **Translations**: the Rust and C code generators (`cpp/codegen/rust/lib_*.rs`, `cpp/codegen/c/lib_*.rs`).
   If you do not add the translation, make the generator refuse with a friendly message; it must never emit
   wrong code. `cpp_codegen_random` checks translations against the interpreter.
7. **Documentation**: the "Library" table in [docs/CPP_SUBSET.md](docs/CPP_SUBSET.md) is generated from the
   stdlib table (`stdlib::markdown_table`), so the row is the documentation; add prose only where the
   semantics need explaining.
8. **Differential coverage**: a snippet in `tests/data/cpp_interp/*.cpp`, and the construct in the proptest
   generator when it can be generated safely.

## Translating

Copy `crates/algoclerk-core/lang/en.toml` to `<code>.toml`, translate the values, keep keys and
`{placeholders}`. See [docs/TRANSLATING.md](docs/TRANSLATING.md). Level texts are translated by adding
translated copies of level files in a later release (see the roadmap); for now translations cover the UI.

## Code style

- Rust 2024 edition, formatted with `rustfmt` (`rustfmt.toml`), clippy clean with `-D warnings`.
- Every Rust source file starts with `// SPDX-License-Identifier: Apache-2.0`.
- `algoclerk-core`, `algoclerk-levels` and `algoclerk-cli` contain no `unsafe` code (the core forbids it);
  the app crate only has the unavoidable `#[unsafe(no_mangle)]` mobile entry points.
- No panics on user input: anything a player can type (programs, solution files, level files, saved
  progress) must produce an error value, never a panic. Add a property test when you touch a parser.
- Player-visible text goes through the translation catalogue (`lang/en.toml`), not string literals.
- Keep GUI logic thin; put rules and algorithms in `algoclerk-core` where they can be tested headlessly.
- Commit messages use conventional commits (`feat(levels): ...`, `fix(core): ...`) without em dashes.

## Dependencies and policy

- New dependencies must be permissively licensed (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0, OFL for
  fonts) and pass `cargo deny check`. No GPL/LGPL/AGPL code, and no Slint.
- **No WebAssembly**: no wasm targets, no web or browser build, no WebAssembly runtimes and no
  wasm-only crates. Player programs run in the built-in interpreter, and the game never calls a compiler at
  run time (no libclang). Please do not propose these.
- Assets (fonts, sounds, images) must be original or CC0/OFL/permissive and listed in `CREDITS.md`.

## Pull request checklist

The pull request template lists these:

- [ ] formatted, clippy clean, tests pass, `validate` passes
- [ ] SPDX header in new Rust files, no new unlicensed or incompatible dependency
- [ ] level texts and art are original
- [ ] `CHANGELOG.md` updated for user-visible changes
- [ ] commits are signed off

## Sign-off (DCO)

We use the [Developer Certificate of Origin](https://developercertificate.org/): by adding
`Signed-off-by: Your Name <you@example.com>` to your commits (`git commit -s`) you certify that you
wrote the contribution or have the right to submit it under the Apache License 2.0. There is no CLA.

## Reporting problems

Use the issue templates (bug report, new level proposal, feature request). For security problems see
[SECURITY.md](SECURITY.md).
