// SPDX-License-Identifier: Apache-2.0
//! Compile-and-compare for the code reveal: every shipped level's reference solution (and naive solution
//! when present) is translated to Rust and C, compiled together with a generated harness by `rustc` /
//! `gcc` / `clang`, and the printed results must equal the interpreter's on the standard tests.
//!
//! A level that cannot be translated is allowed only when it is listed in [`ALLOW`] with a reason; an entry
//! that translates after all is an error (the list must stay honest). Missing compilers are skipped.
//!
//! Environment: `ALGOCLERK_LEVEL=<id>` restricts the run to one level, `ALGOCLERK_LANG=rust|c` to one
//! language, `ALGOCLERK_CODEGEN_KEEP=1` keeps the generated sources.

use algoclerk_core::cpp::codegen::Lang;
use algoclerk_core::cpp::codegen::harness::{Outcome, check_solution, standard_cases, toolchains};
use algoclerk_core::level::Level;
use algoclerk_levels::Campaign;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const THREADS: usize = 4;

/// `(level id, language id, reason)`: levels the translator may refuse.
const ALLOW: &[(&str, &str, &str)] = &[
    (
        "drop-negatives",
        "rust",
        "walks a std::list with an iterator and `it = l.erase(it)`; only `find` iterators are translated",
    ),
    (
        "drop-negatives",
        "c",
        "walks a std::list with an iterator and `it = l.erase(it)`; only `find` iterators are translated",
    ),
];

fn allowed(level: &str, lang: Lang) -> Option<&'static str> {
    ALLOW
        .iter()
        .find(|(l, g, _)| *l == level && *g == lang.id())
        .map(|(_, _, r)| *r)
}

fn run_one(
    level: &Level,
    lang: Lang,
    dir: &std::path::Path,
    problems: &Mutex<Vec<String>>,
    notes: &Mutex<Vec<String>>,
) {
    let tcs = toolchains(lang);
    let cases = match standard_cases(level) {
        Ok(c) => c,
        Err(e) => {
            problems
                .lock()
                .unwrap()
                .push(format!("{}: cannot expand the tests: {e}", level.id));
            return;
        }
    };
    let mut solutions: Vec<(&str, &str)> = vec![("reference", level.reference.as_str())];
    if let Some(n) = &level.naive {
        solutions.push(("naive", n.as_str()));
    }
    let mut untranslatable = Vec::new();
    for (kind, text) in solutions {
        let tag = format!("{}-{kind}", level.id);
        match check_solution(level, text, lang, &tcs, dir, &tag, &cases) {
            Outcome::Ok { cases, warnings } => {
                let w = warnings.lines().filter(|l| l.contains("warning")).count();
                notes.lock().unwrap().push(format!(
                    "{} {kind} {}: ok, {cases} cases, {w} warnings",
                    level.id,
                    lang.id()
                ));
            }
            Outcome::NoCompiler => {}
            Outcome::Untranslatable(m) => untranslatable.push(format!("{kind}: {m}")),
            Outcome::Failed(m) => {
                problems
                    .lock()
                    .unwrap()
                    .push(format!("{} {kind} {}: {m}", level.id, lang.name()))
            }
        }
    }
    match (untranslatable.is_empty(), allowed(&level.id, lang)) {
        (true, Some(r)) => problems.lock().unwrap().push(format!(
            "{}: {} translates now, remove it from ALLOW ({r})",
            level.id,
            lang.name()
        )),
        (false, None) => problems.lock().unwrap().push(format!(
            "{}: {} is not translatable and not allowlisted: {}",
            level.id,
            lang.name(),
            untranslatable.join("; ")
        )),
        _ => {}
    }
}

#[test]
fn every_level_translates_to_rust_and_c_and_matches_the_interpreter() {
    let only_lang = std::env::var("ALGOCLERK_LANG").ok();
    let filter = std::env::var("ALGOCLERK_LEVEL").ok();
    let langs: Vec<Lang> = [Lang::Rust, Lang::C]
        .into_iter()
        .filter(|l| only_lang.as_ref().is_none_or(|o| o == l.id()))
        .collect();
    let campaign = Campaign::shipped();
    let levels: Vec<&Level> = campaign
        .all_levels()
        .filter(|l| filter.as_ref().is_none_or(|f| f == &l.id))
        .collect();
    assert!(!levels.is_empty(), "no level to check");
    let dir = std::env::temp_dir().join(format!("algoclerk-codegen-{}", std::process::id()));
    let jobs: Vec<(&Level, Lang)> = levels
        .iter()
        .flat_map(|l| langs.iter().map(move |g| (*l, *g)))
        .collect();
    let next = AtomicUsize::new(0);
    let problems = Mutex::new(Vec::new());
    let notes = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some((l, g)) = jobs.get(i) else { break };
                    run_one(l, *g, &dir, &problems, &notes);
                }
            });
        }
    });
    if std::env::var("ALGOCLERK_CODEGEN_KEEP").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut notes = notes.into_inner().unwrap_or_default();
    notes.sort();
    eprintln!("{} jobs:\n  {}", jobs.len(), notes.join("\n  "));
    let problems = problems.into_inner().unwrap_or_default();
    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems
            .iter()
            .map(|p| p.chars().take(2500).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n---\n")
    );
}
