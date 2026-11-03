// SPDX-License-Identifier: Apache-2.0
//! Support for the property tests: a random program generator for the subset (UB free by construction),
//! batch compilation with the host compilers and comparison with the interpreter.
#![allow(dead_code)]

pub mod cstmt;
pub mod cstmt2;
pub mod epilogue;
pub mod expr;
pub mod loops;
pub mod num;
pub mod printer;
pub mod prog;
pub mod reads;
pub mod rng;
pub mod stmt;
pub mod strs;
pub mod ub;
pub mod ubc1;
pub mod ubc2;
pub mod ubc3;

use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::diag::line_col;
use algoclerk_core::cpp::harness::{Compiler, available_compilers, compile as cxx_compile, run_binary};
use algoclerk_core::cpp::interp::Limits;
use algoclerk_core::cpp::run::compile;
use prog::Prog;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Constructs the generator must not produce because the interpreter or checker currently diverges from the
/// real compilers on them (each is described in `lvl-bugs/prop.md`). The list shrinks as bugs get fixed.
/// Format: (feature name used by the generator, one line description).
pub const KNOWN: &[(&str, &str)] = &[
    ("ull_mul", "unsigned long long `*` overflows the i128 product in interpreter/ops.rs (panic in debug builds)"),
    ("append_n", "std::string::append(count, char) is valid C++17 but the checker rejects it (type.arity)"),
    ("partial_sum", "std::partial_sum is accepted by the checker but the interpreter reports BadArg"),
    ("for_each", "std::for_each is accepted by the checker but the interpreter reports BadArg (does not stock it)"),
];

pub fn known(feature: &str) -> bool {
    KNOWN.iter().any(|(f, _)| *f == feature)
}

/// Generate a program from the choice bytes.
pub fn generate(bytes: Vec<u8>) -> Prog {
    let mut g = expr::Gen::new(rng::Src::new(bytes));
    g.program()
}

/// Strategy of choice vectors; proptest shrinks them towards shorter, smaller vectors.
pub fn bytes_strategy() -> impl proptest::strategy::Strategy<Value = Vec<u8>> {
    proptest::collection::vec(proptest::prelude::any::<u8>(), 300..2500)
}

