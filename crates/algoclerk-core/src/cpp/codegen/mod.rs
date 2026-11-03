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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpp::check::CheckOpts;
    use crate::cpp::run::compile;

    fn gen_lang(src: &str, lang: Lang, entry: &str) -> Result<Generated, String> {
        let c = compile("", src, &CheckOpts::all()).map_err(|d| d[0].msg.clone())?;
        generate(&c, lang, entry)
    }

    #[test]
    fn cpp_tab_is_the_formatted_player_code() {
        let g = gen_lang(
            "#include <vector>\nint f(int x){return x+1;}\n",
            Lang::Cpp,
            "f",
        )
        .unwrap();
        assert!(g.full().contains("#include <vector>"));
        assert!(g.code.contains("int f(int x) {"));
    }

    #[test]
    fn simple_function_translates_to_both_languages() {
        let src = "int twice(int x) { return x * 2; }\n";
        let r = gen_lang(src, Lang::Rust, "twice").unwrap();
        assert!(r.code.contains("fn twice(x: i32) -> i32 {"), "{}", r.code);
        let c = gen_lang(src, Lang::C, "twice").unwrap();
        assert!(c.code.contains("int twice(int x) {"), "{}", c.code);
    }

    fn rust(src: &str, entry: &str) -> String {
        gen_lang(src, Lang::Rust, entry).unwrap().code
    }

    const TWO_SUM: &str = "#include <unordered_map>\n#include <vector>\n\
        std::vector<int> f(const std::vector<int>& a, int t) {\n\
            std::unordered_map<int, int> seen;\n\
            for (int i = 0; i < (int)a.size(); i++) {\n\
                if (auto it = seen.find(t - a[i]); it != seen.end()) { return {it->second, i}; }\n\
                seen[a[i]] = i;\n\
            }\n\
            return {};\n\
        }\n";

    #[test]
    fn an_init_statement_without_a_name_clash_leaves_no_lone_block() {
        let r = rust(TWO_SUM, "f");
        assert!(!r.lines().any(|l| l.trim() == "{"), "{r}");
        assert!(r.contains("let it = seen.get_key_value("), "{r}");
        // a clash with a visible name keeps the scope
        let src = "#include <unordered_map>\nint f(int it) {\n\
            std::unordered_map<int, int> m;\n\
            if (auto it = m.find(1); it != m.end()) { return it->second; }\n\
            return it;\n}\n";
        let r = rust(src, "f");
        assert!(r.lines().any(|l| l.trim() == "{"), "{r}");
        // a `for` that cannot become a range loop hoists its counter too
        let src = "int f(int n) {\n int s = 0;\n for (int i = 0; i < n; i += n / 3 + 1) { s += i; }\n \
            for (int i = 0; i < n; i += n / 3 + 1) { s += i; }\n return s;\n}\n";
        let r = rust(src, "f");
        assert!(!r.lines().any(|l| l.trim() == "{"), "{r}");
    }

    #[test]
    fn casts_comparisons_and_heap_tops_have_no_redundant_syntax() {
        let src = "#include <string>\n#include <vector>\n#include <queue>\n#include <functional>\n\
            bool f(const std::string& s, std::vector<int>& v, std::priority_queue<int, std::vector<int>, std::greater<int>>& q, char c) {\n\
                int a = q.top();\n\
                char d = (char)('a' + (c - 'a' + 1) % 26);\n\
                return s.find(c) != std::string::npos && v[a] < 3 && s == \"x\" && v.size() > 1 && d == 'b';\n\
            }\n";
        let r = rust(src, "f");
        assert!(r.contains("let a: i32 = q.peek().unwrap().0;"), "{r}");
        assert!(r.contains("s.find(c as char).is_some()"), "{r}");
        assert!(r.contains("v[a as usize] < 3"), "{r}");
        assert!(r.contains("s == \"x\""), "{r}");
        assert!(r.contains("v.len() > 1 "), "{r}");
        assert!(r.contains("% 26) as u8;"), "{r}");
        assert!(!r.contains("as usize as usize"), "{r}");
        assert!(!r.contains("*&"), "{r}");
        assert!(!r.contains("clone()"), "{r}");
    }

    #[test]
    fn untranslatable_programs_give_friendly_errors() {
        let src = "#include <functional>\nint f(int x) { std::function<int(int)> g = [](int a) { return a; }; return g(x); }\n";
        for lang in [Lang::Rust, Lang::C] {
            let e = gen_lang(src, lang, "f").unwrap_err();
            assert!(e.starts_with("this clerk cannot translate"), "{e}");
            assert!(e.ends_with("yet"), "{e}");
        }
    }

    #[test]
    fn missing_entry_is_an_error_not_a_panic() {
        assert!(gen_lang("int f() { return 1; }\n", Lang::Rust, "nope").is_err());
        assert!(gen_lang("int f() { return 1; }\n", Lang::C, "nope").is_err());
    }

    #[test]
    fn generation_never_panics_on_the_corpus() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cpp_corpus");
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap();
            let Ok(c) = compile("", &text, &CheckOpts::all()) else {
                continue;
            };
            let entry = c
                .program
                .fns()
                .last()
                .map(|f| f.name.clone())
                .unwrap_or_default();
            for lang in Lang::ALL {
                let r = generate(&c, lang, &entry);
                if let Err(m) = r {
                    assert!(!m.is_empty());
                }
            }
        }
    }
}
