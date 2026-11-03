// SPDX-License-Identifier: Apache-2.0
//! Fuzzing of the front end and the interpreter: corpus programs (`tests/data/cpp_corpus`, the programs of
//! `tests/data/cpp_interp`) are mutated by token insertion, deletion, duplication, replacement and swaps;
//! every mutant goes through parse, check and (when the checker accepts it) the interpreter with small
//! limits, on a thread with a wall clock bound. Nothing may panic (the interpreter reports a panic as
//! `RtKind::Internal`), nothing may run past the bound, and what parses must print and parse again.
//!
//! `ALGOCLERK_PROP_CASES` sets the number of mutants (default 300).

use algoclerk_core::cpp::ast::{ItemKind, Ty};
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::interp::{Limits, RtKind};
use algoclerk_core::cpp::parse::{ParseOpts, parse};
use algoclerk_core::cpp::print::print_program;
use algoclerk_core::cpp::run::compile;
use proptest::prelude::*;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

mod prop_common;

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

/// Messages of `RtKind::Internal` failures that are known interpreter bugs (see `lvl-bugs/prop.md`); the
/// fuzz property tolerates them until they are fixed.
const KNOWN_INTERNAL: &[(&str, &str)] = &[];

/// Seed programs: (name, source).
fn corpus() -> &'static Vec<(String, String)> {
    static C: OnceLock<Vec<(String, String)>> = OnceLock::new();
    C.get_or_init(|| {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        let mut out = Vec::new();
        let mut files: Vec<_> = std::fs::read_dir(base.join("cpp_corpus"))
            .expect("corpus")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
            .collect();
        files.sort();
        for p in files {
            let name = p.file_name().expect("name").to_string_lossy().to_string();
            out.push((name, std::fs::read_to_string(&p).expect("read")));
        }
        let mut files: Vec<_> = std::fs::read_dir(base.join("cpp_interp"))
            .expect("interp corpus")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
            .collect();
        files.sort();
        for p in files {
            let text = std::fs::read_to_string(&p).expect("read");
            let mut cur: Option<(String, String)> = None;
            for line in text.lines() {
                if let Some(n) = line.strip_prefix("//// ") {
                    out.extend(cur.take());
                    cur = Some((n.trim().to_string(), PRELUDE.to_string()));
                } else if let Some((_, s)) = cur.as_mut() {
                    s.push_str(line);
                    s.push('\n');
                }
            }
            out.extend(cur);
        }
        out
    })
}

/// Split into tokens: identifiers and numbers, string and char literals, operators, single other characters;
/// white space is attached to the token before it so that deleting a token keeps the layout.
fn tokens(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let s = i;
        let c = b[i];
        if c.is_ascii_alphanumeric() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
        } else if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(b.len());
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c.is_ascii_whitespace() {
            i += 1;
            if let Some(last) = out.last_mut() {
                last.push(c as char);
            }
            continue;
        } else {
            i += 1;
            if i < b.len()
                && matches!(
                    (c, b[i]),
                    (b':', b':')
                        | (b'-', b'>')
                        | (b'+', b'+')
                        | (b'-', b'-')
                        | (b'<', b'<')
                        | (b'>', b'>')
                        | (b'=', b'=')
                        | (b'!', b'=')
                        | (b'<', b'=')
                        | (b'>', b'=')
                        | (b'&', b'&')
                        | (b'|', b'|')
                        | (b'+', b'=')
                        | (b'-', b'=')
                )
            {
                i += 1;
            }
        }
        out.push(src[s..i].to_string());
    }
    out
}

const VOCAB: &[&str] = &[
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    ";",
    ",",
    "<",
    ">",
    "&",
    "*",
    "::",
    "->",
    "=",
    "==",
    "+",
    "-",
    "/",
    "%",
    "<<",
    ">>",
    "++",
    "--",
    "!",
    "?",
    ":",
    "0",
    "1",
    "-1",
    "2147483647",
    "4294967296",
    "'a'",
    "\"s\"",
    "int",
    "long long",
    "unsigned",
    "char",
    "auto",
    "auto&",
    "const",
    "std::vector<int>",
    "std::string",
    "std::map<int, int>",
    "nullptr",
    "new",
    "delete",
    "return",
    "break",
    "continue",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "true",
    "false",
    "static_cast<int>",
    "sizeof",
    "goto",
    "try",
    "throw",
    "template",
    "operator",
    "this",
    "std::move",
    "x",
    "v",
    "i",
    "n",
    "s",
    "size()",
    "begin()",
    "end()",
    "std::sort",
    "std::min",
    "0x7fffffff",
    "1e9",
    "100000000",
    "...",
    "#",
    "@",
    "\\",
];

#[derive(Clone, Debug)]
struct Mutation {
    kind: u8,
    a: u16,
    b: u16,
}