pub fn case_count(default: usize) -> usize {
    std::env::var("ALGOCLERK_PROP_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

pub fn have(tool: &str) -> bool {
    std::process::Command::new(tool)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn work_dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&d).expect("work dir");
    d
}

/// Does `cc` produce working binaries with `-fsanitize=undefined,address` (the runtime may be missing)?
pub fn sanitizer_works(cc: &Compiler) -> bool {
    let dir = work_dir("prop_san");
    let name = format!("san_{}", cc.exe.replace('+', "p"));
    cxx_compile(cc, "int main() { return 0; }\n", &dir, &name, true, 60)
        .ok()
        .is_some_and(|b| run_binary(&b.exe, 20, true).is_ok())
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The translation unit of several programs: one namespace each, `main` prints one result per program.
pub fn translation_unit(progs: &[&Prog]) -> String {
    let mut out = String::from(prog::INCLUDES);
    out.push_str(printer::PRINTER);
    for (i, p) in progs.iter().enumerate() {
        let body = p.body.as_str();
        out.push_str(&format!("namespace p{i} {{\n{body}}}\n"));
    }
    out.push_str("int main() {\n");
    for (i, p) in progs.iter().enumerate() {
        out.push_str(&format!(
            "    {{ std::string o; pr::p(o, p{i}::f({})); std::puts(o.c_str()); }}\n",
            p.args.join(", ")
        ));
    }
    out.push_str("    return 0;\n}\n");
    out
}

/// Warnings that mean the program has (or may have) undefined behaviour or a bug of the generator.
fn bad_warning(diag: &str) -> Option<String> {
    let keys = ["overflow in expression", "division by zero", "shift", "uninitialized", "return-type", "array-bounds", "dangling"];
    diag.lines()
        .find(|l| l.contains("warning:") && keys.iter().any(|k| l.contains(k)))
        .map(str::to_string)
}

/// Compile and run the programs with `cc`; on any failure the batch is split to find the culprit(s).
pub fn run_batch(cc: &Compiler, progs: &[&Prog], sanitize: bool) -> Vec<Result<String, String>> {
    let dir = work_dir("prop_diff");
    let tag = format!("b{}_{}_{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst), cc.exe.replace('+', "p"));
    let tu = translation_unit(progs);
    let built = cxx_compile(cc, &tu, &dir, &tag, sanitize, 180);
    let outcome = built.and_then(|b| {
        let r = match bad_warning(&b.diagnostics) {
            Some(w) => Err(format!("{} warns: {w}", cc.exe)),
            None => run_binary(&b.exe, 60, sanitize),
        };
        let _ = std::fs::remove_file(&b.exe);
        r
    });
    let _ = std::fs::remove_file(dir.join(format!("{tag}.cpp")));
    match outcome {
        Ok(lines) if lines.len() == progs.len() => lines.into_iter().map(Ok).collect(),
        Ok(lines) => vec![Err(format!("{} printed {} lines for {} programs", cc.exe, lines.len(), progs.len())); progs.len()],
        Err(e) if progs.len() == 1 => vec![Err(e)],
        Err(_) => {
            let (a, b) = progs.split_at(progs.len() / 2);
            let mut r = run_batch(cc, a, sanitize);
            r.extend(run_batch(cc, b, sanitize));
            r
        }
    }
}

/// The interpreter's answer for `p`, or a classified failure.
pub fn interpret(p: &Prog) -> Result<String, String> {
    let src = p.source();
    let c = compile("", &src, &CheckOpts::all()).map_err(|ds| {
        let d = &ds[0];
        let at = d.span.map(|s| line_col(&src, s.start).0);
        format!("[check {}] line {:?}: {}", d.code, at, d.msg)
    })?;
    let limits = Limits {
        steps: 400_000_000,
        ..Limits::default()
    };
    let called = c.call("f", &p.args, &limits).map_err(|e| format!("[call] {e}"))?;
    match called.report.result {
        Ok(_) => Ok(called.output.unwrap_or_default()),
        Err(e) => {
            let line = c
                .program
                .span(e.node)
                .map_or(0, |s| line_col(&src, s.start).0);
            Err(format!("[rt {:?}] line {line}: {}", e.kind, e.msg))
        }
    }
}

/// The compilers to use for batch number `b`: g++ always; a sampled second pass (all of them when
/// `ALGOCLERK_DIFF_CLANG` is set) with clang++ and with sanitizers where the runtime exists.
pub fn passes(b: usize) -> Vec<(Compiler, bool)> {
    let sampled = std::env::var("ALGOCLERK_DIFF_CLANG").is_ok() || b % 4 == 0;
    let mut out = Vec::new();
    for c in available_compilers() {
        if c.exe == "g++" {
            out.push((c.clone(), false));
            if sampled && cached_sanitizer(&c) {
                out.push((c, true));
            }
        } else if sampled {
            let san = cached_sanitizer(&c);
            out.push((c, san));
        }
    }
    out
}

pub fn cached_sanitizer(cc: &Compiler) -> bool {
    static CACHE: std::sync::Mutex<Vec<(String, bool)>> = std::sync::Mutex::new(Vec::new());
    let mut g = CACHE.lock().expect("cache");
    if let Some((_, v)) = g.iter().find(|(e, _)| *e == cc.exe) {
        return *v;
    }
    let v = sanitizer_works(cc);
    g.push((cc.exe.clone(), v));
    v
}

/// Evaluate programs: check + interpret + compilers. Entry `i` is `Err(description)` when program `i` shows a
/// problem of the generator, the checker, the interpreter or a divergence from the compilers.
pub fn evaluate(progs: &[Prog], b: usize) -> Vec<Result<(), String>> {
    let refs: Vec<&Prog> = progs.iter().collect();
    let mine: Vec<Result<String, String>> = progs.iter().map(interpret).collect();
    let mut verdict: Vec<Result<(), String>> = vec![Ok(()); progs.len()];
    let mut first: Vec<Option<String>> = vec![None; progs.len()];
    for (cc, san) in passes(b) {
        let theirs = run_batch(&cc, &refs, san);
        for (i, t) in theirs.into_iter().enumerate() {
            let label = format!("{}{}", cc.exe, if san { "+san" } else { "" });
            match (t, &mine[i]) {
                (Err(e), _) => verdict[i] = Err(format!("[{label} failed] {e}")),
                (Ok(t), Ok(m)) => {
                    if let Some(prev) = &first[i] {
                        if *prev != t && verdict[i].is_ok() {
                            verdict[i] = Err(format!("[compilers disagree] first: {prev}\n  {label}: {t}"));
                        }
                    } else {
                        first[i] = Some(t.clone());
                    }
                    if t != *m && verdict[i].is_ok() {
                        verdict[i] = Err(format!("[mismatch] {label}: {t}\n  interp: {m}"));
                    }
                }
                (Ok(t), Err(m)) => {
                    if verdict[i].is_ok() {
                        verdict[i] = Err(format!("[interp failed] {m}\n  {label}: {t}"));
                    }
                }
            }
        }
    }
    verdict
}
