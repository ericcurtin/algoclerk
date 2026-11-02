// SPDX-License-Identifier: Apache-2.0
//! The Rust half of the compile-and-compare harness: builds the arguments of every test case from the
//! literals of the level, calls the translated entry function and prints each result in the notation of
//! [`crate::cpp::value`] (maps and unordered sets sorted by key).

use super::Generated;
use super::common::*;
use super::rust::fn_name;
use crate::cpp::ast::*;
use crate::cpp::value::{Heap, Plain, parse_value};
use crate::level::Level;
use std::fmt::Write as _;

const COLL: &str = "std::collections::";

fn rust_str(s: &[u8]) -> R<String> {
    if s.iter().any(|&b| b >= 0x80) {
        return no("Rust", "a test string with non-ASCII bytes");
    }
    let mut o = String::from("String::from(\"");
    for &c in s {
        match c {
            b'"' => o.push_str("\\\""),
            b'\\' => o.push_str("\\\\"),
            0x20..=0x7e => o.push(c as char),
            c => {
                let _ = write!(o, "\\x{c:02x}");
            }
        }
    }
    o.push_str("\")");
    Ok(o)
}

fn list(xs: &[Plain], t: &Ty, h: &Heap) -> R<String> {
    let parts: R<Vec<String>> = xs.iter().map(|x| value(x, t, h)).collect();
    Ok(parts?.join(", "))
}

/// A Rust expression with the type of `ty` holding `v`.
pub fn value(v: &Plain, ty: &Ty, h: &Heap) -> R<String> {
    Ok(match (v, ty) {
        (Plain::Int(i), Ty::Bool) => (if *i != 0 { "true" } else { "false" }).to_string(),
        (Plain::Int(i), Ty::Char) => format!("{}u8", *i as u8),
        (Plain::Int(i), Ty::Int) => format!("({i}i32)"),
        (Plain::Int(i), Ty::UInt) => format!("{i}u32"),
        (Plain::Int(i), Ty::LongLong) => format!("({i}i64)"),
        (Plain::Int(i), Ty::ULongLong) => format!("{i}u64"),
        (Plain::Int(i), Ty::Size) => format!("{i}usize"),
        (Plain::Str(s), Ty::Str | Ty::StrView) => rust_str(s)?,
        (Plain::Opt(None), Ty::Optional(_)) => "None".into(),
        (Plain::Opt(Some(x)), Ty::Optional(t)) => format!("Some({})", value(x, t, h)?),
        (Plain::Seq(xs), Ty::Pair(a, b)) if xs.len() == 2 => {
            format!("({}, {})", value(&xs[0], a, h)?, value(&xs[1], b, h)?)
        }
        (Plain::Seq(xs), Ty::Tuple(ts)) if xs.len() == ts.len() => {
            let parts: R<Vec<String>> = xs.iter().zip(ts).map(|(x, t)| value(x, t, h)).collect();
            format!("({})", parts?.join(", "))
        }
        (Plain::Seq(xs), Ty::Vector(t) | Ty::Stack(t)) => format!("vec![{}]", list(xs, t, h)?),
        (Plain::Seq(xs), Ty::Array(t, _)) => format!("[{}]", list(xs, t, h)?),
        (Plain::Seq(xs), Ty::Deque(t) | Ty::Queue(t) | Ty::List(t)) => {
            format!("{COLL}VecDeque::from(vec![{}])", list(xs, t, h)?)
        }
        (Plain::Seq(xs), Ty::Set(t)) => format!("{COLL}BTreeSet::from([{}])", list(xs, t, h)?),
        (Plain::Seq(xs), Ty::UnorderedSet(t)) => {
            format!("{COLL}HashSet::from([{}])", list(xs, t, h)?)
        }
        (Plain::Map(es), Ty::Map(k, val) | Ty::UnorderedMap(k, val)) => {
            let parts: R<Vec<String>> = es
                .iter()
                .map(|(a, b)| Ok(format!("({}, {})", value(a, k, h)?, value(b, val, h)?)))
                .collect();
            let c = if matches!(ty, Ty::Map(..)) {
                "BTreeMap"
            } else {
                "HashMap"
            };
            format!("{COLL}{c}::from([{}])", parts?.join(", "))
        }
        (Plain::Ptr(0), Ty::Ptr(inner)) => match &**inner {
            Ty::Struct(n) => format!("{n}::NULL"),
            _ => return no("Rust", "this test value"),
        },
        (Plain::Ptr(p), Ty::Ptr(inner)) => match &**inner {
            Ty::Struct(n) if n == "ListNode" => list_value(*p, h)?,
            Ty::Struct(n) if n == "TreeNode" => tree_value(*p, h)?,
            _ => return no("Rust", "this test value"),
        },
        _ => return no("Rust", format!("a test value of type {ty}")),
    })
}

