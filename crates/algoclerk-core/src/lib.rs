// SPDX-License-Identifier: Apache-2.0
//! AlgoClerk core: everything except pixels.
//!
//! * [`cpp`]: the C++17 subset players write (syntax tree, parser, checker, interpreter, cost model), the
//!   differential harness against real compilers and the code generators (C and Rust)
//! * [`level`], [`judge`], [`validate`], [`testgen`], [`complexity`]: levels, the judge and its estimators
//! * [`progress`], [`i18n`]: saved progress and the translation catalogue
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod complexity;
pub mod cpp;
pub mod i18n;
pub mod judge;
pub mod level;
pub mod progress;
pub mod testgen;
pub mod validate;
