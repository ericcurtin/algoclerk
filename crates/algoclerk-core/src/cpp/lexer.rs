// SPDX-License-Identifier: Apache-2.0
//! Tokenizer: a strict one for the parser ([`lex`]) and a lenient one for syntax colouring
//! ([`highlight`]).

use super::ast::{IntBase, IntSuffix, Span};
use super::diag::Diag;

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

/// Every C++17 keyword. The subset supports few of them, the parser reports the others as unsupported.
/// (The alternative operator spellings `and`, `or`, `not`... are keywords here too but [`lex`] turns them
/// into the matching punctuation token.)
const KEYWORDS: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char16_t",
    "char32_t",
    "class",
    "compl",
    "const",
    "const_cast",
    "constexpr",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// Keywords that name a builtin type (coloured as types by [`highlight`]).
const BUILTIN_TYPES: &[&str] = &[
    "bool", "char", "char16_t", "char32_t", "double", "float", "int", "long", "short", "signed",
    "unsigned", "void", "wchar_t",
];

/// Library type names (after `std::`, or unqualified under `using namespace std`).
const STD_TYPES: &[&str] = &[
    "string",
    "string_view",
    "vector",
    "array",
    "pair",
    "tuple",
    "optional",
    "unordered_map",
    "map",
    "unordered_set",
    "set",
    "multiset",
    "multimap",
    "stack",
    "queue",
    "deque",
    "list",
    "priority_queue",
    "function",
    "size_t",
    "ptrdiff_t",
    "greater",
    "less",
    "numeric_limits",
    "bitset",
    "int8_t",
    "int16_t",
    "int32_t",
    "int64_t",
    "uint8_t",
    "uint16_t",
    "uint32_t",
    "uint64_t",
];

/// Well-known library functions and objects, coloured as std names under `using namespace std`.
const STD_NAMES: &[&str] = &[
    "sort",
    "stable_sort",
    "reverse",
    "min",
    "max",
    "swap",
    "abs",
    "accumulate",
    "iota",
    "to_string",
    "stoi",
    "stoll",
    "stoul",
    "stoull",
    "find",
    "find_if",
    "count",
    "count_if",
    "lower_bound",
    "upper_bound",
    "binary_search",
    "make_pair",
    "make_tuple",
    "get",
    "tie",
    "next_permutation",
    "prev_permutation",
    "unique",
    "fill",
    "max_element",
    "min_element",
    "nullopt",
    "isdigit",
    "isalpha",
    "isalnum",
    "islower",
    "isupper",
    "isspace",
    "tolower",
    "toupper",
    "move",
    "gcd",
    "lcm",
    "cout",
    "cin",
    "cerr",
    "endl",
    "sqrt",
    "pow",
    "any_of",
    "all_of",
    "none_of",
    "begin",
    "end",
    "size",
    "empty",
    "rotate",
    "swap_ranges",
    "copy",
    "remove",
    "remove_if",
    "partial_sum",
    "minmax",
    "clamp",
    "make_optional",
    "nth_element",
    "partition",
    "merge",
    "adjacent_find",
    "equal",
    "mismatch",
    "transform",
    "for_each",
    "inner_product",
    "exchange",
];

/// Multi-character operators, longest first. Everything else is a single character.
const PUNCT3: &[&str] = &["<<=", ">>=", "..."];
const PUNCT2: &[&str] = &[
    "::", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+=", "-=", "*=", "/=",
    "%=", "&=", "|=", "^=",
];
const PUNCT1: &[&str] = &[
    "{", "}", "(", ")", "[", "]", ";", ":", ",", ".", "?", "+", "-", "*", "/", "%", "&", "|", "^",
    "~", "!", "=", "<", ">",
];

/// The punctuation token starting at byte `i`, longest match.
fn punct_at(b: &[u8], i: usize) -> Option<&'static str> {
    let rest = &b[i..];
    for table in [PUNCT3, PUNCT2, PUNCT1] {
        for p in table {
            if rest.starts_with(p.as_bytes()) {
                return Some(p);
            }
        }
    }
    None
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn keyword(s: &str) -> Option<&'static str> {
    KEYWORDS.iter().copied().find(|k| *k == s)
}

/// Punctuation spelled as a keyword (`and` is `&&`).
fn alt_operator(k: &str) -> Option<&'static str> {
    Some(match k {
        "and" => "&&",
        "or" => "||",
        "not" => "!",
        "xor" => "^",
        "bitand" => "&",
        "bitor" => "|",
        "compl" => "~",
        "and_eq" => "&=",
        "or_eq" => "|=",
        "xor_eq" => "^=",
        "not_eq" => "!=",
        _ => return None,
    })
}

fn sp(start: usize, end: usize) -> Span {
    Span {
        start: start as u32,
        end: end as u32,
    }
}

// ---------------------------------------------------------------------------------------------------
// Strict lexer
// ---------------------------------------------------------------------------------------------------

/// What a [`Token`] is.
#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// An identifier that is not a keyword.
    Ident(String),
    /// A C++ keyword (see `KEYWORDS`); `true`, `false` and `nullptr` are keywords too.
    Kw(&'static str),
    /// Integer literal: magnitude, suffix and spelling base. A leading `0` followed by more digits is
    /// octal. Digit separators are removed. The suffixes `u`, `ll`, `ull` (any case, `llu` too) are
    /// accepted; a `long` suffix (`l`, `ul`) is an `parse.unsupported` error.
    Int {
        /// Magnitude.
        value: u64,
        /// Suffix.
        suffix: IntSuffix,
        /// Spelling base.
        base: IntBase,
    },
    /// Floating literal (`1.5`, `2e3`, `1.f`). The subset has no floating types: the parser reports it.
    Float(String),
    /// Character literal, escapes resolved (ASCII only, the value fits a byte).
    Char(u8),
    /// String literal, escapes resolved (ASCII only). Adjacent literals are NOT merged.
    Str(String),
    /// Punctuation and operators (`::`, `->`, `<<=`, `>>`, ...). `>>` is one token: the parser splits it
    /// when it closes two template argument lists. The alternative spellings (`and`, `not`, ...) are
    /// returned as the symbol they stand for.
    Punct(&'static str),
    /// A whole preprocessor line, trimmed, continuation lines joined: `#include <vector>`.
    Preproc(String),
    /// A comment. `text` is what follows `//` (without the line end), or the inside of `/* */`.
    Comment {
        /// Comment body.
        text: String,
        /// `/* */` rather than `//`.
        block: bool,
    },
}

/// A token and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// What it is.
    pub kind: Tok,
    /// Byte range in the source.
    pub span: Span,
    /// A line break separates this token from the previous one (always true for the first token).
    pub nl_before: bool,
}

