// SPDX-License-Identifier: Apache-2.0
//! The shipped campaign: structure, reference solutions, medal targets and naive solutions.
//!
//! Tests that need the whole C++ pipeline (parser, checker, interpreter) skip with a message while those
//! parts are still stubs ("not written yet"). While fewer than [`FULL_CAMPAIGN`] levels are converted to
//! the C++ level format the whole-campaign assertions only print a note.

use algoclerk_core::cpp::value::{Heap, parse_value};
use algoclerk_core::judge::{FailKind, Judge, Medal, raw_suite};
use algoclerk_core::validate::validate_level;
use algoclerk_levels::Campaign;

/// Number of levels the finished campaign has.
const FULL_CAMPAIGN: usize = 80;

fn pipeline_missing(e: &str) -> bool {
    e.contains("not written yet")
}

#[test]
fn campaign_has_the_promised_shape() {
    let c = Campaign::shipped();
    let converted = c.level_count();
    let mut ids = std::collections::HashSet::new();
    for l in c.all_levels() {
        assert!(ids.insert(l.id.clone()), "duplicate id {}", l.id);
        assert!(l.hints.len() >= 3, "{} needs a hint ladder", l.id);
        assert!(l.examples.len() >= 2, "{} needs examples", l.id);
        assert!(
            !l.story.is_empty() && !l.technique.is_empty() && !l.big_o.is_empty(),
            "{}",
            l.id
        );
    }
    if converted < FULL_CAMPAIGN {
        eprintln!(
            "note: {converted} of {FULL_CAMPAIGN} levels are converted to C++ ({} files failed to load: expected while converting)",
            c.load_errors.len()
        );
        return;
    }
    assert!(c.load_errors.is_empty(), "{:?}", c.load_errors);
    assert!(c.packs.len() >= 8);
    // at least a dozen levels declare a deliberately naive solution that must time out
    assert!(c.all_levels().filter(|l| l.naive.is_some()).count() >= 40);
}

/// Everything about a level file that can be checked without a parser, checker or interpreter.
#[test]
fn level_files_are_well_formed() {
    let c = Campaign::shipped();
    for l in c.all_levels() {
        let id = &l.id;
        assert!(
            l.reference.contains(&l.signature.name),
            "{id}: the reference does not define {}",
            l.signature.name
        );
        assert!(l.starter_code().starts_with(&l.signature_text), "{id}");
        assert!(l.starter_code().ends_with("}\n"), "{id}");
        assert!(
            !l.reference.contains('\u{2014}') && !l.statement.contains('\u{2014}'),
            "{id}: em dash"
        );
        let suite = raw_suite(l, None).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert!(!suite.is_empty(), "{id}");
        for t in &suite {
            assert_eq!(t.args.len(), l.signature.params.len(), "{id}: {:?}", t.args);
            let mut heap = Heap::default();
            for (p, a) in l.signature.params.iter().zip(&t.args) {
                parse_value(a, &p.spec.ty, &mut heap).unwrap_or_else(|e| {
                    panic!("{id}: argument {} = {:.80}: {e}", p.name, a);
                });
            }
            if let Some(e) = &t.expect {
                parse_value(e, &l.signature.ret.ty, &mut heap)
                    .unwrap_or_else(|er| panic!("{id}: expected {e:.80}: {er}"));
            }
        }
        // every locked or unlocked name is spelled like C++
        for k in &l.locked {
            assert!(
                k.name.starts_with("std::") || k.name.starts_with('<'),
                "{id}: locked name {}",
                k.name
            );
            assert!(!k.why.is_empty(), "{id}: {} needs a reason", k.name);
        }
        for h in &l.unlocked {
            assert!(h.starts_with('<') || h == "*", "{id}: {h}");
        }
        if !l.scalar_only() {
            assert!(
                l.generate.iter().any(|g| g.large),
                "{id}: no large generator"
            );
        }
    }
}

