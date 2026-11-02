# Progress log (branch `pixel-ui`)

## Milestones

- [x] M1 ClerkScript: lexer, parser, printer, type checker, interpreter, cost model, robustness fuzzing (ADR 0006, `docs/LANGUAGE.md`)
- [x] M2 Judge, complexity estimator, level format and validator, CLI; the first ten on-ramp levels end to end
- [x] M3 Code generation to C, C++ and Rust with compile-and-compare tests (every reference solution and random programs)
- [x] M4 Full campaign: 80 levels in ten wings, independent Rust oracles, naive solutions that must time out
- [x] M5 Pixel front end adapted to ClerkScript: value-widget office scene, card and text editors, memo, victory and code reveal, title, select, settings, sandbox, original songs, headless screenshots, real-window smoke test
- [x] M6 Docs, CI and release workflows, Android GameActivity and iOS projects, cross-target checks, final report

## Decisions

- One language, no assembly wing (ADR 0006). Strings and chars are ASCII; `int` is checked 64-bit.
- Program text is the single source of truth; cards are a view that rewrites the text (lossless except comments).
- Interpreter values use `Arc` (not `Rc`) so the whole `Game` is `Send + Sync` for Bevy resources.
- Map and set iteration yields sorted entries in the interpreter and in every generated language, so results are
  deterministic and comparable.
- Compiler and runtime diagnostics stay English in the core (stable codes); UI chrome is in `lang/en.toml`.

## Bugs found by the tests (worth remembering)

- The codegen compile tests found that a `for ... in enumerate(sorted(x))` evaluated a temporary per iteration and
  indexed it through a dangling reference in C++ (now hoisted into a local in C and C++).
- A C `for` loop re-evaluated `range(len(v))` every iteration while the body pushed to `v`; the end is now hoisted
  unless provably stable.
- Rust rejects `v[v.len() - 1][0] = x` (borrow); the index is now computed first.
- Builtins charged a copy for every collection argument (`len(s)` was O(n)); only constructors copy now.
- The complexity estimator mistook early-exit and random-target variance for growth; levels now provide a `[sweep]`
  generator that forces the worst case, and the classifier uses per-doubling increments for O(log n).
- `a b c` token soup and 5000-deep nesting exposed unbounded recursion; expression height is bounded in the parser.
