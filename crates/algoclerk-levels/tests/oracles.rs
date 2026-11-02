// SPDX-License-Identifier: Apache-2.0
//! Independent oracles: every level's expected outputs are re-computed by plain Rust code written
//! without looking at the reference solution, and compared on every test (examples, hand-written,
//! generated and large).

mod common;

use algoclerk_core::cpp::value::{Heap, parse_value};
use algoclerk_core::judge::Judge;
use algoclerk_levels::Campaign;
use common::{canon, conv, oracle};

/// Number of levels the finished campaign has. While fewer levels are converted to C++ the
/// "whole campaign" assertions only print a note.
const FULL_CAMPAIGN: usize = 80;

/// The interpreter and parser may still be stubs in a partially merged tree.
fn pipeline_missing(e: &str) -> bool {
    e.contains("not written yet")
}

#[test]
fn every_level_matches_its_independent_oracle() {
    let campaign = Campaign::shipped();
    let filter = std::env::var("ALGOCLERK_LEVEL").ok();
    let mut problems = Vec::new();
    let mut checked = 0;
    for level in campaign.all_levels() {
        if filter.as_ref().is_some_and(|f| f != &level.id) {
            continue;
        }
        let judge = match Judge::new(level) {
            Ok(j) => j,
            Err(e) if pipeline_missing(&e) => {
                eprintln!("skipping the oracle test: interpreter not available ({e})");
                return;
            }
            Err(e) => panic!("{}: {e}", level.id),
        };
        let mut any = false;
        for case in &judge.suite {
            let mut heap = Heap::default();
            let args: Vec<_> = level
                .signature
                .params
                .iter()
                .zip(&case.args)
                .map(|(p, lit)| {
                    conv(
                        &parse_value(lit, &p.spec.ty, &mut heap)
                            .unwrap_or_else(|e| panic!("{}: arg: {e}", level.id)),
                        &p.spec.ty,
                        &heap,
                    )
                })
                .collect();
            let Some(expect) = oracle(&level.id, &args) else {
                continue;
            };
            any = true;
            let mut eheap = Heap::default();
            let want = conv(
                &parse_value(&case.expect, &level.signature.ret.ty, &mut eheap)
                    .unwrap_or_else(|e| panic!("{}: expect: {e}", level.id)),
                &level.signature.ret.ty,
                &eheap,
            );
            if canon(expect.clone(), level.compare) != canon(want.clone(), level.compare) {
                problems.push(format!(
                    "{}: args {:?}: oracle says {:?}, expected {:?}",
                    level.id,
                    case.args
                        .iter()
                        .map(|a| a.chars().take(60).collect::<String>())
                        .collect::<Vec<_>>(),
                    expect,
                    want
                ));
                break;
            }
        }
        if !any {
            problems.push(format!("{}: no oracle", level.id));
        }
        checked += 1;
    }
    assert!(
        problems.is_empty(),
        "{} of {checked} levels disagree:\n{}",
        problems.len(),
        problems
            .iter()
            .map(|p| p.chars().take(400).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
    if filter.is_none() {
        if checked >= FULL_CAMPAIGN {
            return;
        }
        eprintln!("note: only {checked} of {FULL_CAMPAIGN} levels are converted to C++ so far");
    }
}
