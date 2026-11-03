// SPDX-License-Identifier: Apache-2.0
//! Locks the abstract costs documented in `docs/COST_MODEL.md`: each row runs one statement after a setup and
//! compares the cost of that statement (without the 1 for the statement itself) with the documented formula.
//! Arguments are plain variables, so only the operation is measured; a named iterator, pointer or closure
//! passed as an argument costs 1 (it is copied), which the rows account for.

use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::interp::Limits;
use algoclerk_core::cpp::run::compile;

const INC: &str = "#include <algorithm>\n#include <numeric>\n#include <vector>\n#include <string>\n\
#include <map>\n#include <set>\n#include <unordered_map>\n#include <unordered_set>\n#include <deque>\n\
#include <list>\n#include <queue>\n#include <stack>\n#include <array>\n#include <iterator>\n";

fn cost(body: &str) -> Result<u64, String> {
    let src = format!("{INC}int f() {{\n{body}\nreturn 0;\n}}\n");
    let c = compile("", &src, &CheckOpts::all())
        .map_err(|ds| format!("{}: {}", ds[0].code, ds[0].msg))?;
    let r = c.call("f", &[], &Limits::default())?;
    match r.report.result {
        Ok(_) => Ok(r.report.cost),
        Err(e) => Err(format!("{:?}: {}", e.kind, e.msg)),
    }
}

/// (setup, statement, documented cost of the statement's expression).
type Row = (&'static str, &'static str, u64);

const STR: &str = "std::string s = \"abcdef\"; std::string t = \"xy\"; char c = 'd'; size_t i = 2; size_t k = 2;";
const VEC: &str = "std::vector<int> v(10); std::vector<int> w(10); auto b = v.begin() + 3; auto e = v.begin() + 5; int x = 1; size_t k = 2;";
const ALG: &str = "std::vector<int> v = {3, 1, 2, 5, 4}; auto b = v.begin(); auto e = v.end(); auto m = b + 1; int x = 2;";
const SORTED: &str = "std::vector<int> v = {1, 2, 3, 4, 5}; auto b = v.begin(); auto e = v.end(); int x = 3;";
const LST: &str = "std::list<int> l = {1, 2, 3, 4, 5}; auto b = l.begin(); auto e = l.end(); int x = 1; size_t k = 2;";
const MAP: &str = "std::map<int, int> m = {{1, 1}, {2, 2}, {3, 3}}; auto it = m.begin(); int x = 5; std::pair<int, int> p = {5, 5};";
const UMAP: &str = "std::unordered_map<int, int> m = {{1, 1}, {2, 2}, {3, 3}}; int x = 2;";
const PQ: &str = "std::priority_queue<int> q; q.push(1); q.push(2); q.push(3); int x = 4;";
const DQ: &str = "std::deque<int> d(10); auto b = d.begin() + 2; auto e = d.begin() + 8; int x = 1;";

