// SPDX-License-Identifier: Apache-2.0
//! Unit tests of the statement and item half of the parser.

use super::*;
use crate::cpp::print::{normalize, print_program, print_stmt};

fn prog_with(src: &str, opts: &ParseOpts) -> Program {
    match parse(src, opts) {
        Ok(p) => p,
        Err(d) => panic!("{src}\n-> {:?}", d),
    }
}

fn prog(src: &str) -> Program {
    prog_with(src, &ParseOpts::default())
}

fn perr(src: &str) -> Diag {
    match parse(src, &ParseOpts::default()) {
        Ok(_) => panic!("expected an error for {src}"),
        Err(mut d) => {
            assert_eq!(d.len(), 1, "at most one diagnostic");
            d.remove(0)
        }
    }
}

/// The statements of `void f() { <src> }`.
fn stmts(src: &str) -> Vec<Stmt> {
    let p = prog(&format!("void f() {{\n{src}\n}}"));
    match &p.items[0].kind {
        ItemKind::Func(f) => f.body.clone().expect("body").stmts,
        _ => panic!("not a function"),
    }
}

/// The stmt printed at indent 0, for compact assertions.
fn shown(src: &str) -> String {
    stmts(src).iter().map(|s| print_stmt(s, 0)).collect()
}

fn std_prelude() -> ParseOpts {
    ParseOpts {
        first_id: 0,
        known_types: vec!["ListNode".to_string()],
    }
}

#[test]
fn declarations() {
    let s = stmts(
        "int a = 1, b; const int c = 2; auto d = a; std::vector<int> v(3, 0); int e{4}; int g[3] = {1, 2, 3};",
    );
    assert_eq!(s.len(), 6);
    let StmtKind::Decl { spec, vars } = &s[0].kind else {
        panic!()
    };
    assert_eq!(spec.ty, Ty::Int);
    assert_eq!(vars.len(), 2);
    assert!(matches!(vars[0].init, Init::Copy(_)));
    assert!(matches!(vars[1].init, Init::None));
    let StmtKind::Decl { spec, .. } = &s[1].kind else {
        panic!()
    };
    assert!(spec.konst);
    let StmtKind::Decl { spec, .. } = &s[2].kind else {
        panic!()
    };
    assert_eq!(spec.ty, Ty::Auto);
    let StmtKind::Decl { vars, .. } = &s[3].kind else {
        panic!()
    };
    assert!(matches!(&vars[0].init, Init::Direct(a) if a.len() == 2));
    let StmtKind::Decl { vars, .. } = &s[4].kind else {
        panic!()
    };
    assert!(matches!(&vars[0].init, Init::Brace { with_eq: false, .. }));
    let StmtKind::Decl { vars, .. } = &s[5].kind else {
        panic!()
    };
    assert!(vars[0].array.is_some());
    assert!(matches!(&vars[0].init, Init::Brace { with_eq: true, elems } if elems.len() == 3));
}

#[test]
fn unsized_arrays_get_their_counted_size() {
    assert_eq!(shown("int a[] = {1, 2, 3};"), "int a[3] = {1, 2, 3};\n");
    assert_eq!(perr("void f() { int a[]; }").code, "parse.expected");
}

#[test]
fn structured_bindings() {
    let s = stmts("auto [a, b] = f(); const auto& [k, v] = *it; auto& [x, y, z] = t;");
    for st in &s {
        assert!(matches!(&st.kind, StmtKind::Structured { spec, .. } if spec.ty == Ty::Auto));
    }
    let StmtKind::Structured { names, spec, .. } = &s[1].kind else {
        panic!()
    };
    assert_eq!(names, &["k", "v"]);
    assert!(spec.konst && spec.is_ref);
    assert_eq!(
        shown("auto& [a, b] = p;"),
        "auto& [a, b] = p;\n".to_string()
    );
}

