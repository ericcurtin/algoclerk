// SPDX-License-Identifier: Apache-2.0
//! Tokenizer (STUB: replaced by the PARSE work package).

use super::ast::Span;

/// Colour class of a token in the text editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HlKind {
    /// `for`, `return`, ...
    Keyword,
    /// builtin and std type names, known structs
    Type,
    /// identifier after `std::`
    StdName,
    /// numeric literal
    Number,
    /// string literal
    Str,
    /// char literal
    Char,
    /// comment
    Comment,
    /// preprocessor line
    Preproc,
    /// punctuation and operators
    Punct,
    /// other identifier
    Ident,
}

/// Lenient tokenizer for syntax colouring: never fails.
pub fn highlight(_src: &str) -> Vec<(Span, HlKind)> {
    Vec::new()
}
