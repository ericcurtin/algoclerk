// SPDX-License-Identifier: Apache-2.0
//! Robustness of the whole C++ pipeline: nothing the player types may panic, hang, run out of stack or
//! exhaust memory. Byte soup, token soup and mutated corpus programs go through the lexer, the parser, the
//! checker and (when they check) the interpreter on sample arguments, each within a time bound.

use algoclerk_core::cpp::ast::{ItemKind, Ty};
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::interp::{Limits, RtKind};
use algoclerk_core::cpp::lexer::{highlight, lex};
use algoclerk_core::cpp::run::compile;
use proptest::prelude::*;
use std::time::{Duration, Instant};

/// The number of cases, overridable with `PROPTEST_CASES` for long runs.
fn cases(default: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

const PRELUDE: &str = "#include <vector>
struct ListNode {
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

const TOKENS: &[&str] = &[
    "int",
    "char",
    "bool",
    "unsigned",
    "unsigned char",
    "long long",
    "auto",
    "const",
    "void",
    "struct",
    "using",
    "namespace",
    "std",
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
    "nullptr",
    "true",
    "false",
    "x",
    "y",
    "n",
    "v",
    "i",
    "f",
    "g",
    "ListNode",
    "TreeNode",
    "std::vector<int>",
    "std::string",
    "std::map<int, int>",
    "std::unordered_set<int>",
    "std::stack<int>",
    "std::pair<int, int>",
    "std::size_t",
    "std::sort",
    "std::max",
    "v.push_back(1)",
    "v.size()",
    "v[0]",
    "s.substr(1, 2)",
    "s.find('a', 1)",
    "m[1] += 2",
    "0",
    "1",
    "-5",
    "42",
    "2147483647",
    "5u",
    "7LL",
    "'a'",
    "\"s\"",
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
    "[&]",
    "[=](int a) { return a; }",
    "\n",
];

/// A sample literal for a parameter of this type, when the harness can build one.
fn sample_arg(t: &Ty) -> Option<String> {
    Some(match t {
        Ty::Int | Ty::UInt | Ty::LongLong | Ty::ULongLong | Ty::Size | Ty::UChar => "3".into(),
        Ty::Bool => "true".into(),
        Ty::Char => "'a'".into(),
        Ty::Str => "\"abc\"".into(),
        Ty::Vector(e) => {
            let x = sample_arg(e)?;
            format!("{{{x}, {x}, {x}}}")
        }
        _ => return None,
    })
}

/// Lex, parse, check; when the text is a valid program run every function that takes simple arguments with
/// small limits. Nothing may panic and each stage is time-bounded.
fn run_everything(src: &str) {
    let start = Instant::now();
    let _ = lex(src);
    let _ = highlight(src);
    let compiled = compile(PRELUDE, src, &CheckOpts::all());
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "front end took {:?} on {src:?}",
        start.elapsed()
    );
    let Ok(c) = compiled else { return };
    for it in c.program.player_items() {
        let ItemKind::Func(f) = &it.kind else {
            continue;
        };
        if f.body.is_none() {
            continue;
        }
        let args: Option<Vec<String>> = f.params.iter().map(|p| sample_arg(&p.spec.ty)).collect();
        let Some(args) = args else { continue };
        let limits = Limits {
            steps: 20_000,
            cells: 10_000,
            depth: 100,
            ..Limits::default()
        };
        let t = Instant::now();
        let _ = c.call(&f.name, &args, &limits);
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "running {} took {:?} on {src:?}",
            f.name,
            t.elapsed()
        );
    }
}

fn token_soup() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(TOKENS), 0..60).prop_map(|t| t.join(" "))
}

/// Token soup in the bodies of a well-formed shell, so that the checker and interpreter are reached.
fn program_soup() -> impl Strategy<Value = String> {
    (token_soup(), token_soup()).prop_map(|(a, b)| {
        format!(
            "#include <vector>\n#include <string>\nint f(int n, std::vector<int> v, std::string s) {{ {a}\n}}\nstd::vector<int> g(int n) {{ std::vector<int> v(n, 1); {b}\nreturn v; }}"
        )
    })
}

