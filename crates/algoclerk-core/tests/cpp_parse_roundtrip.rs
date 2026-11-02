// SPDX-License-Identifier: Apache-2.0
//! Round trip over the corpus of realistic C++17 solutions in `tests/data/cpp_corpus`:
//! text -> AST -> text -> AST must be structurally equal, the printed text must be a fixed point of
//! print-parse-print and must still compile with `g++ -std=c++17 -fsyntax-only`; every corpus file itself
//! must compile with both `g++` and `clang++`.

use algoclerk_core::cpp::ast::*;
use algoclerk_core::cpp::parse::{ParseOpts, known_types, parse};
use algoclerk_core::cpp::print::{normalize, print_program};
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
            (name, std::fs::read_to_string(&p).expect("readable corpus file"))
        })
        .collect()
}

fn parse_ok(name: &str, src: &str) -> Program {
    match parse(src, &ParseOpts::default()) {
        Ok(p) => p,
        Err(d) => panic!(
            "{name}: parse failed: {}",
            d.iter()
                .map(|d| format!("{} [{}] at {:?}", d.msg, d.code, d.span))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    }
}

/// `g++`-style syntax check with a hard timeout; `Ok(())` or the compiler output.
fn syntax_check(compiler: &str, path: &Path) -> Result<(), String> {
    let out = std::process::Command::new("timeout")
        .args(["-s", "KILL", "60", compiler, "-std=c++17", "-fsyntax-only"])
        .arg(path)
        .output();
    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        // the compiler is not installed here: nothing to check
        Err(_) => Ok(()),
    }
}

