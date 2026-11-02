// SPDX-License-Identifier: Apache-2.0
//! Syntax colouring of generated code (for the code reveal).

use super::Lang;

/// Token class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Everything else.
    Plain,
    /// `if`, `for`, `return` ...
    Keyword,
    /// Type names.
    Type,
    /// Numbers.
    Number,
    /// String and character literals.
    Str,
    /// Comments.
    Comment,
    /// `#include` lines.
    Preproc,
    /// Brackets and punctuation.
    Punct,
    /// A name followed by `(`.
    Func,
}

const KEYWORDS_C: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "do",
    "return",
    "break",
    "continue",
    "static",
    "inline",
    "typedef",
    "struct",
    "const",
    "sizeof",
    "new",
    "delete",
    "namespace",
    "using",
    "template",
    "class",
    "auto",
    "true",
    "false",
    "nullptr",
    "NULL",
    "switch",
    "case",
    "default",
    "unsigned",
    "size_t",
];
const TYPES_C: &[&str] = &[
    "int",
    "long",
    "char",
    "bool",
    "void",
    "double",
    "float",
    "string",
    "vector",
    "unordered_map",
    "unordered_set",
    "stack",
    "queue",
    "deque",
    "priority_queue",
    "optional",
    "pair",
    "tuple",
    "ListNode",
    "TreeNode",
    "Str",
];
const KEYWORDS_RS: &[&str] = &[
    "fn",
    "let",
    "mut",
    "if",
    "else",
    "for",
    "while",
    "loop",
    "in",
    "return",
    "break",
    "continue",
    "match",
    "struct",
    "impl",
    "const",
    "use",
    "as",
    "true",
    "false",
    "pub",
    "static",
    "thread_local",
    "self",
    "Some",
    "None",
];
const TYPES_RS: &[&str] = &[
    "i64",
    "usize",
    "u8",
    "bool",
    "char",
    "String",
    "Vec",
    "HashMap",
    "HashSet",
    "VecDeque",
    "BinaryHeap",
    "Option",
    "ListRef",
    "TreeRef",
    "RefCell",
    "str",
];

/// Colour every line of `text`: one vector of `(text, class)` spans per line (spans cover each line fully).
pub fn highlight(lang: Lang, text: &str) -> Vec<Vec<(String, Class)>> {
    let (kw, ty) = if lang == Lang::Rust {
        (KEYWORDS_RS, TYPES_RS)
    } else {
        (KEYWORDS_C, TYPES_C)
    };
    let mut in_block = false;
    let mut out = Vec::new();
    for line in text.split('\n') {
        let chars: Vec<char> = line.chars().collect();
        let mut spans: Vec<(String, Class)> = Vec::new();
        let mut i = 0;
        if chars
            .iter()
            .collect::<String>()
            .trim_start()
            .starts_with('#')
            && !in_block
            && lang != Lang::Rust
        {
            out.push(vec![(line.to_string(), Class::Preproc)]);
            continue;
        }
        while i < chars.len() {
            let c = chars[i];
            let start = i;
            if in_block {
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                if i < chars.len() {
                    i += 2;
                    in_block = false;
                }
                spans.push((chars[start..i].iter().collect(), Class::Comment));
            } else if c == '/' && chars.get(i + 1) == Some(&'/') {
                spans.push((chars[i..].iter().collect(), Class::Comment));
                break;
            } else if c == '/' && chars.get(i + 1) == Some(&'*') {
                in_block = true;
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                if i < chars.len() {
                    i += 2;
                    in_block = false;
                }
                spans.push((chars[start..i].iter().collect(), Class::Comment));
            } else if c == '"'
                || (c == '\'' && lang != Lang::Rust)
                || (c == '\'' && chars.get(i + 2) == Some(&'\''))
                || (c == '\'' && chars.get(i + 1) == Some(&'\\'))
            {
                let q = c;
                i += 1;
                while i < chars.len() && chars[i] != q {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i = (i + 1).min(chars.len());
                spans.push((chars[start..i].iter().collect(), Class::Str));
            } else if c.is_ascii_digit() {
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                spans.push((chars[start..i].iter().collect(), Class::Number));
            } else if c.is_ascii_alphabetic() || c == '_' {
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let w: String = chars[start..i].iter().collect();
                let class = if kw.contains(&w.as_str()) {
                    Class::Keyword
                } else if ty.contains(&w.as_str())
                    || (w.chars().next().is_some_and(char::is_uppercase)
                        && w.len() > 2
                        && chars.get(i) != Some(&'('))
                {
                    Class::Type
                } else if chars.get(i) == Some(&'(')
                    || (chars.get(i) == Some(&'!') && lang == Lang::Rust)
                {
                    Class::Func
                } else {
                    Class::Plain
                };
                spans.push((w, class));
            } else if "{}()[]<>;,.:".contains(c) {
                i += 1;
                spans.push((c.to_string(), Class::Punct));
            } else {
                i += 1;
                spans.push((c.to_string(), Class::Plain));
            }
        }
        out.push(spans);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_cover_the_whole_line() {
        let text = "#include <vector>\nstd::vector<long long> f(int x) { // hi\n    return x + 1; /* c */\n}\n";
        let lines = highlight(Lang::Cpp, text);
        for (l, spans) in text.split('\n').zip(lines.iter()) {
            assert_eq!(spans.iter().map(|s| s.0.as_str()).collect::<String>(), l);
        }
        assert_eq!(lines[0][0].1, Class::Preproc);
        assert!(lines[1].iter().any(|s| s.1 == Class::Comment));
    }

    #[test]
    fn rust_chars_and_lifetimes() {
        let lines = highlight(
            Lang::Rust,
            "let c = 'a'; let s: &str = \"x\"; fn f<'a>() {}",
        );
        assert!(lines[0].iter().any(|s| s.1 == Class::Str && s.0 == "'a'"));
        assert!(lines[0].iter().any(|s| s.1 == Class::Keyword));
    }
}
