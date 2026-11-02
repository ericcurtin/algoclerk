// SPDX-License-Identifier: Apache-2.0
//! Differential test against real compilers: every shipped level's reference and naive solution is
//! built into a translation unit (level prelude, solution, generated `main`) and compiled with
//! `g++ -std=c++17 -Wall -Wextra -O0` and `clang++ -std=c++17 -Wall -Wextra -O0`, run on the standard tests
//! plus reduced large tests, and checked:
//!
//! * no compiler warnings (the campaign's code is `-Wall -Wextra` clean);
//! * the printed results equal the level's declared expected values (canonical form, honouring the
//!   level's compare mode) and agree between compilers and between reference and naive;
//! * the independent Rust oracles agree with the compiled reference;
//! * the reference built with `-fsanitize=undefined,address` runs clean (UB freedom);
//! * the interpreter, when available, gives the same results as the compilers.
//!
//! Environment: `ALGOCLERK_DIFF_FULL=1` runs the large tests at full size, `ALGOCLERK_LEVEL=<id>` restricts
//! the run to one level, `ALGOCLERK_DIFF_KEEP=1` keeps the generated sources in the temp directory.
//! Missing compilers are skipped. Every compiler and binary runs under `timeout -s KILL`.

mod common;

use algoclerk_core::cpp::harness::{DiffOpts, available_compilers, diff_level};
use algoclerk_core::cpp::value::{Heap, parse_value};
use algoclerk_core::level::Level;
use algoclerk_levels::Campaign;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const THREADS: usize = 8;

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Compare the compiled reference's results with the independent oracle (when the level has one).
fn oracle_problems(level: &Level, rep: &algoclerk_core::cpp::harness::DiffReport) -> Vec<String> {
    let mut out = Vec::new();
    let mut any = false;
    for (i, case) in rep.cases.iter().enumerate() {
        let mut heap = Heap::default();
        let args: Result<Vec<_>, String> = level
            .signature
            .params
            .iter()
            .zip(&case.args)
            .map(|(p, lit)| {
                parse_value(lit, &p.spec.ty, &mut heap).map(|v| common::conv(&v, &p.spec.ty, &heap))
            })
            .collect();
        let Ok(args) = args else {
            out.push(format!("{}: case {i}: arguments do not parse", level.id));
            break;
        };
        let Some(expect) = common::oracle(&level.id, &args) else {
            return out;
        };
        any = true;
        let mut eheap = Heap::default();
        let Ok(v) = parse_value(&rep.expected[i], &level.signature.ret.ty, &mut eheap) else {
            continue;
        };
        let got = common::conv(&v, &level.signature.ret.ty, &eheap);
        if common::canon(expect.clone(), level.compare) != common::canon(got.clone(), level.compare)
        {
            out.push(format!(
                "{}: case {i}: oracle says {expect:?} but the compiled reference gives {got:?}",
                level.id
            ));
            break;
        }
    }
    if !any && !rep.cases.is_empty() {
        out.push(format!("{}: no oracle for this level", level.id));
    }
    out
}

#[test]
fn compiled_reference_and_naive_solutions_agree_with_the_level_data() {
    let compilers = available_compilers();
    if compilers.is_empty() {
        eprintln!("skipping: neither g++ nor clang++ is installed");
        return;
    }
    let names: Vec<_> = compilers.iter().map(|c| c.exe.clone()).collect();
    let full = env_flag("ALGOCLERK_DIFF_FULL");
    let filter = std::env::var("ALGOCLERK_LEVEL").ok();
    let campaign = Campaign::shipped();
    let levels: Vec<&Level> = campaign
        .all_levels()
        .filter(|l| filter.as_ref().is_none_or(|f| f == &l.id))
        .collect();
    assert!(!levels.is_empty(), "no level to check");
    let dir = std::env::temp_dir().join(format!("algoclerk-diff-{}", std::process::id()));
    let next = AtomicUsize::new(0);
    let problems: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let notes: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..THREADS.min(levels.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(level) = levels.get(i) else { break };
                    let mut o = DiffOpts::new(dir.clone());
                    o.compilers = compilers.clone();
                    if full {
                        o.large_cap = None;
                    }
                    let rep = diff_level(level, &o);
                    let mut mine = rep.problems.clone();
                    if rep.problems.is_empty() {
                        mine.extend(oracle_problems(level, &rep));
                    }
                    if let Ok(mut n) = notes.lock() {
                        n.push(format!(
                            "{}: {} cases{}",
                            level.id,
                            rep.cases.len(),
                            if rep.notes.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", rep.notes.join("; "))
                            }
                        ));
                    }
                    if let Ok(mut p) = problems.lock() {
                        p.extend(mine.into_iter().map(|m| format!("{}: {m}", level.id)));
                    }
                }
            });
        }
    });
    if !env_flag("ALGOCLERK_DIFF_KEEP") {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let notes = notes.into_inner().unwrap_or_default();
    eprintln!(
        "checked {} levels with {}{}:\n  {}",
        levels.len(),
        names.join(" and "),
        if full { " (full large tests)" } else { "" },
        notes.join("\n  ")
    );
    let problems = problems.into_inner().unwrap_or_default();
    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems
            .iter()
            .map(|p| p.chars().take(1800).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n---\n")
    );
}
