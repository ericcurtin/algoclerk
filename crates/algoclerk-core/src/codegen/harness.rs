// SPDX-License-Identifier: Apache-2.0
//! Test harnesses: wrap generated code in a `main` that runs literal test cases and prints the results
//! in the interpreter's canonical text form, so compile-and-compare tests can diff them.

use super::*;
use crate::lang::check::EntrySig;
use crate::lang::value::{NodeData, Value, parse_value};

/// Cycle position of a list (`-1` for none) and its values.
fn list_parts(head: u32, nodes: &[NodeData]) -> (Vec<i64>, i64) {
    let mut vals = Vec::new();
    let mut seen = std::collections::HashMap::new();
    let mut cur = head;
    while cur != 0 {
        if let Some(&p) = seen.get(&cur) {
            return (vals, p);
        }
        seen.insert(cur, vals.len() as i64);
        vals.push(nodes[cur as usize].val);
        cur = nodes[cur as usize].a;
    }
    (vals, -1)
}

/// Level-order items of a tree with holes.
fn tree_parts(root: u32, nodes: &[NodeData]) -> Vec<Option<i64>> {
    let mut items = Vec::new();
    let mut q = std::collections::VecDeque::new();
    q.push_back(root);
    while let Some(n) = q.pop_front() {
        if n == 0 {
            items.push(None);
            continue;
        }
        items.push(Some(nodes[n as usize].val));
        q.push_back(nodes[n as usize].a);
        q.push_back(nodes[n as usize].b);
    }
    while items.last() == Some(&None) {
        items.pop();
    }
    items
}

fn char_lit(c: u8) -> String {
    format!("'{}'", escape_char(c))
}

