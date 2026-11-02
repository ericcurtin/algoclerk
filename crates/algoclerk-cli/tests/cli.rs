// SPDX-License-Identifier: Apache-2.0
//! End-to-end tests of the command line tool.
//!
//! Commands that need the whole C++ pipeline (parser, checker, interpreter) skip with a message while
//! those parts are still stubs ("not written yet").

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

fn stubbed(o: &Output) -> bool {
    let t = text(o);
    let missing = t.contains("not written yet");
    if missing {
        eprintln!("skipping: the C++ pipeline is not available yet");
    }
    missing
}

fn temp(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("algoclerk-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

#[test]
fn list_show_starter_and_reference() {
    let o = cli(&["list"]);
    assert!(o.status.success());
    let t = text(&o);
    assert!(t.contains("two-sum") && t.contains(" levels in "), "{t}");
    let o = cli(&["show", "two-sum"]);
    let t = text(&o);
    assert!(
        t.contains("std::vector<int> twoSum(const std::vector<int>& nums, int target)")
            && t.contains("Hash map lookup")
            && t.contains("<unordered_map>"),
        "{t}"
    );
    assert!(!cli(&["show", "no-such-level"]).status.success());
    let o = cli(&["starter", "two-sum"]);
    assert_eq!(
        String::from_utf8_lossy(&o.stdout),
        "std::vector<int> twoSum(const std::vector<int>& nums, int target) {\n}\n"
    );
    let o = cli(&["reference", "stamp-duty"]);
    assert!(text(&o).contains("int stampDuty(int price, int qty)"));
}

#[test]
fn judge_accepts_the_reference_and_rejects_a_wrong_answer() {
    let reference = cli(&["reference", "sum-the-bills"]);
    let good = temp("good.cpp", &String::from_utf8_lossy(&reference.stdout));
    let o = cli(&["judge", "sum-the-bills", good.to_str().unwrap()]);
    if stubbed(&o) {
        return;
    }
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("ACCEPTED"));
    let bad = temp(
        "bad.cpp",
        "#include <vector>\nint sumTheBills(const std::vector<int>& bills) { return 0; }\n",
    );
    let o = cli(&["judge", "sum-the-bills", bad.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(text(&o).contains("WRONG ANSWER"), "{}", text(&o));
    let locked = temp(
        "locked.cpp",
        "#include <numeric>\nint sumTheBills(const std::vector<int>& bills) { return std::accumulate(bills.begin(), bills.end(), 0); }\n",
    );
    let o = cli(&["judge", "sum-the-bills", locked.to_str().unwrap(), "--json"]);
    assert!(text(&o).contains("does not stock"), "{}", text(&o));
}

#[test]
fn sandbox_run_and_check() {
    let f = temp(
        "sandbox.cpp",
        "#include <vector>\nint total(const std::vector<int>& v) {\n    int t = 0;\n    for (int x : v) {\n        t += x;\n    }\n    return t;\n}\n",
    );
    let o = cli(&["run", f.to_str().unwrap(), "total", "{1, 2, 3}"]);
    if stubbed(&o) {
        return;
    }
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).trim_start().starts_with('6'), "{}", text(&o));
    let broken = temp("broken.cpp", "int main() { return \"x\" + 1; }\n");
    let o = cli(&["check", broken.to_str().unwrap()]);
    assert!(!o.status.success());
}

#[test]
fn harness_prints_a_complete_test_program() {
    let o = cli(&["harness", "stamp-duty"]);
    assert!(o.status.success(), "{}", text(&o));
    let t = text(&o);
    assert!(
        t.contains("int stampDuty(int price, int qty)") && t.contains("int main()"),
        "{t}"
    );
}

#[test]
fn diff_compares_a_level_with_the_real_compilers() {
    let o = cli(&["diff", "two-sum", "--no-sanitize"]);
    let t = text(&o);
    // a missing compiler is skipped, never a failure
    assert!(o.status.success(), "{t}");
    assert!(t.contains("ok") || t.contains("nothing to compare"), "{t}");
}

#[test]
fn codegen_prints_cpp_and_is_polite_about_other_languages() {
    let o = cli(&["codegen", "two-sum", "--lang", "cpp"]);
    if !stubbed(&o) {
        assert!(o.status.success(), "{}", text(&o));
        assert!(text(&o).contains("twoSum"), "{}", text(&o));
    }
    for lang in ["c", "rust"] {
        let o = cli(&["codegen", "two-sum", "--lang", lang]);
        assert!(o.status.success(), "{lang}: {}", text(&o));
        assert!(
            text(&o).contains("not available yet"),
            "{lang}: {}",
            text(&o)
        );
    }
}
