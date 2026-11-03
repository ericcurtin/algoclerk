// SPDX-License-Identifier: Apache-2.0
//! Differential tests: every program of `tests/data/cpp_interp/*.cpp` defines `test()`; the real compiler
//! (`g++ -std=c++17 -O0 -Wall -Wextra`) runs it and prints the result in the brace notation of
//! `cpp::value::render`; the interpreter must print exactly the same text.
//!
//! Skipped (with a message) when `g++` is not installed. Set `ALGOCLERK_DIFF_CLANG=1` to also compile every
//! program with `clang++` and require the same output (this validates the corpus itself).

use algoclerk_core::cpp::check::{CheckOpts, check};
use algoclerk_core::cpp::interp::{Limits, run};
use algoclerk_core::cpp::parse::{ParseOpts, parse};
mod common;
use algoclerk_core::cpp::value::{Heap, render};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// Corpus files run through the real parser and the checker (the others still use the test-support parser).
const REAL_FRONT_END: &[&str] = &["overloads", "smallints", "fixes"];

const PRINTER: &str = r#"
#include <cstdio>
#include <deque>
#include <list>
#include <map>
#include <optional>
#include <queue>
#include <set>
#include <string>
#include <tuple>
#include <type_traits>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>
#include <array>
namespace pr {
inline void esc(std::string& o, char ch, char quote) {
    unsigned char c = static_cast<unsigned char>(ch);
    if (c == '\n') o += "\\n";
    else if (c == '\t') o += "\\t";
    else if (c == '\r') o += "\\r";
    else if (c == 0) o += "\\0";
    else if (c == '\\') o += "\\\\";
    else if (ch == quote) { o += '\\'; o += ch; }
    else if (c >= 0x20 && c < 0x7f) o += ch;
    else { char buf[8]; std::snprintf(buf, sizeof buf, "\\x%02x", c); o += buf; }
}
template <class T> void p(std::string& o, const T& v);
inline void p(std::string& o, ListNode* n) {
    if (!n) { o += "nullptr"; return; }
    o += '{';
    bool first = true;
    for (ListNode* c = n; c; c = c->next) {
        if (!first) o += ", ";
        first = false;
        o += std::to_string(c->val);
    }
    o += '}';
}
inline void p(std::string& o, TreeNode* n) {
    if (!n) { o += "nullptr"; return; }
    std::vector<TreeNode*> items;
    std::queue<TreeNode*> q;
    q.push(n);
    while (!q.empty()) {
        TreeNode* c = q.front();
        q.pop();
        items.push_back(c);
        if (c) { q.push(c->left); q.push(c->right); }
    }
    while (!items.empty() && items.back() == nullptr) items.pop_back();
    o += '{';
    for (size_t i = 0; i < items.size(); ++i) {
        if (i) o += ", ";
        if (items[i]) o += std::to_string(items[i]->val); else o += "nullptr";
    }
    o += '}';
}
template <class T> struct is_opt : std::false_type {};
template <class T> struct is_opt<std::optional<T>> : std::true_type {};
template <class T> struct is_pair : std::false_type {};
template <class A, class B> struct is_pair<std::pair<A, B>> : std::true_type {};
template <class T> struct is_tuple : std::false_type {};
template <class... A> struct is_tuple<std::tuple<A...>> : std::true_type {};
template <class T, class = void> struct has_mapped : std::false_type {};
template <class T> struct has_mapped<T, std::void_t<typename T::mapped_type>> : std::true_type {};
template <class T, class = void> struct iterable : std::false_type {};
template <class T> struct iterable<T, std::void_t<decltype(std::declval<const T&>().begin())>> : std::true_type {};
template <class T> void p(std::string& o, const T& v) {
    if constexpr (std::is_same_v<T, bool>) o += v ? "true" : "false";
    else if constexpr (std::is_same_v<T, char>) { o += '\''; esc(o, v, '\''); o += '\''; }
    else if constexpr (std::is_integral_v<T>) o += std::to_string(v);
    else if constexpr (std::is_same_v<T, std::string>) { o += '"'; for (char c : v) esc(o, c, '"'); o += '"'; }
    else if constexpr (is_opt<T>::value) {
        if (!v) o += "nullopt"; else { o += '{'; p(o, *v); o += '}'; }
    }
    else if constexpr (is_pair<T>::value) { o += '{'; p(o, v.first); o += ", "; p(o, v.second); o += '}'; }
    else if constexpr (is_tuple<T>::value) {
        o += '{';
        bool first = true;
        std::apply([&](const auto&... xs) { ((o += (first ? "" : ", "), first = false, p(o, xs)), ...); }, v);
        o += '}';
    }
    else if constexpr (has_mapped<T>::value) {
        o += '{';
        bool first = true;
        for (const auto& kv : v) {
            if (!first) o += ", ";
            first = false;
            o += '{'; p(o, kv.first); o += ", "; p(o, kv.second); o += '}';
        }
        o += '}';
    }
    else if constexpr (iterable<T>::value) {
        o += '{';
        bool first = true;
        for (const auto& x : v) {
            if (!first) o += ", ";
            first = false;
            p(o, x);
        }
        o += '}';
    }
    else o += "?";
}
}
"#;

