// SPDX-License-Identifier: Apache-2.0
//! Every program of the corpus in `tests/data/cpp_corpus` is valid C++17 (it compiles with g++ and clang++),
//! so the checker must accept all of them with everything unlocked and must type every expression.

use algoclerk_core::cpp::ast::*;
use algoclerk_core::cpp::check::{CheckOpts, check, scope_at};
use algoclerk_core::cpp::diag::line_col;
use algoclerk_core::cpp::parse::{ParseOpts, parse};
use std::path::{Path, PathBuf};

fn corpus() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cpp_corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("corpus directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            (
                name,
                std::fs::read_to_string(&p).expect("readable corpus file"),
            )
        })
        .collect()
}

#[test]
fn every_corpus_file_checks() {
    let mut failures = Vec::new();
    for (name, src) in corpus() {
        let prog = parse(&src, &ParseOpts::default()).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        if let Err(ds) = check(&prog, &CheckOpts::all()) {
            for d in ds {
                let at = d
                    .node
                    .and_then(|n| prog.span(n))
                    .map(|s| line_col(&src, s.start))
                    .map_or(String::new(), |(l, c)| format!("{l}:{c}"));
                failures.push(format!("{name} {at} [{}] {}", d.code, d.msg));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}

/// Every expression of a checked program has a known type.
#[test]
fn every_expression_is_typed() {
    let mut unknown = Vec::new();
    for (name, src) in corpus() {
        let prog = parse(&src, &ParseOpts::default()).expect("parses");
        let Ok(info) = check(&prog, &CheckOpts::all()) else {
            continue;
        };
        let mut visit = |s: &Stmt| {
            stmt_exprs(s, &mut |e| {
                let mut callees = std::collections::HashSet::new();
                walk_expr(e, &mut |x| {
                    if let ExprKind::Call { callee, .. } = &x.kind {
                        callees.insert(callee.id);
                    }
                });
                walk_expr(e, &mut |x| {
                    // the callee of a call names a function or a method, it has no value type of its own
                    if *info.ty(x.id) == Ty::Unknown && !callees.contains(&x.id) {
                        let at = prog.span(x.id).map(|s| line_col(&src, s.start));
                        unknown.push(format!(
                            "{name} {at:?} {:?}",
                            std::mem::discriminant(&x.kind)
                        ));
                    }
                });
            });
        };
        for it in &prog.items {
            if let ItemKind::Func(f) = &it.kind {
                if let Some(b) = &f.body {
                    walk_stmts(b, &mut visit);
                }
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "untyped expressions:\n{}",
        unknown.join("\n")
    );
}

#[test]
fn scope_at_lists_variables_before_a_statement() {
    let src = "#include <vector>\nint f(int n) {\n    int a = 1;\n    std::vector<int> v;\n    return a + n;\n}\n";
    let prog = parse(src, &ParseOpts::default()).expect("parses");
    let info = check(&prog, &CheckOpts::all()).expect("checks");
    let mut ret = None;
    for it in &prog.items {
        if let ItemKind::Func(f) = &it.kind {
            walk_stmts(f.body.as_ref().unwrap(), &mut |s| {
                if matches!(s.kind, StmtKind::Return(_)) {
                    ret = Some(s.id);
                }
            });
        }
    }
    let vars = scope_at(&prog, &info, ret.expect("return statement"));
    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["n", "a", "v"]);
    assert_eq!(vars[2].1, Ty::vector(Ty::Int));
}

/// Deleting any one line of a corpus program may break it in many ways; the checker must never panic.
#[test]
fn damaged_programs_never_panic() {
    let mut checked = 0;
    for (_, src) in corpus() {
        let lines: Vec<&str> = src.lines().collect();
        for skip in 0..lines.len() {
            let damaged: String = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != skip)
                .map(|(_, l)| format!("{l}\n"))
                .collect();
            if let Ok(prog) = parse(&damaged, &ParseOpts::default()) {
                let _ = check(&prog, &CheckOpts::all());
                checked += 1;
            }
        }
    }
    assert!(checked > 500, "only {checked} damaged programs parsed");
}

#[test]
fn long_expression_chains_do_not_overflow_the_stack() {
    let terms = vec!["x"; 3000].join(" + ");
    let src = format!("int f(int x) {{ return {terms}; }}");
    if let Ok(prog) = parse(&src, &ParseOpts::default()) {
        let _ = check(&prog, &CheckOpts::all());
    }
}

#[test]
fn type_info_answers_types_lvalues_and_declarations() {
    let src = "#include <vector>\n#include <map>\nlong long f(const std::vector<int>& v, std::map<int, int>& m) {\n    auto x = v[0] + 1LL;\n    for (auto& [k, w] : m) { w += k; }\n    char c = 'a';\n    int d = c + c;\n    return x + d;\n}\n";
    let prog = parse(src, &ParseOpts::default()).expect("parses");
    let info = check(&prog, &CheckOpts::all()).expect("checks");
    let f = prog.find("f").expect("function");
    let (mut index, mut sum, mut decls, mut structured, mut charsum) =
        (None, None, Vec::new(), None, None);
    walk_stmts(f.body.as_ref().unwrap(), &mut |s| {
        if let StmtKind::Decl { vars, .. } = &s.kind {
            decls.extend(vars.iter().map(|d| (d.name.clone(), d.id)));
        }
        if matches!(s.kind, StmtKind::RangeFor { .. }) {
            structured = Some(s.id);
        }
        stmt_exprs(s, &mut |e| {
            walk_expr(e, &mut |x| match &x.kind {
                ExprKind::Index(..) => index = Some(x.id),
                ExprKind::Binary(BinOp::Add, a, _) if matches!(a.kind, ExprKind::Index(..)) => {
                    sum = Some(x.id)
                }
                ExprKind::Binary(BinOp::Add, a, _) if matches!(&a.kind, ExprKind::Var(n) if n == "c") => {
                    charsum = Some(x.id)
                }
                _ => {}
            });
        });
    });
    let id_of = |n: &str| decls.iter().find(|d| d.0 == n).expect("declared").1;
    assert_eq!(*info.decl_ty(id_of("x")), Ty::LongLong);
    assert_eq!(*info.decl_ty(id_of("c")), Ty::Char);
    assert_eq!(*info.ty(sum.unwrap()), Ty::LongLong);
    assert_eq!(
        *info.ty(charsum.unwrap()),
        Ty::Int,
        "char + char promotes to int"
    );
    assert!(info.is_lvalue(index.unwrap()) && info.is_const(index.unwrap()));
    assert!(!info.is_lvalue(sum.unwrap()));
    let names: Vec<_> = info
        .bindings(structured.unwrap())
        .iter()
        .map(|b| (b.0.as_str(), b.1.clone()))
        .collect();
    assert_eq!(names, [("k", Ty::Int), ("w", Ty::Int)]);
}
