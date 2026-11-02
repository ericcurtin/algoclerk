// SPDX-License-Identifier: Apache-2.0
//! Robustness of the C++ parser and printer: byte soup, token soup and mutated corpus files must never
//! panic or take long; whatever the parser accepts must print to text that parses back to the same
//! program; random expression trees survive print-and-parse (the printer's minimal parentheses never
//! change the meaning).

use algoclerk_core::cpp::ast::*;
use algoclerk_core::cpp::lexer::highlight;
use algoclerk_core::cpp::parse::{ParseOpts, parse};
use algoclerk_core::cpp::print::{normalize, print_program};
use proptest::prelude::*;
use std::time::{Duration, Instant};

/// The number of cases, overridable with `PROPTEST_CASES` for long runs.
fn cases(default: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn opts() -> ParseOpts {
    ParseOpts {
        first_id: 0,
        known_types: vec!["ListNode".into(), "TreeNode".into()],
    }
}

/// Parse `src` (must not panic or be slow); when it parses, check the print-parse-print fixed point.
fn check_text(src: &str) {
    let start = Instant::now();
    let r = parse(src, &opts());
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "parse took {:?} on {src:?}",
        start.elapsed()
    );
    match r {
        Err(d) => {
            assert!(!d.is_empty() && d.len() <= 5, "{src:?}");
            for x in &d {
                assert!(!x.msg.is_empty());
                if let Some(s) = x.span {
                    assert!(s.start <= s.end);
                    assert!(
                        (s.end as usize) <= src.len().max(1),
                        "span outside the text"
                    );
                }
            }
        }
        Ok(p) => {
            let text = print_program(&p, false);
            let again = match parse(&text, &opts()) {
                Ok(a) => a,
                Err(d) => panic!(
                    "printed text does not parse: {d:?}\n--- source\n{src}\n--- printed\n{text}"
                ),
            };
            let text2 = print_program(&again, false);
            assert_eq!(text, text2, "not a fixed point; source:\n{src}");
            let (mut a, mut b) = (p, again);
            normalize(&mut a);
            normalize(&mut b);
            assert_eq!(
                a, b,
                "round trip changed the program; source:\n{src}\n--- printed\n{text}"
            );
        }
    }
}

const TOKENS: &[&str] = &[
    "int",
    "char",
    "bool",
    "unsigned",
    "long",
    "auto",
    "const",
    "static",
    "inline",
    "constexpr",
    "void",
    "struct",
    "class",
    "template",
    "typename",
    "using",
    "namespace",
    "std",
    "typedef",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "return",
    "goto",
    "try",
    "catch",
    "new",
    "delete",
    "nullptr",
    "true",
    "false",
    "sizeof",
    "this",
    "operator",
    "x",
    "y",
    "n",
    "v",
    "i",
    "f",
    "g",
    "main",
    "ListNode",
    "TreeNode",
    "vector",
    "string",
    "map",
    "pair",
    "size_t",
    "0",
    "1",
    "42",
    "0x1F",
    "5u",
    "7LL",
    "1.5",
    "'a'",
    "'\\n'",
    "\"hi\"",
    "\"\"",
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    "<",
    ">",
    ">>",
    "<<",
    ",",
    ";",
    ":",
    "::",
    "?",
    ".",
    "->",
    "=",
    "==",
    "!=",
    "<=",
    ">=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "&",
    "&&",
    "|",
    "||",
    "^",
    "~",
    "!",
    "++",
    "--",
    "+=",
    "-=",
    "<<=",
    "#",
    "#include <vector>",
    "#define X 1",
    "// note",
    "/* block */",
    "\n",
];

fn token_soup() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(TOKENS), 0..60).prop_map(|t| t.join(" "))
}