impl Token {
    /// Is this the punctuation `p`?
    pub fn is_punct(&self, p: &str) -> bool {
        matches!(&self.kind, Tok::Punct(q) if *q == p)
    }
    /// Is this the keyword `k`?
    pub fn is_kw(&self, k: &str) -> bool {
        matches!(&self.kind, Tok::Kw(q) if *q == k)
    }
    /// Is this a comment?
    pub fn is_comment(&self) -> bool {
        matches!(self.kind, Tok::Comment { .. })
    }
}

/// Split the comment tokens from the others (both keep source order).
pub fn split_comments(tokens: Vec<Token>) -> (Vec<Token>, Vec<Token>) {
    tokens.into_iter().partition(|t| !t.is_comment())
}

fn lex_err(start: usize, end: usize, msg: impl Into<String>) -> Diag {
    Diag::at(sp(start, end.max(start + 1)), "parse.lex", msg)
}

fn unsupported(start: usize, end: usize, msg: impl Into<String>) -> Diag {
    Diag::at(sp(start, end.max(start + 1)), "parse.unsupported", msg)
}

/// End (exclusive) of the character starting at byte `i` (`i` is on a char boundary).
fn char_end(src: &str, i: usize) -> usize {
    i + src[i..].chars().next().map_or(1, char::len_utf8)
}

fn digit_val(c: u8) -> Option<u32> {
    (c as char).to_digit(16)
}