struct Case {
    file: String,
    name: String,
    src: String,
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cpp_interp")
}

fn load(file: &str) -> Vec<Case> {
    let path = data_dir().join(format!("{file}.cpp"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out: Vec<Case> = Vec::new();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("//// ") {
            out.push(Case {
                file: file.to_string(),
                name: name.trim().to_string(),
                src: String::new(),
            });
        } else if let Some(c) = out.last_mut() {
            c.src.push_str(line);
            c.src.push('\n');
        }
    }
    out
}

fn split_includes(src: &str) -> (Vec<String>, String) {
    let mut inc = Vec::new();
    let mut body = String::new();
    for l in src.lines() {
        if l.trim_start().starts_with("#include") {
            inc.push(l.trim().to_string());
        } else {
            body.push_str(l);
            body.push('\n');
        }
    }
    (inc, body)
}

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Compile and run one case with a real compiler; returns what `test()` printed.
fn real(case: &Case, cxx: &str, dir: &Path) -> Result<String, String> {
    let (inc, body) = split_includes(&case.src);
    let mut prog = String::new();
    for i in &inc {
        prog.push_str(i);
        prog.push('\n');
    }
    prog.push_str(PRELUDE);
    prog.push_str(PRINTER);
    prog.push_str("namespace cs {\n");
    prog.push_str(&body);
    prog.push_str("}\nint main() {\n    std::string o;\n    pr::p(o, cs::test());\n    std::puts(o.c_str());\n    return 0;\n}\n");
    let tag = format!("{}_{}_{}", case.file, case.name, cxx.replace('+', "p"));
    let cpp = dir.join(format!("{tag}.cpp"));
    let exe = dir.join(tag);
    std::fs::write(&cpp, prog).map_err(|e| e.to_string())?;
    let out = Command::new("timeout")
        .args([
            "-s",
            "KILL",
            "60",
            cxx,
            "-std=c++17",
            "-O0",
            "-Wall",
            "-Wextra",
            "-o",
        ])
        .arg(&exe)
        .arg(&cpp)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "does not compile with {cxx}:\n{}",
            String::from_utf8_lossy(&out.stderr)
                .chars()
                .take(3000)
                .collect::<String>()
        ));
    }
    let run = Command::new("timeout")
        .args(["-s", "KILL", "20"])
        .arg(&exe)
        .output()
        .map_err(|e| e.to_string())?;
    if !run.status.success() {
        return Err(format!("{cxx}-compiled program failed: {:?}", run.status));
    }
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(&cpp);
    Ok(String::from_utf8_lossy(&run.stdout)
        .trim_end_matches('\n')
        .to_string())
}

