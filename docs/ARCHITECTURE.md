# Architecture

```text
levels/<wing>/<level>.toml ──► algoclerk-levels (embedded Campaign, extra packs from a directory)
                                      │
assets/art, assets/music ──► algoclerk-media ──► algoclerk-game ──► algoclerk-app (Bevy shell)
                                      ▲                 ▲                │
              algoclerk-core ─────────┴─────────────────┘        algoclerk-cli, algoclerk-tools
```

| Crate | Responsibility | Depends on |
| --- | --- | --- |
| `algoclerk-core` | **ClerkScript** (lexer, parser, printer, type checker with inference, interpreter with the cost model), level data model and seeded test generators, the judge (tests, budgets, medals), the empirical complexity estimator, the level validator, the C/C++/Rust code generators and their compile-and-compare harnesses, progress, i18n. No graphics, no I/O beyond reading files in tests and the compiler driver. `#![forbid(unsafe_code)]`. | serde, toml |
| `algoclerk-levels` | The 80 shipped levels embedded at build time, loading extra packs, unlock rules. | core |
| `algoclerk-media` | Pixel-art engine: palettes, `.px` sprites, 5x7 and 3x5 fonts, software `Canvas`, text-defined chiptune synthesiser, asset embedding. | png |
| `algoclerk-game` | The whole game as a state machine over engine-free types: `Game::tick(&Input)`, `Game::draw(&mut Canvas)`, events for audio, clipboard, fullscreen. Screens: title, level select, play (office scene, card editor, text editor, run controls), memo, victory, code reveal, settings, about, sandbox. | core, levels, media |
| `algoclerk-app` | Thin Bevy 0.19.1 shell: window, integer-scaled nearest-neighbour texture, mouse / touch / keyboard to `Input`, audio (feature `audio`), clipboard, lifecycle, Android (GameActivity) and iOS entry points. | game, media, bevy |
| `algoclerk-cli` | `list`, `show`, `check`, `run`, `judge`, `validate`, `table`, `tune`, `uses`, `codegen --verify`. | core, levels |
| `algoclerk-tools` | `algoclerk-shots`: renders every scenario headlessly to PNG. | game |

## ClerkScript pipeline (core)

`parse` (source text to `Program`, node ids and spans) → `check` (unification-based inference, unlock lists,
friendly diagnostics, per-function slot tables and mutation facts) → `interp::run` (tree walk over slot-resolved
locals; a `Limits` budget for cost, call depth and live elements; optional recorded **trace** of statement
events with the locals at that moment) → `judge::Judge` (suite = examples + hand tests + seeded generated tests;
verdict with size, average cost, medals, complexity estimate) → `codegen::generate` (typed AST to C, C++ or Rust).

The interpreter never panics: parser depth, expression height and source length are bounded, arithmetic is
checked, recursion is limited, and everything is fuzzed (`crates/algoclerk-core/tests/robustness.rs`).

## The front end

All drawing uses palette letters into a 480x270 (landscape) or 270x480 (portrait) software `Canvas`; the
shell scales it by an integer and letterboxes. Because `algoclerk-game` has no engine types, every screen is
rendered, laid out and tested without a window (`algoclerk-tools`, `crates/algoclerk-game/tests/ui.rs`).

**Play screen.** The scene (`office.rs`) replays a recorded run: each trace event carries the locals, the scene
lays them out as labelled trays drawn by `value_view.rs` (crate, letter tile, ribbon, shelf, rack, drawers,
bin, spring plates, belt, pyramid, tag chain, org chart), sends the clerk to the tray that changed, shows the
returned value in the outbox with the expected value and a tick or cross stamp, and ticks the wall clock with
the abstract cost. The editor (`editor.rs`, `blocks.rs`) shows the same program as nested cards or as text.
The program **text is the source of truth** (`session.rs`): card edits transform the tree and print it back,
text edits re-parse, undo and redo are text history, so the two views are always in sync.

**Input.** The shell converts pointer positions to virtual pixels and latches presses between 60 Hz ticks;
`input.rs` derives taps, drags and long presses. Gamepad support is not wired in the shell.

**Audio.** `GameEvent::Sfx(name)` and `GameEvent::Music(name)`; the shell renders `assets/music/<name>.song`
once, caches the WAV and plays it. Failure to open an audio device or a missing song is ignored.

**Save data.** `progress.json` (versioned, corrupt files fall back to defaults) in the platform data
directory, plus the sandbox program and the draft of every level.
