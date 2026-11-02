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

You need Rust 1.95+. The code generator tests compile generated code, so they use `gcc`, `g++`,
`clang`/`clang++` and `rustc` when present and skip a language whose compiler is missing (CI has all
of them). On Linux the window code needs no `-devel` packages; Wayland, X11 and Vulkan/OpenGL libraries
are loaded at run time. The optional `audio` feature needs ALSA headers (`libasound2-dev`).

### Checks that CI runs (please run them before pushing)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check                       # licences, bans, advisories (cargo install cargo-deny)
cargo run -p algoclerk-cli -- validate
```

Take care with long-running commands: tests run the interpreter with step limits and compile with
compiler time limits, but a runaway process of your own is your responsibility.

### Where things live

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). In short: all logic is in `crates/algoclerk-core`
(no GUI dependency, `#![forbid(unsafe_code)]`), levels are data in `levels/`, the game is a thin
egui layer in `crates/algoclerk-app`.

## Writing levels (the easiest way to contribute)

1. Read [docs/LEVEL_FORMAT.md](docs/LEVEL_FORMAT.md) and [docs/ISA.md](docs/ISA.md).
2. Pick a classic algorithm topic and write the story, statement, hints and names **yourself**.
   Do not copy text, puzzles, level designs or art from other games or from problem websites.
   Topics and ideas are fine; sentences are not.
3. Make it a good puzzle: at least 3 hand-written edge cases with explicit outputs, generated cases
   that make hard-coding impossible, a 3-step hint ladder, a reference solution, par/ace targets
   (`algoclerk-cli tune`), `[code]` metadata so the generated code reads like an answer, and an
   `[explain]` note naming the algorithm.
4. Check: `algoclerk-cli validate levels/<pack>` and `cargo test -p algoclerk-levels`.
   Optionally add an independent Rust oracle for your level in `crates/algoclerk-levels/tests/oracles.rs`.
5. Open a pull request using the template.

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
  wasm-only crates. Player programs run in the built-in interpreter. Please do not propose these.
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
