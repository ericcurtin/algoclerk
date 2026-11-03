// SPDX-License-Identifier: Apache-2.0
//! Programs the C++ checker must reject, each with the stable diagnostic code it must report, and a few
//! tricky valid programs it must accept.

use algoclerk_core::cpp::check::{CheckOpts, EntrySig, check};
use algoclerk_core::cpp::diag::Diag;
use algoclerk_core::cpp::parse::{ParseOpts, parse};

fn diags(src: &str, opts: &CheckOpts) -> Vec<Diag> {
    let prog =
        parse(src, &ParseOpts::default()).unwrap_or_else(|d| panic!("parse failed: {d:?}\n{src}"));
    match check(&prog, opts) {
        Ok(_) => Vec::new(),
        Err(d) => d,
    }
}

/// (snippet, expected code, a word the friendly message must contain)
const NEGATIVE: &[(&str, &str, &str)] = &[
    (
        "#include <algorithm>\nint f(int a, long long b) { return std::max(a, b); }",
        "type.deduce",
        "same type",
    ),
    (
        "int f(long long n) { int x{n}; return x; }",
        "type.narrowing",
        "narrowing",
    ),
    (
        "#include <vector>\nint f(const std::vector<int>& v) { std::vector<int> w{v.size()}; return 0; }",
        "type.narrowing",
        "narrowing",
    ),
    (
        "int f() { unsigned a = 1; int b{a}; return b; }",
        "type.narrowing",
        "narrowing",
    ),
    (
        "#include <string>\nstd::string f(std::string s) { return s + 1; }",
        "type.binary",
        "std::to_string",
    ),
    (
        "int f() { return missing + 1; }",
        "name.undeclared",
        "missing",
    ),
    ("int f() { return helper(2); }", "name.function", "helper"),
    (
        "int f() { return later(2); }\nint later(int x) { return x; }",
        "name.function",
        "further down",
    ),
    (
        "int f() { std::vector<int> v; return 0; }",
        "name.header",
        "<vector>",
    ),
    (
        "#include <vector>\nint f() { std::vector<int> v{2, 1}; std::sort(v.begin(), v.end()); return 0; }",
        "name.header",
        "<algorithm>",
    ),
    ("void f(const int x) { x = 1; }", "const.assign", "const"),
    (
        "#include <vector>\nvoid f(const std::vector<int>& v) { v.push_back(1); }",
        "const.method",
        "push_back",
    ),
    (
        "#include <vector>\nvoid f(const std::vector<int>& v) { v[0] = 1; }",
        "const.assign",
        "const",
    ),
    (
        "void g(int& x) { x = 1; }\nvoid f() { g(5); }",
        "type.temporary",
        "temporary",
    ),
    (
        "void g(int& x) { x = 1; }\nvoid f() { const int a = 1; g(a); }",
        "const.ref",
        "const",
    ),
    (
        "#include <string>\nint f() { return std::string(\"a\"); }",
        "type.mismatch",
        "std::string",
    ),
    (
        "void g() {}\nint f() { int x = g(); return x; }",
        "type.void",
        "void",
    ),
    (
        "int g(int a) { return a; }\nint f() { return g(1, 2); }",
        "type.arity",
        "argument",
    ),
    (
        "#include <vector>\nint f(std::vector<int> v) { return v.length(); }",
        "name.member",
        "length",
    ),
    (
        "int f() { int a = 1; int a = 2; return a; }",
        "name.duplicate",
        "already declared",
    ),
    ("int f() { auto a; return 0; }", "type.auto", "auto"),
    (
        "#include <string>\nint f(std::string s) { if (s) return 1; return 0; }",
        "type.condition",
        "yes/no",
    ),
    (
        "int f(int n) { auto g = [](int x) { return x + n; }; return g(1); }",
        "name.capture",
        "capture",
    ),
    (
        "int f() { int c = 0; auto g = [c]() { return ++c; }; return g(); }",
        "const.assign",
        "const",
    ),
    (
        "#include <string>\nint f(std::string s) { int x = s; return x; }",
        "type.mismatch",
        "std::stoi",
    ),
    (
        "#include <vector>\n#include <string>\nint f(std::vector<int> v, std::string k) { return v[k]; }",
        "type.index",
        "index",
    ),
    (
        "#include <map>\nint f(const std::map<int, int>& m) { return m[1]; }",
        "const.method",
        "at",
    ),
    (
        "#include <utility>\nint f(std::pair<int, int> p) { auto [a, b, c] = p; return a; }",
        "type.structured",
        "part",
    ),
    (
        "#include <list>\nint f(std::list<int> l) { auto it = l.begin() + 1; return *it; }",
        "type.binary",
        "std::next",
    ),
    (
        "int f(int a) { if (a > 0) return 1; }",
        "flow.missing-return",
        "return",
    ),
    (
        "struct P { int x; };\nint f(P p) { return p.z; }",
        "name.member",
        "z",
    ),
    (
        "int f() { int s = 0; for (int x : 5) s += x; return s; }",
        "type.range",
        "container",
    ),
    (
        "#include <vector>\nint g();\nvoid f() { auto& r = g(); }",
        "type.temporary",
        "temporary",
    ),
    ("int f(int* p) { return p.x; }", "type.arrow", "->"),
    (
        "#include <vector>\nvoid f(std::vector<int>& v) { v.push_back(\"a\"); }",
        "type.mismatch",
        "const char*",
    ),
    (
        "#include <vector>\nint f(std::vector<int> v) { return v.size(1); }",
        "type.arity",
        "argument",
    ),
    ("int f(int a) { return a.size(); }", "name.member", "size"),
    (
        "#include <cstdint>\nint f() { return 1; }",
        "unsupported.header",
        "cstdint",
    ),
    (
        "int f(int i) { return i + i++; }",
        "unseq.modify",
        "split it",
    ),
    ("int f(int i) { return i++ + i++; }", "unseq.modify", "`i`"),
    (
        "int g(int a, int b) { return a + b; }\nint f(int i) { return g(i++, i); }",
        "unseq.modify",
        "no order",
    ),
    (
        "int f(int n) { auto g = [](auto a, auto b) { return a + b; }; return g(n, nullptr); }",
        "type.binary",
        "nullptr",
    ),
];

