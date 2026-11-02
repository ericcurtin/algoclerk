// SPDX-License-Identifier: Apache-2.0
//! The level data model: what a `levels/<pack>/<id>.toml` file contains.

use crate::asm::{self, Program};
use crate::isa::Allowed;
use crate::testgen::{self, Generator, Rng};
use crate::vm::{Status, run_program};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A sequence of values written as a list of integers or as text (characters become code points).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Seq {
    /// `[1, 2, 3]`
    Ints(Vec<i32>),
    /// `"abc"`
    Text(String),
}

impl Seq {
    /// The values.
    pub fn to_vec(&self) -> Vec<i32> {
        match self {
            Seq::Ints(v) => v.clone(),
            Seq::Text(s) => s.chars().map(|c| c as i32).collect(),
        }
    }
}

impl Default for Seq {
    fn default() -> Self {
        Seq::Ints(Vec::new())
    }
}

/// How values are drawn on the belts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Display {
    /// As numbers.
    #[default]
    Number,
    /// As characters (code points), when printable.
    Char,
}

/// Values written into floor slots starting at `at`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorFill {
    /// First slot.
    pub at: usize,
    /// Values.
    pub values: Seq,
}

/// A worked example shown in the statement (also run as a visible test).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Example {
    /// Inbox values.
    pub input: Seq,
    /// Extra floor values for this example.
    #[serde(default)]
    pub floor: Vec<FloorFill>,
    /// Expected outbox values.
    pub output: Seq,
    /// Optional explanation.
    #[serde(default)]
    pub note: String,
}

/// A hand-written test case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Inbox values.
    pub input: Seq,
    /// Extra floor values.
    #[serde(default)]
    pub floor: Vec<FloorFill>,
    /// Expected outbox values. If absent the reference solution decides.
    #[serde(default)]
    pub output: Option<Seq>,
    /// Optional description of what the case checks.
    #[serde(default)]
    pub note: String,
}

fn default_step_limit() -> u64 {
    20_000
}

/// Test configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestSpec {
    /// Seed for the generators.
    #[serde(default)]
    pub seed: u64,
    /// Maximum instructions per test run.
    #[serde(default = "default_step_limit")]
    pub step_limit: u64,
    /// Hand-written edge cases with explicit outputs.
    #[serde(default)]
    pub fixed: Vec<Case>,
    /// Generated cases (expected output comes from the reference solution).
    #[serde(default)]
    pub generators: Vec<Generator>,
}

/// The office layout and instruction set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineSpec {
    /// Number of floor slots.
    pub floor: usize,
    /// Optional names for slots (index = slot).
    #[serde(default)]
    pub names: Vec<String>,
    /// Pre-filled slots (`slot = value`).
    #[serde(default)]
    pub prefill: BTreeMap<String, i32>,
    /// Allowed instruction keys, plus `"indirect"`.
    pub allowed: Vec<String>,
}

/// Size and speed targets. `par` earns a staple, `ace` a gold seal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Targets {
    /// Instruction count for the staple medal.
    pub size_par: u32,
    /// Instruction count for the gold seal.
    pub size_ace: u32,
    /// Average steps for the staple medal.
    pub speed_par: u32,
    /// Average steps for the gold seal.
    pub speed_ace: u32,
}

/// The reference solution (used by tests; revealed only after solving).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionSpec {
    /// Assembly text.
    pub asm: String,
}

/// How a parameter reaches the clerk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    /// One value.
    Scalar,
    /// All remaining values (must be the last inbox parameter).
    List,
    /// The length first, then that many values.
    Counted,
    /// The values followed by a terminator value (default 0).
    Terminated,
    /// The length of the list parameter named by `of`, as one inbox value (not a function argument).
    Length,
}

/// A parameter of the generated function.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamSpec {
    /// Parameter name.
    pub name: String,
    /// Shape.
    pub kind: ParamKind,
    /// For `terminated`: the terminator value.
    #[serde(default)]
    pub terminator: i32,
    /// If set, the parameter is placed on the floor starting at this slot instead of the inbox.
    #[serde(default)]
    pub at: Option<usize>,
    /// For `length`: the name of the (earlier) list parameter whose length is passed.
    #[serde(default)]
    pub of: Option<String>,
    /// For floor lists: number of slots reserved for the list (default: up to the next prefilled slot).
    #[serde(default)]
    pub cap: Option<usize>,
}

/// Return value shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReturnKind {
    /// The single output value.
    Scalar,
    /// All output values.
    List,
}

/// Value type of a return.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetType {
    /// Integer.
    #[default]
    Int,
    /// `0` / `1` as a boolean.
    Bool,
}

/// Optional metadata that makes generated code read like an answer to the question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeMeta {
    /// Function name in snake_case.
    pub function: String,
    /// One-line description used as a doc comment.
    #[serde(default)]
    pub doc: String,
    /// Parameters in order.
    #[serde(default)]
    pub params: Vec<ParamSpec>,
    /// Return shape.
    pub returns: ReturnKind,
    /// Return value type.
    #[serde(default)]
    pub return_type: RetType,
}

