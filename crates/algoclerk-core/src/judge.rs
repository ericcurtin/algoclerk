// SPDX-License-Identifier: Apache-2.0
//! The judge: compiles a solution, runs it on all tests with budgets, scores size and speed,
//! estimates complexity.

use crate::complexity::{Complexity, SPACE_NOISE_CELLS, classify, classify_with_floor};
use crate::cpp::ast::{
    Block, ExprKind, Init, ItemKind, Program, StmtKind, Ty, TypeSpec, stmt_exprs, walk_expr,
    walk_stmts,
};
use crate::cpp::check::TypeInfo;
use crate::cpp::diag::Diag;
use crate::cpp::interp::{Limits, RtError, RtKind, UnorderedOrder};
use crate::cpp::run::{Compiled, compile};
use crate::cpp::value::{Heap, Plain, parse_value, render};
use crate::level::{Compare, Level};
use crate::testgen::expand;
use serde::{Deserialize, Serialize};

/// A medal tier. The names are the office's own.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
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
    /// The answer changed when `unordered_map` / `unordered_set` iterate in the opposite order.
    UnorderedOrder,
}

impl FailKind {
    /// A friendly one-line explanation for the result screen.
    pub fn friendly(&self) -> &'static str {
        match self {
            FailKind::Wrong => "The answer is not what the form asks for.",
            FailKind::Error => "The program stopped with an error before it could answer.",
            FailKind::TimeLimit => {
                "The program took too long: the office closes before it finishes."
            }
            FailKind::MemoryLimit => "The program needs more shelf space than the office has.",
            FailKind::UnorderedOrder => {
                "Your answer depends on the order in which an unordered_map or unordered_set hands out \
                 its items. C++ promises no order there: sort what you return or do not rely on it."
            }
        }
    }
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

/// Why a case failed: kind, what was returned, runtime error.
type CaseFailure = (FailKind, Option<String>, Option<RtError>);
type CaseResult = Result<(), CaseFailure>;

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

/// The canonical text of an output: parsed with the declared type and rendered again after the level's
/// comparison mode was applied. The elements of `unordered_map` / `unordered_set` values are always sorted
/// (their iteration order is unspecified in C++). Text that does not parse is returned unchanged.
pub fn canonical(text: &str, ty: &Ty, mode: Compare) -> String {
    let mut heap = Heap::default();
    let Ok(v) = parse_value(text, ty, &mut heap) else {
        return text.to_string();
    };
    let v = canon_value(v, ty, mode, true);
    render(&v, ty, &heap)
}

/// Is `ty` a sequence whose element order the comparison modes may ignore?
fn orderable_seq(ty: &Ty) -> Option<&Ty> {
    match ty {
        Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) | Ty::Array(t, _) => Some(t),
        _ => None,
    }
}

fn canon_value(v: Plain, ty: &Ty, mode: Compare, outer: bool) -> Plain {
    match (v, ty) {
        (Plain::Seq(xs), Ty::UnorderedSet(t)) => {
            let mut xs: Vec<Plain> = xs
                .into_iter()
                .map(|x| canon_value(x, t, mode, false))
                .collect();
            xs.sort();
            Plain::Seq(xs)
        }
        (Plain::Map(es), Ty::UnorderedMap(k, val)) => {
            let mut es: Vec<(Plain, Plain)> = es
                .into_iter()
                .map(|(a, b)| {
                    (
                        canon_value(a, k, mode, false),
                        canon_value(b, val, mode, false),
                    )
                })
                .collect();
            es.sort();
            Plain::Map(es)
        }
        (Plain::Map(es), Ty::Map(k, val)) => Plain::Map(
            es.into_iter()
                .map(|(a, b)| {
                    (
                        canon_value(a, k, mode, false),
                        canon_value(b, val, mode, false),
                    )
                })
                .collect(),
        ),
        (Plain::Seq(xs), t) if orderable_seq(t).is_some() => {
            let et = orderable_seq(t).cloned().unwrap_or(Ty::Unknown);
            let sort = match mode {
                Compare::Exact => false,
                Compare::SortOuter => outer,
                Compare::SortAll => true,
            };
            let mut xs: Vec<Plain> = xs
                .into_iter()
                .map(|x| canon_value(x, &et, mode, false))
                .collect();
            if sort {
                xs.sort();
            }
            Plain::Seq(xs)
        }
        (Plain::Seq(xs), Ty::Pair(a, b)) if xs.len() == 2 => {
            let mut it = xs.into_iter();
            let x = canon_value(it.next().unwrap_or(Plain::Void), a, mode, false);
            let y = canon_value(it.next().unwrap_or(Plain::Void), b, mode, false);
            Plain::Seq(vec![x, y])
        }
        (Plain::Opt(Some(x)), Ty::Optional(t)) => {
            Plain::Opt(Some(Box::new(canon_value(*x, t, mode, false))))
        }
        (v, _) => v,
    }
}

