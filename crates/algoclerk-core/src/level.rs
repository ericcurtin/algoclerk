// SPDX-License-Identifier: Apache-2.0
//! Level data model (loaded from TOML). See `docs/LEVEL_FORMAT.md`.

use crate::cpp::ast::Ty;
use crate::cpp::check::{CheckOpts, EntrySig};
use crate::cpp::sig::{parse_signature, prelude_aliases};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// How the judge compares outputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compare {
    /// Exactly equal.
    #[default]
    Exact,
    /// The outer shelf may be in any order.
    SortOuter,
    /// Every nested shelf and the outer shelf may be in any order.
    SortAll,
}

/// A library name the level locks (`<algorithm>`, `std::sort`, `std::string::substr`), with the reason shown
/// in the brief.
#[derive(Clone, Debug, Deserialize)]
pub struct LockNote {
    /// Header (`<algorithm>`), `std::` name or member (`std::string::substr`).
    pub name: String,
    /// Why it is locked.
    #[serde(default)]
    pub why: String,
}

/// A hand-written test.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RawCase {
    /// Argument literals, one per parameter.
    pub args: Vec<String>,
    /// Expected return value literal.
    #[serde(default)]
    pub expect: String,
    /// Short explanation for the examples.
    #[serde(default)]
    pub note: String,
}

/// A seeded generator block.
#[derive(Clone, Debug, Deserialize)]
pub struct GenSpec {
    /// Seed.
    #[serde(default = "one")]
    pub seed: u64,
    /// Number of cases.
    #[serde(default = "one_usize")]
    pub count: usize,
    /// These cases run with the large budget (performance tests).
    #[serde(default)]
    pub large: bool,
    /// Value of `n` available to size expressions.
    #[serde(default)]
    pub n: i64,
    /// One table per parameter.
    pub args: Vec<toml::Table>,
}

fn one() -> u64 {
    1
}
fn one_usize() -> usize {
    1
}

/// Medal targets (lower is better).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MedalTargets {
    /// Program size for bronze, silver, gold.
    #[serde(default)]
    pub size: [u32; 3],
    /// Average cost for bronze, silver, gold.
    #[serde(default)]
    pub speed: [u64; 3],
}

/// Step budgets.
#[derive(Clone, Debug, Deserialize)]
pub struct Budgets {
    /// Per standard test.
    #[serde(default = "default_steps")]
    pub steps: u64,
    /// Per large test.
    #[serde(default = "default_large")]
    pub large_steps: u64,
    /// `n` for the first size of the complexity sweep.
    #[serde(default = "default_n0")]
    pub sweep_n0: i64,
}

fn default_steps() -> u64 {
    200_000
}
fn default_large() -> u64 {
    3_000_000
}
fn default_n0() -> i64 {
    64
}

