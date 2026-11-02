// SPDX-License-Identifier: Apache-2.0
//! AlgoClerk core: everything except pixels.
//!
//! * [`isa`]: the instruction set
//! * [`asm`]: text assembler producing a [`asm::Program`]
//! * [`vm`]: deterministic sandboxed interpreter with step traces and rewind
//! * [`level`], [`testgen`]: level data model and deterministic test generation
//! * [`judge`]: correctness, size and speed scoring
//! * [`validate`]: level validator used by CI and the CLI
//! * [`codegen`]: translate a program into C, C++ or Rust
//! * [`share`], [`progress`], [`i18n`]: sharing, local saves and translations
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod asm;
pub mod codegen;
pub mod i18n;
pub mod isa;
pub mod judge;
pub mod level;
pub mod progress;
pub mod share;
pub mod testgen;
pub mod validate;
pub mod vm;
