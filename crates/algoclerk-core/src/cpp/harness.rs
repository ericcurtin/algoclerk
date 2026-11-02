// SPDX-License-Identifier: Apache-2.0
//! Differential harness against real C++ compilers.
//!
//! [`translation_unit`] turns a level, a solution text and a list of test cases into one complete C++17
//! translation unit: the level's prelude, the solution, support code (printers and `ListNode*` /
//! `TreeNode*` builders) and a generated `main` that builds the arguments of every case from the test
//! literals, calls the entry function and prints each result on its own line in exactly the notation of
//! [`super::value::render`] (maps and sets that C++ leaves unordered are printed sorted by key).
//!
//! The helpers [`compile`] and [`run_binary`] drive `g++` / `clang++` with `-std=c++17 -Wall -Wextra`
//! under `timeout -s KILL`; `tests/cpp_diff_levels.rs` of `algoclerk-levels` uses all of this to check
//! every reference and naive solution of the campaign.
//!
//! Requirements on a level so that the harness can drive it: a `ListNode` has `int val; ListNode* next;` and
//! a constructor `ListNode(int)`; a `TreeNode` has `int val; TreeNode* left; TreeNode* right;` and a
//! constructor `TreeNode(int)`; entry functions return a value (not `void`).

use super::ast::Ty;
use super::value::{Heap, Plain, parse_value};
use crate::level::Level;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn int_lit(i: i128, ty: &Ty) -> String {
    match ty {
        Ty::Int => {
            if i == i128::from(i32::MIN) {
                "(-2147483647 - 1)".to_string()
            } else {
                i.to_string()
            }
        }
        Ty::UInt => format!("{i}U"),
        Ty::LongLong => {
            if i == i128::from(i64::MIN) {
                "(-9223372036854775807LL - 1)".to_string()
            } else {
                format!("{i}LL")
            }
        }
        Ty::ULongLong | Ty::Size => format!("{i}ULL"),
        _ => i.to_string(),
    }
}

fn char_lit(c: u8) -> String {
    match c {
        b'\\' => "'\\\\'".into(),
        b'\'' => "'\\''".into(),
        0x20..=0x7e if c != b'?' => format!("'{}'", c as char),
        _ => format!("static_cast<char>({})", i32::from(c as i8)),
    }
}

fn str_lit(s: &[u8]) -> String {
    let mut out = String::from("\"");
    for &c in s {
        match c {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'?' => out.push_str("\\?"),
            0x20..=0x7e => out.push(c as char),
            c => {
                let _ = write!(out, "\\{c:03o}");
            }
        }
    }
    out.push('"');
    out
}

fn elem_ty(ty: &Ty) -> Option<&Ty> {
    match ty {
        Ty::Vector(t)
        | Ty::Array(t, _)
        | Ty::Deque(t)
        | Ty::List(t)
        | Ty::Stack(t)
        | Ty::Queue(t)
        | Ty::Set(t)
        | Ty::UnorderedSet(t)
        | Ty::PriorityQueue(t, _) => Some(t),
        _ => None,
    }
}

fn struct_of(ptr: &Ty) -> Option<&str> {
    match ptr {
        Ty::Ptr(inner) => match &**inner {
            Ty::Struct(n) => Some(n),
            _ => None,
        },
        _ => None,
    }
}

