// SPDX-License-Identifier: Apache-2.0
//! Level data model (loaded from TOML). See `docs/LEVEL_FORMAT.md`.

use crate::lang::ast::Ty;
use crate::lang::check::{Allowed, CheckOpts, EntrySig};
use crate::lang::parse::parse;
use serde::Deserialize;
use std::collections::BTreeSet;

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

/// A built-in the level locks, with the reason shown in the brief.
#[derive(Clone, Debug, Deserialize)]
pub struct LockNote {
    /// Built-in name.
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
        Budgets { steps: default_steps(), large_steps: default_large(), sweep_n0: default_n0() }
    }
}

#[derive(Deserialize)]
struct Unlock {
    #[serde(default)]
    helpers: bool,
    #[serde(default)]
    recursion: bool,
}

#[derive(Deserialize)]
struct RawLevel {
    title: String,
    #[serde(default)]
    order: u32,
    #[serde(default)]
    story: String,
    statement: String,
    signature: String,
    #[serde(default)]
    constraints: Vec<String>,
    technique: String,
    big_o: String,
    #[serde(default)]
    types: Vec<String>,
    #[serde(default)]
    builtins: Vec<String>,
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
        Ok(PackInfo { id: id.to_string(), title: r.title, blurb: r.blurb, order: r.order })
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
    /// The fixed function header.
    pub signature: EntrySig,
    /// Constraint bullets.
    pub constraints: Vec<String>,
    /// Name of the standard technique ("Hash map lookup").
    pub technique: String,
    /// Big-O note.
    pub big_o: String,
    /// Unlocked type families (besides those in the signature).
    pub types: Vec<String>,
    /// Unlocked built-ins.
    pub builtins: Vec<String>,
    /// Locked built-ins with reasons.
    pub locked: Vec<LockNote>,
    /// Helper functions allowed.
    pub helpers: bool,
    /// Recursion allowed.
    pub recursion: bool,
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

impl Level {
    /// Parse a level file. `pack` is the directory name.
    pub fn from_toml(text: &str, pack: &str) -> Result<Level, String> {
        let r: RawLevel = toml::from_str(text).map_err(|e| e.to_string())?;
        let header = format!("{} {{}}", r.signature.trim());
        let prog = parse(&header).map_err(|d| format!("signature: {}", d.msg))?;
        let f = prog.fns.first().ok_or("signature: no function")?;
        let signature = EntrySig { name: f.name.clone(), params: f.params.clone(), ret: f.ret.clone() };
        if r.title.trim().is_empty() {
            return Err("empty title".into());
        }
        let u = r.unlock.unwrap_or(Unlock { helpers: false, recursion: false });
        Ok(Level {
            id: String::new(),
            pack: pack.to_string(),
            title: r.title,
            order: r.order,
            story: r.story,
            statement: r.statement,
            signature,
            constraints: r.constraints,
            technique: r.technique,
            big_o: r.big_o,
            types: r.types,
            builtins: r.builtins,
            locked: r.locked,
            helpers: u.helpers,
            recursion: u.recursion,
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

    /// The set of type families the player may use (signature types are always included).
    pub fn allowed_types(&self) -> BTreeSet<String> {
        let mut set: BTreeSet<String> = self.types.iter().cloned().collect();
        let mut fams = Vec::new();
        for p in &self.signature.params {
            p.ty.families(&mut fams);
        }
        self.signature.ret.families(&mut fams);
        for f in fams {
            set.insert(f.to_string());
        }
        for n in ["int", "bool", "unit"] {
            set.remove(n);
        }
        set
    }

    /// Options for the type checker.
    pub fn check_opts(&self) -> CheckOpts {
        let everything = self.builtins.iter().any(|b| b == "*");
        CheckOpts {
            allowed: Allowed {
                types: if everything { None } else { Some(self.allowed_types()) },
                builtins: if everything { None } else { Some(self.builtins.iter().cloned().collect()) },
                helpers: self.helpers,
                recursion: self.recursion,
            },
            entry: Some(self.signature.clone()),
        }
    }

    /// The skeleton shown in a fresh editor.
    pub fn starter(&self) -> String {
        let ret = &self.signature.ret;
        let tail = match ret {
            Ty::Unit => String::new(),
            Ty::Int => "    return 0;\n".into(),
            Ty::Bool => "    return false;\n".into(),
            Ty::Str => "    return \"\";\n".into(),
            Ty::Char => "    return 'a';\n".into(),
            Ty::Vec(_) => "    return [];\n".into(),
            _ => "    // your code here\n".into(),
        };
        format!("{} {{\n{tail}}}\n", self.signature.text())
    }
}