#[test]
fn every_level_validates() {
    let c = Campaign::shipped();
    let mut problems = Vec::new();
    for l in c.all_levels() {
        problems.extend(validate_level(l));
    }
    if problems.iter().any(|p| pipeline_missing(p)) {
        eprintln!("skipping validation: parser or interpreter not available");
        return;
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn reference_solutions_earn_gold_in_both_medals() {
    for l in Campaign::shipped().all_levels() {
        let j = match Judge::new(l) {
            Ok(j) => j,
            Err(e) if pipeline_missing(&e) => {
                eprintln!("skipping: {e}");
                return;
            }
            Err(e) => panic!("{}: {e}", l.id),
        };
        let v = j.reference_verdict();
        assert!(v.passed, "{}: {:?}", l.id, v.failure);
        assert_eq!(v.medals.size, Medal::Gold, "{}", l.id);
        assert_eq!(v.medals.speed, Medal::Gold, "{}", l.id);
        assert_eq!(v.medals.stars(), 3);
    }
}

#[test]
fn locked_tools_really_are_locked() {
    let c = Campaign::shipped();
    let Some(l) = c.find("sum-the-bills") else {
        eprintln!("note: sum-the-bills is not converted yet");
        return;
    };
    let j = match Judge::new(l) {
        Ok(j) => j,
        Err(e) if pipeline_missing(&e) => {
            eprintln!("skipping: {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let v = j.judge(
        "#include <numeric>\nint sumTheBills(const std::vector<int>& bills) {\n    return std::accumulate(bills.begin(), bills.end(), 0);\n}\n",
    );
    assert!(!v.passed);
    assert!(v.diags[0].msg.contains("does not stock"), "{:?}", v.diags);
}

#[test]
fn a_brute_force_answer_is_a_time_limit_not_a_wrong_answer() {
    let c = Campaign::shipped();
    let Some(l) = c.find("two-sum") else {
        eprintln!("note: two-sum is not converted yet");
        return;
    };
    let j = match Judge::new(l) {
        Ok(j) => j,
        Err(e) if pipeline_missing(&e) => {
            eprintln!("skipping: {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let v = j.judge(l.naive.as_ref().expect("two-sum has a naive solution"));
    assert!(!v.passed);
    let f = v.failure.unwrap();
    assert_eq!(f.kind, FailKind::TimeLimit);
    assert!(f.case.large);
}

/// A throw-away level whose reference is order independent, to test the judge's unordered check.
const KEYS_LEVEL: &str = r##"
title = "Keys"
order = 1
story = "s"
statement = "Return the distinct numbers in order of first appearance."
prelude = "#include <vector>"
signature = "std::vector<int> distinctKeys(const std::vector<int>& nums)"
technique = "t"
big_o = "O(n) time, O(n) space"
unlocked = ["<unordered_set>"]
hints = ["a", "b", "c"]
reference = """
std::vector<int> distinctKeys(const std::vector<int>& nums) {
    std::vector<int> out;
    for (int x : nums) {
        bool seen = false;
        for (int y : out) {
            if (y == x) {
                seen = true;
            }
        }
        if (!seen) {
            out.push_back(x);
        }
    }
    return out;
}
"""
[medals]
size = [100, 80, 60]
speed = [1000, 800, 600]
[[examples]]
args = ["{3, 1, 2, 3}"]
expect = "{3, 1, 2}"
[[examples]]
args = ["{}"]
expect = "{}"
[[tests]]
args = ["{5, 4, 5, 4}"]
expect = "{5, 4}"
"##;

#[test]
fn an_answer_that_depends_on_unordered_iteration_order_is_rejected() {
    let level = algoclerk_core::level::Level::from_toml(KEYS_LEVEL, "t").unwrap();
    let j = match Judge::new(&level) {
        Ok(j) => j,
        Err(e) if pipeline_missing(&e) => {
            eprintln!("skipping: {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    // Correct only when the unordered_set happens to iterate in insertion order.
    let src = "#include <unordered_set>\nstd::vector<int> distinctKeys(const std::vector<int>& nums) {\n    std::unordered_set<int> seen(nums.begin(), nums.end());\n    std::vector<int> out;\n    for (int x : seen) {\n        out.push_back(x);\n    }\n    return out;\n}\n";
    let v = j.judge(src);
    assert!(v.diags.is_empty(), "{:?}", v.diags);
    assert!(!v.passed);
    let f = v.failure.unwrap();
    assert_eq!(f.kind, FailKind::UnorderedOrder, "{f:?}");
    assert!(f.kind.friendly().contains("unordered"));
}

#[test]
fn complexity_estimates_match_the_declared_classes() {
    let c = Campaign::shipped();
    for (id, time) in [
        ("two-sum", "O(n)"),
        ("binary-search", "O(log n)"),
        ("merge-sort", "O(n log n)"),
        ("edit-distance", "O(n^2)"),
    ] {
        let Some(l) = c.find(id) else { continue };
        let j = match Judge::new(l) {
            Ok(j) => j,
            Err(e) if pipeline_missing(&e) => {
                eprintln!("skipping: {e}");
                return;
            }
            Err(e) => panic!("{id}: {e}"),
        };
        let v = j.reference_verdict();
        let cx = v.complexity.unwrap_or_else(|| panic!("{id}: no estimate"));
        assert_eq!(cx.time.to_string(), time, "{id}: {:?}", cx.samples);
    }
}

#[test]
fn constant_space_solutions_are_measured_constant_whatever_the_loop_style() {
    let c = Campaign::shipped();
    let Some(l) = c.find("missing-number") else {
        return;
    };
    let j = Judge::new(l).expect("level compiles");
    for (name, body) in [
        (
            "range for, total declared first",
            "const int n = static_cast<int>(nums.size());\n    int total = n * (n + 1) / 2;\n    for (int x : nums) { total -= x; }\n    return total;",
        ),
        (
            "range for, sum then subtract",
            "int n = static_cast<int>(nums.size()); int total = 0;\n    for (int x : nums) { total += x; }\n    return n * (n + 1) / 2 - total;",
        ),
        (
            "range for, spelled out",
            "const int n = static_cast<int>(nums.size());\n    int total = n * (n + 1) / 2;\n    for (int x : nums) { total = total - x; }\n    return total;",
        ),
        (
            "index loop",
            "const int n = static_cast<int>(nums.size());\n    int total = n * (n + 1) / 2;\n    for (int i = 0; i < n; ++i) { total -= nums[i]; }\n    return total;",
        ),
    ] {
        let src = format!("int missingNumber(const std::vector<int>& nums) {{\n    {body}\n}}\n");
        let v = j.judge(&src);
        assert!(v.passed, "{name}: {:?} {:?}", v.failure, v.diags);
        let cx = v.complexity.unwrap_or_else(|| panic!("{name}: no estimate"));
        assert_eq!(cx.space.to_string(), "O(1)", "{name}: {:?}", cx.samples);
    }
}

#[test]
fn linear_space_is_linear_at_every_sweep_start_size() {
    let c = Campaign::shipped();
    for id in ["common-elements", "rotate-shelf"] {
        let Some(l) = c.find(id) else { continue };
        let mut l = l.clone();
        for n0 in [8, 16, 64, 256, 1024] {
            l.budgets.sweep_n0 = n0;
            let j = Judge::new(&l).expect("level compiles");
            let v = j.reference_verdict();
            let cx = v.complexity.unwrap_or_else(|| panic!("{id}: no estimate"));
            assert_eq!(cx.space.to_string(), "O(n)", "{id} at n0 = {n0}: {:?}", cx.samples);
        }
    }
}

#[test]
fn a_locked_header_shows_the_reason_of_its_lock() {
    let c = Campaign::shipped();
    let Some(l) = c.find("binary-search") else {
        return;
    };
    let j = Judge::new(l).expect("level compiles");
    let v = j.judge(
        "#include <vector>\n#include <algorithm>\nint binarySearch(const std::vector<int>& nums, int target) { return -1; }\n",
    );
    assert!(!v.passed);
    let msg = &v.diags[0].msg;
    assert!(
        msg.contains("does not stock `<algorithm>`") && msg.contains("search the shelf for you"),
        "{msg}"
    );
    // naming a locked function without the header shows its lock too
    let v = j.judge(
        "int binarySearch(const std::vector<int>& nums, int target) { auto it = std::lower_bound(nums.begin(), nums.end(), target); return static_cast<int>(it - nums.begin()); }\n",
    );
    assert!(!v.passed);
    assert!(v.diags[0].msg.contains("Same family of tools"), "{:?}", v.diags);
}