/// Scan digits of `radix` starting at `i` allowing `'` separators between digits. Pushes the digits.
fn scan_digits(b: &[u8], mut i: usize, radix: u32, out: &mut Vec<u8>) -> usize {
    let ok = |c: u8| digit_val(c).is_some_and(|d| d < radix);
    while i < b.len() {
        let c = b[i];
        if ok(c) {
            out.push(c);
            i += 1;
        } else if c == b'\'' && !out.is_empty() && ok(b[i - 1]) && i + 1 < b.len() && ok(b[i + 1]) {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn lex_number(src: &str, start: usize) -> Result<(Tok, usize), Diag> {
    let b = src.as_bytes();
    let mut i = start;
    let mut digits = Vec::new();
    let mut base = IntBase::Dec;
    let mut radix = 10;
    if b[i] == b'0' && i + 1 < b.len() && matches!(b[i + 1], b'x' | b'X' | b'b' | b'B') {
        let hex = matches!(b[i + 1], b'x' | b'X');
        radix = if hex { 16 } else { 2 };
        base = if hex { IntBase::Hex } else { IntBase::Bin };
        i = scan_digits(b, i + 2, radix, &mut digits);
        if digits.is_empty() {
            return Err(lex_err(start, i, "this number has a prefix but no digits"));
        }
        if !hex && i < b.len() && b[i].is_ascii_digit() {
            return Err(lex_err(
                start,
                i + 1,
                "a binary number only has the digits 0 and 1",
            ));
        }
    } else {
        i = scan_digits(b, i, 10, &mut digits);
        // floating literal?
        let at = |k: usize| b.get(k).copied().unwrap_or(0);
        let frac = at(i) == b'.' && at(i + 1) != b'.';
        let exp = matches!(at(i), b'e' | b'E')
            && (at(i + 1).is_ascii_digit()
                || (matches!(at(i + 1), b'+' | b'-') && at(i + 2).is_ascii_digit()));
        if digits.is_empty() || frac || exp {
            let mut j = i;
            if at(j) == b'.' {
                j += 1;
                let mut d = Vec::new();
                j = scan_digits(b, j, 10, &mut d);
            }
            if matches!(at(j), b'e' | b'E') {
                let mut k = j + 1;
                if matches!(at(k), b'+' | b'-') {
                    k += 1;
                }
                if at(k).is_ascii_digit() {
                    let mut d = Vec::new();
                    j = scan_digits(b, k, 10, &mut d);
                } else {
                    return Err(lex_err(start, k, "this exponent has no digits"));
                }
            }
            if matches!(at(j), b'f' | b'F' | b'l' | b'L') {
                j += 1;
            }
            if j < b.len() && is_ident_char(b[j]) {
                return Err(lex_err(start, j + 1, "this number has letters stuck to it"));
            }
            return Ok((Tok::Float(src[start..j].replace('\'', "")), j));
        }
        if digits.len() > 1 && digits[0] == b'0' {
            base = IntBase::Oct;
            radix = 8;
            if digits.iter().any(|&c| c >= b'8') {
                return Err(lex_err(
                    start,
                    i,
                    "an octal number (starting with 0) only has the digits 0 to 7",
                ));
            }
        }
    }
    // suffix
    let sfx_start = i;
    while i < b.len() && is_ident_char(b[i]) {
        i += 1;
    }
    let suffix = match &src[sfx_start..i] {
        "" => IntSuffix::None,
        "u" | "U" => IntSuffix::U,
        "ll" | "LL" => IntSuffix::LL,
        "ull" | "uLL" | "Ull" | "ULL" | "llu" | "LLu" | "llU" | "LLU" => IntSuffix::ULL,
        "l" | "L" | "ul" | "uL" | "Ul" | "UL" | "lu" | "Lu" | "lU" | "LU" => {
            return Err(unsupported(
                start,
                i,
                "this office does not stock `long`: use `long long` (suffix `ll`) instead",
            ));
        }
        s => {
            return Err(lex_err(
                sfx_start,
                i,
                format!("`{s}` is not a number suffix I know (use u, ll or ull)"),
            ));
        }
    };
    let mut value: u64 = 0;
    for &c in &digits {
        let d = digit_val(c).unwrap_or(0) as u64;
        value = value
            .checked_mul(radix as u64)
            .and_then(|v| v.checked_add(d))
            .ok_or_else(|| lex_err(start, i, "this number is too big"))?;
    }
    Ok((
        Tok::Int {
            value,
            suffix,
            base,
        },
        i,
    ))
}

/// Parse an escape sequence; `i` is on the backslash. Returns the byte and the index after it.
fn lex_escape(src: &str, i: usize) -> Result<(u8, usize), Diag> {
    let b = src.as_bytes();
    let Some(&c) = b.get(i + 1) else {
        return Err(lex_err(i, i + 1, "this backslash has nothing after it"));
    };
    let simple = |v: u8| Ok((v, i + 2));
    match c {
        b'n' => simple(b'\n'),
        b't' => simple(b'\t'),
        b'r' => simple(b'\r'),
        b'a' => simple(7),
        b'b' => simple(8),
        b'f' => simple(12),
        b'v' => simple(11),
        b'\\' => simple(b'\\'),
        b'\'' => simple(b'\''),
        b'"' => simple(b'"'),
        b'?' => simple(b'?'),
        b'0'..=b'7' => {
            let mut j = i + 1;
            let mut v: u32 = 0;
            while j < b.len() && j < i + 4 && (b'0'..=b'7').contains(&b[j]) {
                v = v * 8 + (b[j] - b'0') as u32;
                j += 1;
            }
            if v > 255 {
                return Err(lex_err(i, j, "this octal escape does not fit in a char"));
            }
            Ok((v as u8, j))
        }
        b'x' => {
            let mut j = i + 2;
            let mut v: u32 = 0;
            while j < b.len() && b[j].is_ascii_hexdigit() {
                v = v
                    .saturating_mul(16)
                    .saturating_add(digit_val(b[j]).unwrap_or(0));
                j += 1;
            }
            if j == i + 2 {
                return Err(lex_err(i, j, "`\\x` needs hexadecimal digits"));
            }
            if v > 255 {
                return Err(lex_err(i, j, "this hex escape does not fit in a char"));
            }
            Ok((v as u8, j))
        }
        b'u' | b'U' => Err(unsupported(
            i,
            i + 2,
            "this office only handles ASCII text: unicode escapes are not stocked",
        )),
        _ => Err(lex_err(
            i,
            char_end(src, i + 1),
            "I do not know this escape sequence",
        )),
    }
}

fn lex_char(src: &str, start: usize) -> Result<(Tok, usize), Diag> {
    let b = src.as_bytes();
    let mut i = start + 1;
    let v = match b.get(i) {
        None | Some(b'\n') | Some(b'\r') => {
            return Err(lex_err(start, i, "this character literal is not closed"));
        }
        Some(b'\'') => return Err(lex_err(start, i + 1, "this character literal is empty")),
        Some(b'\\') => {
            let (v, j) = lex_escape(src, i)?;
            i = j;
            v
        }
        Some(&c) if c < 0x80 => {
            i += 1;
            c
        }
        Some(_) => {
            let e = char_end(src, i);
            return Err(unsupported(i, e, "this office only handles ASCII text"));
        }
    };
    if b.get(i) != Some(&b'\'') {
        let mut j = i;
        while j < b.len() && !matches!(b[j], b'\'' | b'\n') {
            j += 1;
        }
        let msg = if b.get(j) == Some(&b'\'') {
            "a character literal holds a single character (use a string for more)"
        } else {
            "this character literal is not closed"
        };
        return Err(lex_err(start, (j + 1).min(b.len()), msg));
    }
    Ok((Tok::Char(v), i + 1))
}

fn lex_string(src: &str, start: usize) -> Result<(Tok, usize), Diag> {
    let b = src.as_bytes();
    let mut i = start + 1;
    let mut out = String::new();
    loop {
        match b.get(i) {
            None | Some(b'\n') | Some(b'\r') => {
                return Err(lex_err(start, i, "this string literal is not closed"));
            }
            Some(b'"') => return Ok((Tok::Str(out), i + 1)),
            Some(b'\\') => {
                if matches!(b.get(i + 1), Some(b'\n')) {
                    // line continuation inside a string
                    i += 2;
                    continue;
                }
                let (v, j) = lex_escape(src, i)?;
                if v >= 0x80 {
                    return Err(unsupported(i, j, "this office only handles ASCII text"));
                }
                out.push(v as char);
                i = j;
            }
            Some(&c) if c < 0x80 => {
                out.push(c as char);
                i += 1;
            }
            Some(_) => {
                let e = char_end(src, i);
                return Err(unsupported(i, e, "this office only handles ASCII text"));
            }
        }
    }
}

/// End of the preprocessor directive starting at `start` (a `#`) and its joined text. Stops before a
/// comment or at the line end.
fn scan_preproc(src: &str, start: usize) -> (String, usize) {
    let b = src.as_bytes();
    let mut i = start;
    let mut text = String::new();
    let mut in_str = false;
    let mut end = start;
    while i < b.len() {
        let c = b[i];
        if c == b'\\' && matches!(b.get(i + 1), Some(b'\n')) {
            text.push(' ');
            i += 2;
            continue;
        }
        if c == b'\\' && b.get(i + 1) == Some(&b'\r') && b.get(i + 2) == Some(&b'\n') {
            text.push(' ');
            i += 3;
            continue;
        }
        if c == b'\n' {
            break;
        }
        if c == b'"' {
            in_str = !in_str;
        }
        if !in_str && c == b'/' && matches!(b.get(i + 1), Some(b'/') | Some(b'*')) {
            break;
        }
        let e = char_end(src, i);
        text.push_str(&src[i..e]);
        if !c.is_ascii_whitespace() {
            end = e;
        }
        i = e;
    }
    (text.trim().to_string(), end)
}

/// Strict tokenizer used by the parser. Comments and preprocessor lines are tokens ([`Tok::Comment`],
/// [`Tok::Preproc`]); whitespace is dropped. The first malformed piece of text is reported as a `Diag`
/// (`parse.lex`, or `parse.unsupported` for valid C++ the subset does not stock). Never panics.
pub fn lex(src: &str) -> Result<Vec<Token>, Diag> {
    if src.len() >= u32::MAX as usize {
        return Err(Diag::plain(
            "parse.lex",
            "this text is far too long for the office",
        ));
    }
    let b = src.as_bytes();
    let mut toks: Vec<Token> = Vec::new();
    let mut i = 0;
    let mut nl = true; // a line break since the previous token
    let mut line_start = true; // only whitespace so far on this line
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            nl = true;
            line_start = true;
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' || c == 0x0b || c == 0x0c {
            i += 1;
            continue;
        }
        let start = i;
        let kind;
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let mut j = i + 2;
            while j < b.len() && b[j] != b'\n' {
                j += 1;
            }
            let mut text = &src[i + 2..j];
            text = text.strip_suffix('\r').unwrap_or(text);
            kind = Tok::Comment {
                text: text.to_string(),
                block: false,
            };
            i = j;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let mut j = i + 2;
            loop {
                if j + 1 >= b.len() {
                    return Err(lex_err(
                        start,
                        start + 2,
                        "this comment is never closed (I expected `*/`)",
                    ));
                }
                if b[j] == b'*' && b[j + 1] == b'/' {
                    break;
                }
                j += 1;
            }
            kind = Tok::Comment {
                text: src[i + 2..j].to_string(),
                block: true,
            };
            i = j + 2;
        } else if c == b'#' {
            if !line_start {
                return Err(lex_err(
                    start,
                    start + 1,
                    "a `#` only starts a directive at the beginning of a line",
                ));
            }
            let (text, end) = scan_preproc(src, i);
            kind = Tok::Preproc(text);
            i = end;
        } else if is_ident_start(c) {
            let mut j = i + 1;
            while j < b.len() && is_ident_char(b[j]) {
                j += 1;
            }
            let word = &src[i..j];
            if matches!(b.get(j), Some(b'"') | Some(b'\''))
                && matches!(
                    word,
                    "L" | "u" | "U" | "u8" | "R" | "LR" | "uR" | "UR" | "u8R"
                )
            {
                return Err(unsupported(
                    start,
                    j + 1,
                    "prefixed and raw string or character literals are not stocked here",
                ));
            }
            kind = match keyword(word) {
                Some(k) => match alt_operator(k) {
                    Some(p) => Tok::Punct(p),
                    None => Tok::Kw(k),
                },
                None => Tok::Ident(word.to_string()),
            };
            i = j;
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let (k, j) = lex_number(src, i)?;
            kind = k;
            i = j;
        } else if c == b'"' {
            let (k, j) = lex_string(src, i)?;
            kind = k;
            i = j;
        } else if c == b'\'' {
            let (k, j) = lex_char(src, i)?;
            kind = k;
            i = j;
        } else if let Some(p) = punct_at(b, i) {
            kind = Tok::Punct(p);
            i += p.len();
        } else {
            let e = char_end(src, i);
            let ch = &src[i..e];
            return Err(lex_err(
                i,
                e,
                format!("I do not recognise the character `{ch}`"),
            ));
        }
        let block_comment = matches!(&kind, Tok::Comment { block: true, .. });
        toks.push(Token {
            kind,
            span: sp(start, i),
            nl_before: nl,
        });
        nl = false;
        line_start = line_start && block_comment && !src[start..i].contains('\n');
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------------------------------
// Highlighter
// ---------------------------------------------------------------------------------------------------

/// Lenient tokenizer for syntax colouring: never fails, covers comments and preprocessor lines too.
///
/// Spans are byte ranges on char boundaries, ascending and disjoint; whitespace is not covered. An
/// unterminated string, char or block comment runs to the end of the line (comment: of the text).
/// `StdName` is an identifier after `std::` (when it is not a type), or a well-known std function under
/// `using namespace std`; `Type` is a builtin or std type name, `ListNode`, `TreeNode`, and the structs
/// and `using` aliases defined in the text.
pub fn highlight(src: &str) -> Vec<(Span, HlKind)> {
    let b = src.as_bytes();
    let mut out: Vec<(Span, HlKind)> = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() || c == 0x0b || c == 0x0c {
            i += 1;
            continue;
        }
        let start = i;
        let kind;
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            kind = HlKind::Comment;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                i += 1;
            }
            i = (i + 2).min(b.len());
            kind = HlKind::Comment;
            // stays at line start when nothing but whitespace precedes (rare), otherwise not
            if src[start..i].contains('\n') {
                line_start = false;
            }
            out.push((sp(start, i), kind));
            continue;
        } else if c == b'#' && line_start {
            let (_, end) = scan_preproc(src, i);
            i = end.max(i + 1);
            kind = HlKind::Preproc;
        } else if is_ident_start(c) {
            while i < b.len() && is_ident_char(b[i]) {
                i += 1;
            }
            let w = &src[start..i];
            kind = match keyword(w) {
                Some(k) if BUILTIN_TYPES.contains(&k) => HlKind::Type,
                Some(_) => HlKind::Keyword,
                None => HlKind::Ident,
            };
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let hex = c == b'0' && matches!(b.get(i + 1), Some(b'x') | Some(b'X'));
            i += 1;
            while i < b.len() {
                let d = b[i];
                let digit_sep =
                    d == b'\'' && b.get(i + 1).is_some_and(|n| n.is_ascii_alphanumeric());
                let exp_sign = matches!(d, b'+' | b'-')
                    && if hex {
                        matches!(b[i - 1], b'p' | b'P')
                    } else {
                        matches!(b[i - 1], b'e' | b'E')
                    };
                if is_ident_char(d) || d == b'.' || digit_sep || exp_sign {
                    i += 1;
                } else {
                    break;
                }
            }
            kind = HlKind::Number;
        } else if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                if b[i] == b'\\' && i + 1 < b.len() && b[i + 1] != b'\n' {
                    i += 1;
                }
                i += 1;
            }
            if i < b.len() && b[i] == c {
                i += 1;
            }
            kind = if c == b'"' { HlKind::Str } else { HlKind::Char };
        } else if let Some(p) = punct_at(b, i) {
            i += p.len();
            kind = HlKind::Punct;
        } else {
            i = char_end(src, i);
            kind = HlKind::Punct;
        }
        line_start = false;
        out.push((sp(start, i), kind));
    }
    classify_idents(src, &mut out);
    out
}