/// A C++ expression of type `ty` holding `v`. Every element is spelled with its exact type so that no
/// brace-initialisation overload ambiguity or narrowing can arise.
pub fn cxx_value(v: &Plain, ty: &Ty, heap: &Heap) -> Result<String, String> {
    let list = |xs: &[Plain], et: &Ty| -> Result<String, String> {
        let parts: Result<Vec<String>, String> =
            xs.iter().map(|x| cxx_value(x, et, heap)).collect();
        Ok(parts?.join(", "))
    };
    Ok(match (v, ty) {
        (Plain::Int(i), Ty::Bool) => (if *i != 0 { "true" } else { "false" }).to_string(),
        (Plain::Int(i), Ty::Char) => char_lit(*i as u8),
        (Plain::Int(i), t) if t.is_integral() => int_lit(*i, t),
        (Plain::Str(s), Ty::Str) => format!("std::string({}, {})", str_lit(s), s.len()),
        (Plain::Str(s), Ty::StrView) => {
            format!("std::string_view({}, {})", str_lit(s), s.len())
        }
        (Plain::Opt(None), Ty::Optional(t)) => format!("std::optional<{t}>()"),
        (Plain::Opt(Some(x)), Ty::Optional(t)) => {
            format!("std::optional<{t}>({})", cxx_value(x, t, heap)?)
        }
        (Plain::Seq(xs), Ty::Pair(a, b)) if xs.len() == 2 => format!(
            "std::pair<{a}, {b}>({}, {})",
            cxx_value(&xs[0], a, heap)?,
            cxx_value(&xs[1], b, heap)?
        ),
        (Plain::Seq(xs), Ty::Tuple(ts)) if xs.len() == ts.len() => {
            let parts: Result<Vec<String>, String> = xs
                .iter()
                .zip(ts)
                .map(|(x, t)| cxx_value(x, t, heap))
                .collect();
            let tys: Vec<String> = ts.iter().map(ToString::to_string).collect();
            format!("std::tuple<{}>({})", tys.join(", "), parts?.join(", "))
        }
        (Plain::Seq(xs), Ty::Array(t, n)) => {
            if xs.is_empty() {
                format!("std::array<{t}, {n}>{{}}")
            } else {
                format!("std::array<{t}, {n}>{{{{{}}}}}", list(xs, t)?)
            }
        }
        (Plain::Seq(xs), Ty::Stack(t)) => {
            format!("std::stack<{t}>(std::deque<{t}>{{{}}})", list(xs, t)?)
        }
        (Plain::Seq(xs), Ty::Queue(t)) => {
            format!("std::queue<{t}>(std::deque<{t}>{{{}}})", list(xs, t)?)
        }
        (Plain::Seq(xs), Ty::PriorityQueue(t, cmp)) => match &**cmp {
            Ty::Functor(f, _) => {
                let fname = match f {
                    super::ast::Functor::Less => "less",
                    super::ast::Functor::Greater => "greater",
                };
                format!(
                    "std::priority_queue<{t}, std::vector<{t}>, std::{fname}<{t}>>(std::{fname}<{t}>(), std::vector<{t}>{{{}}})",
                    list(xs, t)?
                )
            }
            _ => return Err("a priority_queue argument must use less or greater".into()),
        },
        (Plain::Seq(xs), t) if elem_ty(t).is_some() => {
            let et = elem_ty(t).cloned().unwrap_or(Ty::Unknown);
            format!("{t}{{{}}}", list(xs, &et)?)
        }
        (Plain::Map(es), Ty::Map(k, val) | Ty::UnorderedMap(k, val)) => {
            let parts: Result<Vec<String>, String> = es
                .iter()
                .map(|(a, b)| {
                    Ok(format!(
                        "std::pair<const {k}, {val}>({}, {})",
                        cxx_value(a, k, heap)?,
                        cxx_value(b, val, heap)?
                    ))
                })
                .collect();
            format!("{ty}{{{}}}", parts?.join(", "))
        }
        (Plain::Ptr(0), Ty::Ptr(_)) => format!("static_cast<{ty}>(nullptr)"),
        (Plain::Ptr(p), t) if struct_of(t) == Some("ListNode") => {
            let mut vals = Vec::new();
            let mut order: Vec<u32> = Vec::new();
            let mut cur = *p;
            let mut cycle: i64 = -1;
            while cur != 0 && order.len() < 5_000_000 {
                if let Some(i) = order.iter().position(|&x| x == cur) {
                    cycle = i as i64;
                    break;
                }
                order.push(cur);
                let f = heap.nodes.get(cur as usize);
                vals.push(
                    f.and_then(|f| f.first())
                        .and_then(Plain::as_int)
                        .ok_or("bad list node")?,
                );
                cur = match f.and_then(|f| f.get(1)) {
                    Some(Plain::Ptr(n)) => *n,
                    _ => 0,
                };
            }
            let vs: Vec<String> = vals.iter().map(|v| int_lit(*v, &Ty::Int)).collect();
            format!(
                "ach::make_list(std::vector<int>{{{}}}, {cycle})",
                vs.join(", ")
            )
        }
        (Plain::Ptr(p), t) if struct_of(t) == Some("TreeNode") => {
            let mut items: Vec<String> = Vec::new();
            let mut queue = std::collections::VecDeque::from([*p]);
            while let Some(n) = queue.pop_front() {
                if n == 0 {
                    items.push("{false, 0}".into());
                    continue;
                }
                if items.len() > 5_000_000 {
                    return Err("tree too large".into());
                }
                let f = heap.nodes.get(n as usize);
                let val = f
                    .and_then(|f| f.first())
                    .and_then(Plain::as_int)
                    .ok_or("bad tree node")?;
                items.push(format!("{{true, {}}}", int_lit(val, &Ty::Int)));
                for i in [1usize, 2] {
                    queue.push_back(match f.and_then(|f| f.get(i)) {
                        Some(Plain::Ptr(c)) => *c,
                        _ => 0,
                    });
                }
            }
            while items.last().is_some_and(|s| s == "{false, 0}") {
                items.pop();
            }
            format!(
                "ach::make_tree(std::vector<std::pair<bool, int>>{{{}}})",
                items.join(", ")
            )
        }
        _ => return Err(format!("the harness cannot spell a value of type {ty}")),
    })
}

