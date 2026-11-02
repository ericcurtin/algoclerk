// SPDX-License-Identifier: Apache-2.0
//! ClerkScript language tests: parser, printer, checker, interpreter.

use algoclerk_core::lang::check::{Allowed, CheckOpts};
use algoclerk_core::lang::interp::{Limits, RtKind};
use algoclerk_core::lang::print::print;
use algoclerk_core::lang::run::compile;
use algoclerk_core::lang::parse;

fn run1(src: &str, entry: &str, args: &[&str]) -> Result<String, String> {
    let c = compile(src, &CheckOpts { allowed: Allowed::all(), entry: None }).map_err(|d| d[0].msg.clone())?;
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let r = c.call(entry, &args, &Limits::default())?;
    match r.report.result {
        Ok(_) => Ok(r.output.unwrap()),
        Err(e) => Err(format!("{:?}: {}", e.kind, e.msg)),
    }
}

#[test]
fn two_sum_with_a_map() {
    let src = "fn two_sum(nums: vec<int>, target: int) -> vec<int> {
        let seen = map<int, int>();
        for i in range(len(nums)) {
            let want = target - nums[i];
            if seen.contains(want) { return [seen[want], i]; }
            seen[nums[i]] = i;
        }
        return [];
    }";
    assert_eq!(run1(src, "two_sum", &["[2, 7, 11, 15]", "9"]).unwrap(), "[0, 1]");
    assert_eq!(run1(src, "two_sum", &["[1, 2]", "9"]).unwrap(), "[]");
}

#[test]
fn strings_and_chars() {
    let src = "fn vowels(s: string) -> int {
        let n = 0;
        for c in s { if \"aeiou\".contains(c.lower()) { n += 1; } }
        return n;
    }";
    assert_eq!(run1(src, "vowels", &["\"Hello World\""]).unwrap(), "3");
}

#[test]
fn typed_errors_are_friendly() {
    let e = run1("fn f(a: int) -> int { return a + \"x\"; }", "f", &["1"]).unwrap_err();
    assert!(e.contains("Mix-up"), "{e}");
    let e = run1("fn f(a: int) -> int { return b; }", "f", &["1"]).unwrap_err();
    assert!(e.contains("cannot find"), "{e}");
    let e = run1("fn f(a: vec<int>) -> int { for x in a { a.push(x); } return 0; }", "f", &["[1]"]).unwrap_err();
    assert!(e.contains("walking along"), "{e}");
}

#[test]
fn runtime_errors_are_friendly() {
    let e = run1("fn f(a: vec<int>) -> int { return a[3]; }", "f", &["[1]"]).unwrap_err();
    assert!(e.contains("Index") && e.contains("slots 0 to 0"), "{e}");
    let e = run1("fn f(a: int) -> int { return 10 / a; }", "f", &["0"]).unwrap_err();
    assert!(e.contains("DivZero"), "{e}");
    let e = run1("fn f(a: int) -> int { while true { a += 1; } return a; }", "f", &["0"]).unwrap_err();
    assert!(e.contains("Time"), "{e}");
}

#[test]
fn print_is_idempotent_and_reparses() {
    let src = "fn f(a: vec<int>, b: map<string, vec<int>>) -> (int, string) {
        let x = -(5) + a[0] * (2 + 3) - -4;
        if not (x > 3 and x < 10) or a.is_empty() { x = 1; } else if x == 7 { x = 2; } else { x = 3; }
        for (i, v) in enumerate(a) { x += i * v; }
        for i in range(0, 10, 2) { continue; }
        let s = \"a\\n\\\"q\\\"\" + 'c';
        let t = a[1..]; let u = [0; 5]; let m: map<int, int> = map<int, int>();
        return (x << 2 >> 1, s);
    }";
    let p = parse(src).unwrap();
    let t1 = print(&p);
    let p2 = parse(&t1).unwrap();
    let t2 = print(&p2);
    assert_eq!(t1, t2);
}

#[test]
fn locked_builtins_and_types() {
    use std::collections::BTreeSet;
    let opts = CheckOpts {
        allowed: Allowed {
            types: Some(["vec"].iter().map(|s| s.to_string()).collect::<BTreeSet<_>>()),
            builtins: Some(["len"].iter().map(|s| s.to_string()).collect::<BTreeSet<_>>()),
            helpers: false,
            recursion: false,
        },
        entry: None,
    };
    let e = compile("fn f(a: vec<int>) -> vec<int> { return sorted(a); }", &opts).unwrap_err();
    assert!(e[0].msg.contains("locked"), "{:?}", e[0]);
    let e = compile("fn f(a: vec<int>) -> int { let s = set<int>(); return len(a); }", &opts).unwrap_err();
    assert!(e[0].msg.contains("locked"), "{:?}", e[0]);
    assert!(compile("fn f(a: vec<int>) -> int { return len(a); }", &opts).is_ok());
}

#[test]
fn linked_list_and_tree() {
    let src = "fn rev(head: lnode) -> lnode {
        let prev: lnode = null;
        let cur = head;
        while cur != null { let nx = cur.next; cur.next = prev; prev = cur; cur = nx; }
        return prev;
    }";
    assert_eq!(run1(src, "rev", &["[1, 2, 3]"]).unwrap(), "[3, 2, 1]");
    let src = "fn depth(t: tnode) -> int { if t == null { return 0; } return 1 + max(depth(t.left), depth(t.right)); }";
    assert_eq!(run1(src, "depth", &["[1, 2, 3, null, 4]"]).unwrap(), "3");
}

#[test]
fn recursion_depth_is_limited_not_a_crash() {
    let src = "fn f(n: int) -> int { return f(n + 1); }";
    let e = run1(src, "f", &["0"]).unwrap_err();
    assert!(e.contains("Depth"), "{e}");
}

#[test]
fn by_ref_helpers() {
    let src = "fn fill(v: &mut vec<int>, n: int) { for i in range(n) { v.push(i); } }
    fn f(n: int) -> vec<int> { let v: vec<int> = []; fill(v, n); return v; }";
    assert_eq!(run1(src, "f", &["3"]).unwrap(), "[0, 1, 2]");
}

#[test]
fn cost_model_distinguishes_linear_and_quadratic() {
    let lin = "fn f(v: vec<int>) -> int { let s = 0; for x in v { s += x; } return s; }";
    let quad = "fn f(v: vec<int>) -> int { let s = 0; for i in range(len(v)) { for j in range(len(v)) { s += 1; } } return s; }";
    let big = format!("[{}]", (0..300).map(|i| i.to_string()).collect::<Vec<_>>().join(", "));
    let cl = compile(lin, &CheckOpts::default()).unwrap().call("f", &[big.clone()], &Limits::default()).unwrap();
    let cq = compile(quad, &CheckOpts::default()).unwrap().call("f", &[big], &Limits::default()).unwrap();
    assert!(cq.report.cost > cl.report.cost * 50, "{} vs {}", cq.report.cost, cl.report.cost);
}

#[test]
fn time_limit_kind() {
    let c = compile("fn f(n: int) -> int { let s = 0; for i in range(n) { s += i; } return s; }", &CheckOpts::default()).unwrap();
    let l = Limits { steps: 1000, ..Limits::default() };
    let r = c.call("f", &["100000".to_string()], &l).unwrap();
    assert_eq!(r.report.result.unwrap_err().kind, RtKind::Time);
}