fn kind_of(t: &str) -> u8 {
    let t = t.trim();
    match t.bytes().next() {
        Some(c) if c.is_ascii_digit() => 1,
        Some(c) if c.is_ascii_alphabetic() || c == b'_' => 2,
        Some(b'"' | b'\'') => 3,
        _ => 4,
    }
}

const KEYWORDS: &[&str] = &[
    "int",
    "long",
    "unsigned",
    "char",
    "bool",
    "auto",
    "const",
    "void",
    "for",
    "while",
    "do",
    "if",
    "else",
    "return",
    "break",
    "continue",
    "switch",
    "case",
    "default",
    "std",
    "vector",
    "string",
    "map",
    "set",
    "pair",
    "tuple",
    "optional",
    "stack",
    "queue",
    "deque",
    "priority_queue",
    "unordered_map",
    "unordered_set",
    "array",
    "function",
    "list",
    "true",
    "false",
    "nullptr",
    "new",
    "delete",
    "struct",
    "using",
    "namespace",
    "static_cast",
    "include",
    "size_t",
    "greater",
    "less",
];

const OPS: &[&str] = &[
    "+", "-", "*", "/", "%", "<", ">", "<=", ">=", "==", "!=", "&&", "||", "&", "|", "<<", ">>",
    "++", "--",
];
const NUMS: &[&str] = &[
    "0",
    "1",
    "2",
    "-1",
    "7",
    "255",
    "65536",
    "2147483647",
    "2147483648",
    "4294967295",
    "99999999999",
];

/// A token of the same class as `t` (a number for a number, an operator for an operator, an identifier of the
/// file for an identifier), chosen by `pick`.
fn same_class(t: &[String], i: usize, pick: usize) -> Option<String> {
    let k = kind_of(&t[i]);
    let word = |x: &str| !KEYWORDS.contains(&x.trim());
    let after_scope = i > 0 && matches!(t[i - 1].trim(), "::" | "." | "->");
    if k == 2 && (!word(&t[i]) || after_scope) {
        return None;
    }
    let pool: Vec<&String> = t
        .iter()
        .enumerate()
        .filter(|(n, x)| {
            kind_of(x) == k
                && x.trim() != t[i].trim()
                && (k != 2 || (word(x) && !(*n > 0 && matches!(t[n - 1].trim(), "::" | "."))))
        })
        .map(|(_, x)| x)
        .collect();
    match k {
        1 => Some(format!("{} ", NUMS[pick % NUMS.len()])),
        4 if OPS.contains(&t[i].trim()) => Some(format!("{} ", OPS[pick % OPS.len()])),
        _ if !pool.is_empty() => Some(pool[pick % pool.len()].clone()),
        _ => None,
    }
}

fn mutate(src: &str, muts: &[Mutation]) -> String {
    let mut t = tokens(src);
    // leave the `#include` lines alone: breaking them only tests the include handling
    let start = t
        .iter()
        .rposition(|x| x.trim() == "include")
        .map_or(0, |p| (p + 4).min(t.len().saturating_sub(1)));
    for m in muts {
        if t.len() <= start {
            break;
        }
        let i = start + usize::from(m.a) % (t.len() - start);
        let j = start + usize::from(m.b) % (t.len() - start);
        match m.kind % 16 {
            0 => {
                t.remove(i);
            }
            1 => {
                let x = t[i].clone();
                t.insert(i, x);
            }
            2 => {
                let w = VOCAB[usize::from(m.b) % VOCAB.len()];
                t.insert(i, format!("{w} "));
            }
            3 => {
                let w = VOCAB[usize::from(m.b) % VOCAB.len()];
                t[i] = format!("{w} ");
            }
            4 => t.swap(i, j),
            5 => t[i] = t[j].clone(),
            // the rest keep the program well formed more often: same-class replacements
            _ => {
                if let Some(x) = same_class(&t, i, usize::from(m.b)) {
                    t[i] = x;
                }
            }
        }
    }
    t.concat()
}

/// Default argument text for a parameter type; `None` when the fuzzer cannot make one.
fn default_arg(t: &Ty) -> Option<String> {
    Some(match t {
        Ty::Bool => "true".into(),
        Ty::Char => "'a'".into(),
        Ty::Int | Ty::UInt | Ty::LongLong | Ty::ULongLong | Ty::Size => "3".into(),
        Ty::Str | Ty::StrView => "\"abc\"".into(),
        Ty::Vector(e) => format!("{{{0}, {0}}}", default_arg(e)?),
        Ty::Ptr(_) => "{1, 2, 3}".into(),
        _ => return None,
    })
}

/// What happened to one mutant.
#[derive(Debug, Default)]
struct Outcome {
    parsed: bool,
    checked: bool,
    ran: usize,
}

