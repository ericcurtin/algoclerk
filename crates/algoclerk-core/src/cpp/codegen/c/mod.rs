// SPDX-License-Identifier: Apache-2.0
//! The C translator (placeholder until the C milestone).

use super::Generated;
use super::common::*;
use crate::cpp::run::Compiled;

/// Translate to C.
pub fn generate(_comp: &Compiled, _entry: &str) -> R<Generated> {
    no("C", "programs")
}
