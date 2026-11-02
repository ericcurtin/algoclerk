// SPDX-License-Identifier: Apache-2.0
//! The C half of the compile-and-compare harness (placeholder until the C milestone).

use super::Generated;
use super::common::*;
use crate::level::Level;

/// A complete C program.
pub fn program(_level: &Level, _g: &Generated, _cases: &[Vec<String>]) -> R<String> {
    no("C", "programs")
}