/// Can the support code print a value of this type?
fn printable(ty: &Ty, level: &Level) -> bool {
    match ty {
        Ty::Bool | Ty::Char | Ty::Str | Ty::StrView => true,
        t if t.is_integral() => true,
        Ty::Vector(t)
        | Ty::Array(t, _)
        | Ty::Deque(t)
        | Ty::List(t)
        | Ty::Set(t)
        | Ty::UnorderedSet(t)
        | Ty::Optional(t) => printable(t, level),
        Ty::Pair(a, b) | Ty::Map(a, b) | Ty::UnorderedMap(a, b) => {
            printable(a, level) && printable(b, level)
        }
        Ty::Tuple(ts) => ts.iter().all(|t| printable(t, level)),
        Ty::Ptr(_) => struct_of(ty)
            .is_some_and(|n| (n == "ListNode" || n == "TreeNode") && level.prelude_defines(n)),
        _ => false,
    }
}

const HEADERS: &str = "\
#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdio>
#include <deque>
#include <functional>
#include <list>
#include <map>
#include <optional>
#include <queue>
#include <set>
#include <stack>
#include <string>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>
";

const SUPPORT: &str = r#"
namespace ach {

inline void esc(std::string& o, unsigned char c, char quote) {
    switch (c) {
    case '\n': o += "\\n"; break;
    case '\t': o += "\\t"; break;
    case '\r': o += "\\r"; break;
    case 0: o += "\\0"; break;
    case '\\': o += "\\\\"; break;
    default:
        if (c == static_cast<unsigned char>(quote)) {
            o += '\\';
            o += static_cast<char>(c);
        } else if (c >= 0x20 && c < 0x7f) {
            o += static_cast<char>(c);
        } else {
            const char* hex = "0123456789abcdef";
            o += "\\x";
            o += hex[c >> 4];
            o += hex[c & 15];
        }
    }
}

template <class T, class Enable = void> struct P;

template <> struct P<bool, void> {
    static void put(std::string& o, bool v) { o += v ? "true" : "false"; }
};
template <> struct P<char, void> {
    static void put(std::string& o, char v) {
        o += '\'';
        esc(o, static_cast<unsigned char>(v), '\'');
        o += '\'';
    }
};
template <class T>
struct P<T, std::enable_if_t<std::is_integral<T>::value && !std::is_same<T, bool>::value &&
                             !std::is_same<T, char>::value>> {
    static void put(std::string& o, T v) { o += std::to_string(v); }
};
inline void put_text(std::string& o, const char* p, std::size_t n) {
    o += '"';
    for (std::size_t i = 0; i < n; ++i) esc(o, static_cast<unsigned char>(p[i]), '"');
    o += '"';
}
template <> struct P<std::string, void> {
    static void put(std::string& o, const std::string& v) { put_text(o, v.data(), v.size()); }
};
template <> struct P<std::string_view, void> {
    static void put(std::string& o, std::string_view v) { put_text(o, v.data(), v.size()); }
};

template <class It> void put_range(std::string& o, It b, It e) {
    o += '{';
    bool first = true;
    for (; b != e; ++b) {
        if (!first) o += ", ";
        first = false;
        P<std::decay_t<decltype(*b)>>::put(o, *b);
    }
    o += '}';
}

template <class T, class A> struct P<std::vector<T, A>, void> {
    static void put(std::string& o, const std::vector<T, A>& v) { put_range(o, v.begin(), v.end()); }
};
template <class T, std::size_t N> struct P<std::array<T, N>, void> {
    static void put(std::string& o, const std::array<T, N>& v) { put_range(o, v.begin(), v.end()); }
};
template <class T, class A> struct P<std::deque<T, A>, void> {
    static void put(std::string& o, const std::deque<T, A>& v) { put_range(o, v.begin(), v.end()); }
};
template <class T, class A> struct P<std::list<T, A>, void> {
    static void put(std::string& o, const std::list<T, A>& v) { put_range(o, v.begin(), v.end()); }
};
template <class T, class C, class A> struct P<std::set<T, C, A>, void> {
    static void put(std::string& o, const std::set<T, C, A>& v) { put_range(o, v.begin(), v.end()); }
};
template <class T, class H, class E, class A> struct P<std::unordered_set<T, H, E, A>, void> {
    static void put(std::string& o, const std::unordered_set<T, H, E, A>& v) {
        std::vector<const T*> es;
        for (const auto& x : v) es.push_back(&x);
        std::sort(es.begin(), es.end(), [](const T* a, const T* b) { return *a < *b; });
        o += '{';
        bool first = true;
        for (const T* x : es) {
            if (!first) o += ", ";
            first = false;
            P<T>::put(o, *x);
        }
        o += '}';
    }
};
template <class A, class B> struct P<std::pair<A, B>, void> {
    static void put(std::string& o, const std::pair<A, B>& v) {
        o += '{';
        P<A>::put(o, v.first);
        o += ", ";
        P<B>::put(o, v.second);
        o += '}';
    }
};
template <class Tup, std::size_t... I>
void put_tuple(std::string& o, const Tup& t, std::index_sequence<I...>) {
    o += '{';
    std::size_t n = 0;
    ((void)((n++ ? (void)(o += ", ") : (void)0),
            P<std::tuple_element_t<I, Tup>>::put(o, std::get<I>(t))),
     ...);
    (void)n;
    o += '}';
}
template <class... Ts> struct P<std::tuple<Ts...>, void> {
    static void put(std::string& o, const std::tuple<Ts...>& v) {
        put_tuple(o, v, std::index_sequence_for<Ts...>{});
    }
};
template <class T> struct P<std::optional<T>, void> {
    static void put(std::string& o, const std::optional<T>& v) {
        if (!v) {
            o += "nullopt";
        } else {
            o += '{';
            P<T>::put(o, *v);
            o += '}';
        }
    }
};
template <class K, class V, class C, class A> struct P<std::map<K, V, C, A>, void> {
    static void put(std::string& o, const std::map<K, V, C, A>& m) {
        o += '{';
        bool first = true;
        for (const auto& kv : m) {
            if (!first) o += ", ";
            first = false;
            o += '{';
            P<K>::put(o, kv.first);
            o += ", ";
            P<V>::put(o, kv.second);
            o += '}';
        }
        o += '}';
    }
};
template <class K, class V, class H, class E, class A>
struct P<std::unordered_map<K, V, H, E, A>, void> {
    static void put(std::string& o, const std::unordered_map<K, V, H, E, A>& m) {
        std::vector<const std::pair<const K, V>*> es;
        for (const auto& kv : m) es.push_back(&kv);
        std::sort(es.begin(), es.end(), [](const std::pair<const K, V>* a, const std::pair<const K, V>* b) {
            return a->first < b->first;
        });
        o += '{';
        bool first = true;
        for (const auto* kv : es) {
            if (!first) o += ", ";
            first = false;
            o += '{';
            P<K>::put(o, kv->first);
            o += ", ";
            P<V>::put(o, kv->second);
            o += '}';
        }
        o += '}';
    }
};

template <class T> void emit(const T& v) {
    std::string s;
    P<std::decay_t<T>>::put(s, v);
    s += '\n';
    std::fputs(s.c_str(), stdout);
}

} // namespace ach
"#;