fn node_val(h: &Heap, n: u32) -> R<i128> {
    h.nodes
        .get(n as usize)
        .and_then(|f| f.first())
        .and_then(Plain::as_int)
        .ok_or_else(|| "bad node".to_string())
}

fn list_value(p: u32, h: &Heap) -> R<String> {
    let mut order: Vec<u32> = Vec::new();
    let mut cur = p;
    let mut cycle = -1i64;
    while cur != 0 && order.len() < 5_000_000 {
        if let Some(i) = order.iter().position(|&x| x == cur) {
            cycle = i as i64;
            break;
        }
        order.push(cur);
        cur = match h.nodes.get(cur as usize).and_then(|f| f.get(1)) {
            Some(Plain::Ptr(n)) => *n,
            _ => 0,
        };
    }
    let vals: R<Vec<String>> = order
        .iter()
        .map(|n| node_val(h, *n).map(|v| format!("{v}i32")))
        .collect();
    Ok(format!("ach_make_list(&[{}], {cycle})", vals?.join(", ")))
}

fn tree_value(p: u32, h: &Heap) -> R<String> {
    let mut items: Vec<Option<i128>> = Vec::new();
    let mut q = std::collections::VecDeque::from([p]);
    while let Some(n) = q.pop_front() {
        if n == 0 {
            items.push(None);
            continue;
        }
        if items.len() > 5_000_000 {
            return Err("tree too large".into());
        }
        items.push(Some(node_val(h, n)?));
        for i in [1usize, 2] {
            q.push_back(match h.nodes.get(n as usize).and_then(|f| f.get(i)) {
                Some(Plain::Ptr(c)) => *c,
                _ => 0,
            });
        }
    }
    while items.last() == Some(&None) {
        items.pop();
    }
    let parts: Vec<String> = items
        .iter()
        .map(|i| i.map_or("None".to_string(), |v| format!("Some({v}i32)")))
        .collect();
    Ok(format!("ach_make_tree(&[{}])", parts.join(", ")))
}

/// A complete Rust program: the generated code, the support code and a `main` running every case.
pub fn program(level: &Level, g: &Generated, cases: &[Vec<String>]) -> R<String> {
    let sig = &level.signature;
    let mut out = g.full();
    out.push_str("\n// ---- generated by the AlgoClerk harness ----\n");
    out.push_str(SUPPORT);
    if level.prelude_defines("ListNode") {
        out.push_str(LIST_SUPPORT);
    }
    if level.prelude_defines("TreeNode") {
        out.push_str(TREE_SUPPORT);
    }
    for (ci, args) in cases.iter().enumerate() {
        if args.len() != sig.params.len() {
            return Err(format!(
                "{} takes {} arguments, the test has {}",
                sig.name,
                sig.params.len(),
                args.len()
            ));
        }
        let mut heap = Heap::default();
        let mut body = String::new();
        let mut names = Vec::new();
        for (i, (p, lit)) in sig.params.iter().zip(args).enumerate() {
            let v = parse_value(lit, &p.spec.ty, &mut heap)
                .map_err(|e| format!("argument {}: {e}", p.name))?;
            let _ = writeln!(
                body,
                "    let mut a{i} = {};",
                value(&v, &p.spec.ty, &heap)?
            );
            let by_ref = p.spec.is_ref || p.spec.ty == Ty::StrView;
            names.push(match (by_ref, p.spec.konst || !p.spec.is_ref) {
                (true, true) => format!("&a{i}"),
                (true, false) => format!("&mut a{i}"),
                _ => format!("a{i}"),
            });
        }
        let _ = write!(
            out,
            "fn ach_case_{ci}() {{\n{body}    let r = {}({});\n    ach_emit(&r);\n}}\n\n",
            fn_name(&sig.name),
            names.join(", ")
        );
    }
    out.push_str("fn ach_main() {\n");
    for ci in 0..cases.len() {
        let _ = writeln!(out, "    ach_case_{ci}();");
    }
    out.push_str("}\n\nfn main() {\n    std::thread::Builder::new().stack_size(512 << 20).spawn(ach_main).unwrap().join().unwrap();\n}\n");
    Ok(out)
}

