// SPDX-License-Identifier: Apache-2.0
//! Pretty printer (STUB: replaced by the PARSE work package).

use super::ast::*;

/// Formatted code; `player_only` skips the prelude items.
pub fn print_program(_p: &Program, _player_only: bool) -> String {
    String::new()
}
/// One statement at an indentation level (4 spaces each).
pub fn print_stmt(_s: &Stmt, _indent: usize) -> String {
    String::new()
}
/// One expression.
pub fn print_expr(_e: &Expr) -> String {
    String::new()
}
/// A declared type with `const` and `&`.
pub fn print_type(_t: &TypeSpec) -> String {
    String::new()
}
/// Zero every id and span (structural comparison in tests).
pub fn normalize(_p: &mut Program) {}