const LIST_SUPPORT: &str = r#"
namespace ach {
inline ListNode* make_list(const std::vector<int>& vals, long cycle) {
    ListNode* head = nullptr;
    ListNode* tail = nullptr;
    std::vector<ListNode*> nodes;
    for (int v : vals) {
        ListNode* n = new ListNode(v);
        n->next = nullptr;
        if (tail) tail->next = n; else head = n;
        tail = n;
        nodes.push_back(n);
    }
    if (cycle >= 0 && tail && static_cast<std::size_t>(cycle) < nodes.size())
        tail->next = nodes[static_cast<std::size_t>(cycle)];
    return head;
}
template <> struct P<ListNode*, void> {
    static void put(std::string& o, const ListNode* p) {
        if (!p) { o += "nullptr"; return; }
        std::vector<const ListNode*> order;
        std::unordered_map<const ListNode*, std::size_t> pos;
        long cycle = -1;
        while (p && order.size() < 1000000) {
            auto it = pos.find(p);
            if (it != pos.end()) { cycle = static_cast<long>(it->second); break; }
            pos[p] = order.size();
            order.push_back(p);
            p = p->next;
        }
        o += '{';
        for (std::size_t i = 0; i < order.size(); ++i) {
            if (i) o += ", ";
            o += std::to_string(order[i]->val);
        }
        o += '}';
        if (cycle >= 0) { o += " @"; o += std::to_string(cycle); }
    }
};
} // namespace ach
"#;

