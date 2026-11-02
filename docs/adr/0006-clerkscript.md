# ADR 0006: replace the assembly puzzle language with ClerkScript

Status: accepted. Supersedes the instruction-set part of ADR 0002 and ADR 0003's level semantics.

## Context

The first version of AlgoClerk taught Human-Resource-Machine-style assembly (INBOX, OUTBOX, COPYTO, JUMPZ,
...) over integer floor tiles. The owner's review: that is the wrong level of abstraction for practising
interview questions. A candidate says "a hash map from value to index" and "a sliding window", not "jump if
zero". The goal of the game is to solve the problem, not to write verbose assembly-like instructions.

## Decision

Players write the body of a typed function in **ClerkScript** (see `docs/LANGUAGE.md`): value types `int`,
`bool`, `char`, `string`, `vec`, `set`, `map`, tuples, `option`, `stack`, `queue`, `deque`, `heap`, `lnode`,
`tnode`; `let`/`if`/`while`/`for`; helper functions, `&mut` parameters and recursion where a level unlocks
them; a rich standard library gated per level (the thing a level is about is locked).

- **Interpreter**: a deterministic tree-walking interpreter over a typed AST with slot-resolved locals, a
  step budget, a call-depth limit and a live-element limit. No `unsafe`, no I/O, no panics (fuzzed).
- **Cost model**: abstract cost mirrors real complexity so a brute-force solution fails the hidden large
  tests like a real time limit. An empirical estimator classifies time and space from n, 2n, 4n, 8n.
- **Two editing surfaces**, always in sync: cards and text. The program text is the source of truth: card
  edits transform the syntax tree and print it back; text edits are re-parsed. Undo and redo are text history.
- **Levels stay data** (TOML) with seeded typed generators and a naive solution that must time out.
- **Code reveal** translates the syntax tree directly into idiomatic C, C++ and Rust, with compile-and-compare
  tests (gcc, g++, clang, rustc) for every reference solution and for random type-correct programs.
- The assembler, VM, instruction set and the 48 assembly levels are removed (one clean language). A
  "Boiler Room" wing was considered and rejected: it would double the maintenance for little gain.

## Consequences

- The campaign is rewritten: 80 levels in ten wings (see `docs/LEVEL_FORMAT.md` and the README).
- Strings and chars are ASCII only, and ints are checked 64-bit; this keeps the translation to C, C++ and
  Rust exact. Map and set iteration order is not defined, so `keys()`/`values()`/`items()`/`sorted(set)` return
  entries sorted by key in every backend.
- Linked structures are arena handles in the interpreter and in the Rust translation (C and C++ use real
  pointers). The Rust reveal therefore defines tiny `ListRef`/`TreeRef` helpers.
- Compiler and runtime diagnostics are English strings with stable codes (`type.mismatch`, ...); the pixel UI
  chrome is fully localisable through `crates/algoclerk-core/lang/*.toml`.
