// SPDX-License-Identifier: Apache-2.0
//! Translate a checked C++ program (the player's accepted solution) into idiomatic Rust and C.
//!
//! The translation walks the typed syntax tree of [`crate::cpp::ast`] using the checker's side tables, keeps
//! the player's names and structure, and never guesses: a construct that cannot be translated faithfully
//! gives a friendly `Err` ("this clerk cannot translate ... to Rust yet"), never wrong code and never a
//! panic. See `docs/CODEGEN.md` for what is translatable. The generated code is verified by compiling it
//! together with a generated harness and comparing with the interpreter ([`harness`]).

use super::run::Compiled;

pub mod c;
mod common;
pub mod harness;
mod harness_c;
mod harness_rust;
pub mod rust;

pub use common::Ex;

/// Target language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    /// C11.
    C,
    /// C++17 (the player's own code, formatted).
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
        if self.prelude.is_empty() {
            return self.code.clone();
        }
        format!("{}\n{}", self.prelude, self.code)
    }
}

/// Translate a compiled program; `entry` is the level's function (it is kept even when unused by others).
///
/// `Err` carries a friendly message when a construct cannot be translated to `lang`.
pub fn generate(comp: &Compiled, lang: Lang, entry: &str) -> Result<Generated, String> {
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match lang {
        Lang::Cpp => Ok(cpp(comp)),
        Lang::Rust => rust::generate(comp, entry),
        Lang::C => c::generate(comp, entry),
    }));
    match run {
        Ok(r) => r,
        Err(_) => Err(format!(
            "this clerk cannot translate this program to {} yet",
            lang.name()
        )),
    }
}

/// The C++ tab: the level's prelude (includes and provided structs) and the player's own code, formatted.
fn cpp(c: &Compiled) -> Generated {
    let mut pre = c.program.clone();
    pre.items.truncate(c.program.prelude_items);
    pre.prelude_items = 0;
    Generated {
        lang: Lang::Cpp,
        prelude: super::print::print_program(&pre, false),
        code: super::print::print_program(&c.program, true),
    }
}