/// Second pass of [`highlight`]: type names, std names and user types.
fn classify_idents(src: &str, out: &mut [(Span, HlKind)]) {
    let text = |s: Span| &src[s.start as usize..s.end as usize];
    // significant (non-comment) token indices
    let sig: Vec<usize> = (0..out.len())
        .filter(|&k| out[k].1 != HlKind::Comment && out[k].1 != HlKind::Preproc)
        .collect();
    let mut using_std = false;
    let mut user_types: Vec<&str> = Vec::new();
    for w in 0..sig.len() {
        let t = |d: usize| sig.get(w + d).map(|&k| (text(out[k].0), out[k].1));
        match t(0) {
            Some(("using", HlKind::Keyword)) => {
                if matches!(t(1), Some(("namespace", _))) && matches!(t(2), Some(("std", _))) {
                    using_std = true;
                } else if let (Some((name, HlKind::Ident)), Some(("=", _))) = (t(1), t(2)) {
                    user_types.push(name);
                }
            }
            Some(("struct", HlKind::Keyword)) | Some(("class", HlKind::Keyword)) => {
                if let Some((name, HlKind::Ident)) = t(1) {
                    user_types.push(name);
                }
            }
            _ => {}
        }
    }
    for w in 0..sig.len() {
        let k = sig[w];
        if out[k].1 != HlKind::Ident {
            continue;
        }
        let name = text(out[k].0);
        let prev = |d: usize| {
            w.checked_sub(d)
                .map(|p| (text(out[sig[p]].0), out[sig[p]].1))
        };
        let member = matches!(prev(1), Some((".", _)) | Some(("->", _)));
        if member {
            continue;
        }
        let after_std = matches!(prev(1), Some(("::", _))) && matches!(prev(2), Some(("std", _)));
        let qualified_other = matches!(prev(1), Some(("::", _))) && !after_std;
        if qualified_other {
            continue;
        }
        out[k].1 = if after_std {
            if STD_TYPES.contains(&name) {
                HlKind::Type
            } else {
                HlKind::StdName
            }
        } else if name == "ListNode"
            || name == "TreeNode"
            || user_types.contains(&name)
            || name == "size_t"
            || (using_std && STD_TYPES.contains(&name))
        {
            HlKind::Type
        } else if using_std && STD_NAMES.contains(&name) {
            HlKind::StdName
        } else {
            HlKind::Ident
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }
    fn int(value: u64, suffix: IntSuffix, base: IntBase) -> Tok {
        Tok::Int {
            value,
            suffix,
            base,
        }
    }
    fn err(src: &str) -> Diag {
        lex(src).unwrap_err()
    }
    fn hl(src: &str) -> Vec<(&str, HlKind)> {
        highlight(src)
            .into_iter()
            .map(|(s, k)| (&src[s.start as usize..s.end as usize], k))
            .collect()
    }

    #[test]
    fn idents_and_keywords() {
        assert_eq!(
            kinds("int x_1 = nullptr; foo"),
            vec![
                Tok::Kw("int"),
                Tok::Ident("x_1".into()),
                Tok::Punct("="),
                Tok::Kw("nullptr"),
                Tok::Punct(";"),
                Tok::Ident("foo".into()),
            ]
        );
        assert_eq!(kinds("override"), vec![Tok::Ident("override".into())]);
        assert_eq!(kinds("size_t"), vec![Tok::Ident("size_t".into())]);
    }

    #[test]
    fn integers() {
        use IntBase::*;
        use IntSuffix::*;
        assert_eq!(kinds("0"), vec![int(0, None, Dec)]);
        assert_eq!(kinds("42"), vec![int(42, None, Dec)]);
        assert_eq!(kinds("7u 7U"), vec![int(7, U, Dec), int(7, U, Dec)]);
        assert_eq!(kinds("7ll 7LL"), vec![int(7, LL, Dec), int(7, LL, Dec)]);
        assert_eq!(kinds("7ull 7ULL 7llu 7uLL"), vec![int(7, ULL, Dec); 4]);
        assert_eq!(
            kinds("0x2A 0XffU"),
            vec![int(42, None, Hex), int(255, U, Hex)]
        );
        assert_eq!(kinds("0b1010"), vec![int(10, None, Bin)]);
        assert_eq!(kinds("052"), vec![int(42, None, Oct)]);
        assert_eq!(kinds("1'000'000"), vec![int(1_000_000, None, Dec)]);
        assert_eq!(kinds("0xFF'FF"), vec![int(0xFFFF, None, Hex)]);
        assert_eq!(kinds("0b1111'0000ull"), vec![int(240, ULL, Bin)]);
        assert_eq!(
            kinds("18446744073709551615"),
            vec![int(u64::MAX, None, Dec)]
        );
        assert_eq!(kinds("0xFFFFFFFFFFFFFFFF"), vec![int(u64::MAX, None, Hex)]);
    }

    #[test]
    fn integer_errors() {
        assert_eq!(err("18446744073709551616").code, "parse.lex");
        assert_eq!(err("0x").code, "parse.lex");
        assert_eq!(err("0b102").code, "parse.lex");
        assert_eq!(err("089").code, "parse.lex");
        assert_eq!(err("12abc").code, "parse.lex");
        assert_eq!(err("1lL").code, "parse.lex");
        assert_eq!(err("10l").code, "parse.unsupported");
        assert_eq!(err("10ul").code, "parse.unsupported");
        // separators only between digits
        assert_eq!(err("1''0").code, "parse.lex");
        assert_eq!(err("100'").code, "parse.lex");
    }

    #[test]
    fn floats_are_tokens() {
        assert_eq!(kinds("1.5"), vec![Tok::Float("1.5".into())]);
        assert_eq!(kinds(".5"), vec![Tok::Float(".5".into())]);
        assert_eq!(kinds("2e-3"), vec![Tok::Float("2e-3".into())]);
        assert_eq!(kinds("1.f"), vec![Tok::Float("1.f".into())]);
        assert_eq!(kinds("3.0e+2L"), vec![Tok::Float("3.0e+2L".into())]);
        assert_eq!(err("1e+").code, "parse.lex");
    }

    #[test]
    fn chars() {
        assert_eq!(kinds("'a'"), vec![Tok::Char(b'a')]);
        assert_eq!(kinds(r"'\n' '\t' '\0' '\\' '\'' '\x41' '\101'"), {
            vec![
                Tok::Char(10),
                Tok::Char(9),
                Tok::Char(0),
                Tok::Char(b'\\'),
                Tok::Char(b'\''),
                Tok::Char(b'A'),
                Tok::Char(b'A'),
            ]
        });
        assert_eq!(kinds(r#"'"'"#), vec![Tok::Char(b'"')]);
        assert_eq!(kinds(r"'\xff'"), vec![Tok::Char(255)]);
        for bad in [
            "''", "'ab'", "'a", "'\\q'", "'\\x'", "'\\x1ff'", "'é'", "'\n'",
        ] {
            assert!(lex(bad).is_err(), "{bad:?}");
        }
        assert_eq!(err("'é'").code, "parse.unsupported");
    }

    #[test]
    fn strings() {
        assert_eq!(kinds(r#""hi""#), vec![Tok::Str("hi".into())]);
        assert_eq!(kinds(r#""""#), vec![Tok::Str(String::new())]);
        assert_eq!(
            kinds(r#""a\"b\\c\n\t\0\x41\101//x""#),
            vec![Tok::Str("a\"b\\c\n\t\0AA//x".into())]
        );
        assert_eq!(
            kinds(r#""a" "b""#),
            vec![Tok::Str("a".into()), Tok::Str("b".into())]
        );
        assert_eq!(err("\"abc").code, "parse.lex");
        assert_eq!(err("\"ab\ncd\"").code, "parse.lex");
        assert_eq!(err("\"a\\").code, "parse.lex");
        assert_eq!(err("\"é\"").code, "parse.unsupported");
        assert_eq!(err("\"\\xe9\"").code, "parse.unsupported");
        assert_eq!(err("R\"(x)\"").code, "parse.unsupported");
        assert_eq!(err("u8\"x\"").code, "parse.unsupported");
        assert_eq!(err("L'x'").code, "parse.unsupported");
    }

    #[test]
    fn punctuation_longest_match() {
        let p = |s: &str| kinds(s);
        assert_eq!(
            p("a::b->c"),
            vec![
                Tok::Ident("a".into()),
                Tok::Punct("::"),
                Tok::Ident("b".into()),
                Tok::Punct("->"),
                Tok::Ident("c".into()),
            ]
        );
        assert_eq!(
            p("<<= >>= << >> <= >= == !="),
            vec![
                Tok::Punct("<<="),
                Tok::Punct(">>="),
                Tok::Punct("<<"),
                Tok::Punct(">>"),
                Tok::Punct("<="),
                Tok::Punct(">="),
                Tok::Punct("=="),
                Tok::Punct("!="),
            ]
        );
        assert_eq!(
            p("a+++b"),
            vec![
                Tok::Ident("a".into()),
                Tok::Punct("++"),
                Tok::Punct("+"),
                Tok::Ident("b".into()),
            ]
        );
        assert_eq!(
            p("&& || += -= *= /= %= &= |= ^= ... ~ ? :"),
            vec![
                Tok::Punct("&&"),
                Tok::Punct("||"),
                Tok::Punct("+="),
                Tok::Punct("-="),
                Tok::Punct("*="),
                Tok::Punct("/="),
                Tok::Punct("%="),
                Tok::Punct("&="),
                Tok::Punct("|="),
                Tok::Punct("^="),
                Tok::Punct("..."),
                Tok::Punct("~"),
                Tok::Punct("?"),
                Tok::Punct(":"),
            ]
        );
        assert_eq!(
            p("vector<vector<int>>"),
            vec![
                Tok::Ident("vector".into()),
                Tok::Punct("<"),
                Tok::Ident("vector".into()),
                Tok::Punct("<"),
                Tok::Kw("int"),
                Tok::Punct(">>"),
            ]
        );
        assert_eq!(
            p("{}()[];,."),
            vec![
                Tok::Punct("{"),
                Tok::Punct("}"),
                Tok::Punct("("),
                Tok::Punct(")"),
                Tok::Punct("["),
                Tok::Punct("]"),
                Tok::Punct(";"),
                Tok::Punct(","),
                Tok::Punct("."),
            ]
        );
    }

    #[test]
    fn alternative_operators() {
        assert_eq!(
            kinds("a and not b or c"),
            vec![
                Tok::Ident("a".into()),
                Tok::Punct("&&"),
                Tok::Punct("!"),
                Tok::Ident("b".into()),
                Tok::Punct("||"),
                Tok::Ident("c".into()),
            ]
        );
        assert_eq!(kinds("android"), vec![Tok::Ident("android".into())]);
    }

    #[test]
    fn bad_characters() {
        assert_eq!(err("a @ b").code, "parse.lex");
        assert_eq!(err("a $ b").code, "parse.lex");
        assert_eq!(err("x \\ y").code, "parse.lex");
        assert_eq!(err("é").code, "parse.lex");
        assert_eq!(err("a # b").code, "parse.lex");
        // the span covers the whole character
        let d = err("a é b");
        assert_eq!(d.span, Some(Span { start: 2, end: 4 }));
    }

    #[test]
    fn comments_are_tokens() {
        let toks = lex("int x; // trailing\n// own line\n/* block\nmore */ y").unwrap();
        let got: Vec<_> = toks.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(
            got,
            vec![
                Tok::Kw("int"),
                Tok::Ident("x".into()),
                Tok::Punct(";"),
                Tok::Comment {
                    text: " trailing".into(),
                    block: false
                },
                Tok::Comment {
                    text: " own line".into(),
                    block: false
                },
                Tok::Comment {
                    text: " block\nmore ".into(),
                    block: true
                },
                Tok::Ident("y".into()),
            ]
        );
        let nl: Vec<_> = toks.iter().map(|t| t.nl_before).collect();
        assert_eq!(nl, vec![true, false, false, false, true, true, false]);
        let (code, comments) = split_comments(toks);
        assert_eq!(code.len(), 4);
        assert_eq!(comments.len(), 3);
        assert!(comments.iter().all(Token::is_comment));
        assert_eq!(err("/* never").code, "parse.lex");
        assert_eq!(err("/*").code, "parse.lex");
        assert_eq!(kinds("a //x").len(), 2);
        // `//` inside a string is not a comment
        assert_eq!(kinds(r#""//" /**/"#).len(), 2);
        // CRLF
        assert_eq!(
            kinds("// hi\r\nx"),
            vec![
                Tok::Comment {
                    text: " hi".into(),
                    block: false
                },
                Tok::Ident("x".into())
            ]
        );
        // `/` then `*` split by nothing is a comment, `/ *` is not
        assert_eq!(kinds("a / *b").len(), 4);
    }

    #[test]
    fn preprocessor_lines() {
        assert_eq!(
            kinds("#include <vector>\n  #  include \"a//b.h\" // c\nint"),
            vec![
                Tok::Preproc("#include <vector>".into()),
                Tok::Preproc("#  include \"a//b.h\"".into()),
                Tok::Comment {
                    text: " c".into(),
                    block: false
                },
                Tok::Kw("int"),
            ]
        );
        assert_eq!(
            kinds("#define X \\\n  1\nx"),
            vec![
                Tok::Preproc("#define X    1".into()),
                Tok::Ident("x".into())
            ]
        );
        assert_eq!(
            kinds("#include <bits/stdc++.h>"),
            vec![Tok::Preproc("#include <bits/stdc++.h>".into())]
        );
        assert_eq!(err("int x; #include <vector>").code, "parse.lex");
        let t = lex("#include <a>\nx").unwrap();
        assert_eq!(t[0].span, Span { start: 0, end: 12 });
        assert_eq!(t[1].span, Span { start: 13, end: 14 });
    }

    #[test]
    fn spans_and_nl() {
        let src = "int main() {\n  return 0;\n}";
        let toks = lex(src).unwrap();
        for t in &toks {
            let text = &src[t.span.start as usize..t.span.end as usize];
            assert!(!text.is_empty() && text.trim() == text);
        }
        assert_eq!(
            &src[toks[1].span.start as usize..toks[1].span.end as usize],
            "main"
        );
        let nls: Vec<_> = toks
            .iter()
            .filter(|t| t.nl_before)
            .map(|t| t.span.start)
            .collect();
        assert_eq!(nls, vec![0, 15, 25]);
        assert!(lex("").unwrap().is_empty());
        assert!(lex("  \n\t ").unwrap().is_empty());
    }

    #[test]
    fn lex_never_panics_on_junk() {
        let junk = [
            "'",
            "\"",
            "\\",
            "/*",
            "/",
            "0x",
            "0b",
            ".",
            "..",
            "1.",
            "1e",
            "'\\",
            "\"\\x",
            "#",
            "é",
            "'é",
            "\"é",
            "0'1",
            "1'",
            "'\\777'",
            "9999999999999999999999",
            "0x'1",
            "\u{1F600}",
            "a\u{0}b",
        ];
        for j in junk {
            let _ = lex(j);
            let _ = highlight(j);
        }
        // every prefix of a realistic program
        let src = "#include <vector>\nint f(const std::vector<int>& v) { // c\n return v[0] + 'x' - \"s\\n\"[0] + 0x1F; }\n/* z */";
        let mut end = 0;
        while end <= src.len() {
            if src.is_char_boundary(end) {
                let _ = lex(&src[..end]);
                let _ = highlight(&src[..end]);
            }
            end += 1;
        }
    }

    #[test]
    fn highlight_basic() {
        use HlKind::*;
        assert_eq!(
            hl("int x = 42; // hi"),
            vec![
                ("int", Type),
                ("x", Ident),
                ("=", Punct),
                ("42", Number),
                (";", Punct),
                ("// hi", Comment),
            ]
        );
        assert_eq!(
            hl("for (;;) return nullptr;"),
            vec![
                ("for", Keyword),
                ("(", Punct),
                (";", Punct),
                (";", Punct),
                (")", Punct),
                ("return", Keyword),
                ("nullptr", Keyword),
                (";", Punct),
            ]
        );
    }

    #[test]
    fn highlight_literals() {
        use HlKind::*;
        assert_eq!(
            hl(r#"c = 'a' + '\'' + "s\"x" + 0x1F + 1'000 + 3ull + 1.5e-3"#),
            vec![
                ("c", Ident),
                ("=", Punct),
                ("'a'", Char),
                ("+", Punct),
                (r"'\''", Char),
                ("+", Punct),
                (r#""s\"x""#, Str),
                ("+", Punct),
                ("0x1F", Number),
                ("+", Punct),
                ("1'000", Number),
                ("+", Punct),
                ("3ull", Number),
                ("+", Punct),
                ("1.5e-3", Number),
            ]
        );
        // a - b is not an exponent sign
        assert_eq!(hl("1-2"), vec![("1", Number), ("-", Punct), ("2", Number)]);
        assert_eq!(hl("0x1e-2")[0], ("0x1e", Number));
    }

    #[test]
    fn highlight_lenient() {
        use HlKind::*;
        // unterminated things run to the line end and never fail
        assert_eq!(hl("\"abc\nx"), vec![("\"abc", Str), ("x", Ident)]);
        assert_eq!(hl("'a\nx"), vec![("'a", Char), ("x", Ident)]);
        assert_eq!(hl("x /* never"), vec![("x", Ident), ("/* never", Comment)]);
        assert_eq!(hl("a @ é b")[1], ("@", Punct));
        assert_eq!(hl("a @ é b")[2], ("é", Punct));
        assert_eq!(
            hl("/* a\nb */ x"),
            vec![("/* a\nb */", Comment), ("x", Ident)]
        );
    }

    #[test]
    fn highlight_preproc() {
        use HlKind::*;
        assert_eq!(
            hl("#include <vector> // c\nint"),
            vec![
                ("#include <vector>", Preproc),
                ("// c", Comment),
                ("int", Type)
            ]
        );
        assert_eq!(
            hl("  #define A \\\n  2\nx"),
            vec![("#define A \\\n  2", Preproc), ("x", Ident)]
        );
        assert_eq!(hl("a # b")[1], ("#", Punct));
    }

    #[test]
    fn highlight_std_names_and_types() {
        use HlKind::*;
        assert_eq!(
            hl("std::sort(std::vector<int> v)"),
            vec![
                ("std", Ident),
                ("::", Punct),
                ("sort", StdName),
                ("(", Punct),
                ("std", Ident),
                ("::", Punct),
                ("vector", Type),
                ("<", Punct),
                ("int", Type),
                (">", Punct),
                ("v", Ident),
                (")", Punct),
            ]
        );
        // unqualified names are plain without `using namespace std`
        assert_eq!(hl("sort(vector)")[0], ("sort", Ident));
        assert_eq!(hl("sort(vector)")[2], ("vector", Ident));
        let src = "using namespace std; vector<int> v; sort(v.begin(), v.end()); v.size();";
        let h = hl(src);
        let find = |n: &str, k: usize| h.iter().filter(|(t, _)| *t == n).nth(k).unwrap().1;
        assert_eq!(find("using", 0), Keyword);
        assert_eq!(find("namespace", 0), Keyword);
        assert_eq!(find("std", 0), Ident);
        assert_eq!(find("vector", 0), Type);
        assert_eq!(find("sort", 0), StdName);
        // members are never std names
        assert_eq!(find("begin", 0), Ident);
        assert_eq!(find("size", 0), Ident);
        // names behind another qualifier are plain
        assert_eq!(hl("using namespace std; A::sort(x)")[6], ("sort", Ident));
    }

    #[test]
    fn highlight_known_and_user_types() {
        use HlKind::*;
        let h = hl("ListNode* p; TreeNode* t; size_t n; unsigned long long u;");
        assert_eq!(h[0], ("ListNode", Type));
        assert_eq!(h[4], ("TreeNode", Type));
        assert_eq!(h[8], ("size_t", Type));
        assert_eq!(h[11], ("unsigned", Type));
        assert_eq!(h[12], ("long", Type));
        let h = hl("struct P { int a; }; using V = std::vector<P>; P p; V v; Q q;");
        let find = |n: &str, k: usize| h.iter().filter(|(t, _)| *t == n).nth(k).unwrap().1;
        assert_eq!(find("struct", 0), Keyword);
        assert_eq!(find("P", 0), Type);
        assert_eq!(find("P", 2), Type);
        assert_eq!(find("V", 0), Type);
        assert_eq!(find("V", 1), Type);
        assert_eq!(find("Q", 0), Ident);
        assert_eq!(hl("std::size_t n")[2], ("size_t", Type));
    }

    #[test]
    fn highlight_spans_are_ordered_disjoint_on_boundaries() {
        let src =
            "#include <a>\nint é(\"x\" 'y' /* c */ 0x1) { auto s = std::string(\"é\"); } // é\n@";
        let h = highlight(src);
        let mut last = 0u32;
        for (s, _) in &h {
            assert!(s.start >= last && s.end > s.start);
            assert!(src.is_char_boundary(s.start as usize) && src.is_char_boundary(s.end as usize));
            last = s.end;
        }
        assert!(last as usize <= src.len());
    }

    #[test]
    fn highlight_agrees_with_lex_on_valid_code() {
        let src = "int main() {\n  std::vector<int> v{1, 2, 3}; // c\n  for (auto x : v) { v[0] += x << 2; }\n  return 'a' + \"s\"[0];\n}\n";
        let toks = lex(src).unwrap();
        let h = highlight(src);
        assert_eq!(toks.len(), h.len());
        for (t, (s, _)) in toks.iter().zip(&h) {
            assert_eq!(t.span, *s);
        }
    }
}