/// Run one case in the interpreter and render the result like the printer above.
fn interp(case: &Case) -> Result<String, String> {
    let (inc, body) = split_includes(&case.src);
    let mut src = String::new();
    src.push_str(PRELUDE);
    for i in &inc {
        src.push_str(i);
        src.push('\n');
    }
    src.push_str(&body);
    let prog = if REAL_FRONT_END.contains(&case.file.as_str()) {
        // the real parser and the checker: the program must be accepted exactly like a player's
        let p = parse(
            &src,
            &ParseOpts {
                first_id: 0,
                known_types: vec!["ListNode".into(), "TreeNode".into()],
            },
        )
        .map_err(|d| format!("parse error: {}", d[0].msg))?;
        if let Err(ds) = check(&p, &CheckOpts::all()) {
            return Err(format!(
                "checker refuses it: {}",
                ds.iter()
                    .map(|d| format!("[{}] {}", d.code, d.msg))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        p
    } else {
        common::parse_any(&src, &["ListNode", "TreeNode"])
            .map_err(|d| format!("parse error: {d}"))?
    };
    let f = prog.find("test").ok_or("no test()")?;
    let ty = f.ret.ty.clone();
    let lim = Limits {
        steps: 200_000_000,
        ..Limits::default()
    };
    let rep = run(&prog, "test", vec![], Heap::default(), &lim);
    match rep.result {
        Ok(v) => Ok(render(&v, &ty, &rep.heap)),
        Err(e) => {
            let line = prog
                .span(e.node)
                .map(|s| algoclerk_core::cpp::diag::line_col(&src, s.start).0)
                .unwrap_or(0);
            Err(format!(
                "runtime error {:?} at line {line}: {}",
                e.kind, e.msg
            ))
        }
    }
}

fn run_file(file: &str) {
    let cases = load(file);
    assert!(!cases.is_empty(), "no cases in {file}");
    let have_gpp = have("g++");
    if !have_gpp {
        eprintln!("g++ not found: differential test {file} skipped");
        return;
    }
    let clang = have("clang++") && std::env::var("ALGOCLERK_DIFF_CLANG").is_ok();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cpp_interp_diff");
    std::fs::create_dir_all(&dir).unwrap();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::<(usize, Result<String, String>)>::new());
    std::thread::scope(|sc| {
        for _ in 0..8 {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(c) = cases.get(i) else { break };
                    let mut r = real(c, "g++", &dir);
                    if clang {
                        if let (Ok(a), Ok(b)) = (&r, real(c, "clang++", &dir)) {
                            if *a != b {
                                r = Err(format!(
                                    "g++ and clang++ disagree:\n g++: {a}\n clang: {b}"
                                ));
                            }
                        } else if r.is_ok() {
                            r = Err("clang++ failed on a program g++ accepts".into());
                        }
                    }
                    results.lock().unwrap().push((i, r));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(i, _)| *i);
    let mut failures = Vec::new();
    for (i, expected) in results {
        let c = &cases[i];
        let got = interp(c);
        match (expected, got) {
            (Ok(e), Ok(g)) if e == g => {}
            (Ok(e), Ok(g)) => failures.push(format!(
                "[{}::{}] MISMATCH\n  g++    : {e}\n  interp : {g}",
                c.file, c.name
            )),
            (Ok(e), Err(g)) => failures.push(format!(
                "[{}::{}] interpreter failed: {g}\n  g++    : {e}",
                c.file, c.name
            )),
            (Err(e), _) => failures.push(format!(
                "[{}::{}] corpus program broken: {e}",
                c.file, c.name
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} programs differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

macro_rules! diff {
    ($($name:ident),*) => {$(
        #[test]
        fn $name() {
            run_file(stringify!($name));
        }
    )*};
}

diff!(
    arith, control, strings, vectors, assoc, adaptors, algos, lambdas, structs, misc, overloads,
    smallints, fixes
);
