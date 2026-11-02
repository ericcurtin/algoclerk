// SPDX-License-Identifier: Apache-2.0
//! End-to-end tests of the command line: the binary is run for real, with a kill timeout.

use std::io::Write;
use std::process::{Command, Stdio};

fn cli(args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    let exe = env!("CARGO_BIN_EXE_algoclerk-cli");
    let mut cmd = if cfg!(unix)
        && Command::new("timeout")
            .arg("--version")
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    {
        let mut c = Command::new("timeout");
        c.args(["-s", "KILL", "120", exe]);
        c
    } else {
        Command::new(exe)
    };
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn algoclerk-cli");
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const ECHO: &str = "again:\n INBOX\n OUTBOX\n JUMP again\n";

#[test]
fn list_shows_all_packs_and_json_parses() {
    let (code, out, _) = cli(&["list"], None);
    assert_eq!(code, 0);
    assert!(out.contains("Mail Room") && out.contains("Executive Floor"));
    let (_, json, _) = cli(&["list", "--json"], None);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v["level_count"].as_u64().unwrap() >= 40);
}

#[test]
fn judge_accepts_a_correct_solution_and_rejects_a_wrong_one() {
    let (code, out, _) = cli(&["judge", "--level", "echo", "--solution", "-"], Some(ECHO));
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("All test cases passed"));
    let (code, out, _) = cli(
        &["judge", "--level", "echo", "--solution", "-", "--json"],
        Some("INBOX\nOUTBOX\n"),
    );
    assert_eq!(code, 1);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["passed"], false);
    assert!(v["first_failure"].is_object());
}

#[test]
fn judge_all_references_passes() {
    let (code, out, _) = cli(&["judge", "--all", "--reference"], None);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("0 failed"));
}

#[test]
fn validate_shipped_levels() {
    let (code, out, _) = cli(&["validate"], None);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("with problems"));
}

#[test]
fn run_traces_and_reports_friendly_errors() {
    let (code, out, _) = cli(&["run", "--solution", "-", "--input", "1,2,3"], Some(ECHO));
    assert_eq!(code, 0);
    assert!(out.contains("[1, 2, 3]"));
    let (code, out, _) = cli(&["run", "--solution", "-"], Some("OUTBOX\n"));
    assert_eq!(code, 1);
    assert!(out.contains("hand is empty"), "{out}");
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    let (code, _, err) = cli(
        &["judge", "--level", "echo", "--solution", "-"],
        Some("BOGUS 1\n"),
    );
    assert_eq!(code, 2);
    assert!(err.contains("BOGUS"), "{err}");
    let (code, _, err) = cli(&["judge", "--level", "no-such-level", "--reference"], None);
    assert_eq!(code, 2);
    assert!(err.contains("no level"));
    let (code, _, _) = cli(&["frobnicate"], None);
    assert_eq!(code, 2);
}

#[test]
fn codegen_prints_code_in_every_language_and_gates_unsolved_programs() {
    for (lang, needle) in [("c", "size_t"), ("cpp", "std::vector"), ("rust", "pub fn")] {
        let (code, out, err) = cli(
            &[
                "codegen",
                "--lang",
                lang,
                "--level",
                "echo",
                "--solution",
                "-",
            ],
            Some(ECHO),
        );
        assert_eq!(code, 0, "{err}");
        assert!(out.contains(needle), "{lang}: {out}");
    }
    let (code, _, err) = cli(
        &[
            "codegen",
            "--lang",
            "c",
            "--level",
            "echo",
            "--solution",
            "-",
        ],
        Some("INBOX\n"),
    );
    assert_eq!(code, 2);
    assert!(err.contains("does not solve"), "{err}");
    let (code, out, _) = cli(
        &[
            "codegen",
            "--lang",
            "c",
            "--level",
            "echo",
            "--solution",
            "-",
            "--force",
        ],
        Some("INBOX\n"),
    );
    assert_eq!(code, 0);
    assert!(out.contains("size_t"));
}

#[test]
fn codegen_verify_compiles_and_compares() {
    let (code, out, err) = cli(
        &[
            "codegen",
            "--lang",
            "rust",
            "--level",
            "echo",
            "--reference",
            "--verify",
        ],
        None,
    );
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn export_import_roundtrip() {
    let (code, shared, _) = cli(
        &["export", "--level", "echo", "--solution", "-"],
        Some(ECHO),
    );
    assert_eq!(code, 0);
    assert!(shared.starts_with("-- algoclerk solution v1"));
    let path = std::env::temp_dir().join("algoclerk-cli-test-solution.txt");
    std::fs::write(&path, &shared).unwrap();
    let (code, out, _) = cli(&["import", path.to_str().unwrap()], None);
    assert_eq!(code, 0);
    assert!(out.contains("INBOX"));
    let (code, _, _) = cli(
        &[
            "judge",
            "--level",
            "echo",
            "--solution",
            path.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(code, 0);
}

#[test]
fn tune_suggests_ordered_targets() {
    let (code, out, _) = cli(&["tune", "--level", "echo"], None);
    assert_eq!(code, 0);
    assert!(out.contains("size_ace = 3"));
}
