// SPDX-License-Identifier: Apache-2.0
//! Compile-and-compare: the generated Rust or C code is compiled together with a generated harness (a `main`
//! that builds the arguments of the level's standard tests and prints every result in the notation of
//! [`crate::cpp::value`]) with `rustc` / `gcc` / `clang`, run under `timeout -s KILL`, and the printed
//! results are compared with the interpreter's. A missing compiler is skipped, never an error.

use super::{Generated, Lang, generate};
use crate::cpp::interp::Limits;
use crate::cpp::run::{Compiled, compile};
use crate::judge::{canonical, raw_suite};
use crate::level::Level;
use std::path::Path;
use std::process::{Command, Stdio};

/// A compiler found on this machine.
#[derive(Clone, Debug)]
pub struct Toolchain {
    /// Language it compiles.
    pub lang: Lang,
    /// Executable (`rustc`, `gcc`, `clang`).
    pub exe: &'static str,
    /// Arguments before `-o`.
    pub args: Vec<String>,
}

fn works(exe: &str) -> bool {
    Command::new(exe)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The installed compilers for `lang` (empty for C++, which is not translated).
pub fn toolchains(lang: Lang) -> Vec<Toolchain> {
    let strs = |v: &[&str]| v.iter().map(ToString::to_string).collect::<Vec<_>>();
    let cands: Vec<(&'static str, Vec<String>)> = match lang {
        Lang::Rust => vec![(
            "rustc",
            strs(&[
                "--edition",
                "2021",
                "-C",
                "opt-level=0",
                "-C",
                "debuginfo=0",
                "-C",
                "codegen-units=1",
            ]),
        )],
        Lang::C => ["gcc", "clang"]
            .into_iter()
            .map(|e| {
                (
                    e,
                    strs(&[
                        "-std=c11",
                        "-O0",
                        "-Wall",
                        "-Wextra",
                        "-Wno-unused-function",
                        "-Wno-unused-parameter",
                        "-Wno-unused-variable",
                    ]),
                )
            })
            .collect(),
        Lang::Cpp => Vec::new(),
    };
    cands
        .into_iter()
        .filter(|(e, _)| works(e))
        .map(|(exe, args)| Toolchain { lang, exe, args })
        .collect()
}

/// Source of a complete program: generated code plus harness for these test cases.
pub fn program(level: &Level, g: &Generated, cases: &[Vec<String>]) -> Result<String, String> {
    match g.lang {
        Lang::Rust => super::harness_rust::program(level, g, cases),
        Lang::C => super::harness_c::program(level, g, cases),
        Lang::Cpp => Err("the C++ tab is the player's own code".into()),
    }
}

/// Compile `src` into `dir/<tag>`, run it and return its stdout lines plus the compiler's warnings.
pub fn build_and_run(
    tc: &Toolchain,
    src: &str,
    dir: &Path,
    tag: &str,
    compile_secs: u64,
    run_secs: u64,
) -> Result<(Vec<String>, String), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let ext = if tc.lang == Lang::Rust { "rs" } else { "c" };
    let path = dir.join(format!("{tag}.{ext}"));
    let exe = dir.join(format!("{tag}-{}", tc.exe));
    std::fs::write(&path, src).map_err(|e| e.to_string())?;
    let mut cmd = Command::new("timeout");
    cmd.args(["-s", "KILL", &compile_secs.to_string(), tc.exe])
        .args(&tc.args)
        .arg("-o")
        .arg(&exe)
        .arg(&path);
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {}: {e}", tc.exe))?;
    let msg = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        return Err(format!(
            "{} failed on {} (status {:?}; 137 means timeout):\n{}",
            tc.exe,
            path.display(),
            out.status.code(),
            msg.chars().take(5000).collect::<String>()
        ));
    }
    let run = Command::new("timeout")
        .args(["-s", "KILL", &run_secs.to_string()])
        .arg(&exe)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&exe);
    if !run.status.success() {
        return Err(format!(
            "{} binary failed ({:?}): {}",
            tc.exe,
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
                .chars()
                .take(1500)
                .collect::<String>()
        ));
    }
    let lines = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    Ok((lines, msg))
}

