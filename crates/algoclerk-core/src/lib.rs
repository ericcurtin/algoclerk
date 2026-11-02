// SPDX-License-Identifier: Apache-2.0
//! AlgoClerk core: everything except pixels.
//!
//! * [`cpp`]: the C++17 subset players write (syntax tree, parser, checker, interpreter, cost model) and the
//!   differential harness against real compilers
//! * [`lang`]: ClerkScript, the old invented language (to be deleted)
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod codegen;
pub mod complexity;
pub mod cpp;
pub mod i18n;
pub mod judge;
pub mod lang;
pub mod level;
pub mod progress;
pub mod testgen;
pub mod validate;
