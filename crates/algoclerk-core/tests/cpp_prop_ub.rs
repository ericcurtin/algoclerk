// SPDX-License-Identifier: Apache-2.0
//! Negative table: one or more programs per undefined behaviour class. Each must make the interpreter stop
//! with the expected `RtKind` (or the checker report the expected code) at the line of the offending
//! statement (marked `// @UB` in the case text). Cases flagged `san` are also compiled with
//! `-fsanitize=undefined,address -D_GLIBCXX_ASSERTIONS` by a real compiler (when a sanitizer runtime is
//! installed) and must fail there too. Complements `cpp_interp_ub.rs` with different programs: UB inside
//! helpers, lambdas, comparators, loops and with runtime arguments.

mod prop_common;

use prop_common::ub::{Case, full_source, run_case, sanitizer_rejects};
use prop_common::{ubc1, ubc2, ubc3, work_dir};

fn all() -> Vec<Case> {
    let mut v = ubc1::cases();
    v.extend(ubc2::cases());
    v.extend(ubc3::cases());
    v
}

#[test]
fn case_names_are_unique() {
    let mut names: Vec<&str> = all().iter().map(|c| c.name).collect();
    names.sort_unstable();
    let n = names.len();
    names.dedup();
    assert_eq!(n, names.len(), "duplicate case names");
}

#[test]
fn interpreter_and_checker_report_every_class_at_the_right_line() {
    let mut bad = Vec::new();
    for c in all() {
        match (run_case(&c), prop_common::ub::ub_known(c.name)) {
            (Err(e), false) => bad.push(format!("[{}] {e}", c.name)),
            (Ok(()), true) => eprintln!(
                "UB_KNOWN entry `{}` now passes: remove it from the list",
                c.name
            ),
            _ => {}
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases wrong:\n{}",
        bad.len(),
        all().len(),
        bad.join("\n")
    );
}

#[test]
fn well_defined_sequencing_is_not_flagged() {
    use algoclerk_core::cpp::check::CheckOpts;
    use algoclerk_core::cpp::interp::Limits;
    use algoclerk_core::cpp::run::compile;
    let mut bad = Vec::new();
    for (name, src) in ubc3::SEQUENCED_OK {
        match compile("", src, &CheckOpts::all()) {
            Err(ds) => bad.push(format!("[{name}] rejected: {}", ds[0].msg)),
            Ok(c) => match c.call("f", &[], &Limits::default()) {
                Ok(r) if r.report.result.is_ok() => {}
                Ok(r) => bad.push(format!(
                    "[{name}] runtime error {:?}",
                    r.report.result.err()
                )),
                Err(e) => bad.push(format!("[{name}] {e}")),
            },
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn real_compilers_with_sanitizers_reject_the_flagged_cases() {
    let dir = work_dir("prop_ub");
    // control: a program without undefined behaviour must run cleanly under the sanitizers
    let control = prop_common::ub::c(
        "control_clean",
        prop_common::ub::Expect::Diag("none"),
        &[],
        true,
        "int f() {\n    std::vector<int> v = {1, 2, 3};\n    return v[2];\n}\n",
    );
    if let Ok(Some(rejected)) = sanitizer_rejects(&control, &dir) {
        assert!(!rejected, "the sanitizer build of a clean program failed");
    }
    let cases: Vec<Case> = all().into_iter().filter(|c| c.san).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::<String>::new());
    let skipped = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|sc| {
        for _ in 0..6 {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(c) = cases.get(i) else { break };
                    match sanitizer_rejects(c, &dir) {
                        Ok(Some(true)) => {}
                        Ok(Some(false)) => out.lock().expect("out").push(format!(
                            "[{}] the sanitizer build ran without complaint:\n{}",
                            c.name,
                            full_source(c)
                                .lines()
                                .skip(15)
                                .collect::<Vec<_>>()
                                .join("\n")
                        )),
                        Ok(None) => skipped.store(true, std::sync::atomic::Ordering::SeqCst),
                        Err(e) => out.lock().expect("out").push(format!("[{}] {e}", c.name)),
                    }
                }
            });
        }
    });
    if skipped.load(std::sync::atomic::Ordering::SeqCst) {
        eprintln!("no compiler with a sanitizer runtime: sanitizer sanity check skipped");
    }
    let out = out.into_inner().expect("out");
    assert!(out.is_empty(), "{}", out.join("\n"));
}