const TREE_SUPPORT: &str = r#"
namespace ach {
inline TreeNode* make_tree(const std::vector<std::pair<bool, int>>& items) {
    if (items.empty() || !items[0].first) return nullptr;
    std::vector<TreeNode*> ids;
    for (const auto& it : items) {
        if (it.first) {
            TreeNode* n = new TreeNode(it.second);
            n->left = nullptr;
            n->right = nullptr;
            ids.push_back(n);
        } else {
            ids.push_back(nullptr);
        }
    }
    std::size_t next = 1;
    for (std::size_t i = 0; i < ids.size(); ++i) {
        if (!ids[i]) continue;
        if (next < ids.size()) ids[i]->left = ids[next++];
        if (next < ids.size()) ids[i]->right = ids[next++];
    }
    return ids[0];
}
template <> struct P<TreeNode*, void> {
    static void put(std::string& o, const TreeNode* root) {
        if (!root) { o += "nullptr"; return; }
        std::vector<std::pair<bool, int>> items;
        std::deque<const TreeNode*> q;
        q.push_back(root);
        while (!q.empty() && items.size() < 1000000) {
            const TreeNode* n = q.front();
            q.pop_front();
            if (!n) { items.push_back({false, 0}); continue; }
            items.push_back({true, n->val});
            q.push_back(n->left);
            q.push_back(n->right);
        }
        while (!items.empty() && !items.back().first) items.pop_back();
        o += '{';
        for (std::size_t i = 0; i < items.size(); ++i) {
            if (i) o += ", ";
            if (items[i].first) o += std::to_string(items[i].second); else o += "nullptr";
        }
        o += '}';
    }
};
} // namespace ach
"#;

/// The text of the support code (headers, printers, builders) for a level.
pub fn support_code(level: &Level) -> String {
    let mut s = String::from(HEADERS);
    s.push_str(SUPPORT);
    if level.prelude_defines("ListNode") {
        s.push_str(LIST_SUPPORT);
    }
    if level.prelude_defines("TreeNode") {
        s.push_str(TREE_SUPPORT);
    }
    s
}

/// The generated function that builds the arguments of one case, calls the entry and prints the result.
pub fn case_function(level: &Level, index: usize, args: &[String]) -> Result<String, String> {
    let sig = &level.signature;
    if sig.params.len() != args.len() {
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
            .map_err(|e| format!("argument {} ({lit:.60}): {e}", p.name))?;
        let _ = writeln!(
            body,
            "    {} a{i} = {};",
            p.spec.ty,
            cxx_value(&v, &p.spec.ty, &heap)?
        );
        names.push(format!("a{i}"));
    }
    Ok(format!(
        "static void ach_case_{index}() {{\n{body}    auto r = {}({});\n    ach::emit(r);\n}}\n",
        sig.name,
        names.join(", ")
    ))
}

/// A complete translation unit: prelude, `source` (the solution), support code, one function per case and
/// a `main` that runs them in order. Running the binary prints one canonical line per case.
pub fn translation_unit(
    level: &Level,
    source: &str,
    cases: &[Vec<String>],
) -> Result<String, String> {
    if !printable(&level.signature.ret.ty, level) {
        return Err(format!(
            "the harness cannot print results of type {}",
            level.signature.ret.ty
        ));
    }
    let mut out = level.full_source(source);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n// ---- generated by the AlgoClerk harness ----\n");
    out.push_str(&support_code(level));
    out.push('\n');
    for (i, c) in cases.iter().enumerate() {
        out.push_str(&case_function(level, i, c)?);
    }
    out.push_str("\nint main() {\n");
    for i in 0..cases.len() {
        let _ = writeln!(out, "    ach_case_{i}();");
    }
    out.push_str("    return 0;\n}\n");
    Ok(out)
}

/// A host C++ compiler.
#[derive(Clone, Debug)]
pub struct Compiler {
    /// Executable name (`g++`, `clang++`).
    pub exe: String,
    /// Flags for the plain `-Wall -Wextra` build.
    pub flags: Vec<String>,
}

impl Compiler {
    /// Does the executable run?
    fn works(exe: &str) -> bool {
        Command::new(exe)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// The compilers found on this host (`g++` and `clang++` when installed).
pub fn available_compilers() -> Vec<Compiler> {
    ["g++", "clang++"]
        .into_iter()
        .filter(|e| Compiler::works(e))
        .map(|e| Compiler {
            exe: e.to_string(),
            flags: ["-std=c++17", "-Wall", "-Wextra", "-O0"]
                .map(String::from)
                .to_vec(),
        })
        .collect()
}

/// The outcome of a successful compile.
#[derive(Clone, Debug)]
pub struct Built {
    /// The binary.
    pub exe: PathBuf,
    /// Compiler output (warnings); empty for a clean build.
    pub diagnostics: String,
}

/// Compile `source` into `dir/<name>`; `sanitize` adds `-fsanitize=undefined,address`. The compiler runs under
/// `timeout -s KILL <secs>`. Errors carry the compiler's output.
pub fn compile(
    cc: &Compiler,
    source: &str,
    dir: &Path,
    name: &str,
    sanitize: bool,
    secs: u64,
) -> Result<Built, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let src = dir.join(format!("{name}.cpp"));
    let exe = dir.join(name);
    std::fs::write(&src, source).map_err(|e| format!("cannot write {}: {e}", src.display()))?;
    let mut cmd = Command::new("timeout");
    cmd.args(["-s", "KILL", &secs.to_string(), &cc.exe]);
    cmd.args(&cc.flags);
    if sanitize {
        cmd.args([
            "-fsanitize=undefined,address",
            "-fno-sanitize-recover=undefined",
            "-fno-omit-frame-pointer",
        ]);
    }
    cmd.arg("-o").arg(&exe).arg(&src);
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {}: {e}", cc.exe))?;
    let text =
        String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        return Err(format!(
            "{} failed (status {:?}; 137 means timeout):\n{}",
            cc.exe,
            out.status.code(),
            text.chars().take(6000).collect::<String>()
        ));
    }
    Ok(Built {
        exe,
        diagnostics: text,
    })
}