/// "What the clerk did here": the name of the standard idea and a short explanation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Explain {
    /// Standard algorithm or technique name.
    pub algorithm: String,
    /// A few sentences.
    pub text: String,
}

/// A puzzle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// Unique id (equals the file name without extension).
    pub id: String,
    /// Display title.
    pub title: String,
    /// Position within the pack.
    #[serde(default)]
    pub order: u32,
    /// One-line description for menus.
    pub summary: String,
    /// Original flavour text.
    pub story: String,
    /// The algorithm problem statement.
    pub statement: String,
    /// Constraints list.
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Worked examples.
    #[serde(default)]
    pub examples: Vec<Example>,
    /// Office layout.
    pub machine: MachineSpec,
    /// Tests.
    pub tests: TestSpec,
    /// Medal targets.
    pub targets: Targets,
    /// Hint ladder, gentlest first.
    #[serde(default)]
    pub hints: Vec<String>,
    /// How to draw values.
    #[serde(default)]
    pub display: Display,
    /// Reference solution.
    #[serde(default)]
    pub solution: Option<SolutionSpec>,
    /// Code generation metadata.
    #[serde(default)]
    pub code: Option<CodeMeta>,
    /// Algorithm note.
    #[serde(default)]
    pub explain: Option<Explain>,
    /// Pack id (set by the loader from the directory name).
    #[serde(skip)]
    pub pack: String,
}

/// Where a test came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestOrigin {
    /// A statement example (visible to the player).
    Example,
    /// A hand-written edge case.
    Fixed,
    /// Generated from the seed.
    Generated,
}

/// A concrete test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCase {
    /// Inbox values.
    pub inbox: Vec<i32>,
    /// Initial floor.
    pub floor: Vec<Option<i32>>,
    /// Expected outbox.
    pub expected: Vec<i32>,
    /// Origin.
    pub origin: TestOrigin,
}

/// Pack (wing) metadata from `pack.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackInfo {
    /// Pack id (set from the directory name if empty).
    #[serde(default)]
    pub id: String,
    /// Display title.
    pub title: String,
    /// Short description.
    pub blurb: String,
    /// Sort order of the pack.
    pub order: u32,
}

/// Error type for level loading and test building.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelError(pub String);

impl std::fmt::Display for LevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LevelError {}

fn err<T>(msg: impl Into<String>) -> Result<T, LevelError> {
    Err(LevelError(msg.into()))
}

impl Level {
    /// Parse a level file.
    pub fn from_toml(text: &str, pack: &str) -> Result<Level, LevelError> {
        let mut level: Level = toml::from_str(text).map_err(|e| LevelError(e.to_string()))?;
        level.pack = pack.to_string();
        Ok(level)
    }

    /// The instruction set offered by the level.
    pub fn allowed(&self) -> Result<Allowed, LevelError> {
        Allowed::from_keys(&self.machine.allowed).map_err(|k| {
            LevelError(format!(
                "unknown instruction key \"{k}\" in machine.allowed"
            ))
        })
    }

    /// Prefilled floor (no per-test fills).
    pub fn floor_template(&self) -> Result<Vec<Option<i32>>, LevelError> {
        let mut floor = vec![None; self.machine.floor];
        for (k, v) in &self.machine.prefill {
            let slot: usize = k.parse().map_err(|_| {
                LevelError(format!("machine.prefill key \"{k}\" is not a slot number"))
            })?;
            if slot >= floor.len() {
                return err(format!("machine.prefill slot {slot} is outside the floor"));
            }
            floor[slot] = Some(*v);
        }
        Ok(floor)
    }

