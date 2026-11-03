// SPDX-License-Identifier: Apache-2.0
//! Negative tests: a table of programs, each with exactly one undefined behaviour that the interpreter (or
//! the checker for static classes) must report, with the line of the offending statement. The offending
//! statement carries the marker comment `// @UB`; the expected line is the line of that marker.

use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::diag::line_col;
use algoclerk_core::cpp::harness::available_compilers;
use algoclerk_core::cpp::interp::{Limits, RtKind};
use algoclerk_core::cpp::parse::{ParseOpts, parse};
use algoclerk_core::cpp::run::compile;
use std::process::Command;

#[derive(Clone, Copy, Debug)]
pub enum Expect {
    /// The interpreter stops with this kind.
    Rt(RtKind),
    /// The checker rejects the program with this diagnostic code.
    Diag(&'static str),
    /// Either of the two (a static check may catch what the interpreter would catch at run time).
    Either(RtKind, &'static str),
}

#[derive(Clone, Debug)]
pub struct Case {
    pub name: &'static str,
    pub expect: Expect,
    /// Arguments of `f` in the value notation.
    pub args: &'static [&'static str],
    /// Real compilers with sanitizers must also reject (non-zero exit) the program.
    pub san: bool,
    pub src: &'static str,
}

pub fn c(name: &'static str, expect: Expect, args: &'static [&'static str], san: bool, src: &'static str) -> Case {
    Case { name, expect, args, san, src }
}

pub const PRELUDE_INCLUDES: &str = "#include <algorithm>
#include <array>
#include <deque>
#include <functional>
#include <list>
#include <map>
#include <numeric>
#include <optional>
#include <queue>
#include <set>
#include <stack>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>
";

/// Cases on which the interpreter or checker currently diverges from the expectation (each is described in
/// `lvl-bugs/prop.md`). They are still run; one that now passes is reported on stderr so the entry can go.
pub const UB_KNOWN: &[(&str, &str)] = &[(
    "invalid_map_erase_then_increment",
    "advancing an erased std::map iterator (++it, it++, std::next) is not reported as Invalidated",
)];

pub fn ub_known(name: &str) -> bool {
    UB_KNOWN.iter().any(|(n, _)| *n == name)
}

/// Full text of a case: includes, the optional node struct, then the body.
pub fn full_source(case: &Case) -> String {
    format!("{PRELUDE_INCLUDES}{}", case.src)
}

fn marker_line(src: &str) -> usize {
    src.lines().position(|l| l.contains("// @UB")).map(|i| i + 1).expect("case has a // @UB marker")
}

/// Run the interpreter or checker on a case and compare with its expectation. `Err` describes the problem.
pub fn run_case(case: &Case) -> Result<(), String> {
    let src = full_source(case);
    let want_line = marker_line(&src);
    let limits = Limits { steps: 3_000_000, ..Limits::default() };
    let compiled = compile("", &src, &CheckOpts::all());
    match (compiled, case.expect) {
        (Ok(c), Expect::Rt(k) | Expect::Either(k, _)) => {
            let args: Vec<String> = case.args.iter().map(|s| (*s).to_string()).collect();
            let called = c.call("f", &args, &limits)?;
            match called.report.result {
                Ok(v) => Err(format!("expected {k:?}, the program ran and returned {v:?}")),
                Err(e) => {
                    let line = c.program.span(e.node).map_or(0, |s| line_col(&src, s.start).0);
                    if e.kind != k {
                        Err(format!("expected {k:?}, got {:?} at line {line}: {}", e.kind, e.msg))
                    } else if line != want_line {
                        Err(format!("{k:?} reported at line {line}, expected line {want_line}: {}", e.msg))
                    } else {
                        Ok(())
                    }
                }
            }
        }
        (Ok(_), Expect::Diag(code)) => Err(format!("expected the checker to report {code}, but it accepted the program")),
        (Err(ds), Expect::Diag(code) | Expect::Either(_, code)) => {
            let prog = parse(&src, &ParseOpts::default()).ok();
            let Some(d) = ds.iter().find(|d| d.code == code) else {
                let codes: Vec<String> = ds.iter().map(|d| format!("{}: {}", d.code, d.msg)).collect();
                return Err(format!("expected diagnostic {code}, got {codes:?}"));
            };
            let at = d
                .span
                .or_else(|| d.node.and_then(|n| prog.as_ref().and_then(|p| p.span(n))))
                .map_or(0, |s| line_col(&src, s.start).0);
            if at == want_line { Ok(()) } else { Err(format!("{code} reported at line {at}, expected line {want_line}: {}", d.msg)) }
        }
        (Err(ds), Expect::Rt(k)) => Err(format!(
            "expected the interpreter to report {k:?}, but the checker rejected the program: {}: {}",
            ds[0].code, ds[0].msg
        )),
    }
}

/// A `main` that calls `f` with the case's arguments.
fn driver(case: &Case) -> String {
    let args = case.args.join(", ");
    format!("{}\nint main() {{\n    auto r = f({args});\n    (void)r;\n    return 0;\n}}\n", full_source(case))
}

/// Compile with a sanitizer-capable compiler and run: the program must fail (sanitizer report, assertion,
/// signal, uncaught exception). `Ok(None)` when no compiler with a sanitizer runtime exists.
pub fn sanitizer_rejects(case: &Case, dir: &std::path::Path) -> Result<Option<bool>, String> {
    for cc in available_compilers() {
        if !super::cached_sanitizer(&cc) {
            continue;
        }
        let name = format!("ub_{}_{}", case.name, cc.exe.replace('+', "p"));
        let cpp = dir.join(format!("{name}.cpp"));
        let exe = dir.join(&name);
        std::fs::write(&cpp, driver(case)).map_err(|e| e.to_string())?;
        let out = Command::new("timeout")
            .args(["-s", "KILL", "120", &cc.exe, "-std=c++17", "-O0", "-g0", "-fsanitize=undefined,address"])
            .args(["-fno-sanitize-recover=all", "-D_GLIBCXX_ASSERTIONS", "-w", "-o"])
            .arg(&exe)
            .arg(&cpp)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("{} cannot compile {}:\n{}", cc.exe, case.name, String::from_utf8_lossy(&out.stderr)));
        }
        let run = Command::new("timeout")
            .args(["-s", "KILL", "20"])
            .arg(&exe)
            .env("ASAN_OPTIONS", "detect_leaks=0:detect_stack_use_after_return=1")
            .env("UBSAN_OPTIONS", "halt_on_error=1")
            .output()
            .map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(&cpp);
        return Ok(Some(!run.status.success()));
    }
    Ok(None)
}