impl Default for Budgets {
    fn default() -> Self {
        Budgets {
            steps: default_steps(),
            large_steps: default_large(),
            sweep_n0: default_n0(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Unlock {
    #[serde(default)]
    helpers: bool,
    #[serde(default)]
    recursion: bool,
    #[serde(default)]
    structs: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLevel {
    title: String,
    #[serde(default)]
    order: u32,
    #[serde(default)]
    story: String,
    statement: String,
    #[serde(default)]
    prelude: String,
    signature: String,
    #[serde(default)]
    constraints: Vec<String>,
    technique: String,
    big_o: String,
    #[serde(default)]
    unlocked: Vec<String>,
    #[serde(default)]
    locked: Vec<LockNote>,
    #[serde(default)]
    unlock: Option<Unlock>,
    #[serde(default)]
    hints: Vec<String>,
    reference: String,
    #[serde(default)]
    naive: Option<String>,
    #[serde(default)]
    compare: Compare,
    #[serde(default)]
    budgets: Budgets,
    #[serde(default)]
    medals: MedalTargets,
    #[serde(default)]
    examples: Vec<RawCase>,
    #[serde(default)]
    tests: Vec<RawCase>,
    #[serde(default)]
    generate: Vec<GenSpec>,
    #[serde(default)]
    sweep: Option<GenSpec>,
}

#[derive(Deserialize)]
struct RawPack {
    title: String,
    #[serde(default)]
    blurb: String,
    #[serde(default)]
    order: u32,
}

/// A wing of the office building.
#[derive(Clone, Debug)]
pub struct PackInfo {
    /// Directory name.
    pub id: String,
    /// Display title.
    pub title: String,
    /// One-line description.
    pub blurb: String,
    /// Sort key.
    pub order: u32,
}

impl PackInfo {
    /// Parse `pack.toml`.
    pub fn from_toml(text: &str, id: &str) -> Result<PackInfo, String> {
        let r: RawPack = toml::from_str(text).map_err(|e| e.to_string())?;
        Ok(PackInfo {
            id: id.to_string(),
            title: r.title,
            blurb: r.blurb,
            order: r.order,
        })
    }
}

/// One interview problem.
#[derive(Clone, Debug)]
pub struct Level {
    /// File stem.
    pub id: String,
    /// Pack directory.
    pub pack: String,
    /// Title.
    pub title: String,
    /// Sort key inside the pack.
    pub order: u32,
    /// Office-flavour story.
    pub story: String,
    /// Plain problem statement.
    pub statement: String,
    /// Read-only code above the player's: `#include` lines, helper structs such as `ListNode`.
    pub prelude: String,
    /// The fixed function header, parsed.
    pub signature: EntrySig,
    /// The signature as written in the level file (what the brief and the editor show).
    pub signature_text: String,
    /// Constraint bullets.
    pub constraints: Vec<String>,
    /// Name of the standard technique ("Hash map lookup").
    pub technique: String,
    /// Big-O note.
    pub big_o: String,
    /// Headers the player may `#include` besides those of the prelude (`<algorithm>`); `*` unlocks all.
    pub unlocked: Vec<String>,
    /// Locked names with reasons.
    pub locked: Vec<LockNote>,
    /// Helper functions allowed.
    pub helpers: bool,
    /// Recursion allowed.
    pub recursion: bool,
    /// Player-defined structs allowed.
    pub structs: bool,
    /// Hint ladder.
    pub hints: Vec<String>,
    /// Reference solution text.
    pub reference: String,
    /// Deliberately slow solution used to prove the large tests bite.
    pub naive: Option<String>,
    /// Output comparison.
    pub compare: Compare,
    /// Budgets.
    pub budgets: Budgets,
    /// Medal targets.
    pub medals: MedalTargets,
    /// Visible examples.
    pub examples: Vec<RawCase>,
    /// Hidden hand-written tests.
    pub tests: Vec<RawCase>,
    /// Seeded generators.
    pub generate: Vec<GenSpec>,
    /// Optional generator for the complexity sweep (defaults to the first large generator).
    pub sweep: Option<GenSpec>,
}

/// Does `text` contain `struct name` as a whole word?
fn defines_struct(text: &str, name: &str) -> bool {
    let key = format!("struct {name}");
    text.match_indices(&key).any(|(i, _)| {
        text[i + key.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

fn valid_header(h: &str) -> bool {
    h == "*"
        || (h.len() > 2
            && ((h.starts_with('<') && h.ends_with('>'))
                || (h.starts_with('"') && h.ends_with('"'))))
}

impl Level {
    /// Parse a level file. `pack` is the directory name.
    pub fn from_toml(text: &str, pack: &str) -> Result<Level, String> {
        let r: RawLevel = toml::from_str(text).map_err(|e| e.to_string())?;
        if r.title.trim().is_empty() {
            return Err("empty title".into());
        }
        let aliases = prelude_aliases(&r.prelude);
        let signature =
            parse_signature(&r.signature, &aliases).map_err(|e| format!("signature: {e}"))?;
        let mut missing = Vec::new();
        let mut note = |t: &Ty| {
            t.walk(&mut |x| {
                if let Ty::Struct(n) = x
                    && !defines_struct(&r.prelude, n)
                    && !missing.contains(n)
                {
                    missing.push(n.clone());
                }
            });
        };
        for p in &signature.params {
            note(&p.spec.ty);
        }
        note(&signature.ret.ty);
        if let Some(n) = missing.first() {
            return Err(format!(
                "signature: `{n}` is not defined by the prelude (add `struct {n} {{ ... }};`)"
            ));
        }
        if let Some(h) = r.unlocked.iter().find(|h| !valid_header(h)) {
            return Err(format!(
                "unlocked: `{h}` is not a header such as \"<vector>\" (or \"*\")"
            ));
        }
        let u = r.unlock.unwrap_or_default();
        Ok(Level {
            id: String::new(),
            pack: pack.to_string(),
            title: r.title,
            order: r.order,
            story: r.story,
            statement: r.statement,
            prelude: r.prelude,
            signature,
            signature_text: r.signature.trim().trim_end_matches(';').trim().to_string(),
            constraints: r.constraints,
            technique: r.technique,
            big_o: r.big_o,
            unlocked: r.unlocked,
            locked: r.locked,
            helpers: u.helpers,
            recursion: u.recursion,
            structs: u.structs,
            hints: r.hints,
            reference: r.reference,
            naive: r.naive,
            compare: r.compare,
            budgets: r.budgets,
            medals: r.medals,
            examples: r.examples,
            tests: r.tests,
            generate: r.generate,
            sweep: r.sweep,
        })
    }

    /// Options for the checker: the entry signature and what the level unlocks.
    pub fn check_opts(&self) -> CheckOpts {
        let everything = self.unlocked.iter().any(|h| h == "*");
        let locked: BTreeMap<String, String> = self
            .locked
            .iter()
            .map(|l| (l.name.clone(), l.why.clone()))
            .collect();
        CheckOpts {
            entry: Some(self.signature.clone()),
            unlocked_headers: if everything {
                None
            } else {
                Some(self.unlocked.iter().cloned().collect::<BTreeSet<_>>())
            },
            locked,
            helpers: self.helpers,
            recursion: self.recursion,
            structs: self.structs,
        }
    }

    /// The initial editor text: the signature with an empty body.
    pub fn starter_code(&self) -> String {
        format!("{} {{\n}}\n", self.signature_text)
    }

    /// The complete translation unit text: the prelude, then the given player code.
    pub fn full_source(&self, player: &str) -> String {
        let mut s = self.prelude.trim_end().to_string();
        if !s.is_empty() {
            s.push_str("\n\n");
        }
        s.push_str(player.trim_start_matches('\n'));
        s
    }

    /// Does the prelude define `struct name`?
    pub fn prelude_defines(&self, name: &str) -> bool {
        defines_struct(&self.prelude, name)
    }

    /// Is every parameter and the return type a scalar (no containers, no pointers)?
    pub fn scalar_only(&self) -> bool {
        let scalar = |t: &Ty| {
            matches!(
                t,
                Ty::Bool | Ty::Char | Ty::Int | Ty::UInt | Ty::LongLong | Ty::ULongLong | Ty::Size
            )
        };
        self.signature.params.iter().all(|p| scalar(&p.spec.ty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
title = "T"
statement = "s"
signature = "SIG"
technique = "t"
big_o = "O(1) time, O(1) space"
reference = "REF"
"#;

    fn with_sig(sig: &str) -> String {
        BASE.replace("SIG", sig)
    }

    #[test]
    fn parses_a_full_level() {
        let text = r#"
title = "Pair"
statement = "s"
prelude = '''
#include <vector>
'''
signature = "std::vector<int> twoSum(const std::vector<int>& nums, int target)"
technique = "t"
big_o = "O(n) time, O(n) space"
unlocked = ["<unordered_map>"]
locked = [{ name = "std::sort", why = "w" }, { name = "<algorithm>", why = "w2" }]
unlock = { helpers = true, structs = true }
reference = "x"
"#;
        let l = Level::from_toml(text, "p").unwrap();
        assert_eq!(l.signature.name, "twoSum");
        assert_eq!(l.signature.params.len(), 2);
        assert!(l.helpers && !l.recursion && l.structs);
        let o = l.check_opts();
        assert_eq!(o.entry.as_ref().unwrap().name, "twoSum");
        assert!(
            o.unlocked_headers
                .as_ref()
                .unwrap()
                .contains("<unordered_map>")
        );
        assert_eq!(o.locked.get("std::sort").map(String::as_str), Some("w"));
        assert!(o.helpers && !o.recursion && o.structs);
        assert_eq!(
            l.starter_code(),
            "std::vector<int> twoSum(const std::vector<int>& nums, int target) {\n}\n"
        );
        assert_eq!(l.full_source("int x;\n"), "#include <vector>\n\nint x;\n");
        assert!(!l.scalar_only());
    }

    #[test]
    fn star_unlocks_every_header() {
        let text = format!("unlocked = [\"*\"]\n{}", with_sig("int f(int a)"));
        let l = Level::from_toml(&text, "p").unwrap();
        assert!(l.check_opts().unlocked_headers.is_none());
        assert!(l.scalar_only());
    }

    #[test]
    fn rejects_old_format_and_bad_signatures() {
        assert!(Level::from_toml(&with_sig("fn f(a: int) -> int"), "p").is_err());
        assert!(Level::from_toml(&with_sig("int f(int)"), "p").is_err());
        assert!(Level::from_toml(&with_sig("double f(int a)"), "p").is_err());
        let old = format!("builtins = []\n{}", with_sig("int f(int a)"));
        let e = Level::from_toml(&old, "p").unwrap_err();
        assert!(e.contains("builtins"), "{e}");
        let e = Level::from_toml(&with_sig("ListNode* f(ListNode* h)"), "p").unwrap_err();
        assert!(e.contains("ListNode"), "{e}");
        let e = Level::from_toml(
            &format!("unlocked = [\"vector\"]\n{}", with_sig("int f(int a)")),
            "p",
        )
        .unwrap_err();
        assert!(e.contains("header"), "{e}");
    }

    #[test]
    fn struct_signatures_need_the_prelude_definition() {
        let text = format!(
            "prelude = \"struct ListNode {{ int val; ListNode* next; ListNode(int v) : val(v), next(nullptr) {{}} }};\"\n{}",
            with_sig("ListNode* f(ListNode* h)")
        );
        let l = Level::from_toml(&text, "p").unwrap();
        assert!(l.prelude_defines("ListNode"));
        assert!(!l.prelude_defines("TreeNode"));
        assert!(!l.prelude_defines("List"));
    }

    #[test]
    fn prelude_aliases_can_be_used_in_the_signature() {
        let text = format!(
            "prelude = \"#include <vector>\\nusing Grid = std::vector<std::vector<char>>;\"\n{}",
            with_sig("int f(const Grid& g)")
        );
        let l = Level::from_toml(&text, "p").unwrap();
        assert_eq!(
            l.signature.params[0].spec.ty,
            Ty::vector(Ty::vector(Ty::Char))
        );
    }

    /// The worked example of `docs/LEVEL_FORMAT.md` must load.
    #[test]
    fn the_documented_example_loads() {
        // Windows checkouts may use CRLF line endings (git autocrlf); the example is searched for with \n.
        let doc = include_str!("../../../docs/LEVEL_FORMAT.md").replace("\r\n", "\n");
        let start = doc.find("## A complete example").unwrap();
        let rest = &doc[start..];
        let a = rest.find("```toml\n").unwrap() + "```toml\n".len();
        let b = a + rest[a..].find("\n```").unwrap();
        let l = Level::from_toml(&rest[a..b], "doc").unwrap();
        assert_eq!(l.signature.name, "twoSum");
        assert!(l.naive.is_some() && l.examples.len() == 2 && l.generate.len() == 2);
        assert_eq!(l.locked.len(), 1);
    }
}