/// Token soup that starts like a program, so that deeper parser paths are reached.
fn program_soup() -> impl Strategy<Value = String> {
    (token_soup(), token_soup(), token_soup()).prop_map(|(a, b, c)| {
        format!("#include <vector>\nusing namespace std;\nint f(int n) {{ {a}\n}}\nstruct S {{ {b} }};\nvector<int> g() {{ {c} }}")
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(600), ..ProptestConfig::default() })]

    #[test]
    fn byte_soup_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..400)) {
        let s = String::from_utf8_lossy(&bytes).to_string();
        check_text(&s);
        let _ = highlight(&s);
    }

    #[test]
    fn ascii_soup_never_panics(s in "[ -~\n\t]{0,300}") {
        check_text(&s);
    }

    #[test]
    fn token_soup_never_panics(s in token_soup()) {
        check_text(&s);
        check_text(&format!("int f() {{ {s} }}"));
    }

    #[test]
    fn program_soup_never_panics(s in program_soup()) {
        check_text(&s);
    }
}

fn corpus_texts() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cpp_corpus");
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("corpus")
        .filter_map(|e| e.ok())
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();
    v.sort();
    v
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(800), ..ProptestConfig::default() })]

    /// Corpus files with a random slice deleted, duplicated or replaced by a token: most mutants are
    /// rejected, the accepted ones must round trip.
    #[test]
    fn mutated_corpus_files(
        file in 0usize..70,
        a in 0usize..2000,
        len in 0usize..40,
        op in 0u8..3,
        tok in proptest::sample::select(TOKENS),
    ) {
        let texts = corpus_texts();
        let src = &texts[file % texts.len()];
        let mut start = a % (src.len() + 1);
        while !src.is_char_boundary(start) { start -= 1; }
        let mut end = (start + len).min(src.len());
        while !src.is_char_boundary(end) { end -= 1; }
        let mutant = match op {
            0 => format!("{}{}", &src[..start], &src[end..]),
            1 => format!("{}{}{}", &src[..end], &src[start..end], &src[end..]),
            _ => format!("{}{} {} {}", &src[..start], tok, "", &src[end..]),
        };
        check_text(&mutant);
    }
}

#[test]
fn pathological_inputs_stay_fast() {
    let big = [
        "(".repeat(100_000),
        "{".repeat(100_000),
        "[".repeat(100_000),
        "int f() { ".to_string() + &"(".repeat(100_000),
        "int f() { ".to_string() + &"{".repeat(100_000),
        "int f() { ".to_string() + &"if (a) ".repeat(100_000),
        "int f() { return ".to_string() + &"-".repeat(100_000) + "1; }",
        "int f() { return ".to_string() + &"a+".repeat(100_000) + "1; }",
        "int f() { return ".to_string() + &"a?".repeat(50_000) + "1;",
        "int f() { return ".to_string() + &"f(".repeat(100_000),
        "int f() { x".to_string() + &".y".repeat(100_000) + "; }",
        "int f() { x".to_string() + &"[1]".repeat(100_000) + "; }",
        "int f() { ".to_string() + &"x;".repeat(200_000) + " }",
        "std::vector<".repeat(50_000),
        "int f() { std::vector<".to_string() + &"std::vector<".repeat(50_000),
        "int f() { return ".to_string() + &"(int)".repeat(50_000),
        "int f() { return ".to_string() + &"[](){".repeat(50_000),
        "int f() { return ".to_string() + &"static_cast<int>(".repeat(50_000),
        "int f() { switch (x) { ".to_string() + &"case 1: ".repeat(100_000),
        "int f() { ".to_string() + &"int a[3] = {".repeat(20_000),
        "/*".repeat(100_000),
        "// ".repeat(100_000),
        "#".repeat(100_000),
        "\"".repeat(100_001),
        "struct S { ".to_string() + &"int a; ".repeat(100_000),
    ];
    for src in big {
        let start = Instant::now();
        let r = parse(&src, &opts());
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "slow ({:?}) on input starting {:?}",
            start.elapsed(),
            &src[..src.len().min(40)]
        );
        if let Ok(p) = r {
            let _ = print_program(&p, false);
        }
    }
}

