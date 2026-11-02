// SPDX-License-Identifier: Apache-2.0
//! Compile the generated C, C++ and Rust of every shipped reference solution with the host
//! compilers and check that it behaves exactly like the interpreter on every test case.
//!
//! A language whose compiler is missing is skipped with a message (CI installs them all).

#![cfg(feature = "compile-tests")]

use algoclerk_core::codegen::compile::{self, Compiler};
use algoclerk_core::codegen::harness::HarnessCase;
use algoclerk_core::codegen::{Lang, Options};
use algoclerk_core::judge::Judge;
use algoclerk_levels::Campaign;
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("codegen")
        .join(name)
}

fn verify_all_levels(lang: Lang, force_generic: bool) {
    let compilers = compile::available(lang);
    if compilers.is_empty() {
        eprintln!("SKIPPED: no compiler for {} on this machine", lang.title());
        return;
    }
    let campaign = Campaign::shipped();
    let levels: Vec<_> = campaign.all_levels().collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .min(8);
    let errors = std::sync::Mutex::new(Vec::<String>::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(level) = levels.get(i) else { break };
                    let program = level.reference_program().unwrap();
                    let tests = Judge::new(level).unwrap().tests().to_vec();
                    let cases: Vec<HarnessCase> = tests
                        .iter()
                        .map(|t| HarnessCase {
                            inbox: t.inbox.clone(),
                            floor: t.floor.clone(),
                        })
                        .collect();
                    let expected: Vec<Vec<i32>> =
                        tests.iter().map(|t| t.expected.clone()).collect();
                    let opts = Options {
                        force_generic,
                        ..Options::default()
                    };
                    let item =
                        compile::prepare(&program, lang, Some(level), &opts, cases, expected);
                    for c in &compilers {
                        let stem = format!(
                            "{}_{}_{}_{}",
                            level.id,
                            lang.key(),
                            c.exe.replace('+', "p"),
                            force_generic
                        );
                        let d = dir(&format!(
                            "{}-{}",
                            lang.key(),
                            if force_generic { "generic" } else { "meta" }
                        ));
                        // Reference solutions must compile without any warnings.
                        if let Err(e) =
                            compile::verify(c, std::slice::from_ref(&item), &d, &stem, true)
                        {
                            errors
                                .lock()
                                .unwrap()
                                .push(format!("level {}: {e}", level.id));
                        }
                    }
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    assert!(
        errors.is_empty(),
        "{} failures:\n{}",
        errors.len(),
        errors.join("\n\n")
    );
}

#[test]
fn c_reference_solutions_match_the_interpreter_with_level_signatures() {
    verify_all_levels(Lang::C, false);
}

#[test]
fn cpp_reference_solutions_match_the_interpreter_with_level_signatures() {
    verify_all_levels(Lang::Cpp, false);
}

#[test]
fn rust_reference_solutions_match_the_interpreter_with_level_signatures() {
    verify_all_levels(Lang::Rust, false);
}

#[test]
fn c_generic_inbox_outbox_form_matches() {
    verify_all_levels(Lang::C, true);
}

#[test]
fn cpp_generic_inbox_outbox_form_matches() {
    verify_all_levels(Lang::Cpp, true);
}

#[test]
fn rust_generic_inbox_outbox_form_matches() {
    verify_all_levels(Lang::Rust, true);
}

#[test]
fn most_reference_solutions_are_recovered_as_structured_code() {
    let campaign = Campaign::shipped();
    let mut structured = 0;
    let mut total = 0;
    for level in campaign.all_levels() {
        let program = level.reference_program().unwrap();
        let g =
            algoclerk_core::codegen::generate(&program, Lang::C, Some(level), &Options::default());
        total += 1;
        structured += usize::from(g.structured);
    }
    assert!(
        structured * 10 >= total * 8,
        "only {structured} of {total} reference solutions were structured"
    );
}

#[allow(dead_code)]
fn _unused(_: &Compiler) {}
