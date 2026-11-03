# Progress log (branch `cpp-lang`)

The branch replaces the invented player language of ADR 0006 with a subset of real C++17 (ADR 0007). This file
says what was done, what was decided, and, honestly, what is not finished or not verified.

## Milestones

- [x] M1 Subset definition: `docs/CPP_SUBSET.md`, the C++ syntax tree, plain values and the brace notation shared
  by level files, the judge and the real-compiler harness
- [x] M2 Front end: lexer with highlighter, expression and statement parser, printer, text-to-tree-to-text round
  trip over a corpus of 70 realistic solutions; type checker with the library table (`cpp::stdlib`), strict
  per-header name visibility, lock reasons, unsequenced-modification and missing-return checks
- [x] M3 Interpreter: tree walker with the STL cost model (`docs/COST_MODEL.md`), memory estimate, traces for the
  animation, and a detector for each undefined-behaviour class in `docs/CPP_SUBSET.md`
- [x] M4 Level pipeline on C++: level format with prelude, signature, locks and compare modes; judge, medals and
  complexity estimator on top of the interpreter; CLI (`check`, `run`, `judge`, `validate`, `tune`, `table`,
  `diff`, `harness`, `codegen`); the real-compiler harness (`cpp::harness`)
- [x] M5 Campaign: the original 80 levels converted to C++ (reference, naive, hints, locks), independent Rust
  oracles kept, plus a new eleventh wing, The Supply Cupboard (6 levels where C++ semantics are the lesson), for
  86 levels in 11 wings; 53 levels have a naive solution that times out on the large tests (47 original, 6 new)
- [x] M6 Differential and property tests: level differential tests (`g++`, `clang++`, sanitizers), random UB-free
  program generator with shrinking, negative table per UB class, fuzzing of the lexer, parser, checker and
  interpreter
- [x] M7 Front end port: card editor and text editor on the C++ tree (raw C++ cards for what cards cannot
  express), memo with prelude and locks, play scene with C++ type names, victory, save format version 2 with a
  migration that keeps settings, solved levels and medals; refreshed screenshots
- [x] M8 Code generation from the checked C++ tree to Rust and C (and formatted C++), `codegen --verify`,
  compile-and-compare tests; `docs/CODEGEN.md`
- [x] M9 Documentation, ADR 0007, CI (compilers installed, differential tests job), contributor guide
- [x] M10 The Rust and C tabs of the victory screen are connected to `cpp::codegen`; the game opens the code
  reveal on the C++ tab. The translations compile and match the interpreter for every level reference and naive
  solution with `rustc`, `gcc` and `clang`, with one allowlisted exception (`drop-negatives`, a `std::list`
  erase loop, shows a friendly message in the Rust and C tabs)
- [x] M11 The known-divergence lists of the property tests (`KNOWN`, `UB_KNOWN`, `KNOWN_INTERNAL`) are empty
- [ ] M12 Re-verify every platform on this branch: **partly done** (see "Verification" and the remaining gaps
  below). Done: Linux windowed game built and run in a container, quality gates on the host, cross-target
  type-checks of the engine-free crates. Not done: the Android, iOS, macOS and Windows app builds

## Decisions

- **Real C++17, a strict subset, no compiler at run time** (ADR 0007). Anything the two compilers could disagree
  on is outside the subset: `long`, `short`, `int64_t`, `double`, templates, I/O, exceptions, `std::move`, C++20
  library.
- **Undefined behaviour is an error, not a feature**: every class we can name is detected and reported as a
  clerk-style message on the offending line, and the judge rejects the program.
- **`unordered_*` iteration order is unspecified**, so the interpreter takes a fixed order and the judge re-runs
  the standard tests with the reverse order; answers must not change. `std::sort` is not stable in real C++, so
  levels never depend on the order of equal elements.
- **Names need their real header**: `std::max` needs `<algorithm>` even if another header would drag it in with
  one library and not another.
- **The program text is the single source of truth**; cards are a view that rewrites the text. Comments
  survive the round trip.
- **The cost model charges copies**: a by-value container parameter costs its leaves, `const&` costs nothing.
- **Differential tests are part of the product**: the interpreter's claim to be C++ is exactly as good as the
  suite that compares it with `g++` and `clang++`, so CI installs both and runs it.
- **Old saves are migrated, not crashed on**: the progress format is version 2; drafts and saved
  solutions written in the old language cannot be converted and are dropped with a note.
- Interpreter values use `Arc` (not `Rc`) so the whole `Game` is `Send + Sync` for Bevy resources.
- Compiler and runtime diagnostics stay English in the core (stable codes); UI chrome is in `lang/en.toml`.