#[test]
fn declaration_versus_expression() {
    // an expression statement that merely starts like a type
    let s = stmts("int x; x * x; std::max(1, 2); std::string(\"a\"); int(x); foo(3);");
    assert!(matches!(s[0].kind, StmtKind::Decl { .. }));
    assert!(matches!(s[1].kind, StmtKind::Expr(_)));
    assert!(matches!(s[2].kind, StmtKind::Expr(_)));
    assert!(matches!(s[3].kind, StmtKind::Expr(_)));
    assert!(matches!(&s[4].kind, StmtKind::Expr(e) if matches!(e.kind, ExprKind::Cast { .. })));
    assert!(matches!(s[5].kind, StmtKind::Expr(_)));
    // known struct names and std types under `using namespace std`
    let p = prog_with(
        "using namespace std;\nvoid f() { ListNode* p = nullptr; ListNode d(1); vector<int> v; string s; max(1, 2); }",
        &std_prelude(),
    );
    let ItemKind::Func(f) = &p.items[1].kind else {
        panic!()
    };
    let kinds: Vec<bool> = f
        .body
        .as_ref()
        .unwrap()
        .stmts
        .iter()
        .map(|s| matches!(s.kind, StmtKind::Decl { .. }))
        .collect();
    assert_eq!(kinds, [true, true, true, true, false]);
}

#[test]
fn several_pointer_names_repeat_the_stars() {
    let s = stmts("ListNode *a = nullptr, *b = nullptr;");
    let _ = s;
    let p = prog_with(
        "void f() { ListNode *a = nullptr, *b = nullptr; }",
        &std_prelude(),
    );
    let text = print_program(&p, false);
    assert!(
        text.contains("ListNode *a = nullptr, *b = nullptr;"),
        "{text}"
    );
    let d = perr("void f() { int* a, b; }");
    assert_eq!(d.code, "parse.unsupported");
    assert!(d.msg.contains("pointer"), "{}", d.msg);
    assert_eq!(perr("void f() { int a, *b; }").code, "parse.unsupported");
}

#[test]
fn if_chains_and_init_statements() {
    let s = stmts("if (a) x(); else if (b) y(); else z();");
    let StmtKind::If { els: Some(e), .. } = &s[0].kind else {
        panic!()
    };
    let StmtKind::If { els: Some(e2), .. } = &e.kind else {
        panic!()
    };
    assert!(matches!(e2.kind, StmtKind::Expr(_)));
    let s = stmts(
        "if (auto it = m.find(k); it != m.end()) { use(it); } if (int x = 1; x) {} if (n = f(); n > 0) {}",
    );
    for (i, st) in s.iter().enumerate() {
        let StmtKind::If { init, .. } = &st.kind else {
            panic!("{i}")
        };
        assert!(init.is_some(), "{i}");
    }
    assert!(matches!(
        &s[0].kind,
        StmtKind::If { init: Some(i), .. } if matches!(i.kind, StmtKind::Decl { .. })
    ));
    assert!(matches!(
        &s[2].kind,
        StmtKind::If { init: Some(i), .. } if matches!(i.kind, StmtKind::Expr(_))
    ));
    let d = perr("void f() { if (int x = g()) {} }");
    assert_eq!(d.code, "parse.unsupported");
}

#[test]
fn dangling_else_binds_to_the_inner_if() {
    let s = stmts("if (a) if (b) x(); else y();");
    let StmtKind::If { then, els, .. } = &s[0].kind else {
        panic!()
    };
    assert!(els.is_none());
    assert!(matches!(&then.kind, StmtKind::If { els: Some(_), .. }));
}

