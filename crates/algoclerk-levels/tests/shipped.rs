// SPDX-License-Identifier: Apache-2.0
//! The shipped campaign: structure, reference solutions, medal targets and naive solutions.

use algoclerk_core::judge::{Judge, Medal};
use algoclerk_core::validate::validate_level;
use algoclerk_levels::Campaign;

#[test]
fn campaign_has_the_promised_shape() {
    let c = Campaign::shipped();
    assert!(c.load_errors.is_empty(), "{:?}", c.load_errors);
    assert!(c.level_count() >= 60, "{} levels", c.level_count());
    assert!(c.packs.len() >= 8);
    let mut ids = std::collections::HashSet::new();
    for l in c.all_levels() {
        assert!(ids.insert(l.id.clone()), "duplicate id {}", l.id);
        assert!(l.hints.len() >= 3, "{} needs a hint ladder", l.id);
        assert!(l.examples.len() >= 2, "{} needs examples", l.id);
        assert!(!l.story.is_empty() && !l.technique.is_empty() && !l.big_o.is_empty(), "{}", l.id);
    }
    // at least a dozen levels declare a deliberately naive solution that must time out
    assert!(c.all_levels().filter(|l| l.naive.is_some()).count() >= 40);
}

#[test]
fn every_level_validates() {
    let c = Campaign::shipped();
    let mut problems = Vec::new();
    for l in c.all_levels() {
        problems.extend(validate_level(l));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn reference_solutions_earn_gold_in_both_medals() {
    for l in Campaign::shipped().all_levels() {
        let v = Judge::new(l).unwrap().reference_verdict();
        assert!(v.passed, "{}", l.id);
        assert_eq!(v.medals.size, Medal::Gold, "{}", l.id);
        assert_eq!(v.medals.speed, Medal::Gold, "{}", l.id);
        assert_eq!(v.medals.stars(), 3);
    }
}

#[test]
fn locked_tools_really_are_locked() {
    let c = Campaign::shipped();
    let l = c.find("sum-the-bills").unwrap();
    let j = Judge::new(l).unwrap();
    let v = j.judge("fn sum_the_bills(bills: vec<int>) -> int { return sum(bills); }");
    assert!(!v.passed);
    assert!(v.diags[0].msg.contains("locked"), "{:?}", v.diags);
}

#[test]
fn a_brute_force_answer_is_a_time_limit_not_a_wrong_answer() {
    let c = Campaign::shipped();
    let l = c.find("two-sum").unwrap();
    let j = Judge::new(l).unwrap();
    let v = j.judge(l.naive.as_ref().unwrap());
    assert!(!v.passed);
    let f = v.failure.unwrap();
    assert_eq!(f.kind, algoclerk_core::judge::FailKind::TimeLimit);
    assert!(f.case.large);
}

#[test]
fn complexity_estimates_match_the_declared_classes() {
    let c = Campaign::shipped();
    for (id, time) in [("two-sum", "O(n)"), ("binary-search", "O(log n)"), ("merge-sort", "O(n log n)"), ("edit-distance", "O(n^2)")] {
        let l = c.find(id).unwrap();
        let v = Judge::new(l).unwrap().reference_verdict();
        let cx = v.complexity.unwrap_or_else(|| panic!("{id}: no estimate"));
        assert_eq!(cx.time.to_string(), time, "{id}: {:?}", cx.samples);
    }
}
