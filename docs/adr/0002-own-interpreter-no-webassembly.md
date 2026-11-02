# ADR 0002: Player programs run in our own interpreter; no WebAssembly

- Status: accepted (the interpreter now runs ClerkScript, see ADR 0006; the instruction set described here is gone)

## Context

Players write programs in a tiny assembly language. We need to run them safely, deterministically,
step by step (animation, step back), with hard limits, on five platforms including iOS, where
JIT-compiling runtimes are restricted.

## Decision

- Programs are executed by a small interpreter written in Rust (`algoclerk-core::vm`). It has no I/O,
  no `unsafe`, a per-test step limit and a bounded value range. Each step returns the visible effects
  for the animation and undo records so single steps can be reversed.
- The project contains **no WebAssembly**: no wasm32 targets, no web or browser build, no
  `wasm-bindgen`/`trunk`/`wasm-pack`, no WebAssembly runtime such as wasmtime or wasmer, and no
  wasm-only crates. `deny.toml` bans these crates, and the dependency graph is checked only for the
  five supported platforms.
- No third-party scripting engine either (Lua, Rhai, ...).

## Consequences

- Full control over semantics (empty hand and tile errors with friendly messages), determinism and
  step traces.
- The same interpreter runs in the GUI, the CLI and the tests, and it is the oracle for the
  code-generation tests.
- The lock file lists optional target-specific crates of upstream dependencies for targets we do
  not build (a property of how Cargo records the full dependency graph); none of them is compiled
  or linked for any supported platform, which `cargo deny check` (with `[graph] targets`) enforces.