/// What checking one solution in one language found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Compiled with every compiler and matched the interpreter on this many cases (with warnings text).
    Ok {
        /// Cases compared.
        cases: usize,
        /// Compiler warnings, if any.
        warnings: String,
    },
    /// The translator said it cannot translate this program (friendly message).
    Untranslatable(String),
    /// No compiler for the language is installed.
    NoCompiler,
    /// A real problem: compile error, crash, mismatch.
    Failed(String),
}

/// The interpreter's canonical results for the cases.
pub fn expected(
    level: &Level,
    sol: &Compiled,
    cases: &[Vec<String>],
) -> Result<Vec<String>, String> {
    let limits = Limits {
        steps: level.budgets.large_steps.saturating_mul(40),
        ..Limits::default()
    };
    let ret = &level.signature.ret.ty;
    cases
        .iter()
        .map(|a| {
            let c = sol.call(&level.signature.name, a, &limits)?;
            match c.output {
                Some(o) => Ok(canonical(&o, ret, level.compare)),
                None => Err(format!("the interpreter stopped on {a:?}")),
            }
        })
        .collect()
}

/// The standard (non-large) test arguments of a level.
pub fn standard_cases(level: &Level) -> Result<Vec<Vec<String>>, String> {
    Ok(raw_suite(level, Some(300))?
        .into_iter()
        .filter(|t| !t.large)
        .map(|t| t.args)
        .collect())
}

/// Translate `text` (a solution of `level`) to `lang`, compile it with every compiler and compare with the
/// interpreter on `cases`.
pub fn check_solution(
    level: &Level,
    text: &str,
    lang: Lang,
    tcs: &[Toolchain],
    dir: &Path,
    tag: &str,
    cases: &[Vec<String>],
) -> Outcome {
    let sol = match compile(&level.prelude, text, &level.check_opts()) {
        Ok(c) => c,
        Err(d) => {
            return Outcome::Failed(format!(
                "does not compile: {}",
                d.first().map_or(String::new(), |d| d.msg.clone())
            ));
        }
    };
    let g = match generate(&sol, lang, &level.signature.name) {
        Ok(g) => g,
        Err(m) => return Outcome::Untranslatable(m),
    };
    if tcs.is_empty() {
        return Outcome::NoCompiler;
    }
    let src = match program(level, &g, cases) {
        Ok(s) => s,
        Err(m) => return Outcome::Failed(format!("harness: {m}")),
    };
    let want = match expected(level, &sol, cases) {
        Ok(w) => w,
        Err(m) => return Outcome::Failed(format!("interpreter: {m}")),
    };
    let ret = &level.signature.ret.ty;
    let mut warnings = String::new();
    for tc in tcs {
        let (lines, warn) =
            match build_and_run(tc, &src, dir, &format!("{tag}-{}", lang.id()), 240, 60) {
                Ok(r) => r,
                Err(e) => {
                    return Outcome::Failed(format!(
                        "{e}\n--- generated {} ---\n{}",
                        lang.name(),
                        g.full()
                    ));
                }
            };
        warnings.push_str(&warn);
        if lines.len() != want.len() {
            return Outcome::Failed(format!(
                "{}: printed {} lines for {} cases\n--- generated ---\n{}",
                tc.exe,
                lines.len(),
                want.len(),
                g.full()
            ));
        }
        for (i, (got, w)) in lines.iter().zip(&want).enumerate() {
            let got = canonical(got, ret, level.compare);
            if &got != w {
                let short = |s: &str| s.chars().take(100).collect::<String>();
                return Outcome::Failed(format!(
                    "{}: case {i} {:?} gave {} but the interpreter says {}\n--- generated ---\n{}",
                    tc.exe,
                    cases[i].iter().map(|a| short(a)).collect::<Vec<_>>(),
                    short(&got),
                    short(w),
                    g.full()
                ));
            }
        }
    }
    Outcome::Ok {
        cases: cases.len(),
        warnings,
    }
}