/// Run the whole pipeline on a mutant. Panics here are failures of the property.
fn pipeline(src: &str) -> Result<Outcome, String> {
    let mut out = Outcome::default();
    let Ok(prog) = parse(src, &ParseOpts::default()) else {
        return Ok(out);
    };
    out.parsed = true;
    let printed = print_program(&prog, false);
    if let Err(ds) = parse(&printed, &ParseOpts::default()) {
        return Err(format!(
            "what parses does not print as parseable text: {}\n--- printed:\n{printed}",
            ds[0].msg
        ));
    }
    let Ok(c) = compile("", src, &CheckOpts::all()) else {
        return Ok(out);
    };
    out.checked = true;
    let limits = Limits {
        steps: 200_000,
        depth: 60,
        cells: 100_000,
        ..Limits::default()
    };
    let fns: Vec<_> = c
        .program
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Func(f) if f.body.is_some() => Some(f),
            _ => None,
        })
        .collect();
    for f in fns.iter().take(4) {
        let Some(args) = f
            .params
            .iter()
            .map(|p| default_arg(&p.spec.ty))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Ok(called) = c.call(&f.name, &args, &limits) else {
            continue;
        };
        out.ran += 1;
        if let Err(e) = &called.report.result {
            if e.kind == RtKind::Internal && !KNOWN_INTERNAL.iter().any(|(m, _)| e.msg.contains(m))
            {
                return Err(format!("interpreter failure in `{}`: {}", f.name, e.msg));
            }
        }
    }
    Ok(out)
}

/// `pipeline` on a thread with a wall clock bound.
fn bounded(src: String) -> Result<Outcome, String> {
    let (tx, rx) = mpsc::channel();
    let s = src.clone();
    let h = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let _ = tx.send(pipeline(&s));
        })
        .expect("thread");
    match rx.recv_timeout(Duration::from_secs(20)) {
        Ok(r) => {
            let _ = h.join();
            r
        }
        Err(mpsc::RecvTimeoutError::Timeout) => Err("did not finish within 20 s".into()),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let msg = match h.join() {
                Err(p) => p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_default(),
                Ok(()) => String::new(),
            };
            Err(format!("panic: {msg}"))
        }
    }
}

fn mutation() -> impl Strategy<Value = Mutation> {
    (any::<u8>(), any::<u16>(), any::<u16>()).prop_map(|(kind, a, b)| Mutation { kind, a, b })
}

#[test]
fn the_unmutated_corpus_goes_through_the_pipeline() {
    let mut bad = Vec::new();
    let mut ran = 0;
    for (name, src) in corpus() {
        match bounded(src.clone()) {
            Ok(o) => ran += o.ran,
            Err(e) => bad.push(format!("{name}: {e}")),
        }
    }
    assert!(ran > 20, "the harness ran only {ran} entry calls");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: prop_common::case_count(300) as u32,
        max_shrink_iters: 200,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn mutants_never_panic_or_hang(idx in 0usize..1000, muts in proptest::collection::vec(mutation(), 1..4)) {
        let (name, src) = &corpus()[idx % corpus().len()];
        let m = mutate(src, &muts);
        if let Err(e) = bounded(m.clone()) {
            return Err(TestCaseError::fail(format!("{name}: {e}\n--- mutant:\n{m}")));
        }
    }
}

/// The mutants must be a useful mix: many still parse, a good share is accepted by the checker and runs.
#[test]
fn mutants_cover_every_stage_of_the_pipeline() {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let (mut total, mut parsed, mut checked, mut ran) = (0usize, 0usize, 0usize, 0usize);
    for _ in 0..300 {
        let (_, src) = &corpus()[(next() % corpus().len() as u64) as usize];
        let muts: Vec<Mutation> = (0..1 + next() % 2)
            .map(|_| Mutation {
                kind: next() as u8,
                a: next() as u16,
                b: next() as u16,
            })
            .collect();
        let m = mutate(src, &muts);
        let o = bounded(m.clone()).unwrap_or_else(|e| panic!("{e}\n--- mutant:\n{m}"));
        if !o.parsed && std::env::var("ALGOCLERK_PROP_DEBUG").is_ok() {
            if let Err(d) = parse(&m, &ParseOpts::default()) {
                eprintln!(
                    "PARSE FAIL kinds {:?}: {}",
                    muts.iter().map(|m| m.kind % 16).collect::<Vec<_>>(),
                    d[0].msg
                );
            }
        }
        total += 1;
        parsed += usize::from(o.parsed);
        checked += usize::from(o.checked);
        ran += usize::from(o.ran > 0);
    }
    eprintln!("mutants: {total} total, {parsed} parse, {checked} check, {ran} run");
    assert!(
        parsed * 7 >= total,
        "fewer than 14% of the mutants parse: {parsed} of {total}"
    );
    assert!(
        checked * 15 >= total,
        "fewer than 6% of the mutants check: {checked} of {total}"
    );
    assert!(
        ran * 20 >= total,
        "fewer than 5% of the mutants run: {ran} of {total}"
    );
}