    /// Name of a floor slot for display (`None` if unnamed).
    pub fn slot_name(&self, slot: usize) -> Option<&str> {
        self.machine
            .names
            .get(slot)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    /// Compile the reference solution.
    pub fn reference_program(&self) -> Result<Arc<Program>, LevelError> {
        let sol = self
            .solution
            .as_ref()
            .ok_or_else(|| LevelError(format!("level {} has no reference solution", self.id)))?;
        asm::assemble(&sol.asm).map(Arc::new).map_err(|e| {
            LevelError(format!(
                "reference solution of {} does not assemble: {}",
                self.id,
                e.join("; ")
            ))
        })
    }

    fn apply_fills(
        &self,
        mut floor: Vec<Option<i32>>,
        fills: &[(usize, Vec<i32>)],
    ) -> Result<Vec<Option<i32>>, LevelError> {
        for (at, vals) in fills {
            for (i, v) in vals.iter().enumerate() {
                let slot = at + i;
                if slot >= floor.len() {
                    return err(format!(
                        "level {}: floor fill at {at} with {} values does not fit in {} slots",
                        self.id,
                        vals.len(),
                        floor.len()
                    ));
                }
                floor[slot] = Some(*v);
            }
        }
        Ok(floor)
    }

    /// Build every concrete test: examples, hand-written cases, generated cases.
    pub fn build_tests(&self) -> Result<Vec<TestCase>, LevelError> {
        let template = self.floor_template()?;
        let mut reference: Option<Arc<Program>> = None;
        let mut oracle = |floor: &[Option<i32>], inbox: &[i32]| -> Result<Vec<i32>, LevelError> {
            if reference.is_none() {
                reference = Some(self.reference_program()?);
            }
            let prog = reference
                .as_ref()
                .ok_or_else(|| LevelError("no reference".into()))?;
            let r = run_program(prog, inbox, floor, self.tests.step_limit);
            match r.status {
                Status::Error(e) => err(format!(
                    "level {}: the reference solution failed on input {inbox:?}: {e}",
                    self.id
                )),
                _ => Ok(r.outbox),
            }
        };
        let fills_of = |f: &[FloorFill]| -> Vec<(usize, Vec<i32>)> {
            f.iter().map(|x| (x.at, x.values.to_vec())).collect()
        };
        let mut tests = Vec::new();
        for ex in &self.examples {
            let floor = self.apply_fills(template.clone(), &fills_of(&ex.floor))?;
            tests.push(TestCase {
                inbox: ex.input.to_vec(),
                floor,
                expected: ex.output.to_vec(),
                origin: TestOrigin::Example,
            });
        }
        for c in &self.tests.fixed {
            let floor = self.apply_fills(template.clone(), &fills_of(&c.floor))?;
            let inbox = c.input.to_vec();
            let expected = match &c.output {
                Some(o) => o.to_vec(),
                None => oracle(&floor, &inbox)?,
            };
            tests.push(TestCase {
                inbox,
                floor,
                expected,
                origin: TestOrigin::Fixed,
            });
        }
        for (gi, g) in self.tests.generators.iter().enumerate() {
            let seed = self
                .tests
                .seed
                .wrapping_add((gi as u64 + 1).wrapping_mul(0x1000_0000_01B3));
            let mut rng = Rng::new(seed);
            for _ in 0..g.count {
                let gen_in = testgen::generate_one(&mut rng, g)
                    .map_err(|e| LevelError(format!("level {}: {e}", self.id)))?;
                let floor = self.apply_fills(template.clone(), &gen_in.floor)?;
                let expected = oracle(&floor, &gen_in.inbox)?;
                tests.push(TestCase {
                    inbox: gen_in.inbox,
                    floor,
                    expected,
                    origin: TestOrigin::Generated,
                });
            }
        }
        Ok(tests)
    }
}

impl PackInfo {
    /// Parse `pack.toml`.
    pub fn from_toml(text: &str, dir_name: &str) -> Result<PackInfo, LevelError> {
        let mut p: PackInfo = toml::from_str(text).map_err(|e| LevelError(e.to_string()))?;
        if p.id.is_empty() {
            p.id = dir_name.to_string();
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const SAMPLE: &str = r#"
id = "echo-test"
title = "Echo"
summary = "Pass everything along."
story = "A story."
statement = "Output every input."

[[examples]]
input = [1, 2]
output = [1, 2]

[machine]
floor = 2
allowed = ["inbox", "outbox", "jump"]
[machine.prefill]
0 = 7

[tests]
seed = 3
[[tests.fixed]]
input = []
output = []
[[tests.fixed]]
input = "ab"
output = [97, 98]
[[tests.generators]]
count = 3
inbox = [ { kind = "list", len = [1, 4], range = [0, 9] } ]

[targets]
size_par = 4
size_ace = 3
speed_par = 20
speed_ace = 10

[solution]
asm = """
a:
INBOX
OUTBOX
JUMP a
"""
"#;

    #[test]
    fn parse_and_build() {
        let l = Level::from_toml(SAMPLE, "tutorial").unwrap();
        assert_eq!(l.pack, "tutorial");
        let tests = l.build_tests().unwrap();
        assert_eq!(tests.len(), 1 + 2 + 3);
        assert_eq!(tests[0].floor, vec![Some(7), None]);
        assert_eq!(tests[2].inbox, vec![97, 98]);
        for t in &tests {
            assert_eq!(t.inbox, t.expected);
        }
    }

    #[test]
    fn unknown_field_is_rejected() {
        let bad = SAMPLE.replace("title = \"Echo\"", "title = \"Echo\"\nbogus = 1");
        assert!(Level::from_toml(&bad, "x").is_err());
    }

    #[test]
    fn generation_is_deterministic() {
        let l = Level::from_toml(SAMPLE, "t").unwrap();
        assert_eq!(l.build_tests().unwrap(), l.build_tests().unwrap());
    }

    #[test]
    fn fill_outside_floor_is_error() {
        let bad = SAMPLE.replace(
            "[[tests.fixed]]\ninput = []",
            "[[tests.fixed]]\nfloor = [{ at = 1, values = [1, 2, 3] }]\ninput = []",
        );
        let l = Level::from_toml(&bad, "x").unwrap();
        assert!(l.build_tests().is_err());
    }
}
