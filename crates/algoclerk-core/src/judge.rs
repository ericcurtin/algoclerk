// SPDX-License-Identifier: Apache-2.0
//! The judge: compiles a solution, runs it on all tests with budgets, scores size and speed,
//! estimates complexity.

use crate::complexity::{Complexity, classify};
use crate::lang::ast::Ty;
use crate::lang::check::TypeInfo;
use crate::lang::diag::Diag;
use crate::lang::interp::{Limits, RtError, RtKind};
use crate::lang::run::{Compiled, compile};
use crate::lang::value::{Value, parse_value, render};
use crate::level::{Compare, Level};
use crate::testgen::expand;
use serde::{Deserialize, Serialize};

/// A medal tier. The names are the office's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Medal {
    /// Nothing yet.
    #[default]
    None,
    /// Paper clip.
    Bronze,
    /// Stapler.
    Silver,
    /// Golden rubber stamp.
    Gold,
}

impl Medal {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Medal::None => "none",
            Medal::Bronze => "Paper Clip",
            Medal::Silver => "Stapler",
            Medal::Gold => "Golden Stamp",
        }
    }
    /// 0..=3.
    pub fn rank(self) -> u8 {
        self as u8
    }
}

/// Both medals of a solution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Medals {
    /// For program size.
    pub size: Medal,
    /// For speed (average abstract cost).
    pub speed: Medal,
}

impl Medals {
    /// Stars 0..=3: one for solving, one for each medal of silver or better.
    pub fn stars(&self) -> u8 {
        1 + u8::from(self.size >= Medal::Silver) + u8::from(self.speed >= Medal::Silver)
    }
}

/// One concrete test.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Argument literals.
    pub args: Vec<String>,
    /// Expected result literal.
    pub expect: String,
    /// Performance test (large budget).
    pub large: bool,
    /// Visible example.
    pub example: bool,
    /// Note (examples).
    pub note: String,
}

/// Why a test failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailKind {
    /// Wrong answer.
    Wrong,
    /// Runtime error.
    Error,
    /// Budget exhausted.
    TimeLimit,
    /// Too much memory.
    MemoryLimit,
}

/// The first failing test.
#[derive(Clone, Debug)]
pub struct Failure {
    /// Index in the suite.
    pub index: usize,
    /// The test.
    pub case: Case,
    /// Category.
    pub kind: FailKind,
    /// What the program returned (wrong answer).
    pub got: Option<String>,
    /// Runtime error.
    pub error: Option<RtError>,
}

/// Outcome of judging one solution.
#[derive(Clone, Debug, Default)]
pub struct Verdict {
    /// Everything passed.
    pub passed: bool,
    /// Compile (parse/type) problems.
    pub diags: Vec<Diag>,
    /// First failing test.
    pub failure: Option<Failure>,
    /// Tests passed before stopping.
    pub tests_passed: usize,
    /// Tests in the suite.
    pub tests_total: usize,
    /// Program size (statement and expression nodes).
    pub size: u32,
    /// Average cost over the standard (non-large) tests.
    pub avg_cost: f64,
    /// Highest cost on a standard test.
    pub max_cost: u64,
    /// Cost on the large tests (max).
    pub large_cost: u64,
    /// Medals earned.
    pub medals: Medals,
    /// Measured complexity (only when passed).
    pub complexity: Option<Complexity>,
}

/// A level with its reference compiled and its full test suite expanded.
#[derive(Debug)]
pub struct Judge {
    /// The level.
    pub level: Level,
    /// Compiled reference solution.
    pub reference: Compiled,
    /// All tests.
    pub suite: Vec<Case>,
}

/// Compare outputs, honouring the level's comparison mode.
pub fn canonical(text: &str, ty: &Ty, mode: Compare) -> String {
    let mut nodes = Vec::new();
    let Ok(v) = parse_value(text, ty, &mut nodes) else { return text.to_string() };
    let v = match mode {
        Compare::Exact => v,
        Compare::SortOuter => sort_value(v, false),
        Compare::SortAll => sort_value(v, true),
    };
    render(&v, ty, &nodes)
}

fn sort_value(v: Value, deep: bool) -> Value {
    match v {
        Value::Vec(items) => {
            let mut xs: Vec<Value> = items.iter().cloned().map(|x| if deep { sort_value(x, true) } else { x }).collect();
            xs.sort();
            Value::vec(xs)
        }
        other => other,
    }
}

impl Judge {
    /// Prepare a level: compile the reference and expand every test (hand-written and generated).
    pub fn new(level: &Level) -> Result<Judge, String> {
        let opts = level.check_opts();
        let reference = compile(&level.reference, &opts)
            .map_err(|d| format!("reference does not compile: {}", d.first().map_or(String::new(), |d| d.msg.clone())))?;
        let ret = level.signature.ret.clone();
        let mut suite: Vec<Case> = Vec::new();
        let limits = Limits { steps: level.budgets.large_steps * 4, ..Limits::default() };
        let mut push = |args: Vec<String>, expect: Option<String>, large: bool, example: bool, note: String| -> Result<(), String> {
            let expect = match expect {
                Some(e) if !e.is_empty() => canonical(&e, &ret, level.compare),
                _ => {
                    let r = reference.call(&level.signature.name, &args, &limits).map_err(|e| format!("bad test args {args:?}: {e}"))?;
                    match r.output {
                        Some(o) => canonical(&o, &ret, level.compare),
                        None => {
                            return Err(format!(
                                "reference failed on generated test {args:?}: {}",
                                r.report.result.err().map_or(String::new(), |e| e.msg)
                            ));
                        }
                    }
                }
            };
            suite.push(Case { args, expect, large, example, note });
            Ok(())
        };
        for c in &level.examples {
            push(c.args.clone(), Some(c.expect.clone()), false, true, c.note.clone())?;
        }
        for c in &level.tests {
            push(c.args.clone(), Some(c.expect.clone()), false, false, c.note.clone())?;
        }
        for g in &level.generate {
            for args in expand(g, None)? {
                push(args, None, g.large, false, String::new())?;
            }
        }
        Ok(Judge { level: level.clone(), reference, suite })
    }