const SUPPORT: &str = r#"
trait Put {
    fn put(&self, o: &mut String);
}

fn ach_esc(o: &mut String, c: u8, quote: u8) {
    match c {
        b'\n' => o.push_str("\\n"),
        b'\t' => o.push_str("\\t"),
        b'\r' => o.push_str("\\r"),
        0 => o.push_str("\\0"),
        b'\\' => o.push_str("\\\\"),
        c if c == quote => {
            o.push('\\');
            o.push(c as char);
        }
        0x20..=0x7e => o.push(c as char),
        c => o.push_str(&format!("\\x{c:02x}")),
    }
}

impl Put for bool {
    fn put(&self, o: &mut String) {
        o.push_str(if *self { "true" } else { "false" });
    }
}
impl Put for u8 {
    fn put(&self, o: &mut String) {
        o.push('\'');
        ach_esc(o, *self, b'\'');
        o.push('\'');
    }
}
macro_rules! put_int {
    ($($t:ty),*) => {$(impl Put for $t { fn put(&self, o: &mut String) { o.push_str(&self.to_string()); } })*};
}
put_int!(i32, u32, i64, u64, usize);
fn ach_put_text(o: &mut String, s: &str) {
    o.push('"');
    for c in s.bytes() {
        ach_esc(o, c, b'"');
    }
    o.push('"');
}
impl Put for String {
    fn put(&self, o: &mut String) {
        ach_put_text(o, self);
    }
}
impl Put for &str {
    fn put(&self, o: &mut String) {
        ach_put_text(o, self);
    }
}
fn ach_put_all<'a, T: Put + 'a>(o: &mut String, it: impl Iterator<Item = &'a T>) {
    o.push('{');
    for (i, x) in it.enumerate() {
        if i > 0 {
            o.push_str(", ");
        }
        x.put(o);
    }
    o.push('}');
}
impl<T: Put> Put for Vec<T> {
    fn put(&self, o: &mut String) {
        ach_put_all(o, self.iter());
    }
}
impl<T: Put, const N: usize> Put for [T; N] {
    fn put(&self, o: &mut String) {
        ach_put_all(o, self.iter());
    }
}
impl<T: Put> Put for std::collections::VecDeque<T> {
    fn put(&self, o: &mut String) {
        ach_put_all(o, self.iter());
    }
}
impl<T: Put> Put for std::collections::BTreeSet<T> {
    fn put(&self, o: &mut String) {
        ach_put_all(o, self.iter());
    }
}
impl<T: Put + Ord> Put for std::collections::HashSet<T> {
    fn put(&self, o: &mut String) {
        let mut v: Vec<&T> = self.iter().collect();
        v.sort();
        o.push('{');
        for (i, x) in v.into_iter().enumerate() {
            if i > 0 {
                o.push_str(", ");
            }
            x.put(o);
        }
        o.push('}');
    }
}
fn ach_put_entries<'a, K: Put + 'a, V: Put + 'a>(o: &mut String, it: impl Iterator<Item = (&'a K, &'a V)>) {
    o.push('{');
    for (i, (k, v)) in it.enumerate() {
        if i > 0 {
            o.push_str(", ");
        }
        o.push('{');
        k.put(o);
        o.push_str(", ");
        v.put(o);
        o.push('}');
    }
    o.push('}');
}
impl<K: Put, V: Put> Put for std::collections::BTreeMap<K, V> {
    fn put(&self, o: &mut String) {
        ach_put_entries(o, self.iter());
    }
}
impl<K: Put + Ord, V: Put> Put for std::collections::HashMap<K, V> {
    fn put(&self, o: &mut String) {
        let mut v: Vec<(&K, &V)> = self.iter().collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        ach_put_entries(o, v.into_iter());
    }
}
impl<T: Put> Put for Option<T> {
    fn put(&self, o: &mut String) {
        match self {
            None => o.push_str("nullopt"),
            Some(x) => {
                o.push('{');
                x.put(o);
                o.push('}');
            }
        }
    }
}
macro_rules! put_tuple {
    ($(($($n:tt $t:ident),+))*) => {$(
        impl<$($t: Put),+> Put for ($($t,)+) {
            fn put(&self, o: &mut String) {
                o.push('{');
                let mut first = true;
                $( if !first { o.push_str(", "); } first = false; self.$n.put(o); )+
                let _ = first;
                o.push('}');
            }
        }
    )*};
}
put_tuple!((0 A, 1 B) (0 A, 1 B, 2 C) (0 A, 1 B, 2 C, 3 D) (0 A, 1 B, 2 C, 3 D, 4 E));

