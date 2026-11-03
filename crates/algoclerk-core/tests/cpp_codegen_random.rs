// SPDX-License-Identifier: Apache-2.0
//! Translation of the random corpus programs: every snippet of `tests/data/cpp_interp/*.cpp` (each defines
//! `test()`) and every function of `tests/data/cpp_corpus/*.cpp` (run on generated sample arguments) is
//! translated to Rust and C where the translator can, compiled with the host compilers together with the
//! generated harness and compared with the interpreter. Untranslatable programs are fine (they are counted
//! and their reasons printed), wrong code is not; a floor on the number translated catches regressions.
//! Skipped when no compiler is installed.

use algoclerk_core::cpp::ast::{ItemKind, Ty};
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::codegen::Lang;
use algoclerk_core::cpp::codegen::harness::{Outcome, check_compiled, toolchains};
use algoclerk_core::cpp::run::compile;
use algoclerk_core::level::Compare;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Programs whose translation is known not to match, with the reason (Rust only). The first group relies on
/// `char` being a signed 8 bit type with values of 128 and above (Rust's `u8` is unsigned; the subset is
/// ASCII), the second one on C++ references that alias a variable the program also uses while the reference
/// is alive (Rust's borrow rules forbid that).
const KNOWN: &[(&str, &str)] = &[
    ("arith/promote_char", "signed char values of 128 and above"),
    (
        "arith/compound_assign",
        "signed char values of 128 and above",
    ),
    (
        "arith/tricky_promotion",
        "signed char values of 128 and above",
    ),
    (
        "misc/const_refs_and_copies",
        "a reference aliasing a variable that is then modified",
    ),
    (
        "vectors/vec_front_back_refs",
        "references to elements alive while the vector is used",
    ),
];

const PRELUDE: &str = "struct ListNode {
    int val;
    ListNode* next;
    ListNode() : val(0), next(nullptr) {}
    ListNode(int x) : val(x), next(nullptr) {}
    ListNode(int x, ListNode* n) : val(x), next(n) {}
};
struct TreeNode {
    int val;
    TreeNode* left;
    TreeNode* right;
    TreeNode() : val(0), left(nullptr), right(nullptr) {}
    TreeNode(int x) : val(x), left(nullptr), right(nullptr) {}
    TreeNode(int x, TreeNode* l, TreeNode* r) : val(x), left(l), right(r) {}
};
";

struct Job {
    name: String,
    prelude: &'static str,
    source: String,
    /// `None`: call `test()`; `Some`: call the last function on generated arguments.
    sample: bool,
}

fn files(dir: &str) -> Vec<PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(dir);
    let mut v: Vec<PathBuf> = std::fs::read_dir(d)
        .expect("corpus directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    v.sort();
    v
}

fn jobs() -> Vec<Job> {
    let mut out = Vec::new();
    for p in files("cpp_interp") {
        let text = std::fs::read_to_string(&p).expect("readable");
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        for part in text.split("//// ").skip(1) {
            let (title, body) = part.split_once('\n').unwrap_or((part, ""));
            out.push(Job {
                name: format!("{stem}/{}", title.trim()),
                prelude: PRELUDE,
                source: body.to_string(),
                sample: false,
            });
        }
    }
    for p in files("cpp_corpus") {
        let text = std::fs::read_to_string(&p).expect("readable");
        out.push(Job {
            name: format!("corpus/{}", p.file_name().unwrap().to_string_lossy()),
            prelude: "",
            source: text,
            sample: true,
        });
    }
    out
}

/// A deterministic sample value of a type in the brace notation (`None` when we cannot make one).
fn sample(t: &Ty, seed: usize) -> Option<String> {
    let n = seed + 2;
    Some(match t {
        Ty::Bool => (if seed.is_multiple_of(2) {
            "true"
        } else {
            "false"
        })
        .into(),
        Ty::Char => format!("'{}'", (b'a' + (seed % 4) as u8) as char),
        Ty::Int | Ty::LongLong => (n as i64 * 3 - 4).to_string(),
        Ty::UInt | Ty::ULongLong | Ty::Size => (n * 2).to_string(),
        Ty::Str | Ty::StrView => {
            ["\"abca\"", "\"hello world\"", "\"\"", "\"aab\""][seed % 4].into()
        }
        Ty::Vector(e)
        | Ty::Array(e, _)
        | Ty::Stack(e)
        | Ty::Deque(e)
        | Ty::Queue(e)
        | Ty::List(e) => {
            let len = if let Ty::Array(_, k) = t { *k } else { n + 1 };
            let items: Option<Vec<String>> = (0..len).map(|i| sample(e, seed + i)).collect();
            format!("{{{}}}", items?.join(", "))
        }
        Ty::Pair(a, b) => format!("{{{}, {}}}", sample(a, seed)?, sample(b, seed + 1)?),
        Ty::Tuple(ts) => {
            let items: Option<Vec<String>> = ts
                .iter()
                .enumerate()
                .map(|(i, t)| sample(t, seed + i))
                .collect();
            format!("{{{}}}", items?.join(", "))
        }
        Ty::Optional(e) => format!("{{{}}}", sample(e, seed)?),
        Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
            let items: Option<Vec<String>> = (0..n)
                .map(|i| {
                    Some(format!(
                        "{{{}, {}}}",
                        sample(k, i * 2 + 1)?,
                        sample(v, seed + i)?
                    ))
                })
                .collect();
            let mut items = items?;
            items.dedup();
            return keys_distinct(&items).then(|| format!("{{{}}}", items.join(", ")));
        }
        Ty::Set(e) | Ty::UnorderedSet(e) => {
            let items: Option<Vec<String>> = (0..n).map(|i| sample(e, i * 2 + 1)).collect();
            let items = items?;
            let mut uniq = items.clone();
            uniq.sort();
            uniq.dedup();
            format!("{{{}}}", uniq.join(", "))
        }
        Ty::Ptr(inner) if matches!(&**inner, Ty::Struct(s) if s == "ListNode" || s == "TreeNode") =>
        {
            format!("{{{}, {}, {}}}", seed + 1, seed + 3, seed + 2)
        }
        _ => return None,
    })
}