/// Run a binary under `timeout -s KILL <secs>` and return its stdout lines. A non-zero exit (crash,
/// sanitizer report, timeout) is an error that carries the output so far and stderr.
pub fn run_binary(exe: &Path, secs: u64, sanitize: bool) -> Result<Vec<String>, String> {
    let mut cmd = Command::new("timeout");
    cmd.args(["-s", "KILL", &secs.to_string()]).arg(exe);
    if sanitize {
        cmd.env("ASAN_OPTIONS", "detect_leaks=0:abort_on_error=0");
        cmd.env("UBSAN_OPTIONS", "print_stacktrace=1:halt_on_error=1");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        let done = stdout.lines().count();
        return Err(format!(
            "{} exited with {:?} after printing {done} lines (137/KILL means timeout):\n{}",
            exe.display(),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
                .chars()
                .take(4000)
                .collect::<String>()
        ));
    }
    Ok(stdout.lines().map(str::to_string).collect())
}

/// Options of [`diff_level`].
#[derive(Clone, Debug)]
pub struct DiffOpts {
    /// Cap for the `n` of large generators so that naive solutions finish quickly in real C++; `None`
    /// runs the large tests at full size.
    pub large_cap: Option<i64>,
    /// Also build the reference with `-fsanitize=undefined,address` and run it.
    pub sanitize: bool,
    /// Also run the interpreter ([`super::run::Compiled::call`]) and compare it with the compilers.
    pub interpreter: bool,
    /// Seconds allowed for one compiler invocation.
    pub compile_secs: u64,
    /// Seconds allowed for one binary run.
    pub run_secs: u64,
    /// Scratch directory (created; files are left for inspection).
    pub dir: PathBuf,
    /// The compilers to use (see [`available_compilers`]).
    pub compilers: Vec<Compiler>,
}

impl DiffOpts {
    /// Defaults: large tests capped at n = 2500, sanitizer and interpreter on, host compilers.
    pub fn new(dir: PathBuf) -> DiffOpts {
        DiffOpts {
            large_cap: Some(2500),
            sanitize: true,
            interpreter: true,
            compile_secs: 240,
            run_secs: 90,
            dir,
            compilers: available_compilers(),
        }
    }
}

/// What [`diff_level`] found for one level.
#[derive(Clone, Debug, Default)]
pub struct DiffReport {
    /// Level id.
    pub level_id: String,
    /// Real problems: mismatches, warnings, crashes, compile errors.
    pub problems: Vec<String>,
    /// Informational notes (skipped parts).
    pub notes: Vec<String>,
    /// The cases that were run.
    pub cases: Vec<crate::judge::RawTest>,
    /// Canonical expected output of every case: the level's declared value, else the first compiler's
    /// output of the reference.
    pub expected: Vec<String>,
    /// How many cases the interpreter ran (0 when it is not available).
    pub interpreter_cases: usize,
}

fn short(s: &str) -> String {
    let t: String = s.chars().take(70).collect();
    if s.chars().count() > 70 {
        format!("{t}...")
    } else {
        t
    }
}

fn mismatch(
    level: &Level,
    who: &str,
    cases: &[crate::judge::RawTest],
    got: &[String],
    want: &[String],
) -> Option<String> {
    use crate::judge::canonical;
    if got.len() != want.len() {
        return Some(format!(
            "{who}: printed {} lines for {} cases",
            got.len(),
            want.len()
        ));
    }
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        let gc = canonical(g, &level.signature.ret.ty, level.compare);
        if &gc != w {
            return Some(format!(
                "{who}: case {i} ({}) gave {} but expected {}",
                cases[i]
                    .args
                    .iter()
                    .map(|a| short(a))
                    .collect::<Vec<_>>()
                    .join(", "),
                short(&gc),
                short(w)
            ));
        }
    }
    None
}

