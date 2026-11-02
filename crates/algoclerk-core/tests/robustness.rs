// SPDX-License-Identifier: Apache-2.0
//! Robustness: nothing the player types may panic, hang or overflow the stack.

use algoclerk_core::lang::ast::Ty;
use algoclerk_core::lang::check::{Allowed, CheckOpts};
use algoclerk_core::lang::interp::Limits;
use algoclerk_core::lang::parse::parse;
use algoclerk_core::lang::print::print;
use algoclerk_core::lang::run::compile;
use proptest::prelude::*;

const TOKENS: &[&str] = &[
    "fn", "f", "(", ")", "{", "}", "[", "]", ",", ";", ":", "->", "let", "x", "y", "=", "==", "+", "-", "*", "/", "%", "<", ">", ">>", "<<",
    "if", "else", "while", "for", "in", "range", "return", "break", "continue", "true", "false", "null", "none", "int", "vec", "<int>",
    "map<int, int>", "stack<int>", "1", "0", "-5", "\"s\"", "'c'", ".", "..", "len", "push", "pop", "get", "and", "or", "not", "&mut", "_",
    "x.push(1)", "v[0]", "(1, 2)", ".0", "&", "|", "^", "~", "!", "#", "//", "\n",
];

fn run_everything(src: &str) {
    let opts = CheckOpts { allowed: Allowed::all(), entry: None };
    if let Ok(prog) = parse(src) {
        let printed = print(&prog);
        let again = parse(&printed).map(|p| print(&p));
        assert_eq!(again.as_deref(), Ok(printed.as_str()), "printing is not stable for {src:?}");
    }
    if let Ok(c) = compile(src, &opts) {
        for f in &c.program.fns {
            let args: Option<Vec<String>> = f
                .params
                .iter()
                .map(|p| match p.ty {
                    Ty::Int => Some("3".to_string()),
                    Ty::Bool => Some("true".to_string()),
                    Ty::Str => Some("\"abc\"".to_string()),
                    Ty::Vec(ref t) if **t == Ty::Int => Some("[1, 2, 3]".to_string()),
                    _ => None,
                })
                .collect();
            if let Some(args) = args {
                let limits = Limits { steps: 20_000, cells: 10_000, ..Limits::default() };
                let _ = c.call(&f.name, &args, &limits);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn arbitrary_text_never_panics(s in "\\PC{0,200}") {
        run_everything(&s);
    }

    #[test]
    fn token_soup_never_panics(toks in proptest::collection::vec(0..TOKENS.len(), 0..80)) {
        let src = toks.iter().map(|&i| TOKENS[i]).collect::<Vec<_>>().join(" ");
        run_everything(&src);
    }

    #[test]
    fn mutated_programs_never_panic(seed in 0u64..10_000, cuts in proptest::collection::vec((0usize..400, 0usize..6), 1..4)) {
        let base = "fn f(a: vec<int>, n: int) -> int {\n    let m = map<int, int>();\n    let total = 0;\n    for i in range(len(a)) {\n        m[a[i]] = m.get(a[i], 0) + 1;\n        if a[i] > n and not (i == 2) { total += a[i] % 7; }\n    }\n    while total > 100 { total = total / 2; }\n    return total + len(m);\n}\n";
        let mut s: Vec<char> = base.chars().collect();
        for (at, kind) in cuts {
            if s.is_empty() { break; }
            let at = at % s.len();
            match kind {
                0 => { s.remove(at); }
                1 => { let c = s[at]; s.insert(at, c); }
                2 => { let j = (at + seed as usize) % s.len(); s.swap(at, j); }
                3 => { s.insert(at, '{'); }
                4 => { s.insert(at, ')'); }
                _ => { s.truncate(at); }
            }
        }
        run_everything(&s.into_iter().collect::<String>());
    }
}

#[test]
fn absurdly_deep_or_long_input_is_rejected_not_fatal() {
    let nest = "(".repeat(5000) + "1" + &")".repeat(5000);
    assert!(parse(&format!("fn f() -> int {{ return {nest}; }}")).is_err());
    let chain = vec!["1"; 20000].join(" + ");
    assert!(parse(&format!("fn f() -> int {{ return {chain}; }}")).is_err());
    let blocks = "{".repeat(3000);
    assert!(parse(&format!("fn f() {blocks}")).is_err());
    let neg = "-".repeat(10000);
    assert!(parse(&format!("fn f() -> int {{ return {neg}1; }}")).is_err());
    let calls = "f(".repeat(3000) + &")".repeat(3000);
    assert!(parse(&format!("fn f() -> int {{ return {calls}; }}")).is_err());
}

#[test]
fn deep_recursion_fits_in_a_small_stack() {
    let h = std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(|| {
        let src = "fn f(n: int) -> int { if n == 0 { return 0; } return 1 + ((((f(n - 1) + 1) * 2) - 1) / 2); }";
        let c = compile(src, &CheckOpts { allowed: Allowed::all(), entry: None }).unwrap();
        let r = c.call("f", &["100000".to_string()], &Limits::default()).unwrap();
        assert!(r.report.result.is_err());
        let r = c.call("f", &["150".to_string()], &Limits::default()).unwrap();
        assert!(r.report.result.is_ok());
    });
    h.unwrap().join().unwrap();
}

#[test]
fn memory_bombs_are_stopped() {
    let c = compile("fn f(n: int) -> int { let v = [0; n]; let w = [v; 1000]; return len(w); }", &CheckOpts { allowed: Allowed::all(), entry: None }).unwrap();
    let r = c.call("f", &["100000000".to_string()], &Limits::default()).unwrap();
    assert!(r.report.result.is_err());
    let c = compile("fn f() -> int { let s = \"ab\"; while true { s = s + s; } return 0; }", &CheckOpts { allowed: Allowed::all(), entry: None }).unwrap();
    let r = c.call("f", &[], &Limits::default()).unwrap();
    assert!(r.report.result.is_err());
}