#[test]
fn switch_with_markers_and_init() {
    let s = stmts("switch (x) { case 1: case 2: f(); break; default: g(); }");
    let StmtKind::Switch { body, .. } = &s[0].kind else {
        panic!()
    };
    let kinds: Vec<&str> = body
        .stmts
        .iter()
        .map(|s| match &s.kind {
            StmtKind::Case(Some(_)) => "case",
            StmtKind::Case(None) => "default",
            StmtKind::Expr(_) => "expr",
            StmtKind::Break => "break",
            _ => "?",
        })
        .collect();
    assert_eq!(kinds, ["case", "case", "expr", "break", "default", "expr"]);
    // an init statement is hoisted into a block
    let s = stmts("switch (int k = f(); k) { case 0: break; }");
    assert_eq!(s.len(), 1);
    let StmtKind::Block(b) = &s[0].kind else {
        panic!()
    };
    assert!(matches!(b.stmts[0].kind, StmtKind::Decl { .. }));
    assert!(matches!(b.stmts[1].kind, StmtKind::Switch { .. }));
    assert_eq!(perr("void f() { case 1: break; }").code, "parse.unexpected");
    assert_eq!(
        perr("void f() { default: break; }").code,
        "parse.unexpected"
    );
    // a lambda inside a switch is not "inside the switch"
    assert_eq!(
        perr("void f() { switch (x) { case 1: auto g = [] { case 2: return; }; } }").code,
        "parse.unexpected"
    );
}

#[test]
fn loops() {
    let s = stmts(
        "for (int i = 0, n = 5; i < n; ++i, --n) {} for (;;) break; for (i = 0; i < 3; i++) ; for (auto x : v) {} for (const auto& [k, w] : m) {} for (char c : s) {} for (int x : {1, 2}) {}",
    );
    assert!(matches!(
        &s[0].kind,
        StmtKind::For {
            init: Some(_),
            cond: Some(_),
            incr: Some(_),
            ..
        }
    ));
    assert!(matches!(
        &s[1].kind,
        StmtKind::For {
            init: None,
            cond: None,
            incr: None,
            ..
        }
    ));
    assert!(matches!(
        &s[2].kind,
        StmtKind::For { init: Some(i), .. } if matches!(i.kind, StmtKind::Expr(_))
    ));
    assert!(matches!(
        &s[3].kind,
        StmtKind::RangeFor {
            structured: false,
            ..
        }
    ));
    let StmtKind::RangeFor {
        structured,
        names,
        spec,
        ..
    } = &s[4].kind
    else {
        panic!()
    };
    assert!(*structured && names == &["k", "w"] && spec.konst && spec.is_ref);
    assert!(matches!(&s[5].kind, StmtKind::RangeFor { spec, .. } if spec.ty == Ty::Char));
    assert!(matches!(&s[6].kind, StmtKind::RangeFor { .. }));
    let s = stmts("while (x) y(); do { z(); } while (w); do z(); while (w);");
    assert!(matches!(s[0].kind, StmtKind::While { .. }));
    assert!(matches!(s[1].kind, StmtKind::DoWhile { .. }));
    assert!(matches!(s[2].kind, StmtKind::DoWhile { .. }));
}

#[test]
fn jumps_blocks_and_empty() {
    let s = stmts("break; continue; return; return 1, 2; { int a; } ;");
    assert!(matches!(s[0].kind, StmtKind::Break));
    assert!(matches!(s[1].kind, StmtKind::Continue));
    assert!(matches!(s[2].kind, StmtKind::Return(None)));
    assert!(matches!(s[3].kind, StmtKind::Return(Some(_))));
    assert!(matches!(&s[4].kind, StmtKind::Block(b) if b.stmts.len() == 1));
    assert!(matches!(s[5].kind, StmtKind::Empty));
}

#[test]
fn lambda_bodies_are_real_blocks() {
    let s = stmts("auto f = [&](int a) { int b = a; if (b) return 1; return 2; };");
    let StmtKind::Decl { vars, .. } = &s[0].kind else {
        panic!()
    };
    let Init::Copy(e) = &vars[0].init else {
        panic!()
    };
    let ExprKind::Lambda(l) = &e.kind else {
        panic!()
    };
    assert_eq!(l.body.stmts.len(), 3);
    assert!(matches!(l.body.stmts[1].kind, StmtKind::If { .. }));
}