    /// Standard (non-large) test count.
    pub fn standard_count(&self) -> usize {
        self.suite.iter().filter(|c| !c.large).count()
    }

    fn limits_for(&self, case: &Case, measure: bool) -> Limits {
        Limits {
            steps: if case.large { self.level.budgets.large_steps } else { self.level.budgets.steps },
            measure,
            ..Limits::default()
        }
    }

    /// Run a compiled solution on one case; returns `(passed, cost, report)`.
    fn run_case(&self, sol: &Compiled, case: &Case) -> (Result<(), (FailKind, Option<String>, Option<RtError>)>, u64) {
        let name = &self.level.signature.name;
        let ret = &self.level.signature.ret;
        match sol.call(name, &case.args, &self.limits_for(case, false)) {
            Err(e) => (Err((FailKind::Error, None, Some(RtError { kind: RtKind::Internal, node: 0, msg: e }))), 0),
            Ok(r) => {
                let cost = r.report.cost;
                match r.report.result {
                    Ok(_) => {
                        let got = canonical(&r.output.unwrap_or_default(), ret, self.level.compare);
                        if got == case.expect { (Ok(()), cost) } else { (Err((FailKind::Wrong, Some(got), None)), cost) }
                    }
                    Err(e) => {
                        let k = match e.kind {
                            RtKind::Time => FailKind::TimeLimit,
                            RtKind::Memory => FailKind::MemoryLimit,
                            _ => FailKind::Error,
                        };
                        (Err((k, None, Some(e))), cost)
                    }
                }
            }
        }
    }

    /// Judge program text.
    pub fn judge(&self, text: &str) -> Verdict {
        let sol = match compile(text, &self.level.check_opts()) {
            Ok(c) => c,
            Err(d) => return Verdict { diags: d, tests_total: self.suite.len(), ..Verdict::default() },
        };
        self.judge_compiled(&sol, true)
    }

    /// Judge an already compiled solution.
    pub fn judge_compiled(&self, sol: &Compiled, with_complexity: bool) -> Verdict {
        let mut v = Verdict { tests_total: self.suite.len(), size: sol.program.size(), ..Verdict::default() };
        let mut total = 0u64;
        let mut std_n = 0u64;
        for (i, case) in self.suite.iter().enumerate() {
            let (r, cost) = self.run_case(sol, case);
            match r {
                Ok(()) => {
                    v.tests_passed += 1;
                    if case.large {
                        v.large_cost = v.large_cost.max(cost);
                    } else {
                        total += cost;
                        std_n += 1;
                        v.max_cost = v.max_cost.max(cost);
                    }
                }
                Err((kind, got, error)) => {
                    v.failure = Some(Failure { index: i, case: case.clone(), kind, got, error });
                    return v;
                }
            }
        }
        v.passed = true;
        v.avg_cost = if std_n == 0 { 0.0 } else { total as f64 / std_n as f64 };
        v.medals = self.medals_for(v.size, v.avg_cost);
        if with_complexity {
            v.complexity = self.complexity(sol);
        }
        v
    }

    /// Medals for a size and an average cost.
    pub fn medals_for(&self, size: u32, avg_cost: f64) -> Medals {
        let m = &self.level.medals;
        let tier = |value: f64, t: [f64; 3]| {
            if value <= t[2] {
                Medal::Gold
            } else if value <= t[1] {
                Medal::Silver
            } else if value <= t[0] {
                Medal::Bronze
            } else {
                Medal::None
            }
        };
        Medals {
            size: tier(f64::from(size), m.size.map(f64::from)),
            speed: tier(avg_cost, m.speed.map(|x| x as f64)),
        }
    }

    /// Run the solution at n0, 2 n0, 4 n0, 8 n0 using the first large generator and classify.
    pub fn complexity(&self, sol: &Compiled) -> Option<Complexity> {
        let spec = self.level.generate.iter().find(|g| g.large)?;
        let n0 = self.level.budgets.sweep_n0.max(8);
        let mut samples = Vec::new();
        let name = &self.level.signature.name;
        for k in 0..4 {
            let n = n0 << k;
            let mut one = spec.clone();
            one.count = 1;
            let args = expand(&one, Some(n)).ok()?.into_iter().next()?;
            let limits = Limits { steps: 400_000_000, measure: true, cells: 40_000_000, ..Limits::default() };
            let r = sol.call(name, &args, &limits).ok()?;
            r.report.result.as_ref().ok()?;
            samples.push((n, r.report.cost, r.report.peak_cells.saturating_sub(r.report.input_cells)));
        }
        let time = classify(&samples.iter().map(|s| s.1 as f64).collect::<Vec<_>>());
        let space = classify(&samples.iter().map(|s| s.2 as f64).collect::<Vec<_>>());
        Some(Complexity { time, space, samples })
    }

    /// The reference verdict (used by validation and the CLI).
    pub fn reference_verdict(&self) -> Verdict {
        self.judge_compiled(&self.reference, true)
    }
}

/// Type information of a compiled solution (re-exported for front ends).
pub fn info_of(c: &Compiled) -> &TypeInfo {
    &c.info
}