/// Compile and run the level's reference and naive solutions with every available compiler on the standard
/// tests plus (reduced) large tests, and compare the printed results with the level's declared expected
/// values, with each other, with a sanitizer build and (optionally) with the interpreter.
pub fn diff_level(level: &Level, o: &DiffOpts) -> DiffReport {
    use crate::judge::{canonical, raw_suite};
    let mut rep = DiffReport {
        level_id: level.id.clone(),
        ..DiffReport::default()
    };
    let suite = match raw_suite(level, o.large_cap) {
        Ok(s) => s,
        Err(e) => {
            rep.problems.push(format!("cannot expand the tests: {e}"));
            return rep;
        }
    };
    let args: Vec<Vec<String>> = suite.iter().map(|t| t.args.clone()).collect();
    rep.cases = suite;
    if o.compilers.is_empty() {
        rep.notes.push("no C++ compiler found".into());
    }
    let ret = level.signature.ret.ty.clone();
    let mut solutions: Vec<(&str, &str)> = vec![("reference", level.reference.as_str())];
    if let Some(n) = &level.naive {
        solutions.push(("naive", n.as_str()));
    }
    let mut outputs: Vec<(String, Vec<String>)> = Vec::new();
    for (kind, text) in &solutions {
        let tu = match translation_unit(level, text, &args) {
            Ok(t) => t,
            Err(e) => {
                rep.problems.push(format!("{kind}: harness: {e}"));
                continue;
            }
        };
        for cc in &o.compilers {
            let name = format!("{}-{kind}-{}", level.id, cc.exe.replace('+', "p"));
            let who = format!("{kind} [{}]", cc.exe);
            match compile(cc, &tu, &o.dir, &name, false, o.compile_secs) {
                Err(e) => rep.problems.push(format!("{who}: {e}")),
                Ok(b) => {
                    if !b.diagnostics.trim().is_empty() {
                        rep.problems.push(format!(
                            "{who}: compiler diagnostics with -Wall -Wextra:\n{}",
                            b.diagnostics.chars().take(1500).collect::<String>()
                        ));
                    }
                    match run_binary(&b.exe, o.run_secs, false) {
                        Err(e) => rep.problems.push(format!("{who}: {e}")),
                        Ok(lines) => outputs.push((who, lines)),
                    }
                }
            }
        }
        if *kind == "reference" && o.sanitize {
            // g++ first, then clang++: the sanitizer runtimes are separate packages and a host may lack
            // one of them (a link error naming libasan/libubsan is a skip, any other error a problem).
            let mut cands: Vec<&Compiler> = o.compilers.iter().filter(|c| c.exe == "g++").collect();
            cands.extend(o.compilers.iter().filter(|c| c.exe != "g++"));
            let mut ran = false;
            for cc in cands {
                let name = format!("{}-sanitized-{}", level.id, cc.exe.replace('+', "p"));
                let who = format!("reference [{} -fsanitize=undefined,address]", cc.exe);
                match compile(cc, &tu, &o.dir, &name, true, o.compile_secs) {
                    Err(e)
                        if e.contains("libasan")
                            || e.contains("libubsan")
                            || e.contains("sanitizer") =>
                    {
                        rep.notes
                            .push(format!("{} sanitizer runtime missing", cc.exe));
                    }
                    Err(e) => {
                        rep.problems.push(format!("{who}: {e}"));
                        ran = true;
                        break;
                    }
                    Ok(b) => {
                        match run_binary(&b.exe, o.run_secs * 2, true) {
                            Err(e) => rep.problems.push(format!("{who}: {e}")),
                            Ok(lines) => outputs.push((who, lines)),
                        }
                        ran = true;
                        break;
                    }
                }
            }
            if !ran {
                rep.notes.push("no sanitizer build possible".into());
            }
        }
    }
    // The baseline: declared values, else the first reference output.
    let first_ref = outputs
        .iter()
        .find(|(w, _)| w.starts_with("reference"))
        .map(|(_, l)| l.clone());
    rep.expected = rep
        .cases
        .iter()
        .enumerate()
        .map(|(i, t)| match (&t.expect, &first_ref) {
            (Some(e), _) => canonical(e, &ret, level.compare),
            (None, Some(l)) => l
                .get(i)
                .map_or(String::new(), |g| canonical(g, &ret, level.compare)),
            (None, None) => String::new(),
        })
        .collect();
    if first_ref.is_some() {
        for (who, lines) in &outputs {
            if let Some(m) = mismatch(level, who, &rep.cases, lines, &rep.expected) {
                rep.problems.push(m);
            }
        }
    }
    if o.interpreter && first_ref.is_some() {
        interpreter_half(level, o, &mut rep);
    }
    rep
}

