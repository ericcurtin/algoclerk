# Architecture

AlgoClerk is a Cargo workspace. **All logic lives in toolkit-free crates** with no GPU or display
dependency, so tests, headless simulation, CI and screenshots work anywhere. The game is a thin
layer on top.

```text
                  +---------------------------+
                  |  algoclerk-app (egui)     |  desktop bin, Android cdylib, iOS staticlib
                  |  thin UI over the core    |  + headless software renderer (screenshots)
                  +------------+--------------+
                               |
 +------------------+   +------v-------------+   +--------------------+
 | algoclerk-cli    +-->| algoclerk-levels   |   | levels/*.toml      |
 | run/judge/...    |   | embedded packs     |<--+ (data, no Rust)    |
 +--------+---------+   +------+-------------+   +--------------------+
          |                    |
          +--------------------v---------------------------------------+
          |  algoclerk-core   #![forbid(unsafe_code)]                  |
          |  isa  asm  vm  level  testgen  judge  validate  share      |
          |  progress  i18n  codegen{structure,render,meta,harness,    |
          |  highlight,compile}                                        |
          +------------------------------------------------------------+
```

## Crates

### `algoclerk-core`

| Module | Responsibility |
| --- | --- |
| `isa` | `Instr<L>` (generic over the jump target type), `Opcode`, `Addr`, `Allowed`, value limits |
| `asm` | text parser to `Listing` (the editor model: labels, instructions, comments), `compile` to `Program` (resolved jumps), printer, rule checks against a level (`Program::check`) |
| `vm` | `Machine`: deterministic interpreter, per-step `Effect`s for animation, undo records for step-back, hard step limit, no I/O, `RuntimeError` with message keys |
| `level` | serde data model of level files, `build_tests` (examples + hand-written + generated cases) |
| `testgen` | SplitMix64 PRNG and the generator DSL (`list`, `perm`, `balanced`, `linked`, ...) |
| `judge` | `Judge`: runs all tests, `Verdict` with size/speed and medals |
| `validate` | level validator used by the CLI, CI and tests |
| `codegen` | program to C / C++ / Rust (see [CODEGEN.md](CODEGEN.md)) |
| `share`, `progress`, `i18n` | solution text format, local saves, translation catalogues |

Design rules: no panics on user input (parsers and VM are fuzz-tested with proptest), deterministic
results, everything player-visible goes through message keys, `#![forbid(unsafe_code)]`.

### `algoclerk-levels`

`build.rs` embeds every `levels/**/*.toml` with `include_str!` (no runtime file access, which matters on
mobile). `Campaign` groups levels into packs ("wings"), implements unlock rules and can load extra
packs from a user directory on desktop. Integration tests live here: validation of every level,
reference solutions meet targets, independent oracles, and the compile-and-compare tests of generated
code for every level in both calling conventions.

### `algoclerk-cli`

A small hand-written argument parser and the commands `list`, `show`, `run`, `judge`, `validate`,
`tune`, `codegen`, `export`, `import`, all with `--json` where it makes sense.

### `algoclerk-app`

egui/eframe (see [ADR 0001](adr/0001-gui-toolkit.md)). Modules: `app` (screens, navigation,
persistence), `play` (session state: program, machine, animation clock), `play_ui` (layouts: three
columns on wide screens, tabs on phones), `editor` (program list with drag and drop and tap buttons),
`brief`, `result` (victory screen and code view), `office` (the animated scene, all procedural),
`widgets` (vector icons), `theme`, `fonts`, `sound` (optional `audio` feature, synthesised tones),
`raster` + `shot` (CPU renderer of egui meshes for screenshots and smoke tests).

Entry points: `main.rs` (desktop), `android_main` (NativeActivity, `cdylib`), `algoclerk_ios_main`
(called from the Objective-C wrapper, `staticlib`).

## Data flow of a play session

1. `Campaign` provides a `Level`; `Judge::new` builds the concrete tests once.
2. The player edits a `Listing` (list or text; both views stay in sync through `PlayState::after_edit`
   and `PlayState::set_text`).
3. `Listing::compile` gives a `Program`; `Program::check` enforces the level's rules.
4. Run/step creates a `Machine` per test; each `step()` returns `Effect`s that the office scene animates.
5. **Check** runs `Judge::judge` on all tests. A pass records progress (`Progress::record_verdict`) and
   opens the result screen, which calls `codegen::generate` for C, C++ and Rust.

## Testing strategy

- Unit tests next to the code; property tests (proptest) for the parser, VM, rewind and text round trips.
- `crates/algoclerk-core/tests/codegen_random.rs`: random structured and irreducible programs are
  translated, compiled with every available C/C++/Rust compiler and compared to the interpreter.
- `crates/algoclerk-levels/tests/`: validation, targets, oracles, hard-coding resistance, compile tests.
- `crates/algoclerk-app/tests/render_smoke.rs`: every screen, theme, level and phone/desktop size is
  rendered headlessly.
- CI: fmt, clippy `-D warnings`, tests on Linux/macOS/Windows, `cargo deny`, level validation, Android
  and iOS type-checks.

## Decisions

Architecture decision records live in [adr/](adr/).
