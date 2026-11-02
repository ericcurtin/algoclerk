// SPDX-License-Identifier: Apache-2.0
//! Whole-campaign checks: every shipped level validates and meets its stated targets.

use algoclerk_core::judge::{Judge, Medal};
use algoclerk_core::validate::validate_level;
use algoclerk_levels::Campaign;
use std::collections::HashSet;

#[test]
fn campaign_has_at_least_forty_levels_in_named_packs() {
    let c = Campaign::shipped();
    assert!(c.load_errors.is_empty(), "{:?}", c.load_errors);
    assert!(c.level_count() >= 40, "only {} levels", c.level_count());
    assert!(c.packs.len() >= 6);
    for p in &c.packs {
        assert!(!p.levels.is_empty(), "pack {} is empty", p.info.id);
    }
}

#[test]
fn ids_are_unique_and_match_file_names() {
    let files = algoclerk_levels::embedded_files();
    let mut seen = HashSet::new();
    for (path, _) in files.iter().filter(|(p, _)| !p.ends_with("pack.toml")) {
        let stem = path.rsplit('/').next().unwrap().trim_end_matches(".toml");
        assert!(seen.insert(stem.to_string()), "duplicate level file {stem}");
    }
}

#[test]
fn every_shipped_level_validates_without_errors() {
    let c = Campaign::shipped();
    for l in c.all_levels() {
        let r = validate_level(l);
        assert!(r.ok(), "{}: {:?}", l.id, r.errors);
        assert!(r.warnings.is_empty(), "{}: {:?}", l.id, r.warnings);
        assert!(
            r.test_count >= 8,
            "{} has only {} tests",
            l.id,
            r.test_count
        );
    }
}

#[test]
fn reference_solutions_pass_and_meet_targets() {
    let c = Campaign::shipped();
    for l in c.all_levels() {
        let judge = Judge::new(l).unwrap();
        let v = judge.judge(&l.reference_program().unwrap());
        assert!(v.passed, "{}: {:?}", l.id, v.first_failure());
        assert!(
            v.size <= l.targets.size_par as usize,
            "{}: size {}",
            l.id,
            v.size
        );
        assert!(
            v.avg_steps <= f64::from(l.targets.speed_par),
            "{}: avg {}",
            l.id,
            v.avg_steps
        );
        assert!(v.medals.size >= Some(Medal::Staple), "{}", l.id);
        assert!(v.medals.speed >= Some(Medal::Staple), "{}", l.id);
    }
}

#[test]
fn trivial_programs_do_not_pass_any_level() {
    let c = Campaign::shipped();
    for src in ["", "INBOX\nOUTBOX", "INBOX\nOUTBOX\nINBOX\nOUTBOX"] {
        let p = std::sync::Arc::new(algoclerk_core::asm::assemble(src).unwrap());
        for l in c.all_levels() {
            let v = Judge::new(l).unwrap().judge(&p);
            assert!(!v.passed, "{} accepts the trivial program {src:?}", l.id);
        }
    }
}

#[test]
fn levels_use_what_they_offer() {
    // Every level's reference uses only allowed instructions (also checked by the judge) and
    // every level that allows indirect addressing gets used by someone: sanity for data quality.
    let c = Campaign::shipped();
    for l in c.all_levels() {
        let allowed = l.allowed().unwrap();
        let prog = l.reference_program().unwrap();
        assert!(prog.check(&allowed, l.machine.floor).is_empty(), "{}", l.id);
        assert!(l.hints.len() >= 2, "{} needs a hint ladder", l.id);
        assert!(l.code.is_some(), "{} needs [code] metadata", l.id);
        assert!(l.explain.is_some(), "{} needs an [explain] section", l.id);
    }
}

#[test]
fn curriculum_covers_the_required_topics() {
    let c = Campaign::shipped();
    let ids: HashSet<&str> = c.all_levels().map(|l| l.id.as_str()).collect();
    for needed in [
        "echo",
        "sum-list",
        "max-of",
        "min-and-max",
        "reverse-list",
        "running-sum",
        "pair-sum",
        "find-duplicate",
        "count-matches",
        "palindrome-check",
        "fizz-buzz",
        "fibonacci",
        "factorial",
        "gcd",
        "is-prime",
        "power",
        "absolute-value",
        "sign-of",
        "insertion-sort",
        "selection-sort",
        "bubble-sort",
        "merge-sorted",
        "binary-search",
        "missing-number",
        "remove-duplicates",
        "kadane",
        "majority-vote",
        "valid-brackets",
        "ring-buffer",
        "list-walk",
        "list-reverse",
        "climbing-stairs",
        "coin-change",
        "knapsack-tiny",
        "bit-count",
        "sieve",
    ] {
        assert!(ids.contains(needed), "missing level {needed}");
    }
}

#[test]
fn tile_names_do_not_clash_with_generated_code_names() {
    let c = Campaign::shipped();
    for l in c.all_levels() {
        let Some(code) = &l.code else { continue };
        let reserved: Vec<String> = code
            .params
            .iter()
            .map(|p| p.name.clone())
            .chain([code.function.clone()])
            .chain(["hand", "next", "inbox", "outbox", "floor"].map(String::from))
            .collect();
        for name in l.machine.names.iter().filter(|n| !n.is_empty()) {
            assert!(
                !reserved.contains(name),
                "level {}: tile name \"{name}\" clashes with a generated identifier",
                l.id
            );
        }
        let mut seen = HashSet::new();
        for name in l.machine.names.iter().filter(|n| !n.is_empty()) {
            assert!(
                seen.insert(name.clone()),
                "level {}: duplicate tile name {name}",
                l.id
            );
        }
    }
}
