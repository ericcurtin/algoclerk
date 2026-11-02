# Security Policy

## Supported versions

Only the latest release (and `main`) receives security fixes while the project is pre-1.0.

## Reporting a vulnerability

Please **do not open a public issue**. Report privately through the hosting platform's
"Report a vulnerability" (private security advisory) feature. 

You can expect an acknowledgement within a week and a fix or a mitigation plan within 30 days for
confirmed problems.

## Threat model

AlgoClerk is an offline game. Things that deserve extra care:

- **Player programs and shared solution text** are untrusted input. They are parsed by a parser that
  never panics (fuzz-style property tests) and run by an interpreter with a hard step limit, no I/O,
  no `unsafe` code and no way to reach the host. Values are bounded, so arithmetic cannot overflow
  the host integers.
- **Level files** (shipped or loaded from the user's level folder on desktop) are untrusted data
  parsed with `serde`/`toml` and validated; unknown fields are rejected and errors never panic.
  Test generation is deterministic and bounded.
- **Saved progress** is JSON; a corrupt file falls back to defaults instead of crashing.
- **Generated code** (C, C++, Rust) is text shown to the player. AlgoClerk never compiles or runs it
  in the game; only the test-suite and `algoclerk-cli codegen --verify` do, with compiler and run
  time limits.
- **Supply chain**: dependencies are permissively licensed, checked by `cargo deny` (including
  RustSec advisories) in CI, and pinned by `Cargo.lock`.

The game has no network access and requests no Android permissions.
