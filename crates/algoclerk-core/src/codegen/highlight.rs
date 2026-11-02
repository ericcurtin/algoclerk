// SPDX-License-Identifier: Apache-2.0
//! A tiny dependency-free syntax highlighter for the generated C, C++ and Rust code.

use super::Lang;

/// Token class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// Language keyword.
    Keyword,
    /// Built-in type name.
    Type,
    /// Number literal.
    Number,
    /// Comment.
    Comment,
    /// String or character literal.
    Str,
    /// Preprocessor line or attribute.
    Preproc,
    /// Identifier.
    Ident,
    /// Operators and punctuation.
    Punct,
    /// Whitespace.
    Space,
}

/// A slice of the source with its class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token<'a> {
    /// Class.
    pub kind: TokenKind,
    /// Text.
    pub text: &'a str,
}

const C_KW: &[&str] = &[
    "if", "else", "while", "for", "do", "break", "continue", "return", "goto", "static", "const", "struct",
    "sizeof", "switch", "case", "default", "true", "false",
];
const CPP_KW: &[&str] = &[
    "if", "else", "while", "for", "do", "break", "continue", "return", "goto", "static", "const", "struct",
    "sizeof", "switch", "case", "default", "true", "false", "auto", "namespace", "using", "static_cast",
    "constexpr",
];
const RUST_KW: &[&str] = &[
    "if", "else", "while", "for", "loop", "break", "continue", "return", "fn", "let", "mut", "pub", "match",
    "as", "in", "true", "false", "const", "static", "use", "struct", "impl",
];
const TYPES: &[&str] = &[
    "int", "char", "void", "bool", "size_t", "long", "short", "unsigned", "signed", "float", "double",
    "i32", "i64", "u8", "u32", "u64", "usize", "isize", "Vec", "std", "vector", "array", "size_type",
];

/// Split `code` into classified tokens. Concatenating the token texts gives `code` back.
pub fn highlight(lang: Lang, code: &str) -> Vec<Token<'_>> {
    let kws = match lang {
        Lang::C => C_KW,
        Lang::Cpp => CPP_KW,
        Lang::Rust => RUST_KW,
    };
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    while i < bytes.len() {
        let c = bytes[i];
        let start = i;
        let kind;
        if c == b'\n' {
            i += 1;
            kind = TokenKind::Space;
            line_start = true;
            out.push(Token {
                kind,
                text: &code[start..i],
            });
            continue;
        } else if c.is_ascii_whitespace() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() && bytes[i] != b'\n' {
                i += 1;
            }
            kind = TokenKind::Space;
        } else if code[i..].starts_with("//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            kind = TokenKind::Comment;
        } else if c == b'#' && (lang != Lang::Rust && line_start || lang == Lang::Rust && code[i..].starts_with("#[")) {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            kind = TokenKind::Preproc;
        } else if c == b'"' || c == b'\'' && lang != Lang::Rust {
            i += 1;
            while i < bytes.len() && bytes[i] != c && bytes[i] != b'\n' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(bytes.len());
            kind = TokenKind::Str;
        } else if c.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            kind = TokenKind::Number;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let w = &code[start..i];
            kind = if kws.contains(&w) {
                TokenKind::Keyword
            } else if TYPES.contains(&w) {
                TokenKind::Type
            } else {
                TokenKind::Ident
            };
        } else {
            // One (possibly multi-byte) punctuation character.
            i += code[i..].chars().next().map_or(1, char::len_utf8);
            kind = TokenKind::Punct;
        }
        line_start = false;
        out.push(Token {
            kind,
            text: &code[start..i],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_cover_input() {
        let src = "#include <stddef.h>\nint f(int x) { // hi\n  return x + 42; \"s\\\"q\" }\n";
        for lang in Lang::ALL {
            let t = highlight(lang, src);
            let joined: String = t.iter().map(|t| t.text).collect();
            assert_eq!(joined, src);
        }
    }

    #[test]
    fn classes() {
        let t = highlight(Lang::Rust, "let mut x: i32 = 5; // c");
        let kinds: Vec<_> = t.iter().filter(|t| t.kind != TokenKind::Space).map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::Keyword);
        assert!(kinds.contains(&TokenKind::Type));
        assert!(kinds.contains(&TokenKind::Number));
        assert_eq!(*kinds.last().unwrap(), TokenKind::Comment);
    }

    #[test]
    fn handles_non_ascii_and_garbage() {
        let src = "é \"unterminated\n'x ü";
        for lang in Lang::ALL {
            let t = highlight(lang, src);
            let joined: String = t.iter().map(|t| t.text).collect();
            assert_eq!(joined, src);
        }
    }
}
