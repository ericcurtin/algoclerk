// SPDX-License-Identifier: Apache-2.0
//! Compile the generated C, C++ and Rust of every reference solution, run it on the level's
//! tests through a generated harness, and compare with the interpreter.

use algoclerk_core::codegen::compile::{Compiler, available, compile_and_run};
use algoclerk_core::codegen::{Lang, generate, harness};
use algoclerk_core::judge::{Judge, canonical};
use algoclerk_core::lang::run::compile;
use algoclerk_levels::Campaign;
use std::path::PathBuf;
use std::sync::Mutex;

fn scratch() -> PathBuf {
    let d = std::env::temp_dir().join(format!("algoclerk-cg-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn check_level(level: &algoclerk_core::level::Level, lang: Lang, compilers: &[Compiler], dir: &PathBuf) -> Vec<String> {
    let mut problems = Vec::new();
    let judge = Judge::new(level).expect("judge");
    let sol = compile(&level.reference, &level.check_opts()).expect("reference compiles");
    let g = generate(&sol, lang, &level.signature.name);
    let cases: Vec<_> = judge.suite.iter().filter(|c| !c.large).collect();
    let args: Vec<Vec<String>> = cases.iter().map(|c| c.args.clone()).collect();
    let src = match harness::program(lang, &level.signature, &g, &args) {
        Ok(s) => s,
        Err(e) => return vec![format!("{} {}: harness: {e}", level.id, lang.name())],
    };
    for comp in compilers {
        let tag = format!("{}-{}", level.id, lang.id());
        match compile_and_run(comp, &src, dir, &tag, false) {
            Err(e) => problems.push(format!("{} [{}]: {e}", level.id, comp.exe)),
            Ok(out) => {
                let lines: Vec<&str> = out.lines().collect();
                if lines.len() != cases.len() {
                    problems.push(format!("{} [{}]: {} output lines for {} cases", level.id, comp.exe, lines.len(), cases.len()));
                    continue;
                }
                for (c, l) in cases.iter().zip(lines) {
                    let got = canonical(l, &level.signature.ret, level.compare);
                    if got != c.expect {
                        problems.push(format!("{} [{}]: args {:?}: expected {} got {}", level.id, comp.exe, c.args, c.expect, got));
                        break;
                    }
                }
            }
        }
    }
    problems
}

fn run_lang(lang: Lang) {
    let compilers = available(lang);
    if compilers.is_empty() {
        eprintln!("skipping {}: no compiler installed", lang.name());
        return;
    }
    let campaign = Campaign::shipped();
    let levels: Vec<_> = campaign.all_levels().cloned().collect();
    let dir = scratch();
    let problems = Mutex::new(Vec::new());
    let next = Mutex::new(0usize);
    let filter = std::env::var("ALGOCLERK_LEVEL").ok();
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| loop {
                let i = {
                    let mut n = next.lock().unwrap();
                    let i = *n;
                    *n += 1;
                    i
                };
                let Some(l) = levels.get(i) else { break };
                if filter.as_ref().is_some_and(|f| f != &l.id) {
                    continue;
                }
                let p = check_level(l, lang, &compilers, &dir);
                problems.lock().unwrap().extend(p);
            });
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
    let problems = problems.into_inner().unwrap();
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n---\n"));
}

#[test]
fn cpp_matches_interpreter() {
    run_lang(Lang::Cpp);
}

#[test]
fn rust_matches_interpreter() {
    run_lang(Lang::Rust);
}

#[test]
fn c_matches_interpreter() {
    run_lang(Lang::C);
}