fn rows() -> Vec<Row> {
    vec![
        // strings: the call node is 1; find-like calls pay 1 more plus the positions examined
        (STR, "s.size();", 1),
        (STR, "s += t;", 3),
        (STR, "std::string u = s + c;", 8),
        (STR, "s.find(c);", 7),
        (STR, "c = 'z'; s.find(c);", 11),
        (STR, "s.rfind(c);", 6),
        (STR, "s.find_first_of(t);", 8),
        (STR, "s.compare(t);", 4),
        (STR, "bool r = s < t;", 4),
        (STR, "s.insert(i, t);", 7),
        (STR, "s.erase(i, k);", 5),
        (STR, "s.resize(i);", 3),
        (STR, "s.resize(9);", 10),
        (STR, "std::string u = s.substr(i, k);", 3),
        (STR, "std::string u = std::to_string(123);", 4),
        (STR, "s = \"123456\"; int r = std::stoi(s);", 10),
        (STR, "s = std::string(i, c);", 4),
        // vector: iterators passed by name cost 1 each
        (VEC, "v.insert(b, x);", 9),
        (VEC, "v.insert(b, k, x);", 11),
        (VEC, "v.erase(b);", 9),
        (VEC, "v.erase(b, e);", 10),
        (VEC, "v.resize(k);", 3),
        (VEC, "v.resize(20);", 21),
        (VEC, "v.assign(k, x);", 3),
        (VEC, "v.clear();", 11),
        (VEC, "bool r = v == w;", 12),
        (VEC, "std::vector<int> u(v);", 10),
        (VEC, "w = v;", 11),
        (VEC, "v.push_back(x);", 1),
        (VEC, "v.swap(w);", 1),
        // deque, list
        (DQ, "d.insert(b, x);", 5),
        (DQ, "d.erase(b);", 4),
        (DQ, "d.clear();", 11),
        (LST, "l.erase(b, e);", 7),
        (LST, "l.erase(b);", 2),
        (LST, "l.insert(b, x);", 3),
        (LST, "l.resize(k);", 3),
        (LST, "l.reverse();", 6),
        (LST, "l.sort();", 16),
        // ordered and unordered containers, priority queue
        (MAP, "m.find(x);", 3),
        (MAP, "m[x];", 3),
        (MAP, "m.insert(p);", 5),
        (MAP, "m.erase(it);", 4),
        (MAP, "m.clear();", 4),
        (UMAP, "m.find(x);", 1),
        (UMAP, "m[x];", 1),
        (UMAP, "m.clear();", 4),
        (PQ, "q.push(x);", 4),
        (PQ, "q.pop();", 4),
        (PQ, "q.top();", 1),
        // algorithms: 1 for the call, the iterator arguments (1 each), then the elements touched
        (ALG, "std::sort(b, e);", 18),
        (ALG, "std::reverse(b, e);", 8),
        (ALG, "std::rotate(b, m, e);", 9),
        (ALG, "std::count(b, e, x);", 8),
        (ALG, "std::find(b, e, x);", 6),
        (ALG, "std::accumulate(b, e, x);", 8),
        (ALG, "std::next_permutation(b, e);", 8),
        (ALG, "std::for_each(b, e, [](int& a) { a = a % 100; });", 29),
        (ALG, "std::partial_sum(b, e, b);", 14),
        (SORTED, "std::binary_search(b, e, x);", 6),
        (SORTED, "std::lower_bound(b, e, x);", 6),
        (SORTED, "auto d = std::distance(b, e);", 3),
    ]
}

#[test]
fn documented_costs_match_the_interpreter() {
    let mut bad = Vec::new();
    for (setup, stmt, want) in rows() {
        let base = cost(setup);
        let full = cost(&format!("{setup}\n{stmt}"));
        match (base, full) {
            (Ok(b), Ok(f)) => {
                let got = f - b - 1;
                if got != want {
                    bad.push(format!("`{stmt}`: documented {want}, interpreter charges {got}"));
                }
            }
            (Err(e), _) | (_, Err(e)) => bad.push(format!("`{stmt}`: {e}")),
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn copy_rules() {
    // scalars are free to copy, iterators, pointers and closures cost 1, containers their leaves
    let c = |setup: &str, stmt: &str| {
        let b = cost(setup).expect("setup");
        cost(&format!("{setup}\n{stmt}")).expect("stmt") - b - 1
    };
    let it = "std::vector<int> v(10); auto b = v.begin();";
    assert_eq!(c("int x = 1;", "int y = x;"), 0);
    assert_eq!(c(it, "auto d = b;"), 1);
    assert_eq!(c("std::vector<int> v(10);", "auto w = v;"), 10);
    assert_eq!(c("std::vector<int> v(10);", "auto& w = v;"), 0);
    assert_eq!(c("std::vector<std::vector<int>> v(3, std::vector<int>(4));", "auto w = v;"), 12);
    assert_eq!(c("std::vector<std::vector<int>> v(3, std::vector<int>(4));", "for (auto& x : v) { }"), 4);
    assert_eq!(c("std::string s = \"abcdef\";", "std::string t = s;"), 6);
}