#[test]
fn items() {
    let p = prog(
        "#include <vector>\n#include \"mine.h\"\n#  include <bits/stdc++.h>\nusing namespace std;\nusing Grid = vector<vector<int>>;\ntypedef long long ll;\nconst int N = 5;\nconstexpr int M = N * 2 + 1, K = 3;\nint f(int);\nstatic inline int g(const vector<int>& v, int) { return 1; }\nstruct P { int x = 0; int y; P() {} P(int a, int b) : x(a), y{b} { y++; } };\n",
    );
    let kinds: Vec<&str> = p
        .items
        .iter()
        .map(|i| match &i.kind {
            ItemKind::Include { angle: true, .. } => "include<>",
            ItemKind::Include { .. } => "include\"\"",
            ItemKind::UsingNamespaceStd => "using",
            ItemKind::Alias { typedef: false, .. } => "alias",
            ItemKind::Alias { .. } => "typedef",
            ItemKind::Global {
                constexpr_: true, ..
            } => "constexpr",
            ItemKind::Global { .. } => "const",
            ItemKind::Func(f) if f.body.is_none() => "fwd",
            ItemKind::Func(_) => "func",
            ItemKind::Struct(_) => "struct",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "include<>",
            "include\"\"",
            "include<>",
            "using",
            "alias",
            "typedef",
            "const",
            "constexpr",
            "fwd",
            "func",
            "struct"
        ]
    );
    assert_eq!(known_types(&p), ["Grid", "ll", "P"]);
    let ItemKind::Struct(s) = &p.items[10].kind else {
        panic!()
    };
    assert_eq!(s.fields.len(), 2);
    assert!(s.fields[0].default.is_some());
    assert_eq!(s.ctors.len(), 2);
    assert_eq!(s.ctors[1].inits.len(), 2);
    let ItemKind::Func(g) = &p.items[9].kind else {
        panic!()
    };
    assert!(g.is_static);
    assert_eq!(g.params[1].name, "");
    assert_eq!(p.prelude_items, 0);
    assert_eq!(p.next_id as usize, p.spans.len().max(p.next_id as usize));
}

#[test]
fn alias_uses_keep_their_spelling() {
    let p = prog(
        "using namespace std;\nusing V = vector<int>;\nvoid f(const V& v, vector<int>& w) { V a; vector<V> b; std::vector<int> c; }",
    );
    let ItemKind::Func(f) = &p.items[2].kind else {
        panic!()
    };
    assert_eq!(f.params[0].spec.ty, Ty::vector(Ty::Int));
    assert_eq!(f.params[0].spec.spelled.as_deref(), Some("V"));
    assert_eq!(f.params[1].spec.spelled.as_deref(), Some("vector<int>"));
    let text = print_program(&p, false);
    assert!(
        text.contains("void f(const V& v, vector<int>& w)"),
        "{text}"
    );
    assert!(text.contains("vector<V> b;"), "{text}");
    assert!(text.contains("using V = std::vector<int>;"), "{text}");
}

#[test]
fn constants_size_std_arrays() {
    let p = prog(
        "#include <array>\nconst int N = 4;\nvoid f() { std::array<int, N + 1> a{}; std::array<int, N * 2> b; }",
    );
    let ItemKind::Func(f) = &p.items[2].kind else {
        panic!()
    };
    let StmtKind::Decl { spec, .. } = &f.body.as_ref().unwrap().stmts[0].kind else {
        panic!()
    };
    assert_eq!(spec.ty, Ty::Array(Box::new(Ty::Int), 5));
    let StmtKind::Decl { spec, .. } = &f.body.as_ref().unwrap().stmts[1].kind else {
        panic!()
    };
    assert_eq!(spec.ty, Ty::Array(Box::new(Ty::Int), 8));
}

