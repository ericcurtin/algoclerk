// SPDX-License-Identifier: Apache-2.0
//! Level validation used by CI, the CLI and the shipped-campaign tests.

use crate::complexity::Class;
use crate::judge::{FailKind, Judge, Medal};
use crate::lang::run::compile;
use crate::level::Level;

/// Parse the leading class of a Big-O note such as `"O(n log n) time, O(n) space"`.
pub fn declared_time_class(big_o: &str) -> Option<Class> {
    let head = big_o.split(" time").next().unwrap_or(big_o).trim();
    Some(match head {
        "O(1)" => Class::Constant,
        "O(log n)" => Class::Log,
        "O(sqrt n)" => Class::Sqrt,
        "O(n)" => Class::Linear,
        "O(n log n)" => Class::NLogN,
        "O(n^1.5)" => Class::N15,
        "O(n^2)" => Class::Quadratic,
        "O(n^3)" => Class::Cubic,
        "O(2^n)" => Class::Exponential,
        _ => return None,
    })
}

/// Parse the space class after the comma of a Big-O note.
pub fn declared_space_class(big_o: &str) -> Option<Class> {
    let tail = big_o.split(',').nth(1)?.trim();
    declared_time_class(tail.split(" space").next().unwrap_or(tail))
}

/// Validate one level; returns human-readable problems (empty when it is sound).
pub fn validate_level(level: &Level) -> Vec<String> {
    let mut problems = Vec::new();
    let mut p = |m: String| problems.push(format!("{}: {m}", level.id));
    if level.examples.len() < 2 {
        p("needs at least two examples".into());
    }
    if level.hints.len() < 3 {
        p("needs a hint ladder of at least three hints".into());
    }
    if level.story.trim().is_empty() || level.statement.trim().is_empty() {
        p("story and statement must not be empty".into());
    }
    if level.technique.trim().is_empty() {
        p("missing technique".into());
    }
    if !(level.medals.size[0] >= level.medals.size[1] && level.medals.size[1] >= level.medals.size[2] && level.medals.size[2] > 0) {
        p("size medal targets must be bronze >= silver >= gold > 0".into());
    }
    if !(level.medals.speed[0] >= level.medals.speed[1] && level.medals.speed[1] >= level.medals.speed[2] && level.medals.speed[2] > 0) {
        p("speed medal targets must be bronze >= silver >= gold > 0".into());
    }
    let judge = match Judge::new(level) {
        Ok(j) => j,
        Err(e) => {
            p(e);
            return problems;
        }
    };
    let scalar_only = level.signature.params.iter().all(|p| p.ty.is_scalar());
    if !scalar_only && !level.generate.iter().any(|g| g.large) {
        p("needs at least one large generator (performance tests and the complexity sweep)".into());
    }
    let v = judge.reference_verdict();
    if !v.passed {
        p(format!("the reference solution fails: {:?}", v.failure.map(|f| (f.kind, f.case.args, f.got, f.error.map(|e| e.msg)))));
        return problems;
    }
    if v.medals.size != Medal::Gold {
        p(format!("reference size {} misses the gold size target {}", v.size, level.medals.size[2]));
    }
    if v.medals.speed != Medal::Gold {
        p(format!("reference speed {:.1} misses the gold speed target {}", v.avg_cost, level.medals.speed[2]));
    }
    // large tests must leave generous headroom for the reference
    let headroom = |cost: u64, budget: u64| cost.saturating_mul(3) <= budget;
    if !headroom(v.large_cost, level.budgets.large_steps) {
        p(format!("reference uses {} of the {} large budget; need 3x headroom", v.large_cost, level.budgets.large_steps));
    }
    if !headroom(v.max_cost, level.budgets.steps) {
        p(format!("reference uses {} of the {} standard budget; need 3x headroom", v.max_cost, level.budgets.steps));
    }
    match (&v.complexity, declared_time_class(&level.big_o)) {
        (Some(c), Some(decl)) => {
            if c.time > decl {
                p(format!("measured time {} is worse than the declared {}", c.time, decl));
            }
            if c.time < decl && decl >= Class::Quadratic {
                // allowed (faster than declared), but suspicious for the stated technique
            }
            if let Some(sp) = declared_space_class(&level.big_o)
                && c.space > sp
            {
                p(format!("measured space {} is worse than the declared {}", c.space, sp));
            }
        }
        (None, _) if scalar_only => {}
        (None, _) => p("no complexity sweep result".into()),
        (_, None) => p(format!("cannot parse the Big-O note '{}'", level.big_o)),
    }
    if let Some(naive) = &level.naive {
        match compile(naive, &level.check_opts()) {
            Err(d) => p(format!("naive solution does not compile: {}", d.first().map_or(String::new(), |d| d.msg.clone()))),
            Ok(sol) => {
                // it must be correct on the standard tests ...
                let mut small = judge_subset(&judge, &sol, false);
                if let Some(f) = small.take() {
                    p(format!("naive solution is wrong (not just slow) on {:?}: {:?}", f.case.args, f.kind));
                }
                // ... and must run out of time on a large test
                match judge_subset(&judge, &sol, true) {
                    Some(f) if f.kind == FailKind::TimeLimit => {}
                    Some(f) => p(format!("naive solution fails a large test for the wrong reason: {:?}", f.kind)),
                    None => p("naive solution passes the large tests; they do not bite".into()),
                }
            }
        }
    }
    problems
}

fn judge_subset(judge: &Judge, sol: &crate::lang::run::Compiled, large: bool) -> Option<crate::judge::Failure> {
    // Re-use the judge on a filtered suite.
    let mut sub = Judge { level: judge.level.clone(), reference: judge.reference.clone(), suite: judge.suite.iter().filter(|c| c.large == large).cloned().collect() };
    if sub.suite.is_empty() {
        sub.suite = vec![];
    }
    sub.judge_compiled(sol, false).failure
}
