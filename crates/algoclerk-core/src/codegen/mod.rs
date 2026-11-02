// SPDX-License-Identifier: Apache-2.0
//! Translate a checked ClerkScript program into idiomatic C, C++ or Rust.
//!
//! The translation walks the syntax tree directly (no `goto`), keeps the player's names and adds the
//! level's signature. See `docs/CODEGEN.md`.

use crate::lang::ast::*;
use crate::lang::check::TypeInfo;
use crate::lang::run::Compiled;

pub mod c;
pub mod c_templates;
pub mod compile;
pub mod cpp;
pub mod harness;
pub mod highlight;
pub mod rust;

/// Target language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    /// C11.
    C,
    /// C++17.
    Cpp,
    /// Rust 2021.
    Rust,
}

impl Lang {
    /// Parse `c`, `cpp` or `rust`.
    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "c" => Some(Lang::C),
            "cpp" | "c++" => Some(Lang::Cpp),
            "rust" | "rs" => Some(Lang::Rust),
            _ => None,
        }
    }
    /// Short id.
    pub fn id(self) -> &'static str {
        match self {
            Lang::C => "c",
            Lang::Cpp => "cpp",
            Lang::Rust => "rust",
        }
    }
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Lang::C => "C",
            Lang::Cpp => "C++",
            Lang::Rust => "Rust",
        }
    }
    /// All languages.
    pub const ALL: [Lang; 3] = [Lang::C, Lang::Cpp, Lang::Rust];
}

/// Generated source, split into the support prelude and the player's code.
#[derive(Clone, Debug)]
pub struct Generated {
    /// Language.
    pub lang: Lang,
    /// Includes and helper definitions (collapsed by default in the UI).
    pub prelude: String,
    /// The translated functions.
    pub code: String,
}

impl Generated {
    /// Prelude and code in one string.
    pub fn full(&self) -> String {
        format!("{}\n{}", self.prelude, self.code)
    }
}

/// Translate a compiled program.
pub fn generate(c: &Compiled, lang: Lang, entry: &str) -> Generated {
    match lang {
        Lang::Cpp => cpp::generate(&c.program, &c.info),
        Lang::Rust => rust::generate(&c.program, &c.info, entry),
        Lang::C => c::generate(&c.program, &c.info),
    }
}

// ----- helpers shared by the emitters -----

/// A precedence-tagged piece of generated expression text.
#[derive(Clone, Debug)]
pub struct Ex {
    /// Text.
    pub s: String,
    /// Binding strength of the outermost operator (20 = atom).
    pub p: u8,
}

impl Ex {
    /// An atom.
    pub fn atom(s: impl Into<String>) -> Ex {
        Ex { s: s.into(), p: 20 }
    }
    /// With a precedence.
    pub fn new(s: impl Into<String>, p: u8) -> Ex {
        Ex { s: s.into(), p }
    }
    /// Text, parenthesised when binding weaker than `min`.
    pub fn at(&self, min: u8) -> String {
        if self.p < min {
            format!("({})", self.s)
        } else {
            self.s.clone()
        }
    }
}

/// Words that cannot be identifiers in the target languages.
const RESERVED: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "bool",
    "class",
    "namespace",
    "new",
    "delete",
    "this",
    "template",
    "typename",
    "operator",
    "private",
    "public",
    "protected",
    "virtual",
    "friend",
    "try",
    "catch",
    "throw",
    "using",
    "main",
    "string",
    "vector",
    "map",
    "set",
    "list",
    "stack",
    "queue",
    "deque",
    "tuple",
    "pair",
    "optional",
    "std",
    "ac",
    "NULL",
    "true",
    "false",
    "nullptr",
    "and",
    "or",
    "not",
    "xor",
    "as",
    "async",
    "await",
    "dyn",
    "fn",
    "impl",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "self",
    "Self",
    "super",
    "trait",
    "type",
    "unsafe",
    "use",
    "where",
    "yield",
    "abstract",
    "become",
    "box",
    "final",
    "macro",
    "override",
    "priv",
    "try",
    "typeof",
    "unsized",
    "str",
    "Vec",
    "String",
    "Option",
    "Some",
    "None",
    "i64",
    "usize",
    "len",
    "min",
    "max",
    "abs",
    "swap",
    "free",
    "malloc",
    "exit",
    "printf",
    "puts",
    "assert",
    "errno",
    "stdin",
    "stdout",
    "stderr",
    "index",
    "remove",
    "insert",
    "read",
    "write",
    "time",
    "div",
    "rand",
    "y0",
    "y1",
    "j0",
    "j1",
];