const BASES: &[&str] = &[
    "#include <vector>\n#include <unordered_map>\nint f(const std::vector<int>& a, int n) {\n    std::unordered_map<int, int> m;\n    int total = 0;\n    for (std::size_t i = 0; i < a.size(); ++i) {\n        m[a[i]] += 1;\n        if (a[i] > n && !(i == 2)) { total += a[i] % 7; }\n    }\n    while (total > 100) { total = total / 2; }\n    return total + static_cast<int>(m.size());\n}\n",
    "#include <string>\n#include <algorithm>\nstd::string g(std::string s, int k) {\n    std::string out;\n    for (char c : s) {\n        if (c >= 'a' && c <= 'z') { out += static_cast<char>('a' + (c - 'a' + k) % 26); }\n        else { out.push_back(c); }\n    }\n    std::sort(out.begin(), out.end());\n    return out.substr(0, out.find('z'));\n}\n",
    "#include <algorithm>\n#include <vector>\n#include <stack>\nint h(std::vector<int> v) {\n    std::stack<int> st;\n    int best = 0;\n    for (int x : v) {\n        while (!st.empty() && st.top() < x) { best += st.top(); st.pop(); }\n        st.push(x);\n    }\n    auto cmp = [](int a, int b) { return a > b; };\n    std::vector<int> w(v.begin(), v.end());\n    std::sort(w.begin(), w.end(), cmp);\n    return best + w[0];\n}\n",
];

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(300), ..ProptestConfig::default() })]

    #[test]
    fn arbitrary_text_never_panics(s in "\\PC{0,200}") {
        run_everything(&s);
    }

    #[test]
    fn token_soup_never_panics(s in token_soup()) {
        run_everything(&s);
    }

    #[test]
    fn program_soup_never_panics(s in program_soup()) {
        run_everything(&s);
    }

    #[test]
    fn mutated_programs_never_panic(
        which in 0..BASES.len(),
        seed in 0u64..10_000,
        cuts in proptest::collection::vec((0usize..600, 0usize..7), 1..4),
    ) {
        let mut s: Vec<char> = BASES[which].chars().collect();
        for (at, kind) in cuts {
            if s.is_empty() { break; }
            let at = at % s.len();
            match kind {
                0 => { s.remove(at); }
                1 => { let c = s[at]; s.insert(at, c); }
                2 => { let j = (at + seed as usize) % s.len(); s.swap(at, j); }
                3 => { s.insert(at, '{'); }
                4 => { s.insert(at, ')'); }
                5 => { s.insert(at, '0'); }
                _ => { s.truncate(at); }
            }
        }
        run_everything(&s.into_iter().collect::<String>());
    }
}

#[test]
fn the_base_programs_run() {
    for b in BASES {
        let c = compile(PRELUDE, b, &CheckOpts::all()).unwrap_or_else(|d| panic!("{d:?}\n{b}"));
        assert!(!c.program.player_items().is_empty());
        run_everything(b);
    }
}

#[test]
fn mutated_corpus_programs_never_panic() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cpp_corpus");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("corpus directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    files.sort();
    for (k, p) in files.iter().enumerate() {
        let text = std::fs::read_to_string(p).expect("corpus file");
        let chars: Vec<char> = text.chars().collect();
        for round in 0..6usize {
            let mut s = chars.clone();
            // deterministic damage: drop, duplicate or swap a few characters
            for j in 0..3usize {
                let at = (k * 7919 + round * 104_729 + j * 1_299_709) % s.len();
                match (round + j) % 4 {
                    0 => {
                        s.remove(at);
                    }
                    1 => {
                        let c = s[at];
                        s.insert(at, c);
                    }
                    2 => {
                        let o = (at * 31 + 17) % s.len();
                        s.swap(at, o);
                    }
                    _ => s.truncate(at.max(1)),
                }
            }
            run_everything(&s.into_iter().collect::<String>());
        }
    }
}