#[test]
fn rejected_with_the_right_code() {
    assert!(NEGATIVE.len() >= 30);
    let mut bad = Vec::new();
    for (src, code, word) in NEGATIVE {
        let ds = diags(src, &CheckOpts::all());
        match ds.iter().find(|d| d.code == *code) {
            None => bad.push(format!(
                "{src}\n  wanted {code}, got {:?}",
                ds.iter()
                    .map(|d| (d.code, d.msg.as_str()))
                    .collect::<Vec<_>>()
            )),
            Some(d) => {
                if !d.msg.contains(word) || d.node.is_none() {
                    bad.push(format!(
                        "{src}\n  message `{}` should mention `{word}` and carry a node",
                        d.msg
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// Valid C++17 that must not be rejected (tricky typing).
const POSITIVE: &[&str] = &[
    "#include <algorithm>\nint f(int a, int b) { return std::max(a, b); }",
    "#include <algorithm>\nlong long f(int a) { return std::max<long long>(a, 5LL); }",
    "#include <algorithm>\nlong long f(long long a) { return std::max(a, 5LL); }",
    "#include <string>\nstd::string f(std::string s) { s += 'x'; s = s + \"y\" + 'z'; return \"a\" + s; }",
    "#include <vector>\nint f(const std::vector<int>& v) { int s = 0; for (const auto& x : v) s += x; return s + v[0] + v.back(); }",
    "#include <map>\nint f(const std::map<int, int>& m) { int s = 0; for (const auto& [k, v] : m) s += k + v; return s + m.at(1); }",
    "#include <unordered_map>\nint f(std::unordered_map<int, int>& m) { for (auto& [k, v] : m) v += k; auto it = m.find(3); if (it != m.end()) return it->second; return 0; }",
    "#include <vector>\nstd::vector<int> f() { return {1, 2, 3}; }",
    "#include <vector>\nstd::vector<std::vector<int>> f(int n) { std::vector<std::vector<int>> g(n, std::vector<int>(n, 0)); g[0][0] = 1; return g; }",
    "#include <vector>\n#include <algorithm>\nvoid f(std::vector<int>& v) { std::sort(v.begin(), v.end(), [](int a, int b) { return a > b; }); }",
    "#include <vector>\n#include <algorithm>\nvoid f(std::vector<int>& v) { std::sort(v.begin(), v.end(), [](const auto& a, const auto& b) { return a < b; }); }",
    "#include <functional>\nint f() { std::function<int(int)> fib = [&](int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }; return fib(10); }",
    "int f() { char c = 'a'; int x = c + 1; unsigned u = 3; long long w = u + 5LL; return x + static_cast<int>(w); }",
    "#include <vector>\nint f(const std::vector<int>& v) { int n = v.size(); for (size_t i = 0; i < v.size(); ++i) n += v[i]; return n; }",
    "#include <optional>\nstd::optional<int> f(bool b) { if (b) return 1; return std::nullopt; }",
    "#include <numeric>\n#include <vector>\nint f(const std::vector<int>& v) { return std::accumulate(v.begin(), v.end(), 0); }",
    "#include <functional>\n#include <queue>\n#include <vector>\nint f() { std::priority_queue<int, std::vector<int>, std::greater<int>> q; q.push(1); int t = q.top(); q.pop(); return t; }",
    "#include <set>\nint f(std::set<int> s) { auto it = s.lower_bound(2); if (it == s.end()) return -1; return *it; }",
    "struct N { int v; N* next; N(int x) : v(x), next(nullptr) {} };\nint f() { N* n = new N(1); n->next = new N(2); int r = n->next->v; delete n->next; delete n; return r; }",
    "#include <climits>\nint f() { int m = INT_MAX; long long big = LLONG_MAX; return m < 0 ? 0 : static_cast<int>(big % 7); }",
    "#include <string>\nint f(const std::string& s) { int n = 0; for (char c : s) if (c == 'a') ++n; return n + static_cast<int>(s.find('b')); }",
    "int f() { int a[3] = {1, 2, 3}; return a[0] + a[2]; }",
    "int f(int n) { switch (n) { case 1: return 1; case 2: return 2; default: return 0; } }",
    "int f(int n) { while (true) { if (n > 3) return n; ++n; } }",
    "int f(int i) { int a[4] = {0, 0, 0, 0}; a[i] = i++; i = i++; i = i + 1; int c = (i++, i); int d = i > 0 && i++ > 1 ? i : 0; return a[0] + c + d; }",
];

#[test]
fn tricky_valid_programs_are_accepted() {
    let mut bad = Vec::new();
    for src in POSITIVE {
        let ds = diags(src, &CheckOpts::all());
        if !ds.is_empty() {
            bad.push(format!(
                "{src}\n  {:?}",
                ds.iter()
                    .map(|d| (d.code, d.msg.as_str()))
                    .collect::<Vec<_>>()
            ));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

#[test]
fn locks_and_entry_signature() {
    let src = "#include <vector>\n#include <algorithm>\nint helper(int x) { return x; }\nint solve(std::vector<int> v) { std::sort(v.begin(), v.end()); return helper(v[0]); }";
    let mut o = CheckOpts::all();
    o.locked
        .insert("std::sort".into(), "sorting is for the next floor".into());
    assert!(
        diags(src, &o)
            .iter()
            .any(|d| d.code == "lock.name" && d.msg.contains("next floor"))
    );
    let mut o = CheckOpts::all();
    o.helpers = false;
    o.entry = Some(entry("solve"));
    assert!(diags(src, &o).iter().any(|d| d.code == "lock.helpers"));
    let mut o = CheckOpts::all();
    o.unlocked_headers = Some(["<vector>".to_string()].into_iter().collect());
    assert!(diags(src, &o).iter().any(|d| d.code == "lock.header"));
    let mut o = CheckOpts::all();
    o.entry = Some(entry("other"));
    assert!(diags(src, &o).iter().any(|d| d.code == "name.entry"));
    let mut o = CheckOpts::all();
    o.entry = Some(entry("solve"));
    assert!(diags(src, &o).is_empty(), "{:?}", diags(src, &o));
    let wrong = src.replace(
        "int solve(std::vector<int> v)",
        "int solve(std::vector<int>& v)",
    );
    let ds = diags(&wrong, &o);
    assert!(
        ds.iter()
            .any(|d| d.code == "type.entry" && d.msg.contains("std::vector<int> v")),
        "{ds:?}"
    );
}

fn entry(name: &str) -> EntrySig {
    let p = parse(
        "#include <vector>\nint solve(std::vector<int> v) { return 0; }",
        &ParseOpts::default(),
    )
    .unwrap();
    let f = p.find("solve").unwrap();
    EntrySig {
        name: name.into(),
        params: f.params.clone(),
        ret: f.ret.clone(),
    }
}

#[test]
fn recursion_and_structs_locks() {
    let rec = "int f(int n) { return n < 2 ? n : f(n - 1) + f(n - 2); }";
    let mut o = CheckOpts::all();
    o.recursion = false;
    assert!(diags(rec, &o).iter().any(|d| d.code == "lock.recursion"));
    let lam = "#include <functional>\nint g() { std::function<int(int)> h = [&](int n) { return n < 1 ? 0 : h(n - 1); }; return h(3); }";
    assert!(diags(lam, &o).iter().any(|d| d.code == "lock.recursion"));
    let st = "struct S { int a; };\nint f() { S s{1}; return s.a; }";
    o.structs = false;
    assert!(diags(st, &o).iter().any(|d| d.code == "lock.structs"));
}

fn level_diags(prelude: &str, player: &str, opts: &CheckOpts) -> Vec<Diag> {
    match algoclerk_core::cpp::run::compile(prelude, player, opts) {
        Ok(_) => Vec::new(),
        Err(d) => d,
    }
}

#[test]
fn reincluding_a_prelude_header_is_accepted() {
    let mut o = CheckOpts::all();
    o.unlocked_headers = Some(["<algorithm>".to_string()].into_iter().collect());
    let pre = "#include <vector>\n";
    let ok = "#include <vector>\n#include <algorithm>\nint f(const std::vector<int>& v) { return static_cast<int>(v.size()); }";
    assert!(level_diags(pre, ok, &o).is_empty());
    // a header that is neither in the prelude nor unlocked stays refused
    let bad = "#include <vector>\n#include <map>\nint f(const std::vector<int>& v) { return static_cast<int>(v.size()); }";
    let ds = level_diags(pre, bad, &o);
    assert!(
        ds.iter()
            .any(|d| d.code == "lock.header" && d.msg.contains("<map>")),
        "{ds:?}"
    );
    assert!(!ds.iter().any(|d| d.msg.contains("<vector>")), "{ds:?}");
}

#[test]
fn a_locked_header_shows_its_reason() {
    let mut o = CheckOpts::all();
    o.locked.insert(
        "<algorithm>".into(),
        "that drawer only holds search tools".into(),
    );
    let pre = "#include <vector>\n";
    // the include itself, with every header unlocked or not
    let inc = "#include <vector>\n#include <algorithm>\nint f(int a) { return a; }";
    for unlocked in [None, Some(Vec::new()), Some(vec!["<algorithm>".to_string()])] {
        o.unlocked_headers = unlocked.map(|v| v.into_iter().collect());
        let ds = level_diags(pre, inc, &o);
        assert!(
            ds.iter().any(|d| d.code == "lock.name"
                && d.msg.contains("does not stock `<algorithm>`: that drawer")),
            "{ds:?}"
        );
    }
    // using a name from the locked header without including it shows the lock, not "include it"
    o.unlocked_headers = None;
    let uses = "#include <vector>\nint f(std::vector<int> v) { std::sort(v.begin(), v.end()); return v[0]; }";
    let ds = level_diags(pre, uses, &o);
    assert!(
        ds.iter()
            .any(|d| d.code == "lock.name" && d.msg.contains("that drawer")),
        "{ds:?}"
    );
    assert!(!ds.iter().any(|d| d.code == "name.header"), "{ds:?}");
}

#[test]
fn a_locked_name_shows_its_reason_even_without_its_header() {
    let mut o = CheckOpts::all();
    o.locked
        .insert("std::lower_bound".into(), "halving is the lesson".into());
    let pre = "#include <vector>\n";
    let src = "#include <vector>\nint f(std::vector<int> v) { auto it = std::lower_bound(v.begin(), v.end(), 3); return static_cast<int>(it - v.begin()); }";
    let ds = level_diags(pre, src, &o);
    assert!(
        ds.iter()
            .any(|d| d.code == "lock.name" && d.msg.contains("halving is the lesson")),
        "{ds:?}"
    );
    assert!(!ds.iter().any(|d| d.code == "name.header"), "{ds:?}");
}