fn interpreter_half(level: &Level, o: &DiffOpts, rep: &mut DiffReport) {
    use crate::cpp::interp::{Limits, RtKind};
    use crate::judge::canonical;
    let _ = o;
    let ret = level.signature.ret.ty.clone();
    let mut solutions: Vec<(&str, &str)> = vec![("reference", level.reference.as_str())];
    if let Some(n) = &level.naive {
        solutions.push(("naive", n.as_str()));
    }
    for (kind, text) in solutions {
        let sol = match super::run::compile(&level.prelude, text, &level.check_opts()) {
            Ok(s) => s,
            Err(d) => {
                let msg = d.first().map_or(String::new(), |d| d.msg.clone());
                if msg.contains("not written yet") {
                    rep.notes.push("interpreter not available".into());
                } else {
                    rep.problems
                        .push(format!("interpreter: {kind} does not compile: {msg}"));
                }
                return;
            }
        };
        let limits = Limits {
            steps: level.budgets.large_steps.saturating_mul(40),
            ..Limits::default()
        };
        for (i, t) in rep.cases.clone().iter().enumerate() {
            if kind == "naive" && t.large {
                continue;
            }
            let called = match sol.call(&level.signature.name, &t.args, &limits) {
                Ok(c) => c,
                Err(e) => {
                    rep.problems
                        .push(format!("interpreter: {kind} case {i}: {e}"));
                    return;
                }
            };
            match &called.report.result {
                Err(e) if e.kind == RtKind::Internal => {
                    rep.notes.push("interpreter not available".into());
                    return;
                }
                Err(e) => {
                    rep.problems.push(format!(
                        "interpreter: {kind} case {i} stops with {:?} ({}) but the real compilers ran it",
                        e.kind, e.msg
                    ));
                    return;
                }
                Ok(_) => {
                    let got = canonical(&called.output.unwrap_or_default(), &ret, level.compare);
                    rep.interpreter_cases += 1;
                    if got != rep.expected[i] {
                        rep.problems.push(format!(
                            "interpreter: {kind} case {i} ({}) gave {} but the compilers and the level say {}",
                            t.args.iter().map(|a| short(a)).collect::<Vec<_>>().join(", "),
                            short(&got),
                            short(&rep.expected[i])
                        ));
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpp::ast::Functor;

    fn lit(text: &str, ty: &Ty) -> String {
        let mut heap = Heap::default();
        let v = parse_value(text, ty, &mut heap).unwrap();
        cxx_value(&v, ty, &heap).unwrap()
    }

    #[test]
    fn spells_values_with_exact_types() {
        assert_eq!(lit("-5", &Ty::LongLong), "-5LL");
        assert_eq!(lit("-2147483648", &Ty::Int), "(-2147483647 - 1)");
        assert_eq!(lit("'a'", &Ty::Char), "'a'");
        assert_eq!(lit("'\\n'", &Ty::Char), "static_cast<char>(10)");
        assert_eq!(
            lit("\"a?b\\\"\"", &Ty::Str),
            "std::string(\"a\\?b\\\"\", 4)"
        );
        assert_eq!(
            lit("{{1, 2}, {}}", &Ty::vector(Ty::vector(Ty::Int))),
            "std::vector<std::vector<int>>{std::vector<int>{1, 2}, std::vector<int>{}}"
        );
        assert_eq!(
            lit(
                "{{1, true}}",
                &Ty::vector(Ty::Pair(Box::new(Ty::Int), Box::new(Ty::Bool)))
            ),
            "std::vector<std::pair<int, bool>>{std::pair<int, bool>(1, true)}"
        );
        assert_eq!(
            lit("nullopt", &Ty::Optional(Box::new(Ty::Int))),
            "std::optional<int>()"
        );
        assert_eq!(
            lit("{1, 2}", &Ty::Array(Box::new(Ty::Int), 2)),
            "std::array<int, 2>{{1, 2}}"
        );
        let l = Ty::Ptr(Box::new(Ty::Struct("ListNode".into())));
        assert_eq!(
            lit("{1, 2, 3} @1", &l),
            "ach::make_list(std::vector<int>{1, 2, 3}, 1)"
        );
        assert_eq!(lit("nullptr", &l), "static_cast<ListNode*>(nullptr)");
        let t = Ty::Ptr(Box::new(Ty::Struct("TreeNode".into())));
        assert_eq!(
            lit("{1, nullptr, 2}", &t),
            "ach::make_tree(std::vector<std::pair<bool, int>>{{true, 1}, {false, 0}, {true, 2}})"
        );
        let _ = Functor::Less;
    }

    #[test]
    fn strings_use_octal_escapes_that_never_merge_with_digits() {
        let s = lit("\"\\x01\\x31\"", &Ty::Str);
        assert_eq!(s, "std::string(\"\\0011\", 2)");
    }
}