/// Does the player's part of the program mention an unordered container type anywhere?
fn uses_unordered(p: &Program) -> bool {
    let unordered = |s: &TypeSpec| {
        let mut hit = false;
        s.ty.walk(&mut |t| {
            hit |= matches!(t, Ty::UnorderedMap(..) | Ty::UnorderedSet(_));
        });
        hit
    };
    fn block_hit(b: &Block, unordered: &dyn Fn(&TypeSpec) -> bool) -> bool {
        let mut hit = false;
        walk_stmts(b, &mut |s| {
            match &s.kind {
                StmtKind::Decl { spec, .. }
                | StmtKind::Structured { spec, .. }
                | StmtKind::RangeFor { spec, .. } => hit |= unordered(spec),
                _ => {}
            }
            stmt_exprs(s, &mut |e| {
                walk_expr(e, &mut |x| match &x.kind {
                    ExprKind::Construct { ty, .. }
                    | ExprKind::New { ty, .. }
                    | ExprKind::Cast { ty, .. } => hit |= unordered(ty),
                    ExprKind::Lambda(l) => {
                        hit |= l.params.iter().any(|p| unordered(&p.spec))
                            || l.ret.as_ref().is_some_and(unordered)
                            || block_hit(&l.body, unordered);
                    }
                    _ => {}
                });
            });
        });
        hit
    }
    p.player_items().iter().any(|item| match &item.kind {
        ItemKind::Func(f) => {
            f.params.iter().any(|q| unordered(&q.spec))
                || unordered(&f.ret)
                || f.body.as_ref().is_some_and(|b| block_hit(b, &unordered))
        }
        ItemKind::Alias { ty, .. } => unordered(&TypeSpec::plain(ty.clone())),
        ItemKind::Global { spec, vars, .. } => {
            unordered(spec)
                || vars.iter().any(|v| {
                    let mut hit = false;
                    if let Init::Copy(e) = &v.init {
                        walk_expr(e, &mut |x| {
                            if let ExprKind::Construct { ty, .. } = &x.kind {
                                hit |= unordered(ty);
                            }
                        });
                    }
                    hit
                })
        }
        ItemKind::Struct(s) => s.fields.iter().any(|f| unordered(&f.spec)),
        _ => false,
    })
}

/// A test before its expected value is settled.
#[derive(Clone, Debug, PartialEq)]
pub struct RawTest {
    /// Argument literals.
    pub args: Vec<String>,
    /// Expected literal when the level file states one (hand-written tests); `None` for generated tests
    /// (the reference decides).
    pub expect: Option<String>,
    /// Performance test (large budget).
    pub large: bool,
    /// Visible example.
    pub example: bool,
    /// Note (examples).
    pub note: String,
}

