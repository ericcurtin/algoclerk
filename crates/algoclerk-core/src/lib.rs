// SPDX-License-Identifier: Apache-2.0
//! AlgoClerk core: everything except pixels.
//!
//! * [`lang`]: ClerkScript, the typed high-level language (parser, checker, interpreter, cost model)
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod i18n;
pub mod codegen;
pub mod complexity;
pub mod judge;
pub mod lang;
pub mod level;
pub mod progress;
pub mod testgen;
pub mod validate;