fn cpp_value(v: &Value, t: &Ty, nodes: &[NodeData]) -> Result<String, String> {
    let ty = cpp::ty_name(t);
    Ok(match (v, t) {
        (Value::Int(n), _) => {
            if *n == i64::MIN {
                "(-9223372036854775807LL - 1)".to_string()
            } else {
                format!("{n}LL")
            }
        }
        (Value::Bool(b), _) => b.to_string(),
        (Value::Char(c), _) => char_lit(*c),
        (Value::Str(s), _) => format!("std::string(\"{}\", {})", escape_str(s), s.len()),
        (Value::Vec(items), Ty::Vec(et)) => {
            let parts: Result<Vec<_>, _> = items.iter().map(|x| cpp_value(x, et, nodes)).collect();
            format!("{ty}{{{}}}", parts?.join(", "))
        }
        (Value::Set(items), Ty::Set(et)) => {
            let parts: Result<Vec<_>, _> = items.iter().map(|x| cpp_value(x, et, nodes)).collect();
            format!("{ty}{{{}}}", parts?.join(", "))
        }
        (Value::Map(items), Ty::Map(kt, vt)) => {
            let mut parts = Vec::new();
            for (k, x) in items.iter() {
                parts.push(format!(
                    "{{{}, {}}}",
                    cpp_value(k, kt, nodes)?,
                    cpp_value(x, vt, nodes)?
                ));
            }
            format!("{ty}{{{}}}", parts.join(", "))
        }
        (Value::Tuple(items), Ty::Tuple(ts)) => {
            let parts: Result<Vec<_>, _> = items
                .iter()
                .zip(ts.iter())
                .map(|(x, t)| cpp_value(x, t, nodes))
                .collect();
            format!("{ty}({})", parts?.join(", "))
        }
        (Value::Opt(None), _) => format!("{ty}()"),
        (Value::Opt(Some(x)), Ty::Option(et)) => format!("{ty}({})", cpp_value(x, et, nodes)?),
        (Value::Node(h), Ty::LNode) => {
            let (vals, cyc) = list_parts(*h, nodes);
            format!(
                "h::build_list({{{}}}, {cyc})",
                vals.iter()
                    .map(|v| format!("{v}LL"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        (Value::Node(h), Ty::TNode) => {
            let items = tree_parts(*h, nodes);
            format!(
                "h::build_tree({{{}}})",
                items
                    .iter()
                    .map(|v| v.map_or("std::nullopt".to_string(), |x| format!("{x}LL")))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => {
            return Err(format!(
                "cannot pass a value of type {t} to the C++ harness"
            ));
        }
    })
}

const CPP_HARNESS: &str = r#"
#include <iostream>
#include <sstream>
namespace h {
void show(std::ostream& o, long long x);
void show(std::ostream& o, bool x);
void show(std::ostream& o, char c);
void show(std::ostream& o, const std::string& s);
template <class T> void show(std::ostream& o, const std::vector<T>& v);
template <class A, class B> void show(std::ostream& o, const std::pair<A, B>& p);
template <class... Ts> void show(std::ostream& o, const std::tuple<Ts...>& t);
template <class T> void show(std::ostream& o, const std::optional<T>& x);
template <class T, class H> void show(std::ostream& o, const std::unordered_set<T, H>& s);
template <class K, class V, class H> void show(std::ostream& o, const std::unordered_map<K, V, H>& m);
void show(std::ostream& o, ListNode* n);
void show(std::ostream& o, TreeNode* n);

void esc(std::ostream& o, char c, char quote) {
    if (c == '\n') o << "\\n"; else if (c == '\t') o << "\\t"; else if (c == '\r') o << "\\r";
    else if (c == 0) o << "\\0"; else if (c == '\\') o << "\\\\"; else if (c == quote) o << '\\' << c;
    else if (c >= 32 && c < 127) o << c;
    else { char b[8]; std::snprintf(b, sizeof b, "\\x%02x", (unsigned char)c); o << b; }
}
void show(std::ostream& o, long long x) { o << x; }
void show(std::ostream& o, bool x) { o << (x ? "true" : "false"); }
void show(std::ostream& o, char c) { o << '\''; esc(o, c, '\''); o << '\''; }
void show(std::ostream& o, const std::string& s) { o << '"'; for (char c : s) esc(o, c, '"'); o << '"'; }
template <class T> void show(std::ostream& o, const std::vector<T>& v) {
    o << '[';
    for (size_t i = 0; i < v.size(); ++i) { if (i) o << ", "; show(o, v[i]); }
    o << ']';
}
template <class A, class B> void show(std::ostream& o, const std::pair<A, B>& p) { o << '('; show(o, p.first); o << ", "; show(o, p.second); o << ')'; }
template <class Tup, size_t... I> void show_tuple(std::ostream& o, const Tup& t, std::index_sequence<I...>) {
    o << '(';
    ((o << (I ? ", " : ""), show(o, std::get<I>(t))), ...);
    o << ')';
}
template <class... Ts> void show(std::ostream& o, const std::tuple<Ts...>& t) { show_tuple(o, t, std::index_sequence_for<Ts...>{}); }
template <class T> void show(std::ostream& o, const std::optional<T>& x) { if (x) { o << "some("; show(o, *x); o << ')'; } else o << "none"; }
template <class T, class H> void show(std::ostream& o, const std::unordered_set<T, H>& s) {
    std::vector<T> v(s.begin(), s.end());
    std::sort(v.begin(), v.end());
    o << '{';
    for (size_t i = 0; i < v.size(); ++i) { if (i) o << ", "; show(o, v[i]); }
    o << '}';
}
template <class K, class V, class H> void show(std::ostream& o, const std::unordered_map<K, V, H>& m) {
    std::vector<std::pair<K, V>> v(m.begin(), m.end());
    std::sort(v.begin(), v.end(), [](const auto& a, const auto& b) { return a.first < b.first; });
    o << '{';
    for (size_t i = 0; i < v.size(); ++i) { if (i) o << ", "; show(o, v[i].first); o << ": "; show(o, v[i].second); }
    o << '}';
}
void show(std::ostream& o, ListNode* n) {
    o << '[';
    std::vector<ListNode*> seen;
    int pos = -1;
    for (int i = 0; n; ++i) {
        for (size_t j = 0; j < seen.size(); ++j) if (seen[j] == n) pos = (int)j;
        if (pos >= 0) break;
        seen.push_back(n);
        if (i) o << ", ";
        o << n->val;
        n = n->next;
    }
    o << ']';
    if (pos >= 0) o << " @" << pos;
}
void show(std::ostream& o, TreeNode* root) {
    std::vector<std::string> items;
    std::vector<TreeNode*> q{root};
    for (size_t i = 0; i < q.size(); ++i) {
        if (!q[i]) { items.push_back("null"); continue; }
        items.push_back(std::to_string(q[i]->val));
        q.push_back(q[i]->left);
        q.push_back(q[i]->right);
    }
    while (!items.empty() && items.back() == "null") items.pop_back();
    o << '[';
    for (size_t i = 0; i < items.size(); ++i) { if (i) o << ", "; o << items[i]; }
    o << ']';
}
ListNode* build_list(const std::vector<long long>& vals, long long cycle) {
    ListNode* head = nullptr;
    ListNode* tail = nullptr;
    std::vector<ListNode*> all;
    for (long long v : vals) {
        ListNode* n = new ListNode{v, nullptr};
        if (tail) tail->next = n; else head = n;
        tail = n;
        all.push_back(n);
    }
    if (cycle >= 0 && tail) tail->next = all[cycle];
    return head;
}
TreeNode* build_tree(const std::vector<std::optional<long long>>& items) {
    if (items.empty() || !items[0]) return nullptr;
    TreeNode* root = new TreeNode{*items[0], nullptr, nullptr};
    std::vector<TreeNode*> q{root};
    size_t i = 1;
    for (size_t k = 0; k < q.size() && i < items.size(); ++k) {
        if (i < items.size() && items[i]) { q[k]->left = new TreeNode{*items[i], nullptr, nullptr}; q.push_back(q[k]->left); }
        ++i;
        if (i < items.size() && items[i]) { q[k]->right = new TreeNode{*items[i], nullptr, nullptr}; q.push_back(q[k]->right); }
        ++i;
    }
    return root;
}
}  // namespace h
"#;

fn cpp_program(sig: &EntrySig, g: &Generated, cases: &[Vec<String>]) -> Result<String, String> {
    let mut out = String::from(
        "#include <algorithm>\n#include <cstdio>\n#include <optional>\n#include <string>\n#include <tuple>\n#include <unordered_map>\n#include <unordered_set>\n#include <vector>\n#include <utility>\n",
    );
    out.push_str(&g.prelude);
    // make sure both node structs exist for the harness printers
    if !g.prelude.contains("struct ListNode") {
        out.push_str("struct ListNode { long long val; ListNode* next; };\n");
    }
    if !g.prelude.contains("struct TreeNode") {
        out.push_str("struct TreeNode { long long val; TreeNode* left; TreeNode* right; };\n");
    }
    out.push('\n');
    out.push_str(&g.code);
    out.push_str(CPP_HARNESS);
    out.push_str("\nint main() {\n");
    for case in cases {
        let mut nodes = vec![NodeData::default()];
        let mut names = Vec::new();
        out.push_str("    {\n");
        for (i, (p, lit)) in sig.params.iter().zip(case.iter()).enumerate() {
            let v = parse_value(lit, &p.ty, &mut nodes)?;
            out.push_str(&format!(
                "        {} a{i} = {};\n",
                cpp::ty_name(&p.ty),
                cpp_value(&v, &p.ty, &nodes)?
            ));
            names.push(format!("a{i}"));
        }
        out.push_str(&format!("        auto r = {}({});\n        std::ostringstream o;\n        h::show(o, r);\n        std::puts(o.str().c_str());\n    }}\n", ident(&sig.name), names.join(", ")));
    }
    out.push_str("    return 0;\n}\n");
    Ok(out)
}

/// A complete program that runs `cases` through the entry function and prints one result per line.
pub fn program(
    lang: Lang,
    sig: &EntrySig,
    g: &Generated,
    cases: &[Vec<String>],
) -> Result<String, String> {
    match lang {
        Lang::Cpp => cpp_program(sig, g, cases),
        Lang::Rust => rust_program(sig, g, cases),
        Lang::C => c_program(sig, g, cases),
    }
}

fn rust_value(v: &Value, t: &Ty, nodes: &[NodeData]) -> Result<String, String> {
    Ok(match (v, t) {
        (Value::Int(n), _) => {
            if *n == i64::MIN {
                "i64::MIN".to_string()
            } else {
                format!("{n}_i64")
            }
        }
        (Value::Bool(b), _) => b.to_string(),
        (Value::Char(c), _) => char_lit(*c),
        (Value::Str(s), _) => format!("String::from(\"{}\")", escape_str(s)),
        (Value::Vec(items), Ty::Vec(et)) => {
            let parts: Result<Vec<_>, _> = items.iter().map(|x| rust_value(x, et, nodes)).collect();
            let ty = rust::ty_name(t);
            if items.is_empty() {
                format!("Vec::<{}>::new()", rust::ty_name(et))
            } else {
                let _ = ty;
                format!("vec![{}]", parts?.join(", "))
            }
        }
        (Value::Set(items), Ty::Set(et)) => {
            let parts: Result<Vec<_>, _> = items.iter().map(|x| rust_value(x, et, nodes)).collect();
            format!("{}::from([{}])", "HashSet", parts?.join(", "))
        }
        (Value::Map(items), Ty::Map(kt, vt)) => {
            let mut parts = Vec::new();
            for (k, x) in items.iter() {
                parts.push(format!(
                    "({}, {})",
                    rust_value(k, kt, nodes)?,
                    rust_value(x, vt, nodes)?
                ));
            }
            format!(
                "HashMap::<{}, {}>::from([{}])",
                rust::ty_name(kt),
                rust::ty_name(vt),
                parts.join(", ")
            )
        }
        (Value::Tuple(items), Ty::Tuple(ts)) => {
            let parts: Result<Vec<_>, _> = items
                .iter()
                .zip(ts.iter())
                .map(|(x, t)| rust_value(x, t, nodes))
                .collect();
            format!("({})", parts?.join(", "))
        }
        (Value::Opt(None), Ty::Option(et)) => format!("None::<{}>", rust::ty_name(et)),
        (Value::Opt(Some(x)), Ty::Option(et)) => format!("Some({})", rust_value(x, et, nodes)?),
        (Value::Node(h), Ty::LNode) => {
            let (vals, cyc) = list_parts(*h, nodes);
            format!(
                "build_list(&[{}], {cyc})",
                vals.iter()
                    .map(|v| format!("{v}_i64"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        (Value::Node(h), Ty::TNode) => {
            let items = tree_parts(*h, nodes);
            format!(
                "build_tree(&[{}])",
                items
                    .iter()
                    .map(|v| v.map_or("None".to_string(), |x| format!("Some({x}_i64)")))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => {
            return Err(format!(
                "cannot pass a value of type {t} to the Rust harness"
            ));
        }
    })
}

const RUST_HARNESS: &str = r#"
trait Show {
    fn show(&self, out: &mut String);
}
impl Show for i64 {
    fn show(&self, out: &mut String) { out.push_str(&self.to_string()); }
}
impl Show for bool {
    fn show(&self, out: &mut String) { out.push_str(if *self { "true" } else { "false" }); }
}
fn esc(c: char, quote: char, out: &mut String) {
    match c {
        '\n' => out.push_str("\\n"),
        '\t' => out.push_str("\\t"),
        '\r' => out.push_str("\\r"),
        '\0' => out.push_str("\\0"),
        '\\' => out.push_str("\\\\"),
        c if c == quote => { out.push('\\'); out.push(c); }
        c if (' '..='~').contains(&c) => out.push(c),
        c => out.push_str(&format!("\\x{:02x}", c as u32)),
    }
}
impl Show for char {
    fn show(&self, out: &mut String) { out.push('\''); esc(*self, '\'', out); out.push('\''); }
}
impl Show for String {
    fn show(&self, out: &mut String) { out.push('"'); for c in self.chars() { esc(c, '"', out); } out.push('"'); }
}
impl<T: Show> Show for Vec<T> {
    fn show(&self, out: &mut String) {
        out.push('[');
        for (i, x) in self.iter().enumerate() { if i > 0 { out.push_str(", "); } x.show(out); }
        out.push(']');
    }
}
impl<T: Show> Show for Option<T> {
    fn show(&self, out: &mut String) {
        match self { Some(x) => { out.push_str("some("); x.show(out); out.push(')'); } None => out.push_str("none") }
    }
}
impl<A: Show, B: Show> Show for (A, B) {
    fn show(&self, out: &mut String) { out.push('('); self.0.show(out); out.push_str(", "); self.1.show(out); out.push(')'); }
}
impl<A: Show, B: Show, C: Show> Show for (A, B, C) {
    fn show(&self, out: &mut String) { out.push('('); self.0.show(out); out.push_str(", "); self.1.show(out); out.push_str(", "); self.2.show(out); out.push(')'); }
}
impl<T: Show + Ord + Clone> Show for HashSet<T> {
    fn show(&self, out: &mut String) {
        let mut v: Vec<T> = self.iter().cloned().collect();
        v.sort();
        out.push('{');
        for (i, x) in v.iter().enumerate() { if i > 0 { out.push_str(", "); } x.show(out); }
        out.push('}');
    }
}
impl<K: Show + Ord + Clone, V: Show + Clone> Show for HashMap<K, V> {
    fn show(&self, out: &mut String) {
        let mut v: Vec<(K, V)> = self.iter().map(|(k, x)| (k.clone(), x.clone())).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        out.push('{');
        for (i, (k, x)) in v.iter().enumerate() { if i > 0 { out.push_str(", "); } k.show(out); out.push_str(": "); x.show(out); }
        out.push('}');
    }
}
impl Show for ListRef {
    fn show(&self, out: &mut String) {
        out.push('[');
        let mut seen: Vec<ListRef> = Vec::new();
        let mut cur = *self;
        let mut pos: Option<usize> = None;
        while cur != ListRef::NULL {
            if let Some(p) = seen.iter().position(|s| *s == cur) { pos = Some(p); break; }
            if !seen.is_empty() { out.push_str(", "); }
            out.push_str(&cur.val().to_string());
            seen.push(cur);
            cur = cur.next();
        }
        out.push(']');
        if let Some(p) = pos { out.push_str(&format!(" @{p}")); }
    }
}
impl Show for TreeRef {
    fn show(&self, out: &mut String) {
        let mut items: Vec<String> = Vec::new();
        let mut q: Vec<TreeRef> = vec![*self];
        let mut i = 0;
        while i < q.len() {
            let n = q[i];
            i += 1;
            if n == TreeRef::NULL { items.push("null".into()); continue; }
            items.push(n.val().to_string());
            q.push(n.left());
            q.push(n.right());
        }
        while items.last().is_some_and(|s| s == "null") { items.pop(); }
        out.push('[');
        out.push_str(&items.join(", "));
        out.push(']');
    }
}
fn build_list(vals: &[i64], cycle: i64) -> ListRef {
    let mut refs: Vec<ListRef> = Vec::new();
    let mut next = ListRef::NULL;
    for v in vals.iter().rev() {
        next = ListRef::new(*v, next);
        refs.push(next);
    }
    refs.reverse();
    if cycle >= 0 && !refs.is_empty() {
        let last = refs[refs.len() - 1];
        last.set_next(refs[cycle as usize]);
    }
    refs.first().copied().unwrap_or(ListRef::NULL)
}
fn build_tree(items: &[Option<i64>]) -> TreeRef {
    let Some(Some(root_v)) = items.first() else { return TreeRef::NULL };
    let root = TreeRef::new(*root_v, TreeRef::NULL, TreeRef::NULL);
    let mut q = vec![root];
    let mut i = 1;
    let mut k = 0;
    while k < q.len() && i < items.len() {
        let n = q[k];
        k += 1;
        if let Some(v) = items.get(i).copied().flatten() { let c = TreeRef::new(v, TreeRef::NULL, TreeRef::NULL); n.set_left(c); q.push(c); }
        i += 1;
        if let Some(v) = items.get(i).copied().flatten() { let c = TreeRef::new(v, TreeRef::NULL, TreeRef::NULL); n.set_right(c); q.push(c); }
        i += 1;
    }
    root
}
"#;

fn rust_program(sig: &EntrySig, g: &Generated, cases: &[Vec<String>]) -> Result<String, String> {
    let mut out = String::from(
        "#![allow(unused, dead_code)]
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
",
    );
    let mut pre = g.prelude.clone();
    // the harness always needs both node kinds (the printers mention them)
    let (has_l, has_t) = (
        pre.contains("struct ListRef"),
        pre.contains("struct TreeRef"),
    );
    pre = pre
        .lines()
        .filter(|l| !l.starts_with("use std::collections::"))
        .collect::<Vec<_>>()
        .join("\n");
    out.push_str(&pre);
    out.push('\n');
    if !has_l || !has_t {
        let extra = nodes_prelude_for_harness(has_l, has_t);
        out.push_str(&extra);
    }
    out.push_str(&g.code);
    out.push_str(RUST_HARNESS);
    out.push_str("\nfn main() {\n");
    for case in cases {
        let mut nodes = vec![NodeData::default()];
        let mut names = Vec::new();
        out.push_str("    {\n");
        for (i, (p, lit)) in sig.params.iter().zip(case.iter()).enumerate() {
            let v = parse_value(lit, &p.ty, &mut nodes)?;
            out.push_str(&format!(
                "        let a{i}: {} = {};\n",
                rust::ty_name(&p.ty),
                rust_value(&v, &p.ty, &nodes)?
            ));
            names.push(format!("a{i}"));
        }
        out.push_str(&format!("        let r = {}({});\n        let mut s = String::new();\n        r.show(&mut s);\n        println!(\"{{}}\", s);\n    }}\n", ident(&sig.name), names.join(", ")));
    }
    out.push_str("}\n");
    Ok(out)
}

fn nodes_prelude_for_harness(has_l: bool, has_t: bool) -> String {
    rust::nodes_prelude_pub(!has_l, !has_t, has_l || has_t)
}

// ----- C harness -----

fn c_build(v: &Value, t: &Ty, nodes: &[NodeData]) -> Result<String, String> {
    Ok(match (v, t) {
        (Value::Int(n), _) => {
            if *n == i64::MIN {
                "(-9223372036854775807LL - 1)".to_string()
            } else {
                format!("{n}LL")
            }
        }
        (Value::Bool(b), _) => b.to_string(),
        (Value::Char(c), _) => char_lit(*c),
        (Value::Str(s), _) => format!("str_from(\"{}\", {})", escape_str(s), s.len()),
        (Value::Vec(items), Ty::Vec(et)) => {
            let sn = c::sname(t);
            if items.is_empty() {
                format!("{sn}_new()")
            } else {
                let parts: Result<Vec<_>, _> =
                    items.iter().map(|x| c_build(x, et, nodes)).collect();
                format!(
                    "{sn}_from(({}[]){{{}}}, {})",
                    c::cty(et),
                    parts?.join(", "),
                    items.len()
                )
            }
        }
        (Value::Tuple(items), Ty::Tuple(ts)) => {
            let parts: Result<Vec<_>, _> = items
                .iter()
                .zip(ts.iter())
                .map(|(x, t)| c_build(x, t, nodes))
                .collect();
            format!("{}_make({})", c::sname(t), parts?.join(", "))
        }
        (Value::Opt(None), _) => format!("{}_none()", c::sname(t)),
        (Value::Opt(Some(x)), Ty::Option(et)) => {
            format!("{}_some({})", c::sname(t), c_build(x, et, nodes)?)
        }
        (Value::Node(h), Ty::LNode) => {
            let (vals, cyc) = list_parts(*h, nodes);
            if vals.is_empty() {
                "NULL".to_string()
            } else {
                format!(
                    "h_build_list((long long[]){{{}}}, {}, {cyc})",
                    vals.iter()
                        .map(|v| format!("{v}LL"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    vals.len()
                )
            }
        }
        (Value::Node(h), Ty::TNode) => {
            let items = tree_parts(*h, nodes);
            if items.is_empty() {
                "NULL".to_string()
            } else {
                format!(
                    "h_build_tree((long long[]){{{}}}, (bool[]){{{}}}, {})",
                    items
                        .iter()
                        .map(|v| format!("{}LL", v.unwrap_or(0)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    items
                        .iter()
                        .map(|v| v.is_some().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    items.len()
                )
            }
        }
        _ => return Err(format!("the C harness cannot build a value of type {t}")),
    })
}

/// Emit `h_show_<mangle>` for a type and (first) its components.
fn c_show_fns(t: &Ty, done: &mut std::collections::BTreeSet<String>, out: &mut String) {
    let name = c::mangle(t);
    if done.contains(&name) {
        return;
    }
    let ty = c::cty(t);
    let body = match t {
        Ty::Int => Some("printf(\"%lld\", x);".to_string()),
        Ty::Bool => Some("fputs(x ? \"true\" : \"false\", stdout);".to_string()),
        Ty::Char => Some("h_show_char(x);".to_string()),
        Ty::Unit => Some(String::new()),
        Ty::Str => Some("h_show_str(x);".to_string()),
        Ty::Vec(a) | Ty::Stack(a) => {
            c_show_fns(a, done, out);
            Some(format!(
                "putchar('['); for (size_t i = 0; i < x.len; i++) {{ if (i) fputs(\", \", stdout); h_show_{}(x.data[i]); }} putchar(']');",
                c::mangle(a)
            ))
        }
        Ty::Tuple(ts) => {
            let mut b = String::from("putchar('(');");
            for (i, e) in ts.iter().enumerate() {
                c_show_fns(e, done, out);
                if i > 0 {
                    b.push_str(" fputs(\", \", stdout);");
                }
                b.push_str(&format!(" h_show_{}(x.f{i});", c::mangle(e)));
            }
            b.push_str(" putchar(')');");
            Some(b)
        }
        Ty::Option(a) => {
            c_show_fns(a, done, out);
            Some(format!(
                "if (x.has) {{ fputs(\"some(\", stdout); h_show_{}(x.v); putchar(')'); }} else fputs(\"none\", stdout);",
                c::mangle(a)
            ))
        }
        Ty::Set(a) => {
            c_show_fns(a, done, out);
            let vs = c::sname(&Ty::Vec(a.clone()));
            Some(format!(
                "{vs} v = {}_sorted(x); putchar('{{'); for (size_t i = 0; i < v.len; i++) {{ if (i) fputs(\", \", stdout); h_show_{}(v.data[i]); }} putchar('}}');",
                c::sname(t),
                c::mangle(a)
            ))
        }
        Ty::Map(k, v) => {
            c_show_fns(k, done, out);
            c_show_fns(v, done, out);
            let ks = c::sname(&Ty::Vec(k.clone()));
            Some(format!(
                "{ks} ks = {}_keys(x); putchar('{{'); for (size_t i = 0; i < ks.len; i++) {{ if (i) fputs(\", \", stdout); h_show_{}(ks.data[i]); fputs(\": \", stdout); h_show_{}(*{}_at(&x, ks.data[i])); }} putchar('}}');",
                c::sname(t),
                c::mangle(k),
                c::mangle(v),
                c::sname(t)
            ))
        }
        Ty::LNode => Some("h_show_list(x);".to_string()),
        Ty::TNode => Some("h_show_tree(x);".to_string()),
        _ => None,
    };
    let Some(body) = body else { return };
    if matches!(t, Ty::Char | Ty::Str) {
        done.insert(name);
        return;
    }
    done.insert(name.clone());
    out.push_str(&format!("static void h_show_{name}({ty} x) {{ {body} }}\n"));
}

const C_HARNESS: &str = r#"
static void h_show_char_raw(char c, char quote) {
    unsigned char u = (unsigned char)c;
    if (c == '\n') fputs("\\n", stdout); else if (c == '\t') fputs("\\t", stdout); else if (c == '\r') fputs("\\r", stdout);
    else if (c == 0) fputs("\\0", stdout); else if (c == '\\') fputs("\\\\", stdout); else if (c == quote) { putchar('\\'); putchar(c); }
    else if (u >= 32 && u < 127) putchar(c); else printf("\\x%02x", u);
}
static void h_show_char(char c) { putchar('\''); h_show_char_raw(c, '\''); putchar('\''); }
static void h_show_str(Str s) { putchar('"'); for (size_t i = 0; i < s.len; i++) h_show_char_raw(s.data[i], '"'); putchar('"'); }
"#;

const C_HARNESS_LIST: &str = r#"
static void h_show_list(ListNode *n) {
    ListNode *seen[4096];
    size_t ns = 0;
    long pos = -1;
    putchar('[');
    while (n && ns < 4096) {
        for (size_t j = 0; j < ns; j++) if (seen[j] == n) pos = (long)j;
        if (pos >= 0) break;
        seen[ns++] = n;
        if (ns > 1) fputs(", ", stdout);
        printf("%lld", n->val);
        n = n->next;
    }
    putchar(']');
    if (pos >= 0) printf(" @%ld", pos);
}
static ListNode *h_build_list(const long long *vals, size_t n, long long cycle) {
    ListNode *head = NULL, *tail = NULL;
    ListNode **all = (ListNode **)malloc(n * sizeof *all);
    for (size_t i = 0; i < n; i++) {
        ListNode *x = ListNode_new(vals[i], NULL);
        if (tail) tail->next = x; else head = x;
        tail = x;
        all[i] = x;
    }
    if (cycle >= 0 && tail) tail->next = all[cycle];
    free(all);
    return head;
}
"#;

const C_HARNESS_TREE: &str = r#"
static void h_show_tree(TreeNode *root) {
    size_t cap = 64, n = 0;
    TreeNode **q = (TreeNode **)malloc(cap * sizeof *q);
    q[n++] = root;
    for (size_t i = 0; i < n; i++) {
        if (!q[i]) continue;
        if (n + 2 > cap) { cap *= 2; q = (TreeNode **)realloc(q, cap * sizeof *q); }
        q[n++] = q[i]->left;
        q[n++] = q[i]->right;
    }
    while (n > 0 && !q[n - 1]) n--;
    putchar('[');
    for (size_t i = 0; i < n; i++) { if (i) fputs(", ", stdout); if (q[i]) printf("%lld", q[i]->val); else fputs("null", stdout); }
    putchar(']');
    free(q);
}
static TreeNode *h_build_tree(const long long *vals, const bool *present, size_t n) {
    if (n == 0 || !present[0]) return NULL;
    TreeNode **q = (TreeNode **)malloc(n * sizeof *q);
    size_t qn = 0, i = 1;
    TreeNode *root = TreeNode_new(vals[0], NULL, NULL);
    q[qn++] = root;
    for (size_t k = 0; k < qn && i < n; k++) {
        if (i < n && present[i]) { q[k]->left = TreeNode_new(vals[i], NULL, NULL); q[qn++] = q[k]->left; }
        i++;
        if (i < n && present[i]) { q[k]->right = TreeNode_new(vals[i], NULL, NULL); q[qn++] = q[k]->right; }
        i++;
    }
    free(q);
    return root;
}
"#;

fn c_program(sig: &EntrySig, g: &Generated, cases: &[Vec<String>]) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(&g.prelude);
    // ensure the list/tree helpers exist when the signature mentions them
    let mut done = std::collections::BTreeSet::new();
    let mut shows = String::new();
    c_show_fns(&sig.ret, &mut done, &mut shows);
    out.push_str(C_HARNESS);
    if g.prelude.contains("typedef struct ListNode") {
        out.push_str(C_HARNESS_LIST);
    }
    if g.prelude.contains("typedef struct TreeNode") {
        out.push_str(C_HARNESS_TREE);
    }
    out.push('\n');
    out.push_str(&g.code);
    out.push('\n');
    // show functions that are not fixed helpers
    let composite = shows;
    out.push_str(&composite);
    out.push_str("\nint main(void) {\n");
    let ret_m = c::mangle(&sig.ret);
    for case in cases {
        let mut nodes = vec![NodeData::default()];
        let mut names = Vec::new();
        out.push_str("    {\n");
        for (i, (p, lit)) in sig.params.iter().zip(case.iter()).enumerate() {
            let v = parse_value(lit, &p.ty, &mut nodes)?;
            out.push_str(&format!(
                "        {} a{i} = {};\n",
                c::cty(&p.ty),
                c_build(&v, &p.ty, &nodes)?
            ));
            names.push(if p.by_ref {
                format!("&a{i}")
            } else {
                format!("a{i}")
            });
        }
        out.push_str(&format!(
            "        {} r = {}({});\n        h_show_{ret_m}(r);\n        putchar('\\n');\n    }}\n",
            c::cty(&sig.ret),
            ident(&sig.name),
            names.join(", ")
        ));
    }
    out.push_str("    return 0;\n}\n");
    Ok(out)
}