fn ach_emit<T: Put>(v: &T) {
    let mut s = String::new();
    v.put(&mut s);
    println!("{s}");
}
"#;

const LIST_SUPPORT: &str = r#"
fn ach_make_list(vals: &[i32], cycle: i64) -> ListNode {
    let nodes: Vec<ListNode> = vals.iter().map(|&v| ListNode::new(v)).collect();
    for w in nodes.windows(2) {
        w[0].set_next(w[1]);
    }
    if cycle >= 0 && (cycle as usize) < nodes.len() {
        nodes[nodes.len() - 1].set_next(nodes[cycle as usize]);
    }
    nodes.first().copied().unwrap_or(ListNode::NULL)
}
impl Put for ListNode {
    fn put(&self, o: &mut String) {
        if *self == ListNode::NULL {
            o.push_str("nullptr");
            return;
        }
        let mut order: Vec<ListNode> = Vec::new();
        let mut pos: std::collections::HashMap<ListNode, usize> = std::collections::HashMap::new();
        let mut cur = *self;
        let mut cycle: Option<usize> = None;
        while cur != ListNode::NULL && order.len() < 1_000_000 {
            if let Some(&i) = pos.get(&cur) {
                cycle = Some(i);
                break;
            }
            pos.insert(cur, order.len());
            order.push(cur);
            cur = cur.next();
        }
        o.push('{');
        for (i, n) in order.iter().enumerate() {
            if i > 0 {
                o.push_str(", ");
            }
            o.push_str(&n.val().to_string());
        }
        o.push('}');
        if let Some(c) = cycle {
            o.push_str(&format!(" @{c}"));
        }
    }
}
"#;

const TREE_SUPPORT: &str = r#"
fn ach_make_tree(items: &[Option<i32>]) -> TreeNode {
    if items.is_empty() || items[0].is_none() {
        return TreeNode::NULL;
    }
    let ids: Vec<TreeNode> = items.iter().map(|i| i.map_or(TreeNode::NULL, TreeNode::new)).collect();
    let mut next = 1;
    for n in &ids {
        if *n == TreeNode::NULL {
            continue;
        }
        if next < ids.len() {
            n.set_left(ids[next]);
            next += 1;
        }
        if next < ids.len() {
            n.set_right(ids[next]);
            next += 1;
        }
    }
    ids[0]
}
impl Put for TreeNode {
    fn put(&self, o: &mut String) {
        if *self == TreeNode::NULL {
            o.push_str("nullptr");
            return;
        }
        let mut items: Vec<Option<i32>> = Vec::new();
        let mut q = std::collections::VecDeque::from([*self]);
        while let Some(n) = q.pop_front() {
            if items.len() > 1_000_000 {
                break;
            }
            if n == TreeNode::NULL {
                items.push(None);
                continue;
            }
            items.push(Some(n.val()));
            q.push_back(n.left());
            q.push_back(n.right());
        }
        while items.last() == Some(&None) {
            items.pop();
        }
        o.push('{');
        for (i, it) in items.iter().enumerate() {
            if i > 0 {
                o.push_str(", ");
            }
            match it {
                Some(v) => o.push_str(&v.to_string()),
                None => o.push_str("nullptr"),
            }
        }
        o.push('}');
    }
}
"#;