fn keys_distinct(entries: &[String]) -> bool {
    let mut keys: Vec<&str> = entries
        .iter()
        .filter_map(|e| e.trim_start_matches('{').split(", ").next())
        .collect();
    let n = keys.len();
    keys.sort();
    keys.dedup();
    keys.len() == n
}

fn check_job(
    job: &Job,
    tcs: &[(Lang, Vec<algoclerk_core::cpp::codegen::harness::Toolchain>)],
    dir: &Path,
    idx: usize,
    notes: &Mutex<Vec<String>>,
    problems: &Mutex<Vec<String>>,
) {
    let sol = match compile(job.prelude, &job.source, &CheckOpts::all()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let entry = if job.sample {
        let last = sol
            .program
            .player_items()
            .iter()
            .rev()
            .find_map(|i| match &i.kind {
                ItemKind::Func(f) if f.body.is_some() => Some(f.name.clone()),
                _ => None,
            });
        match last {
            Some(n) => n,
            None => return,
        }
    } else {
        "test".to_string()
    };
    let Some(f) = sol.program.find(&entry) else {
        return;
    };
    let cases: Vec<Vec<String>> = if job.sample {
        (0..3)
            .filter_map(|seed| {
                f.params
                    .iter()
                    .map(|p| sample(&p.spec.ty, seed))
                    .collect::<Option<Vec<String>>>()
            })
            .collect()
    } else {
        vec![Vec::new()]
    };
    if cases.is_empty() || f.ret.ty == Ty::Void {
        notes.lock().unwrap().push(format!(
            "{}: skipped (no sample arguments or no result)",
            job.name
        ));
        return;
    }
    // programs the interpreter stops on (undefined behaviour on the sample data) are not compared
    let ok_cases: Vec<Vec<String>> = cases
        .into_iter()
        .filter(|c| {
            sol.call(&entry, c, &algoclerk_core::cpp::interp::Limits::default())
                .is_ok_and(|r| r.output.is_some())
        })
        .collect();
    if ok_cases.is_empty() {
        notes.lock().unwrap().push(format!(
            "{}: skipped (the interpreter stops on the sample data)",
            job.name
        ));
        return;
    }
    for (lang, list) in tcs {
        let tag = format!("r{idx}");
        match check_compiled(
            &sol,
            &entry,
            *lang,
            list,
            dir,
            &tag,
            &ok_cases,
            Compare::Exact,
            50_000_000,
        ) {
            Outcome::Ok { .. } => {
                notes
                    .lock()
                    .unwrap()
                    .push(format!("{} {}: ok", job.name, lang.id()))
            }
            Outcome::NoCompiler => {}
            Outcome::Untranslatable(m) => notes.lock().unwrap().push(format!(
                "{} {}: untranslatable: {m}",
                job.name,
                lang.id()
            )),
            Outcome::Failed(m) => match KNOWN.iter().find(|(n, _)| job.name.contains(n)) {
                Some((_, why)) if *lang == Lang::Rust => notes.lock().unwrap().push(format!(
                    "{} {}: known limitation ({why})",
                    job.name,
                    lang.id()
                )),
                _ => problems
                    .lock()
                    .unwrap()
                    .push(format!("{} {}: {m}", job.name, lang.name())),
            },
        }
    }
}

#[test]
fn random_corpus_programs_translate_and_match_the_interpreter() {
    let tcs: Vec<_> = [Lang::Rust, Lang::C]
        .into_iter()
        .map(|l| (l, toolchains(l)))
        .filter(|(_, t)| !t.is_empty())
        .collect();
    if tcs.is_empty() {
        eprintln!("skipping: neither rustc nor a C compiler is installed");
        return;
    }
    let only = std::env::var("ALGOCLERK_CORPUS").ok();
    let jobs: Vec<Job> = jobs()
        .into_iter()
        .filter(|j| only.as_ref().is_none_or(|o| j.name.contains(o.as_str())))
        .collect();
    let dir = std::env::temp_dir().join(format!("algoclerk-codegen-random-{}", std::process::id()));
    let next = AtomicUsize::new(0);
    let notes = Mutex::new(Vec::new());
    let problems = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = jobs.get(i) else { break };
                    check_job(job, &tcs, &dir, i, &notes, &problems);
                }
            });
        }
    });
    if std::env::var("ALGOCLERK_CODEGEN_KEEP").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut notes = notes.into_inner().unwrap_or_default();
    notes.sort();
    let translated = notes.iter().filter(|n| n.ends_with(": ok")).count();
    let refused = notes
        .iter()
        .filter(|n| n.contains("untranslatable"))
        .count();
    eprintln!(
        "{} programs, {translated} translations verified, {refused} refused:\n  {}",
        jobs.len(),
        notes.join("\n  ")
    );
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
    if only.is_none() {
        assert!(
            translated >= 150,
            "only {translated} translations verified: the translators regressed"
        );
    }
}
