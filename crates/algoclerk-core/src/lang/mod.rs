// SPDX-License-Identifier: Apache-2.0
//! ClerkScript: the typed, high-level language players write. See `docs/LANGUAGE.md`.

pub mod ast;
pub mod check;
pub mod diag;
pub mod infer;
pub mod interp;
pub mod parse;
pub mod print;
pub mod run;
pub mod value;

pub use ast::{Program, Ty};
pub use check::{Allowed, CheckOpts, EntrySig, TypeInfo, check};
pub use diag::Diag;
pub use parse::parse;
