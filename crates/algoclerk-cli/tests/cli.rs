// SPDX-License-Identifier: Apache-2.0
//! End-to-end tests of the command line tool.

use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_algoclerk-cli"))
        .args(args)
        .output()
        .expect("run algoclerk-cli")
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)
}

fn temp(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("algoclerk-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

#[test]
fn list_and_show() {
    let o = cli(&["list"]);
    assert!(o.status.success());
    let t = text(&o);
    assert!(
        t.contains("two-sum") && t.contains("80 levels in 10 wings"),
        "{t}"
    );
    let o = cli(&["show", "two-sum"]);
    let t = text(&o);
    assert!(
        t.contains("fn two_sum(nums: vec<int>, target: int) -> vec<int>")
            && t.contains("Hash map lookup"),
        "{t}"
    );
    assert!(!cli(&["show", "no-such-level"]).status.success());
}

#[test]
fn judge_accepts_the_reference_and_rejects_a_wrong_answer() {
    let reference = cli(&["reference", "sum-the-bills"]);
    let good = temp("good.ck", &String::from_utf8_lossy(&reference.stdout));
    let o = cli(&["judge", "sum-the-bills", good.to_str().unwrap()]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("ACCEPTED"));
    let bad = temp(
        "bad.ck",
        "fn sum_the_bills(bills: vec<int>) -> int { return 0; }",
    );
    let o = cli(&["judge", "sum-the-bills", bad.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(text(&o).contains("WRONG ANSWER"), "{}", text(&o));
    let locked = temp(
        "locked.ck",
        "fn sum_the_bills(bills: vec<int>) -> int { return sum(bills); }",
    );
    let o = cli(&["judge", "sum-the-bills", locked.to_str().unwrap(), "--json"]);
    assert!(text(&o).contains("locked"), "{}", text(&o));
}

#[test]
fn sandbox_run_and_check() {
    let f = temp(
        "sandbox.ck",
        "fn main(v: vec<int>) -> int { let t = 0; for x in v { t += x; } return t; }",
    );
    let o = cli(&["run", f.to_str().unwrap(), "main", "[1, 2, 3]"]);
    assert!(o.status.success());
    assert!(text(&o).trim_start().starts_with('6'), "{}", text(&o));
    let broken = temp("broken.ck", "fn main() -> int { return \"x\"; }");
    let o = cli(&["check", broken.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(text(&o).contains("Mix-up"), "{}", text(&o));
}

#[test]
fn validate_one_level_and_codegen_in_every_language() {
    let o = cli(&["validate", "valid-brackets"]);
    assert!(o.status.success(), "{}", text(&o));
    for (lang, needle) in [
        ("c", "Vec_char"),
        ("cpp", "std::vector"),
        ("rust", "fn is_balanced"),
    ] {
        let o = cli(&["codegen", "valid-brackets", "--lang", lang]);
        assert!(o.status.success(), "{lang}: {}", text(&o));
        assert!(text(&o).contains(needle), "{lang}: {}", text(&o));
    }
}

#[test]
fn codegen_verify_compares_with_the_interpreter() {
    let o = cli(&["codegen", "fizz-memo", "--lang", "rust", "--verify"]);
    let t = text(&o);
    // a missing compiler is skipped, never a failure
    assert!(o.status.success() || t.contains("FAIL"), "{t}");
    assert!(!t.contains("FAIL"), "{t}");
}