#[test]
fn comments_attach_to_statements_items_and_blocks() {
    let src = "// about f\n// second line\nint f() {\n    // above a\n    int a = 1; // trailing a\n    /* block above b */\n    int b = 2; /* trailing b */\n    if (a) { // open\n        // inner\n        a++;\n        // tail of if\n    }\n    // tail of f\n}\n";
    let p = prog(src);
    assert_eq!(p.items[0].comments, [" about f", " second line"]);
    let ItemKind::Func(f) = &p.items[0].kind else {
        panic!()
    };
    assert_eq!(f.comments, p.items[0].comments);
    let b = f.body.as_ref().unwrap();
    assert_eq!(b.stmts[0].comments, [" above a"]);
    assert_eq!(b.stmts[0].trailing.as_deref(), Some(" trailing a"));
    assert_eq!(b.stmts[1].comments, [" block above b"]);
    assert_eq!(b.stmts[1].trailing.as_deref(), Some(" trailing b"));
    assert_eq!(b.stmts[2].trailing.as_deref(), Some(" open"));
    let StmtKind::If { then, .. } = &b.stmts[2].kind else {
        panic!()
    };
    let StmtKind::Block(tb) = &then.kind else {
        panic!()
    };
    assert_eq!(tb.stmts[0].comments, [" inner"]);
    assert_eq!(tb.tail_comments, [" tail of if"]);
    assert_eq!(b.tail_comments, [" tail of f"]);
}

#[test]
fn comments_survive_printing() {
    let src = "// c\nint f() {\n    // a\n    int x = 1; // t\n    // end\n}\n";
    let text = print_program(&prog(src), false);
    assert_eq!(
        text,
        "// c\nint f() {\n    // a\n    int x = 1; // t\n    // end\n}\n"
    );
    let text = print_program(&prog("void f() {/* one\n two */ g();}"), false);
    assert_eq!(text, "void f() {\n    // one\n    // two\n    g();\n}\n");
}

#[test]
fn friendly_unsupported_constructs() {
    let cases: &[(&str, &str)] = &[
        ("template <typename T> T id(T x) { return x; }", "templates"),
        ("class A { int x; };", "classes"),
        ("enum E { A };", "enum"),
        ("namespace n { }", "namespaces"),
        ("#define N 5\nint x();", "#define"),
        ("#pragma once\n", "pragma"),
        ("int g;", "global variables"),
        ("static int g = 1;", "global variables"),
        ("constexpr int f() { return 1; }", "constexpr"),
        ("int f() noexcept { return 1; }", "noexcept"),
        ("using std::vector;", "using"),
        ("using namespace foo;", "namespaces other"),
        ("struct A : B {};", "inheritance"),
        ("struct A { int f() { return 1; } };", "member functions"),
        ("struct A { private: int x; };", "private"),
        ("struct A { ~A() {} };", "destructors"),
        ("struct A { int a[3]; };", "C arrays"),
        ("typedef struct A { } A;", "typedef"),
        ("void f() { try { } catch (...) { } }", "try"),
        ("void f() { goto x; }", "goto"),
        ("void f() { double d = 1; }", "decimal"),
        ("void f() { int x = 1.5; }", "decimal"),
        ("void f() { long x = 1; }", "long"),
        ("void f() { int&& r = 1; }", "rvalue"),
        ("void f() { static int x = 1; }", "static"),
        ("void f() { using X = int; }", "using"),
        (
            "void f() { struct L { int a; }; }",
            "structs declared inside",
        ),
        ("void f() { int a[2][3]; }", "multi-dimensional"),
        ("void f() { int x(); }", "function"),
        ("void f() {\n#define X 1\n }", "#define"),
        ("void f(int a = 3) { }", "default arguments"),
        ("void f() { throw 1; }", "throw"),
        (
            "void f() { auto p = std::make_shared<int>(1); std::shared_ptr<int> q; }",
            "shared_ptr",
        ),
    ];
    for (src, word) in cases {
        let d = perr(src);
        assert!(
            d.code == "parse.unsupported" || d.code == "parse.vexing",
            "{src}: code {} ({})",
            d.code,
            d.msg
        );
        assert!(
            d.msg.contains(word),
            "{src}: message {:?} should mention {word:?}",
            d.msg
        );
        let span = d.span.expect("a span");
        assert!(span.end > span.start, "{src}");
    }
    let d = perr("template <class T> void f() {}");
    assert!(d.msg.starts_with("this office does not stock"));
}