#[test]
fn deepest_accepted_programs_print_and_reparse() {
    // right at the nesting limit: must either parse (then round trip) or fail politely
    for depth in [50usize, 90, 120, 150, 190, 199, 200, 201, 250] {
        let nested_blocks = format!(
            "int f() {{ {}x = 1;{} }}",
            "{ ".repeat(depth),
            " }".repeat(depth)
        );
        let nested_parens = format!(
            "int f() {{ return {}1{}; }}",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        let nested_calls = format!(
            "int f() {{ return {}1{}; }}",
            "g(".repeat(depth),
            ")".repeat(depth)
        );
        let nested_unary = format!(
            "int f() {{ return {}1; }}",
            "-(".repeat(depth) + &")".repeat(depth)
        );
        let nested_ifs = format!("int f() {{ {} x(); }}", "if (a) ".repeat(depth));
        let nested_lambdas = format!(
            "int f() {{ return {}1{}; }}",
            "[]() { return ".repeat(depth / 2),
            "; }".repeat(depth / 2)
        );
        for src in [
            nested_blocks,
            nested_parens,
            nested_calls,
            nested_unary,
            nested_ifs,
            nested_lambdas,
        ] {
            check_text(&src);
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Printer property: random expression trees survive print-and-parse
// ---------------------------------------------------------------------------------------------------

fn e(kind: ExprKind) -> Expr {
    Expr { id: 0, kind }
}

fn spec(ty: Ty) -> TypeSpec {
    TypeSpec::plain(ty)
}

fn leaf() -> impl Strategy<Value = Expr> {
    prop_oneof![
        (
            0u64..2000,
            prop_oneof![
                Just(IntSuffix::None),
                Just(IntSuffix::U),
                Just(IntSuffix::LL),
                Just(IntSuffix::ULL)
            ],
            prop_oneof![
                Just(IntBase::Dec),
                Just(IntBase::Hex),
                Just(IntBase::Bin),
                Just(IntBase::Oct)
            ],
        )
            .prop_map(|(v, s, b)| {
                e(ExprKind::Int {
                    value: v,
                    suffix: s,
                    base: b,
                })
            }),
        any::<bool>().prop_map(|b| e(ExprKind::Bool(b))),
        any::<u8>().prop_map(|c| e(ExprKind::Char(c))),
        proptest::collection::vec(0u8..128, 0..6)
            .prop_map(|v| e(ExprKind::Str(v.into_iter().map(|c| c as char).collect()))),
        Just(e(ExprKind::NullPtr)),
        proptest::sample::select(&["a", "b", "c", "x", "y", "p"][..])
            .prop_map(|n| e(ExprKind::Var(n.to_string()))),
        Just(e(ExprKind::Path(vec![
            PathSeg {
                name: "std".into(),
                targs: None
            },
            PathSeg {
                name: "sort".into(),
                targs: None
            },
        ]))),
        Just(e(ExprKind::Path(vec![
            PathSeg {
                name: "std".into(),
                targs: None
            },
            PathSeg {
                name: "string".into(),
                targs: None
            },
            PathSeg {
                name: "npos".into(),
                targs: None
            },
        ]))),
    ]
}

const BINOPS: &[BinOp] = &[
    BinOp::Add,
    BinOp::Sub,
    BinOp::Mul,
    BinOp::Div,
    BinOp::Rem,
    BinOp::BitAnd,
    BinOp::BitOr,
    BinOp::BitXor,
    BinOp::Shl,
    BinOp::Shr,
    BinOp::Eq,
    BinOp::Ne,
    BinOp::Lt,
    BinOp::Le,
    BinOp::Gt,
    BinOp::Ge,
    BinOp::And,
    BinOp::Or,
    BinOp::Comma,
];
const UNOPS: &[UnOp] = &[
    UnOp::Neg,
    UnOp::Plus,
    UnOp::Not,
    UnOp::BitNot,
    UnOp::Deref,
    UnOp::AddrOf,
    UnOp::PreInc,
    UnOp::PreDec,
    UnOp::PostInc,
    UnOp::PostDec,
];

fn expr_tree() -> impl Strategy<Value = Expr> {
    leaf().prop_recursive(5, 40, 3, |inner| {
        prop_oneof![
            (proptest::sample::select(UNOPS), inner.clone())
                .prop_map(|(op, a)| e(ExprKind::Unary(op, Box::new(a)))),
            (
                proptest::sample::select(BINOPS),
                inner.clone(),
                inner.clone()
            )
                .prop_map(|(op, a, b)| e(ExprKind::Binary(
                    op,
                    Box::new(a),
                    Box::new(b)
                ))),
            (
                proptest::option::of(proptest::sample::select(&BINOPS[..10])),
                inner.clone(),
                inner.clone()
            )
                .prop_map(|(op, l, r)| e(ExprKind::Assign {
                    op,
                    lhs: Box::new(l),
                    rhs: Box::new(r)
                })),
            (inner.clone(), inner.clone(), inner.clone())
                .prop_map(|(a, b, c)| e(ExprKind::Ternary(Box::new(a), Box::new(b), Box::new(c)))),
            (inner.clone(), inner.clone())
                .prop_map(|(a, i)| e(ExprKind::Index(Box::new(a), Box::new(i)))),
            (
                inner.clone(),
                proptest::collection::vec(inner.clone(), 0..3)
            )
                .prop_map(|(c, args)| e(ExprKind::Call {
                    callee: Box::new(c),
                    args
                })),
            (
                inner.clone(),
                any::<bool>(),
                proptest::sample::select(&["first", "val", "next"][..])
            )
                .prop_map(|(o, arrow, n)| e(ExprKind::Member {
                    obj: Box::new(o),
                    name: n.to_string(),
                    arrow
                })),
            (
                prop_oneof![
                    Just(CastStyle::Static),
                    Just(CastStyle::CStyle),
                    Just(CastStyle::Functional)
                ],
                prop_oneof![Just(Ty::Int), Just(Ty::Bool), Just(Ty::Char)],
                inner.clone()
            )
                .prop_map(|(style, ty, a)| e(ExprKind::Cast {
                    style,
                    ty: spec(ty),
                    e: Box::new(a)
                })),
            (
                prop_oneof![Just(CastStyle::Static), Just(CastStyle::CStyle)],
                inner.clone()
            )
                .prop_map(|(style, a)| e(ExprKind::Cast {
                    style,
                    ty: spec(Ty::LongLong),
                    e: Box::new(a)
                })),
            proptest::collection::vec(inner.clone(), 0..3).prop_map(|a| e(ExprKind::InitList(a))),
            (
                proptest::collection::vec(inner.clone(), 0..3),
                any::<bool>()
            )
                .prop_map(|(args, brace)| {
                    e(ExprKind::Construct {
                        ty: spec(Ty::vector(Ty::Int)),
                        args,
                        brace,
                    })
                }),
            (
                proptest::collection::vec(inner.clone(), 0..3),
                any::<bool>()
            )
                .prop_map(|(args, brace)| {
                    e(ExprKind::New {
                        ty: spec(Ty::Int),
                        args,
                        brace,
                    })
                }),
            inner.prop_map(|a| e(ExprKind::Delete(Box::new(a)))),
        ]
    })
}

/// `int f() { return <expr>; }`
fn with_expr(expr: Expr) -> Program {
    let mut p = parse("int f() { return 0; }", &opts()).expect("template parses");
    if let ItemKind::Func(f) = &mut p.items[0].kind
        && let Some(b) = &mut f.body
        && let StmtKind::Return(r) = &mut b.stmts[0].kind
    {
        *r = Some(expr);
    }
    p
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(1500), ..ProptestConfig::default() })]

    #[test]
    fn printed_expressions_parse_back_to_the_same_tree(ex in expr_tree()) {
        let mut original = with_expr(ex);
        let text = print_program(&original, false);
        let mut back = match parse(&text, &opts()) {
            Ok(p) => p,
            Err(d) => return Err(TestCaseError::fail(format!("{d:?}\n{text}"))),
        };
        normalize(&mut original);
        normalize(&mut back);
        prop_assert_eq!(&original, &back, "printed as:\n{}", text);
        // and printing the reparsed tree gives the same text
        prop_assert_eq!(print_program(&back, false), text);
    }
}