#[test]
fn absurdly_deep_or_long_input_is_rejected_not_fatal() {
    let all = CheckOpts::all();
    let nest = "(".repeat(5000) + "1" + &")".repeat(5000);
    assert!(compile("", &format!("int f() {{ return {nest}; }}"), &all).is_err());
    let blocks = "{".repeat(3000);
    assert!(compile("", &format!("void f() {blocks}"), &all).is_err());
    let neg = "-".repeat(10000);
    assert!(compile("", &format!("int f() {{ return {neg}1; }}"), &all).is_err());
    let calls = "f(".repeat(3000) + &")".repeat(3000);
    assert!(compile("", &format!("int f() {{ return {calls}; }}"), &all).is_err());
    let ifs = "if (true) ".repeat(3000);
    assert!(compile("", &format!("void f() {{ {ifs}; }}"), &all).is_err());
    let lists = "{".repeat(2000) + &"}".repeat(2000);
    assert!(
        compile(
            "#include <vector>\n",
            &format!("void f() {{ auto x = {lists}; }}"),
            &all
        )
        .is_err()
    );
    // a long flat chain must be handled in bounded time, whether it is accepted or refused
    let t = Instant::now();
    let chain = vec!["1"; 20000].join(" + ");
    let _ = compile("", &format!("int f() {{ return {chain}; }}"), &all);
    assert!(t.elapsed() < Duration::from_secs(10));
    // a huge text is lexed, not hung on
    let big = "int x = 1; ".repeat(200_000);
    let t = Instant::now();
    let _ = compile("", &big, &all);
    assert!(t.elapsed() < Duration::from_secs(10));
}

#[test]
fn deep_recursion_fits_in_a_small_stack() {
    let h = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let src = "int f(int n) { if (n == 0) { return 0; } return 1 + ((((f(n - 1) + 1) * 2) - 1) / 2); }";
            let c = compile("", src, &CheckOpts::all()).expect("compiles");
            let r = c
                .call("f", &["100000".to_string()], &Limits::default())
                .expect("call");
            assert!(matches!(r.report.result, Err(ref e) if e.kind == RtKind::Depth));
            let r = c
                .call("f", &["150".to_string()], &Limits::default())
                .expect("call");
            assert!(r.report.result.is_ok());
        })
        .expect("spawn");
    h.join().expect("no stack overflow");
}

#[test]
fn memory_and_time_bombs_are_stopped() {
    let all = CheckOpts::all();
    let c = compile(
        "#include <vector>\n",
        "int f(int n) { std::vector<int> v(n); std::vector<std::vector<int>> w(1000, v); return static_cast<int>(w.size()); }",
        &all,
    )
    .expect("compiles");
    let r = c
        .call("f", &["100000000".to_string()], &Limits::default())
        .expect("call");
    assert!(r.report.result.is_err());
    let c = compile(
        "#include <string>\n",
        "int f() { std::string s = \"ab\"; while (true) { s = s + s; } return 0; }",
        &all,
    )
    .expect("compiles");
    let t = Instant::now();
    let r = c.call("f", &[], &Limits::default()).expect("call");
    assert!(r.report.result.is_err());
    assert!(t.elapsed() < Duration::from_secs(20));
    let c = compile(
        "#include <vector>\n",
        "int f() { std::vector<int> v; while (true) { v.push_back(1); v.insert(v.begin(), 2); } return 0; }",
        &all,
    )
    .expect("compiles");
    let t = Instant::now();
    let r = c.call("f", &[], &Limits::default()).expect("call");
    assert!(r.report.result.is_err());
    assert!(t.elapsed() < Duration::from_secs(20));
    let c = compile("", "int f() { while (true) { } return 0; }", &all).expect("compiles");
    let r = c.call("f", &[], &Limits::default()).expect("call");
    assert!(matches!(r.report.result, Err(ref e) if e.kind == RtKind::Time));
}