/// Expand every example, hand-written test and generator of a level (no solution is run).
/// `large_cap` limits the `n` of large generators (used to run the large tests in real C++ quickly).
pub fn raw_suite(level: &Level, large_cap: Option<i64>) -> Result<Vec<RawTest>, String> {
    let mut out = Vec::new();
    for (list, example) in [(&level.examples, true), (&level.tests, false)] {
        for c in list {
            out.push(RawTest {
                args: c.args.clone(),
                expect: Some(c.expect.clone()).filter(|e| !e.is_empty()),
                large: false,
                example,
                note: c.note.clone(),
            });
        }
    }
    for g in &level.generate {
        let n_override = match large_cap {
            Some(cap) if g.large && g.n > cap => Some(cap),
            _ => None,
        };
        for args in expand(g, n_override)? {
            out.push(RawTest {
                args,
                expect: None,
                large: g.large,
                example: false,
                note: String::new(),
            });
        }
    }
    Ok(out)
}

impl Judge {
    /// Prepare a level: compile the reference and expand every test (hand-written and generated). The
    /// expected value of a generated test is what the reference returns.
    pub fn new(level: &Level) -> Result<Judge, String> {
        let opts = level.check_opts();
        let reference = compile(&level.prelude, &level.reference, &opts).map_err(|d| {
            format!(
                "reference does not compile: {}",
                d.first().map_or(String::new(), |d| d.msg.clone())
            )
        })?;
        let ret = level.signature.ret.ty.clone();
        let limits = Limits {
            steps: level.budgets.large_steps * 4,
            ..Limits::default()
        };
        let mut suite: Vec<Case> = Vec::new();
        for t in raw_suite(level, None)? {
            let expect = match t.expect {
                Some(e) => canonical(&e, &ret, level.compare),
                None => {
                    let r = reference
                        .call(&level.signature.name, &t.args, &limits)
                        .map_err(|e| format!("bad test args {:?}: {e}", t.args))?;
                    match r.output {
                        Some(o) => canonical(&o, &ret, level.compare),
                        None => {
                            return Err(format!(
                                "reference failed on generated test {:?}: {}",
                                t.args
                                    .iter()
                                    .map(|a| a.chars().take(80).collect::<String>())
                                    .collect::<Vec<_>>(),
                                r.report.result.err().map_or(String::new(), |e| e.msg)
                            ));
                        }
                    }
                }
            };
            suite.push(Case {
                args: t.args,
                expect,
                large: t.large,
                example: t.example,
                note: t.note,
            });
        }
        Ok(Judge {
            level: level.clone(),
            reference,
            suite,
        })
    }

    /// Standard (non-large) test count.
    pub fn standard_count(&self) -> usize {
        self.suite.iter().filter(|c| !c.large).count()
    }

    fn limits_for(&self, case: &Case, measure: bool) -> Limits {
        self.limits_with(case, measure, UnorderedOrder::Insertion)
    }

    fn limits_with(&self, case: &Case, measure: bool, unordered: UnorderedOrder) -> Limits {
        Limits {
            unordered,
            steps: if case.large {
                self.level.budgets.large_steps
            } else {
                self.level.budgets.steps
            },
            measure,
            ..Limits::default()
        }
    }

