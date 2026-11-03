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

**Play screen.** The scene (`office.rs`) replays a recorded run: each trace event carries the visible locals
(with their C++ types), the scene lays them out as labelled trays drawn by `value_view.rs` (crate, letter tile,
ribbon, shelf, rack, drawers, bin, spring plates, belt, pyramid, tag chain, org chart for `ListNode*` and
`TreeNode*`) and writes the type (`vector<int>`) in the tray header when there is room. The clerk walks to the
tray that changed, the returned value goes to the outbox with the expected value and a tick or cross stamp,
and the wall clock shows the abstract cost. A runtime error becomes a speech bubble and the failing line is
highlighted in both editor views (`RtError.node` through `Program.spans`).

Next to the trays the **watch board** (`watch.rs`) lists the visible locals as `type name = value` in C++ spelling
(`vector<int> nums = {2, 7}`), wrapped to two lines at most, with the variables that have no shelf room first and
the one that just changed in amber.

The editor (`editor.rs`, `cards.rs`, `blocks.rs`, `textedit.rs`, `complete.rs`) shows the player's part of the
program (the level's read-only prelude is summarised in a strip above it and shown in full on the brief) as
nested cards or as text.

*Text view.* Tokens are coloured with `cpp::lexer::highlight` (`syntax.rs`); line numbers, sideways scrolling that
follows the cursor (keys, wheel, touch drag), matching-bracket highlight, `Ctrl+/` and a `//` chip toggle
comments, Enter keeps the indent (one level deeper after `{`, `}` dedents), optional auto-closing of brackets and
quotes, error squiggles with the clerk's message, and autocomplete for `std::` names and for the members of the
receiver before `.` or `->` (`stdlib::members_of` filtered by the unlocked headers and the level's locks;
receiver types come from the checker's `scope_at`). The touch keyboard has a symbol layer with the C++ symbols
and one-tap keyword and type chips.

*Cards.* `blocks.rs` turns the syntax tree into rows (`Row`: kind, depth, the block it sits in, the block it holds,
comments) and performs structural edits (`edit_stmt`, `insert_stmt`, `delete_stmt`, `move_stmt`, `step_stmt`,
`move_to`) that transform the tree and print it back with `cpp::print`. `if`/`else`, `for`, range-`for`, `while`,
`do`-`while` and `switch` hold their statements as nested cards; `cards.rs` lays them out as paper cards inside
container cards, wraps the (syntax coloured) text and the comments that travel with a card so nothing is cut off,
and maps a pointer to a drop target (before a card, inside a block, after a container by its left margin). A mouse
drag (or a touch long press) drops a card into any block; `case`/`default` labels only stay inside a `switch`.
What cards cannot express (bare blocks, statements that print on several lines) is a read-only *raw C++* card that
can still be moved and deleted, so the round trip is lossless. Tapping a selected card (or Enter) opens the card
editor: a one-line syntax coloured editor and chip pages for the variables in scope (`check::scope_at`), literals,
operators, member functions of the value left of the cursor (only rows with `card: true`, filtered by the receiver's
static type) and `std::` names (only unlocked headers).

The program **text is the source of truth** (`session.rs`): card edits transform the tree and print it back, text
edits re-parse, undo and redo (buttons, `Ctrl+Z`, `Ctrl+Shift+Z`) are one text history shared by both views, so the
two views are always in sync. The current statement of a run is highlighted in the text view and on its card.
`tests/editors.rs` round-trips the whole C++ corpus and all level references through cards and drives both editors
with real input events and fuzzed key and pointer sequences.

The code reveal (`result.rs`) has C, C++ and Rust tabs behind `result::translations`, which calls
`cpp::codegen::generate`: C (with the helper prelude folded away by default) and Rust come from the generator, C++
is the player's own formatted program with the level's includes plus the level's technique and complexity note;
a construct a generator cannot express turns into a friendly message on that tab only. Lines wrap or scroll
sideways and keep their line numbers, so no line is ever cut off.

**Input.** The shell converts pointer positions to virtual pixels and latches presses between 60 Hz ticks;
`input.rs` derives taps, drags and long presses. Gamepad support is not wired in the shell.

**Audio.** `GameEvent::Sfx(name)` and `GameEvent::Music(name)`; the shell renders `assets/music/<name>.song`
once, caches the WAV and plays it. Failure to open an audio device or a missing song is ignored.

**Save data.** `progress.json` (versioned, corrupt files fall back to defaults) in the platform data
directory, plus the sandbox program and the draft of every level.