/// A safe identifier for the target languages (the player's name when possible).
pub fn ident(name: &str) -> String {
    if RESERVED.contains(&name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

/// Every distinct type used by a program (expressions, locals, signatures), with all their sub-types, inner first.
pub fn used_types(prog: &Program, info: &TypeInfo) -> Vec<Ty> {
    fn add(t: &Ty, out: &mut Vec<Ty>) {
        match t {
            Ty::Vec(a)
            | Ty::Set(a)
            | Ty::Option(a)
            | Ty::Stack(a)
            | Ty::Queue(a)
            | Ty::Deque(a)
            | Ty::Heap(a) => add(a, out),
            Ty::Map(k, v) => {
                add(k, out);
                add(v, out);
            }
            Ty::Tuple(ts) => ts.iter().for_each(|x| add(x, out)),
            _ => {}
        }
        if !out.contains(t) && !matches!(t, Ty::Var(_) | Ty::Null) {
            out.push(t.clone());
        }
    }
    let mut out = Vec::new();
    for t in &info.types {
        add(t, &mut out);
    }
    for f in &info.fns {
        for (_, t) in &f.locals {
            add(t, &mut out);
        }
    }
    for f in &prog.fns {
        for p in &f.params {
            add(&p.ty, &mut out);
        }
        add(&f.ret, &mut out);
    }
    out
}

/// Does this expression denote an existing value (so reading it in an owning position copies it)?
pub fn is_place(e: &Expr) -> bool {
    matches!(
        e.kind,
        ExprKind::Var(_) | ExprKind::Index(..) | ExprKind::TupleIdx(..)
    )
}

/// Is a (possibly nested) tuple, option... made only of cheap-to-copy things?
pub fn is_copy(t: &Ty) -> bool {
    match t {
        Ty::Int | Ty::Bool | Ty::Char | Ty::LNode | Ty::TNode | Ty::Unit => true,
        Ty::Tuple(ts) => ts.iter().all(is_copy),
        Ty::Option(t) => is_copy(t),
        _ => false,
    }
}

/// Does the type need a custom hasher in C++ (anything std::hash does not cover)?
pub fn needs_custom_hash(t: &Ty) -> bool {
    !matches!(t, Ty::Int | Ty::Char | Ty::Bool | Ty::Str)
}

/// Escape text for a C-family or Rust string literal body.
pub fn escape_str(s: &[u8]) -> String {
    let mut out = String::new();
    for &c in s {
        match c {
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0 => out.push_str("\\0"),
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            c if (0x20..0x7f).contains(&c) => out.push(c as char),
            c => out.push_str(&format!("\\x{c:02x}")),
        }
    }
    out
}

/// Escape one char for a C-family or Rust char literal body.
pub fn escape_char(c: u8) -> String {
    match c {
        b'\'' => "\\'".into(),
        b'"' => "\"".into(),
        c => escape_str(&[c]),
    }
}

/// Indentation helper.
pub fn pad(n: usize) -> String {
    "    ".repeat(n)
}

/// Is this iterable one of the special `enumerate(..)` or `zip(..)` forms?
pub fn iter_special(e: &Expr) -> Option<(&'static str, &Vec<Expr>)> {
    match &e.kind {
        ExprKind::Call(n, a) if n == "enumerate" => Some(("enumerate", a)),
        ExprKind::Call(n, a) if n == "zip" => Some(("zip", a)),
        _ => None,
    }
}