fn have(compiler: &str) -> bool {
    std::process::Command::new(compiler)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("algoclerk_cpp_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

#[test]
fn corpus_is_big_enough() {
    assert!(corpus().len() >= 60, "the corpus should hold about 60 solutions");
}

#[test]
fn corpus_files_parse() {
    let mut failures = Vec::new();
    for (name, src) in corpus() {
        match parse(&src, &ParseOpts::default()) {
            Ok(p) => {
                assert_eq!(p.prelude_items, 0, "{name}");
                assert!(!p.items.is_empty(), "{name}");
            }
            Err(d) => failures.push(format!("{name}: {} [{}] at {:?}", d[0].msg, d[0].code, d[0].span)),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn roundtrip_text_ast_text_ast() {
    for (name, src) in corpus() {
        let p1 = parse_ok(&name, &src);
        let text1 = print_program(&p1, false);
        let p2 = parse_ok(&format!("{name} (printed)"), &text1);
        let text2 = print_program(&p2, false);
        assert_eq!(text1, text2, "{name}: printing is not a fixed point");
        let (mut a, mut b) = (p1.clone(), p2);
        normalize(&mut a);
        normalize(&mut b);
        assert_eq!(a, b, "{name}: AST changed by the round trip\n{text1}");
        // known types are the same either way
        assert_eq!(known_types(&p1), known_types(&parse_ok(&name, &text1)), "{name}");
    }
}

#[test]
fn printed_text_has_four_space_indent_and_no_tabs() {
    for (name, src) in corpus() {
        let text = print_program(&parse_ok(&name, &src), false);
        assert!(text.ends_with('\n'), "{name}");
        for (i, line) in text.lines().enumerate() {
            assert!(!line.contains('\t'), "{name}:{}", i + 1);
            assert_eq!(line, line.trim_end(), "{name}:{} trailing blanks", i + 1);
            let lead = line.len() - line.trim_start().len();
            assert_eq!(lead % 4, 0, "{name}:{} odd indent: {line:?}", i + 1);
        }
    }
}

#[test]
fn every_node_has_a_span() {
    for (name, src) in corpus() {
        let p = parse_ok(&name, &src);
        assert!(p.spans.len() >= p.next_id as usize, "{name}");
        let check = |id: NodeId, what: &str| {
            assert!(p.span(id).is_some(), "{name}: {what} node {id} has no span");
        };
        for it in &p.items {
            check(it.id, "item");
            match &it.kind {
                ItemKind::Func(f) => {
                    check(f.id, "function");
                    f.params.iter().for_each(|x| check(x.id, "param"));
                    if let Some(b) = &f.body {
                        walk_stmts(b, &mut |s| {
                            check(s.id, "statement");
                            stmt_exprs(s, &mut |e| walk_expr(e, &mut |e| check(e.id, "expr")));
                            if let StmtKind::Decl { vars, .. } = &s.kind {
                                vars.iter().for_each(|v| check(v.id, "declarator"));
                            }
                        });
                    }
                }
                ItemKind::Struct(s) => {
                    check(s.id, "struct");
                    for c in &s.ctors {
                        check(c.id, "ctor");
                        c.params.iter().for_each(|x| check(x.id, "param"));
                    }
                }
                ItemKind::Global { vars, .. } => vars.iter().for_each(|v| check(v.id, "declarator")),
                _ => {}
            }
        }
    }
}

#[test]
fn ids_are_unique_and_below_next_id() {
    for (name, src) in corpus() {
        let p = parse_ok(&name, &src);
        let mut seen = std::collections::HashSet::new();
        for it in &p.items {
            assert!(it.id < p.next_id && seen.insert(it.id), "{name}: item id");
            if let ItemKind::Func(f) = &it.kind {
                assert!(f.id < p.next_id && seen.insert(f.id), "{name}: fn id");
                if let Some(b) = &f.body {
                    walk_stmts(b, &mut |s| {
                        assert!(s.id < p.next_id && seen.insert(s.id), "{name}: stmt id");
                    });
                }
            }
        }
    }
}

#[test]
fn corpus_compiles_with_gxx_and_clangxx() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cpp_corpus");
    let files: Vec<String> = corpus().into_iter().map(|(n, _)| n).collect();
    for compiler in ["g++", "clang++"] {
        if !have(compiler) {
            eprintln!("{compiler} not found: skipping");
            continue;
        }
        let failures = run_parallel(&files, |f| syntax_check(compiler, &dir.join(f)));
        assert!(failures.is_empty(), "{compiler} rejects: {failures:#?}");
    }
}

#[test]
fn printed_corpus_compiles_with_gxx() {
    if !have("g++") {
        eprintln!("g++ not found: skipping");
        return;
    }
    let dir = scratch_dir("printed");
    let mut names = Vec::new();
    for (name, src) in corpus() {
        let text = print_program(&parse_ok(&name, &src), false);
        std::fs::write(dir.join(&name), text).expect("write printed file");
        names.push(name);
    }
    let failures = run_parallel(&names, |f| syntax_check("g++", &dir.join(f)));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "g++ rejects printed text: {failures:#?}");
}

/// Run `f` for every name on four threads; the names with their error output that failed.
fn run_parallel(
    names: &[String],
    f: impl Fn(&str) -> Result<(), String> + Sync,
) -> Vec<(String, String)> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failed = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(name) = names.get(i) else { break };
                    if let Err(e) = f(name) {
                        let short: String = e.lines().take(6).collect::<Vec<_>>().join("\n");
                        failed.lock().expect("lock").push((name.clone(), short));
                    }
                }
            });
        }
    });
    failed.into_inner().expect("lock")
}

#[test]
fn prelude_known_types_are_used_by_the_parser() {
    let opts = ParseOpts {
        first_id: 1000,
        known_types: vec!["ListNode".into(), "Pairs".into()],
    };
    let src = "ListNode* f(ListNode* h) { ListNode* p = h; Pairs q; return p; }";
    let p = parse(src, &opts).expect("parses with prelude names");
    assert!(p.next_id > 1000);
    let f = p.find("f").expect("f");
    let b = f.body.as_ref().unwrap();
    assert!(matches!(b.stmts[0].kind, StmtKind::Decl { .. }));
    assert!(matches!(b.stmts[1].kind, StmtKind::Decl { .. }));
    let printed = print_program(&p, true);
    assert!(printed.contains("Pairs q;"), "{printed}");
}