#[test]
fn friendly_syntax_errors() {
    let d = perr("int f() { return 1 }");
    assert_eq!(d.code, "parse.expected");
    assert!(d.msg.contains("';'"), "{}", d.msg);
    let d = perr("int f() { if (x) { return 1; }");
    assert!(d.msg.contains("'}'"), "{}", d.msg);
    let d = perr("int f() { else { } }");
    assert!(d.msg.contains("`else`"), "{}", d.msg);
    let d = perr("int f( { }");
    assert_eq!(d.code, "parse.expected");
    let d = perr("int f() { do { } (x); }");
    assert!(d.msg.contains("while"), "{}", d.msg);
    let d = perr("int f() { switch (x) y; }");
    assert!(d.msg.contains("'{'"), "{}", d.msg);
    let d = perr("foo bar() {}");
    assert!(d.msg.contains("`foo`"), "{}", d.msg);
    let d = perr("#include vector\n");
    assert!(d.msg.contains("#include"), "{}", d.msg);
    let d = perr("void f() {\n#include <vector>\n }");
    assert_eq!(d.code, "parse.unexpected");
    // errors carry a span inside the text
    let src = "int f() { return 1 }";
    let span = perr(src).span.unwrap();
    assert!((span.end as usize) <= src.len() && span.start < span.end);
}

#[test]
fn nesting_limit_is_friendly() {
    let deep = |n: usize| format!("void f() {{ {}{} }}", "{".repeat(n), "}".repeat(n));
    assert!(parse(&deep(150), &ParseOpts::default()).is_ok());
    let d = parse(&deep(400), &ParseOpts::default()).unwrap_err();
    assert_eq!(d[0].code, "parse.depth");
    let chain = format!("void f() {{ {} x(); }}", "if (a) ".repeat(400));
    assert_eq!(
        parse(&chain, &ParseOpts::default()).unwrap_err()[0].code,
        "parse.depth"
    );
    let elses = format!(
        "void f() {{ if (a) x(); {} }}",
        "else if (a) x(); ".repeat(400)
    );
    assert_eq!(
        parse(&elses, &ParseOpts::default()).unwrap_err()[0].code,
        "parse.depth"
    );
}

#[test]
fn first_id_offsets_all_ids() {
    let opts = ParseOpts {
        first_id: 500,
        known_types: vec![],
    };
    let p = prog_with("int f(int a) { int b = a + 1; return b; }", &opts);
    assert!(p.next_id > 500);
    assert!(p.spans.len() >= p.next_id as usize);
    assert!(p.items[0].id >= 500);
    let mut q = p.clone();
    normalize(&mut q);
    assert_eq!(q.next_id, 0);
    assert!(q.spans.is_empty());
}

#[test]
fn no_diagnostics_beyond_a_few() {
    for src in [
        "",
        "   ",
        "// only a comment\n",
        "int",
        "int f(",
        "{{{{",
        "}}}}",
        ")))",
        "struct",
        "#",
    ] {
        match parse(src, &ParseOpts::default()) {
            Ok(p) => assert!(p.items.is_empty(), "{src:?}"),
            Err(d) => assert!(!d.is_empty() && d.len() <= 3, "{src:?}"),
        }
    }
}
