// SPDX-License-Identifier: Apache-2.0
//! Translate a program into equivalent, readable C, C++ or Rust.
//!
//! The machine is modelled as a function with an integer `hand`, a floor (named variables or an
//! array) and inbox/outbox semantics. Control flow is re-structured into `while` / `if` where the
//! jump pattern allows it; otherwise a goto (C, C++) or `pc` state machine (Rust) form is used.
//! Both forms are always correct for programs that run without errors. See `docs/CODEGEN.md`.

pub mod harness;
pub mod highlight;
pub mod meta;
mod render;
pub mod structure;

use crate::asm::Program;
use crate::level::{CodeMeta, Explain, Level};

/// Target language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    /// C99.
    C,
    /// C++17.
    Cpp,
    /// Rust 2021+.
    Rust,
}

impl Lang {
    /// All languages in display order.
    pub const ALL: [Lang; 3] = [Lang::C, Lang::Cpp, Lang::Rust];

    /// Parse `c`, `cpp` (or `c++`), `rust` (or `rs`).
    pub fn from_key(s: &str) -> Option<Lang> {
        match s.to_ascii_lowercase().as_str() {
            "c" => Some(Lang::C),
            "cpp" | "c++" | "cxx" => Some(Lang::Cpp),
            "rust" | "rs" => Some(Lang::Rust),
            _ => None,
        }
    }

    /// Stable key.
    pub fn key(self) -> &'static str {
        match self {
            Lang::C => "c",
            Lang::Cpp => "cpp",
            Lang::Rust => "rust",
        }
    }

    /// Display name.
    pub fn title(self) -> &'static str {
        match self {
            Lang::C => "C",
            Lang::Cpp => "C++",
            Lang::Rust => "Rust",
        }
    }

    /// File extension.
    pub fn extension(self) -> &'static str {
        match self {
            Lang::C => "c",
            Lang::Cpp => "cpp",
            Lang::Rust => "rs",
        }
    }
}

/// Score numbers printed in the header comment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    /// Instruction count.
    pub size: usize,
    /// Average steps.
    pub avg_steps: f64,
    /// Worst-case steps.
    pub worst_steps: u64,
}

/// Code generation options.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Override the function name (generic mode default: `clerk_run`).
    pub function_name: Option<String>,
    /// Floor size when no level is given (default: highest slot used + 1).
    pub floor_len: Option<usize>,
    /// Ignore the level's `[code]` metadata and emit the generic inbox/outbox form.
    pub force_generic: bool,
    /// Never use structured control flow (used by tests to exercise the fallback).
    pub force_fallback: bool,
    /// Statistics for the header comment.
    pub stats: Option<Stats>,
}

/// Calling convention of the generated function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    /// `inbox` in, `outbox` out; optionally the initial floor as a second argument.
    Generic {
        /// Whether the function takes `floor_start`.
        floor_param: bool,
    },
    /// Interview-style signature from the level metadata.
    Meta(CodeMeta),
}

/// Result of code generation.
#[derive(Clone, Debug)]
pub struct Generated {
    /// Language.
    pub lang: Lang,
    /// Source text (a complete function plus includes).
    pub code: String,
    /// Name of the generated function.
    pub function: String,
    /// True if loops and branches were recovered; false for the goto / state machine form.
    pub structured: bool,
    /// Calling convention.
    pub shape: Shape,
    /// Number of floor slots.
    pub floor_len: usize,
    /// Algorithm note from the level, if any.
    pub explain: Option<Explain>,
}

/// Translate `program` into `lang`. `level` supplies slot names, prefilled tiles, and the optional
/// `[code]` metadata. Works for any program that compiled.
pub fn generate(program: &Program, lang: Lang, level: Option<&Level>, opts: &Options) -> Generated {
    render::generate(program, lang, level, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;

    #[test]
    fn lang_keys() {
        for l in Lang::ALL {
            assert_eq!(Lang::from_key(l.key()), Some(l));
        }
        assert_eq!(Lang::from_key("c++"), Some(Lang::Cpp));
        assert_eq!(Lang::from_key("go"), None);
    }

    #[test]
    fn echo_in_three_languages() {
        let p = assemble("a:\nINBOX\nOUTBOX\nJUMP a").unwrap();
        let c = generate(&p, Lang::C, None, &Options::default());
        assert!(c.structured);
        assert!(c.code.contains("for (;;)"), "{}", c.code);
        assert!(c.code.contains("outbox[out_len++] = hand;"));
        let cpp = generate(&p, Lang::Cpp, None, &Options::default());
        assert!(cpp.code.contains("std::vector<int>"));
        let rs = generate(&p, Lang::Rust, None, &Options::default());
        assert!(rs.code.contains("loop {"), "{}", rs.code);
        assert!(rs.code.contains("outbox.push(hand);"));
    }

    #[test]
    fn fallback_forms_have_gotos_or_pc() {
        let p = assemble("a:\nINBOX\nOUTBOX\nJUMP a").unwrap();
        let o = Options {
            force_fallback: true,
            ..Options::default()
        };
        assert!(generate(&p, Lang::C, None, &o).code.contains("goto"));
        assert!(generate(&p, Lang::Rust, None, &o).code.contains("match pc"));
    }
}
