# ADR 0004: Code generation: structure recovery with a correct fallback, verified by compilers

- Status: accepted

## Context

The signature feature is showing the player's program as idiomatic C, C++ and Rust. It must work for
any valid program, read well for typical ones, and never be wrong.

## Decision

- The program is translated as a function over an `int` hand, floor variables or an array, and
  inbox/outbox semantics.
- A structurer recovers `while`/`do-while`/`if`/`else`/`break`/`continue`/`return` directly from jump
  patterns; forward jumps that leave constructs use labelled blocks (Rust) or `goto` (C, C++).
  Whenever any jump cannot be classified, the whole function uses an always-correct fallback form
  (goto with the clerk's labels in C and C++, a `pc` state machine in Rust).
- Per-level `[code]` metadata maps inbox and floor to interview-style parameters and return values
  (see LEVEL_FORMAT.md); without it a generic `clerk_run(inbox)` form is used.
- Trust is established by tests, not by argument: every shipped reference solution in both calling
  conventions and hundreds of random programs per run (each in both the structured and the forced fallback form) are compiled with
  every installed compiler (gcc, clang, g++, clang++, rustc, warnings as errors for shipped
  levels) and compared with the interpreter.

## Consequences

- Readability is best-effort for exotic control flow, correctness is not.
- Runtime errors of the clerk have no translation (documented in CODEGEN.md).
- CI must have the compilers installed (they are available on all GitHub runners or installed by
  the workflow).
