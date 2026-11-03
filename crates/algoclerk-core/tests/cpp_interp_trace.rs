// SPDX-License-Identifier: Apache-2.0
//! What the interpreter's trace snapshots report: live locals at `return`, and iterator variables.

use algoclerk_core::cpp::ast::Ty;
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::interp::{Event, Limits, VarSnap};
use algoclerk_core::cpp::run::compile;
use algoclerk_core::cpp::value::{Plain, render};

fn limits() -> Limits {
    Limits {
        trace_events: 1000,
        ..Limits::default()
    }
}

fn events(src: &str, entry: &str, args: &[&str]) -> Vec<Event> {
    let c = compile("", src, &CheckOpts::all()).expect("compiles");
    let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let called = c.call(entry, &args, &limits()).expect("runs");
    called.report.result.as_ref().expect("no runtime error");
    called.report.trace.expect("trace").events
}

fn var<'a>(e: &'a Event, name: &str) -> &'a VarSnap {
    e.vars
        .iter()
        .find(|v| v.name == name)
        .unwrap_or_else(|| panic!("no variable {name} in {:?}", e.vars))
}

#[test]
fn a_returned_local_is_still_live_in_the_return_snapshot() {
    let src = "#include <vector>\n\
        int sumAll(const std::vector<int>& nums) {\n\
            int total = 0;\n\
            for (int x : nums) { total += x; }\n\
            return total;\n\
        }\n";
    let ev = events(src, "sumAll", &["{3, 1, 4, 1, 5}"]);
    let last = ev.last().unwrap();
    assert_eq!(last.returned, Some(Plain::Int(14)));
    let t = var(last, "total");
    assert_eq!((&t.ty, &t.value), (&Ty::Int, &Plain::Int(14)));
    assert!(last.vars.iter().all(|v| v.ty != Ty::Void));
}

#[test]
fn a_returned_vector_local_keeps_its_contents_in_the_snapshot() {
    let src = "#include <vector>\n\
        std::vector<int> twice(const std::vector<int>& a) {\n\
            std::vector<int> out;\n\
            for (int x : a) { out.push_back(2 * x); }\n\
            return out;\n\
        }\n";
    let ev = events(src, "twice", &["{1, 2}"]);
    let last = ev.last().unwrap();
    let o = var(last, "out");
    assert_eq!(o.ty, Ty::vector(Ty::Int));
    assert_eq!(o.value, Plain::Seq(vec![Plain::Int(2), Plain::Int(4)]));
    assert_eq!(last.returned, Some(o.value.clone()));
}

#[test]
fn tracing_does_not_change_the_cost() {
    let src = "#include <vector>\n\
        std::vector<int> twice(const std::vector<int>& a) {\n\
            std::vector<int> out;\n\
            for (int x : a) { out.push_back(2 * x); }\n\
            return out;\n\
        }\n";
    let c = compile("", src, &CheckOpts::all()).unwrap();
    let args = vec!["{1, 2, 3}".to_string()];
    let a = c.call("twice", &args, &Limits::default()).unwrap();
    let b = c.call("twice", &args, &limits()).unwrap();
    assert_eq!(a.report.cost, b.report.cost);
    assert_eq!(a.report.result, b.report.result);
}

const FIND: &str = "#include <unordered_map>\n#include <map>\n#include <vector>\n\
    int f(const std::vector<int>& v) {\n\
        std::unordered_map<int, int> seen;\n\
        std::map<int, int> ord;\n\
        for (int i = 0; i < (int)v.size(); i++) {\n\
            seen[v[i]] = i;\n\
            ord[v[i]] = i;\n\
        }\n\
        if (auto it = seen.find(4); it != seen.end()) {\n\
            auto o = ord.begin();\n\
            auto miss = seen.find(99);\n\
            auto p = v.begin() + 1;\n\
            auto r = v.rbegin();\n\
            auto e = v.end();\n\
            return it->second + o->first + (miss == seen.end() ? 100 : 0) + *p + *r + (e == v.end());\n\
        }\n\
        return -1;\n\
    }\n";

#[test]
fn an_auto_iterator_reports_its_deduced_type_and_element() {
    let ev = events(FIND, "f", &["{3, 1, 4}"]);
    let last = ev.last().unwrap();
    let heap = &last.heap;

    let it = var(last, "it");
    assert_eq!(it.ty.to_string(), "std::unordered_map<int, int>::iterator");
    assert_eq!(
        it.value,
        Plain::Seq(vec![Plain::Int(4), Plain::Int(2)]),
        "the entry the iterator points at"
    );
    assert_eq!(render(&it.value, &it.ty, heap), "-> {4, 2}");

    let o = var(last, "o");
    assert_eq!(o.ty.to_string(), "std::map<int, int>::iterator");
    assert_eq!(o.value, Plain::Seq(vec![Plain::Int(1), Plain::Int(1)]));

    let miss = var(last, "miss");
    assert_eq!(
        miss.ty.to_string(),
        "std::unordered_map<int, int>::iterator"
    );
    assert_eq!(miss.value, Plain::Void);
    assert_eq!(render(&miss.value, &miss.ty, heap), "end()");

    let p = var(last, "p");
    assert_eq!(p.ty.to_string(), "std::vector<int>::iterator");
    assert_eq!(p.value, Plain::Int(1));
    assert_eq!(render(&p.value, &p.ty, heap), "-> 1");

    let r = var(last, "r");
    assert_eq!(r.ty.to_string(), "std::vector<int>::reverse_iterator");
    assert_eq!(r.value, Plain::Int(4));

    let e = var(last, "e");
    assert_eq!(e.value, Plain::Void);
    assert!(
        ev.iter()
            .flat_map(|e| &e.vars)
            .all(|v| v.ty != Ty::Unknown && v.ty != Ty::Void)
    );
}

#[test]
fn an_invalidated_iterator_is_marked() {
    let src = "#include <vector>\n\
        int f() {\n\
            std::vector<int> v = {1, 2, 3};\n\
            auto it = v.begin();\n\
            v.push_back(4);\n\
            int x = 0;\n\
            return x;\n\
        }\n";
    let ev = events(src, "f", &[]);
    let last = ev.last().unwrap();
    let it = var(last, "it");
    assert_eq!(it.ty.to_string(), "std::vector<int>::iterator");
    assert_eq!(it.value, Plain::Uninit);
    assert_eq!(render(&it.value, &it.ty, &last.heap), "<invalidated>");
}