    /// Run a compiled solution on one case; returns `(passed, cost, report)`.
    fn run_case(&self, sol: &Compiled, case: &Case) -> (CaseResult, u64) {
        let name = &self.level.signature.name;
        let ret = &self.level.signature.ret.ty;
        match sol.call(name, &case.args, &self.limits_for(case, false)) {
            Err(e) => (
                Err((
                    FailKind::Error,
                    None,
                    Some(RtError {
                        kind: RtKind::Internal,
                        node: 0,
                        msg: e,
                    }),
                )),
                0,
            ),
            Ok(r) => {
                let cost = r.report.cost;
                match r.report.result {
                    Ok(_) => {
                        let got = canonical(&r.output.unwrap_or_default(), ret, self.level.compare);
                        if got == case.expect {
                            (Ok(()), cost)
                        } else {
                            (Err((FailKind::Wrong, Some(got), None)), cost)
                        }
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
        let sol = match compile(&self.level.prelude, text, &self.level.check_opts()) {
            Ok(c) => c,
            Err(d) => {
                return Verdict {
                    diags: d,
                    tests_total: self.suite.len(),
                    ..Verdict::default()
                };
            }
        };
        self.judge_compiled(&sol, true)
    }

    /// Judge an already compiled solution.
    pub fn judge_compiled(&self, sol: &Compiled, with_complexity: bool) -> Verdict {
        let mut v = Verdict {
            tests_total: self.suite.len(),
            size: sol.size(),
            ..Verdict::default()
        };
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
                    v.failure = Some(Failure {
                        index: i,
                        case: case.clone(),
                        kind,
                        got,
                        error,
                    });
                    return v;
                }
            }
        }
        if uses_unordered(&sol.program)
            && let Some(f) = self.unordered_failure(sol)
        {
            v.failure = Some(f);
            return v;
        }
        v.passed = true;
        v.avg_cost = if std_n == 0 {
            0.0
        } else {
            total as f64 / std_n as f64
        };
        v.medals = self.medals_for(v.size, v.avg_cost);
        if with_complexity {
            v.complexity = self.complexity(sol);
        }
        v
    }

    /// Re-run the standard tests with `unordered_map` / `unordered_set` iterating in the opposite order: a
    /// solution whose answer changes depends on an order the C++ standard leaves unspecified.
    fn unordered_failure(&self, sol: &Compiled) -> Option<Failure> {
        let name = &self.level.signature.name;
        let ret = &self.level.signature.ret.ty;
        for (i, case) in self.suite.iter().enumerate().filter(|(_, c)| !c.large) {
            let limits = self.limits_with(case, false, UnorderedOrder::Reverse);
            let (got, error) = match sol.call(name, &case.args, &limits) {
                Ok(r) => match r.report.result {
                    Ok(_) => {
                        let got = canonical(&r.output.unwrap_or_default(), ret, self.level.compare);
                        if got == case.expect {
                            continue;
                        }
                        (Some(got), None)
                    }
                    Err(e) => (None, Some(e)),
                },
                Err(e) => (
                    None,
                    Some(RtError {
                        kind: RtKind::Internal,
                        node: 0,
                        msg: e,
                    }),
                ),
            };
            return Some(Failure {
                index: i,
                case: case.clone(),
                kind: FailKind::UnorderedOrder,
                got,
                error,
            });
        }
        None
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
        let spec = self
            .level
            .sweep
            .as_ref()
            .or_else(|| self.level.generate.iter().find(|g| g.large))?;
        if self.level.budgets.sweep_n0 <= 0 {
            return None;
        }
        let n0 = self.level.budgets.sweep_n0.max(8);
        let mut samples = Vec::new();
        let name = &self.level.signature.name;
        for k in 0..4 {
            let n = n0 << k;
            let mut one = spec.clone();
            one.count = 1;
            let args = expand(&one, Some(n)).ok()?.into_iter().next()?;
            let limits = Limits {
                steps: 400_000_000,
                measure: true,
                cells: 40_000_000,
                ..Limits::default()
            };
            let r = sol.call(name, &args, &limits).ok()?;
            r.report.result.as_ref().ok()?;
            samples.push((
                n,
                r.report.cost,
                r.report.peak_cells.saturating_sub(r.report.input_cells),
            ));
        }
        let time = classify(&samples.iter().map(|s| s.1 as f64).collect::<Vec<_>>());
        let space = classify_with_floor(
            &samples.iter().map(|s| s.2 as f64).collect::<Vec<_>>(),
            SPACE_NOISE_CELLS,
        );
        Some(Complexity {
            time,
            space,
            samples,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpp::ast::Functor;

    fn canon(text: &str, ty: &Ty, mode: Compare) -> String {
        canonical(text, ty, mode)
    }

    #[test]
    fn canonical_normalises_spelling() {
        let v = Ty::vector(Ty::Int);
        assert_eq!(canon("{1,2,  3}", &v, Compare::Exact), "{1, 2, 3}");
        assert_eq!(canon("{1, 2, 3,}", &v, Compare::Exact), "{1, 2, 3}");
        assert_eq!(
            canon(
                "{}",
                &Ty::Ptr(Box::new(Ty::Struct("ListNode".into()))),
                Compare::Exact
            ),
            "nullptr"
        );
        // unparsable text is returned unchanged
        assert_eq!(canon("garbage", &v, Compare::Exact), "garbage");
        let _ = Functor::Less;
    }

    #[test]
    fn compare_modes_sort_the_right_levels() {
        let vv = Ty::vector(Ty::vector(Ty::Int));
        assert_eq!(canon("{{3, 1}, {2}}", &vv, Compare::Exact), "{{3, 1}, {2}}");
        assert_eq!(
            canon("{{3, 1}, {2}}", &vv, Compare::SortOuter),
            "{{2}, {3, 1}}"
        );
        assert_eq!(
            canon("{{3, 1}, {2}}", &vv, Compare::SortAll),
            "{{1, 3}, {2}}"
        );
        let vs = Ty::vector(Ty::Str);
        assert_eq!(
            canon("{\"b\", \"a\"}", &vs, Compare::SortOuter),
            "{\"a\", \"b\"}"
        );
    }

    #[test]
    fn unordered_containers_are_always_sorted() {
        let us = Ty::UnorderedSet(Box::new(Ty::Int));
        assert_eq!(canon("{3, 1, 2}", &us, Compare::Exact), "{1, 2, 3}");
        let um = Ty::UnorderedMap(Box::new(Ty::Str), Box::new(Ty::Int));
        assert_eq!(
            canon("{{\"z\", 1}, {\"a\", 2}}", &um, Compare::Exact),
            "{{\"a\", 2}, {\"z\", 1}}"
        );
        let m = Ty::Map(Box::new(Ty::Int), Box::new(Ty::Int));
        assert_eq!(
            canon("{{2, 1}, {1, 2}}", &m, Compare::Exact),
            "{{2, 1}, {1, 2}}"
        );
        // a vector of unordered sets: the inner sets are sorted, the outer order is kept
        let vus = Ty::vector(us);
        assert_eq!(
            canon("{{2, 1}, {0}}", &vus, Compare::Exact),
            "{{1, 2}, {0}}"
        );
    }

    #[test]
    fn medal_tiers() {
        let level =
            crate::level::Level::from_toml(include_str!("../tests/data/tiny.toml"), "t").unwrap();
        let j = Judge {
            level,
            reference: dummy_compiled(),
            suite: Vec::new(),
        };
        assert_eq!(j.medals_for(6, 6.0).size, Medal::Gold);
        assert_eq!(j.medals_for(8, 6.0).size, Medal::Silver);
        assert_eq!(j.medals_for(10, 6.0).size, Medal::Bronze);
        assert_eq!(j.medals_for(11, 6.0).size, Medal::None);
        assert_eq!(j.medals_for(6, 20.0).speed, Medal::Bronze);
        assert_eq!(j.medals_for(6, 6.0).stars(), 3);
    }

    fn dummy_compiled() -> Compiled {
        Compiled {
            program: Program::default(),
            info: TypeInfo::default(),
        }
    }

    #[test]
    fn raw_suite_expands_examples_tests_and_generators() {
        let level =
            crate::level::Level::from_toml(include_str!("../tests/data/tiny.toml"), "t").unwrap();
        let suite = raw_suite(&level, None).unwrap();
        assert_eq!(suite.len(), 1 + 4);
        assert!(suite[0].example && suite[0].expect.as_deref() == Some("3"));
        assert!(suite[1..].iter().all(|t| t.expect.is_none() && !t.large));
    }

    #[test]
    fn friendly_texts_exist() {
        for k in [
            FailKind::Wrong,
            FailKind::Error,
            FailKind::TimeLimit,
            FailKind::MemoryLimit,
            FailKind::UnorderedOrder,
        ] {
            assert!(!k.friendly().is_empty());
        }
    }
}