## Known divergences and gaps (honest status)

Deliberate differences from `g++` (the property tests exclude nothing else; the `KNOWN`, `UB_KNOWN` and
`KNOWN_INTERNAL` lists are empty and must stay empty or shrink, never hide a new divergence):

- The checker rejects `std::string += unsigned char` and `std::string += int` (real compilers accept them with
  different meanings across platforms; write `static_cast<char>`).
- Erased-iterator detection for ordered containers (`std::map`, `std::set`) is key based: advancing an iterator
  whose element was erased is reported, but by key identity rather than by node address.
- `short` and `unsigned short` are refused (use `int`); `unsigned char` is supported.
- The Rust and C translators refuse one campaign level, `drop-negatives` (`it = l.erase(it)` over a `std::list`);
  the allowlist is `ALLOW` in `crates/algoclerk-levels/tests/cpp_codegen_levels.rs`.

Not finished or not verified:

- **Wide editor mode in landscape** (cards and text side by side on large windows) is not implemented.
- **Touch drag is unverified on a device.** Mouse and keyboard were driven in the windowed game; touch input
  comes from the same pointer path but nobody has used it on a phone or tablet.
- **No real GPU and no mobile device.** The windowed game ran under Xvfb with Mesa lavapipe (software Vulkan);
  frame times, GPU drivers, soft keyboards, notches and safe areas are untested.
- **The interview polish list** (suggestions for the C++ tab) is not implemented.
- **The Android, iOS, macOS and Windows app builds were not re-verified** on this branch. Only the engine-free
  crates were type-checked for `x86_64-pc-windows-gnu`, `aarch64-apple-darwin` and `aarch64-apple-ios`. See
  `docs/BUILDING.md`.
- **`short`, `long` and `double` are not in the subset** (`docs/CPP_SUBSET.md`, "Not supported (yet)").
- The compile-and-compare suites wrap every compiler and binary in GNU `timeout`. They are only expected to run
  on Linux and on macOS with coreutils; the Windows CI job runs the interpreter-only tests (see
  `docs/BUILDING.md`).

## Verification

Quality gates run on this branch (the facts the lead verified):

- `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`: clean. Clippy ran on the host
  for every crate except `algoclerk-app`, which was checked and built in the container (it needs the Bevy
  system libraries).
- `cargo test --workspace` excluding the app, on the host: 440+ tests pass (the levels crate alone takes about
  five minutes because of the compiler suites).
- `cargo deny check licenses bans`: ok.
- Cross-target `cargo check` of the engine-free crates for `x86_64-pc-windows-gnu`, `aarch64-apple-darwin` and
  `aarch64-apple-ios`: ok.
- Property and fuzz tests: 9000+ random programs agreed with `g++` (and a `clang++` plus sanitizer sample), about
  300000 fuzz mutants ran without a panic, and 110 negative undefined-behaviour programs are detected.
- Translations: every level reference and naive solution translated to Rust and C compiles with `rustc`, `gcc`
  and `clang` and matches the interpreter (one allowlisted level, `drop-negatives`).
- The windowed game was built and run in the `algoclerk-build` container (Fedora 44, Xvfb, Mesa lavapipe): typing
  a reference solution, cards with the mouse only, a runtime error, level select, settings, a map level with the
  watch board, the victory screen and the code tabs. `docs/screenshots/window-*.png` are real captures of that
  run. The headless screenshots in the other `docs/screenshots/*.png` files come from `scripts/screenshots.sh`.

What was **not** done: see "Not finished or not verified" above. In particular the app builds for Android, iOS,
macOS and Windows were not re-run, and `scripts/gui-smoke.sh` was last run on the pre-C++ code (its click
coordinates depend on the layout, so it must be re-run and may need updating).

## Bugs found by the tests (worth remembering)

- Differential tests found interpreter and checker gaps in `string`, `vector`, `list`, `deque`, `map` and `set`
  overloads, `unsigned char`, re-including a prelude header, and the lock message for headers and names; all fixed.
- The complexity estimator mistook early-exit and random-target variance for growth; levels provide a `[sweep]`
  generator that forces the worst case, and the space estimator was reworked to ignore a noise floor and offsets.
- Builtins that took containers by reference were charged as copies; only copies and by-value parameters charge.
- `a b c` token soup and 5000-deep nesting exposed unbounded recursion; parser depth and expression height are
  bounded.
- Fuzzing found self-initialisation (`int x = x;`, `T* p = p;`) and unsigned compound multiplication cases that
  needed exact C++ semantics; they are now covered by negative tests.
