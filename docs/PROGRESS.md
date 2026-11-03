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
- [x] M5 Campaign: all 80 levels converted to C++ (reference, naive, hints, locks), independent Rust oracles kept
- [x] M6 Differential and property tests: level differential tests (`g++`, `clang++`, sanitizers), random UB-free
  program generator with shrinking, negative table per UB class, fuzzing of the lexer, parser, checker and
  interpreter
- [x] M7 Front end port: card editor and text editor on the C++ tree (raw C++ cards for what cards cannot
  express), memo with prelude and locks, play scene with C++ type names, victory, save format version 2 with a
  migration that keeps settings, solved levels and medals; refreshed screenshots
- [x] M8 Code generation from the checked C++ tree to Rust and C (and formatted C++), `codegen --verify`,
  compile-and-compare tests; `docs/CODEGEN.md`
- [x] M9 Documentation, ADR 0007, CI (compilers installed, differential tests job), contributor guide
- [ ] M10 Connect the Rust and C translations to the tabs of the victory screen (`result::translations` still
  marks them `pending`, so the game shows "translation coming"; the CLI prints them)
- [ ] M11 Shrink the known-divergence list to zero (see below)
- [ ] M12 Re-verify every platform on this branch (see the matrix in `docs/BUILDING.md`)

## Decisions

- **Real C++17, a strict subset, no compiler at run time** (ADR 0007). Anything the two compilers could disagree
  on is outside the subset: `long`, `int64_t`, `double`, templates, I/O, exceptions, `std::move`, C++20 library.
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

- `crates/algoclerk-core/tests/prop_common/mod.rs::KNOWN` lists constructs the property generator must not
  produce because the interpreter or checker diverges from the compilers: `ull_mul` (an
  `unsigned long long` multiplication of two large values panics in a debug build), `str_plus_charref`,
  `append_n` (`string::append(count, ch)` rejected), `partial_sum` and `for_each` (checked but not
  interpreted). The reproductions are in the work-package notes (`lvl-bugs/prop.md` in the run directory); the
  list has to shrink, it must never be used to hide a new divergence.
- `prop_common/ub.rs::UB_KNOWN` lists undefined-behaviour cases that the interpreter does **not** yet report
  with the right class, so the "every UB class is detected" policy has these exceptions today:
  `int x = x;` stops with a generic bad-argument error instead of `uninit`; `T* p = p;` stops with an internal
  error instead of `uninit`; advancing an erased `std::map` iterator (`++it` after `m.erase(it)`) is **not
  reported at all** (only dereferencing it is), so the classic erase-in-a-loop bug is accepted. The first two
  still stop the run, the third does not.
- The Rust and C tabs of the victory screen are not connected (M10). The translators exist and are tested
  through the CLI and `cpp_codegen_levels`; a level may be refused by a translator only if it is listed with a
  reason (the list is empty at the time of writing).
- `docs/CPP_SUBSET.md` documents what is out of scope for v1: templates and classes with methods, operator
  overloading, `double`, I/O, exceptions, smart pointers and `std::move`, C++20 library features.
- The compile-and-compare suites wrap every compiler and binary in GNU `timeout`. They are only expected to run
  on Linux and on macOS with coreutils; the Windows CI job runs the interpreter-only tests (see
  `docs/BUILDING.md`).
- The documentation work package ran no build or test: it read the sources, tests and commits. Every number and
  command in the documentation comes from them; the combined quality-gate run belongs to the lead's final
  report for the branch.

## Verification

What exists (and what each gives you):

- Per work package, the owners reported green test runs of their package at commit time (parser, checker,
  interpreter, glue, codegen, UI, property tests). They were run in separate worktrees; the merged branch is
  verified by the commands in `docs/BUILDING.md`, which the final report of the branch records.
- The headless screenshots in `docs/screenshots/` were regenerated for the C++ UI by the UI package. The
  Rust and C tabs show the placeholder until M10.
- The real-window smoke test (`scripts/gui-smoke.sh`) was last run on the pre-C++ code. Its click coordinates
  depend on the layout, so it must be re-run and may need updating (M12).

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
