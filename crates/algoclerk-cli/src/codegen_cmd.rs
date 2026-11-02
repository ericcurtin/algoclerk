// SPDX-License-Identifier: Apache-2.0
//! `algoclerk-cli codegen` (implemented with the code generator milestone).

use crate::args::Args;

/// Translate a program to C, C++ or Rust.
pub fn codegen(_args: &Args) -> Result<u8, String> {
    Err("codegen is not built yet".into())
}
