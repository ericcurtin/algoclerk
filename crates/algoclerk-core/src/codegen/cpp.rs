// SPDX-License-Identifier: Apache-2.0
//! C++17 code generator: `std::vector`, `std::string`, `std::unordered_map`, ranges, structured bindings.

use super::*;
use std::collections::BTreeSet;

struct Gen<'a> {
    prog: &'a Program,
    info: &'a TypeInfo,
    f: usize,
    out: String,
    ind: usize,
    uses: BTreeSet<&'static str>,
    inc: BTreeSet<&'static str>,
    tmp: usize,
}

fn cprec(op: BinOp) -> u8 {
    match op {
        BinOp::Mul | BinOp::Div | BinOp::Rem => 13,
        BinOp::Add | BinOp::Sub => 12,
        BinOp::Shl | BinOp::Shr => 11,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 10,
        BinOp::Eq | BinOp::Ne => 9,
        BinOp::BitAnd => 8,
        BinOp::BitXor => 7,
        BinOp::BitOr => 6,
        BinOp::And => 5,
        BinOp::Or => 4,
    }
}

const P_UNARY: u8 = 15;

fn helper(name: &str) -> &'static str {
    match name {
        "pop" => "template <class T> T pop(std::vector<T>& v) { T x = std::move(v.back()); v.pop_back(); return x; }\ntemplate <class T> T pop(std::stack<T>& s) { T x = std::move(s.top()); s.pop(); return x; }\ntemplate <class T> T pop(std::queue<T>& q) { T x = std::move(q.front()); q.pop(); return x; }\ntemplate <class T> T pop(std::priority_queue<T>& h) { T x = h.top(); h.pop(); return x; }",
        "pop_dq" => "template <class T> T pop_front(std::deque<T>& d) { T x = std::move(d.front()); d.pop_front(); return x; }\ntemplate <class T> T pop_back(std::deque<T>& d) { T x = std::move(d.back()); d.pop_back(); return x; }",
        "clear" => "template <class C> void clear_all(C& c) { c = C(); }",
        "erase_at" => "template <class T> T erase_at(std::vector<T>& v, long long i) { T x = std::move(v[i]); v.erase(v.begin() + i); return x; }",
        "index_of" => "template <class T> long long index_of(const std::vector<T>& v, const T& x) {\n    auto it = std::find(v.begin(), v.end(), x);\n    return it == v.end() ? -1 : it - v.begin();\n}",
        "slice" => "template <class C> C slice(const C& c, long long lo, long long hi) { return C(c.begin() + lo, c.begin() + hi); }",
        "concat" => "template <class T> std::vector<T> concat(std::vector<T> a, const std::vector<T>& b) { a.insert(a.end(), b.begin(), b.end()); return a; }",
        "sorted" => "template <class T> std::vector<T> sorted(std::vector<T> v) { std::sort(v.begin(), v.end()); return v; }\ninline std::string sorted(std::string s) { std::sort(s.begin(), s.end()); return s; }\ntemplate <class T, class H> std::vector<T> sorted(const std::unordered_set<T, H>& s) { std::vector<T> v(s.begin(), s.end()); std::sort(v.begin(), v.end()); return v; }",
        "reversed" => "template <class T> std::vector<T> reversed(std::vector<T> v) { std::reverse(v.begin(), v.end()); return v; }\ninline std::string reversed(std::string s) { std::reverse(s.begin(), s.end()); return s; }",
        "join" => "inline std::string join(const std::vector<std::string>& v, const std::string& sep) {\n    std::string out;\n    for (size_t i = 0; i < v.size(); ++i) { if (i) out += sep; out += v[i]; }\n    return out;\n}",
        "upper" => "inline std::string upper(std::string s) { for (char& c : s) c = (char)std::toupper((unsigned char)c); return s; }\ninline std::string lower(std::string s) { for (char& c : s) c = (char)std::tolower((unsigned char)c); return s; }",
        "trim" => "inline std::string trim(const std::string& s) {\n    size_t a = 0, b = s.size();\n    while (a < b && std::isspace((unsigned char)s[a])) ++a;\n    while (b > a && std::isspace((unsigned char)s[b - 1])) --b;\n    return s.substr(a, b - a);\n}",
        "find" => "inline long long find(const std::string& s, const std::string& x) { auto p = s.find(x); return p == std::string::npos ? -1 : (long long)p; }\ninline long long find(const std::string& s, char x) { auto p = s.find(x); return p == std::string::npos ? -1 : (long long)p; }",
        "ends_with" => "inline bool ends_with(const std::string& s, const std::string& p) { return s.size() >= p.size() && s.compare(s.size() - p.size(), p.size(), p) == 0; }",
        "split" => "inline std::vector<std::string> split(const std::string& s, const std::string& sep) {\n    std::vector<std::string> out;\n    size_t start = 0, pos;\n    while (!sep.empty() && (pos = s.find(sep, start)) != std::string::npos) {\n        out.push_back(s.substr(start, pos - start));\n        start = pos + sep.size();\n    }\n    out.push_back(s.substr(start));\n    return out;\n}\ninline std::vector<std::string> split(const std::string& s, char sep) { return split(s, std::string(1, sep)); }",
        "replace" => "inline std::string replace(std::string s, const std::string& a, const std::string& b) {\n    if (a.empty()) return s;\n    std::string out;\n    size_t i = 0;\n    while (i < s.size()) {\n        if (s.compare(i, a.size(), a) == 0) { out += b; i += a.size(); } else { out += s[i++]; }\n    }\n    return out;\n}",
        "repeat" => "inline std::string repeat(const std::string& s, long long k) { std::string out; for (long long i = 0; i < k; ++i) out += s; return out; }",
        "hash" => "inline void hash_mix(size_t& h, size_t v) { h ^= v + 0x9e3779b97f4a7c15ULL + (h << 6) + (h >> 2); }\ntemplate <class T> size_t hash_of(const T& x) { return std::hash<T>()(x); }\ntemplate <class A, class B> size_t hash_of(const std::pair<A, B>& p) { size_t h = hash_of(p.first); hash_mix(h, hash_of(p.second)); return h; }\ntemplate <class... Ts> size_t hash_of(const std::tuple<Ts...>& t) { size_t h = 0; std::apply([&](const Ts&... xs) { (hash_mix(h, hash_of(xs)), ...); }, t); return h; }\ntemplate <class T> size_t hash_of(const std::vector<T>& v) { size_t h = v.size(); for (const T& x : v) hash_mix(h, hash_of(x)); return h; }\nstruct Hash { template <class T> size_t operator()(const T& x) const { return hash_of(x); } };",
        "sets" => "template <class T, class H> std::unordered_set<T, H> set_union(std::unordered_set<T, H> a, const std::unordered_set<T, H>& b) { a.insert(b.begin(), b.end()); return a; }\ntemplate <class T, class H> std::unordered_set<T, H> set_intersection(const std::unordered_set<T, H>& a, const std::unordered_set<T, H>& b) { std::unordered_set<T, H> r; for (const T& x : a) if (b.count(x)) r.insert(x); return r; }\ntemplate <class T, class H> std::unordered_set<T, H> set_difference(const std::unordered_set<T, H>& a, const std::unordered_set<T, H>& b) { std::unordered_set<T, H> r; for (const T& x : a) if (!b.count(x)) r.insert(x); return r; }",
        "maps" => "// Maps and sets have no defined order, so keys(), values() and items() hand back the entries sorted by key.\ntemplate <class M> auto items(const M& m) {\n    std::vector<std::pair<typename M::key_type, typename M::mapped_type>> v(m.begin(), m.end());\n    std::sort(v.begin(), v.end(), [](const auto& a, const auto& b) { return a.first < b.first; });\n    return v;\n}\ntemplate <class M> auto keys(const M& m) { std::vector<typename M::key_type> v; for (const auto& kv : items(m)) v.push_back(kv.first); return v; }\ntemplate <class M> auto values(const M& m) { std::vector<typename M::mapped_type> v; for (const auto& kv : items(m)) v.push_back(kv.second); return v; }\ntemplate <class M, class K, class V> V get_or(const M& m, const K& k, const V& d) { auto it = m.find(k); return it == m.end() ? d : it->second; }\ntemplate <class M, class K> auto get(const M& m, const K& k) { auto it = m.find(k); using V = typename M::mapped_type; return it == m.end() ? std::optional<V>() : std::optional<V>(it->second); }",
        _ => "",
    }
}

fn helper_includes(name: &str) -> &'static [&'static str] {
    match name {
        "pop" => &["vector", "stack", "queue"],
        "pop_dq" => &["deque"],
        "index_of" | "sorted" | "reversed" | "upper" | "trim" | "maps" => &["algorithm", "cctype"],
        "hash" => &["tuple", "vector", "functional"],
        "sets" => &["unordered_set"],
        _ => &[],
    }
}

impl<'a> Gen<'a> {
    fn ty(&mut self, t: &Ty) -> String {
        match t {
            Ty::Int => "long long".into(),
            Ty::Bool => "bool".into(),
            Ty::Char => "char".into(),
            Ty::Str => {
                self.inc.insert("string");
                "std::string".into()
            }
            Ty::Unit => "void".into(),
            Ty::Vec(a) => {
                self.inc.insert("vector");
                format!("std::vector<{}>", self.ty(a))
            }
            Ty::Set(a) => {
                self.inc.insert("unordered_set");
                if needs_custom_hash(a) {
                    self.uses.insert("hash");
                    format!("std::unordered_set<{}, ac::Hash>", self.ty(a))
                } else {
                    format!("std::unordered_set<{}>", self.ty(a))
                }
            }
            Ty::Map(k, v) => {
                self.inc.insert("unordered_map");
                if needs_custom_hash(k) {
                    self.uses.insert("hash");
                    format!("std::unordered_map<{}, {}, ac::Hash>", self.ty(k), self.ty(v))
                } else {
                    format!("std::unordered_map<{}, {}>", self.ty(k), self.ty(v))
                }
            }
            Ty::Tuple(ts) => {
                let parts: Vec<String> = ts.iter().map(|x| self.ty(x)).collect();
                if ts.len() == 2 {
                    self.inc.insert("utility");
                    format!("std::pair<{}>", parts.join(", "))
                } else {
                    self.inc.insert("tuple");
                    format!("std::tuple<{}>", parts.join(", "))
                }
            }
            Ty::Option(a) => {
                self.inc.insert("optional");
                format!("std::optional<{}>", self.ty(a))
            }
            Ty::Stack(a) => {
                self.inc.insert("stack");
                format!("std::stack<{}>", self.ty(a))
            }
            Ty::Queue(a) => {
                self.inc.insert("queue");
                format!("std::queue<{}>", self.ty(a))
            }
            Ty::Deque(a) => {
                self.inc.insert("deque");
                format!("std::deque<{}>", self.ty(a))
            }
            Ty::Heap(a) => {
                self.inc.insert("queue");
                format!("std::priority_queue<{}>", self.ty(a))
            }
            Ty::LNode => "ListNode*".into(),
            Ty::TNode => "TreeNode*".into(),
            Ty::Null | Ty::Var(_) => "auto".into(),
        }
    }

    fn ety(&self, e: &Expr) -> Ty {
        self.info.ty(e.id).clone()
    }

    fn line(&mut self, s: &str) {
        self.out.push_str(&pad(self.ind));
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn slot_mutated(&self, slot: u32) -> bool {
        slot != u32::MAX && self.info.fns[self.f].mutated.get(slot as usize).copied().unwrap_or(false)
    }

    // ----- expressions -----
    fn lit_str(&mut self, s: &str) -> Ex {
        self.uses.insert("literals");
        Ex::atom(format!("\"{}\"s", escape_str(s.as_bytes())))
    }

    fn expr(&mut self, e: &Expr) -> Ex {
        match &e.kind {
            ExprKind::Int(n) => {
                if *n == i64::MIN {
                    Ex::new("(-9223372036854775807LL - 1)", 20)
                } else if *n < 0 {
                    Ex::new(format!("{n}"), P_UNARY)
                } else {
                    Ex::atom(n.to_string())
                }
            }
            ExprKind::Bool(b) => Ex::atom(b.to_string()),
            ExprKind::Char(c) => Ex::atom(format!("'{}'", escape_char(*c))),
            ExprKind::Str(s) => self.lit_str(s),
            ExprKind::Null => Ex::atom("nullptr"),
            ExprKind::NoneLit => {
                let t = self.ety(e);
                Ex::atom(format!("{}()", self.ty(&t)))
            }
            ExprKind::Var(n) => Ex::atom(ident(n)),
            ExprKind::Unary(op, a) => {
                let x = self.expr(a);
                let s = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                    UnOp::BitNot => "~",
                };
                let inner = if x.p >= 20 && !x.s.starts_with('-') { x.s } else { format!("({})", x.s) };
                Ex::new(format!("{s}{inner}"), P_UNARY)
            }
            ExprKind::Binary(op, a, b) => {
                let l = self.expr(a);
                let r = self.expr(b);
                let p = cprec(*op);
                if *op == BinOp::Add && matches!(self.ety(a), Ty::Vec(_)) {
                    self.uses.insert("concat");
                    return Ex::atom(format!("ac::concat({}, {})", l.s, r.s));
                }
                Ex::new(format!("{} {} {}", l.at(p), if *op == BinOp::And { "&&" } else if *op == BinOp::Or { "||" } else { op.text() }, r.at(p + 1)), p)
            }
            ExprKind::Index(a, i) => {
                let x = self.expr(a);
                let idx = self.expr(i);
                if matches!(self.ety(a), Ty::Map(..)) {
                    Ex::atom(format!("{}.at({})", x.at(20), idx.s))
                } else {
                    Ex::atom(format!("{}[{}]", x.at(20), idx.s))
                }
            }
            ExprKind::Slice(a, lo, hi) => {
                self.uses.insert("slice");
                let x = self.expr(a);
                let l = lo.as_ref().map_or("0".to_string(), |l| self.expr(l).s);
                let h = hi.as_ref().map_or(format!("(long long){}.size()", x.at(20)), |h| self.expr(h).s);
                Ex::atom(format!("ac::slice({}, {}, {})", x.s, l, h))
            }
            ExprKind::Field(a, f) => {
                let x = self.expr(a);
                Ex::atom(format!("{}->{}", x.at(20), f))
            }
            ExprKind::TupleIdx(a, n) => {
                let x = self.expr(a);
                if matches!(self.ety(a), Ty::Tuple(ref ts) if ts.len() == 2) {
                    Ex::atom(format!("{}.{}", x.at(20), if *n == 0 { "first" } else { "second" }))
                } else {
                    Ex::atom(format!("std::get<{n}>({})", x.s))
                }
            }
            ExprKind::VecLit(items) => {
                let t = self.ety(e);
                let ty = self.ty(&t);
                let parts: Vec<String> = items.iter().map(|x| self.expr(x).s).collect();
                Ex::atom(format!("{ty}{{{}}}", parts.join(", ")))
            }
            ExprKind::VecRepeat(x, n) => {
                let t = self.ety(e);
                let ty = self.ty(&t);
                let (xs, ns) = (self.expr(x).s, self.expr(n).s);
                Ex::atom(format!("{ty}({ns}, {xs})"))
            }
            ExprKind::TupleLit(items) => {
                let t = self.ety(e);
                let ty = self.ty(&t);
                let parts: Vec<String> = items.iter().map(|x| self.expr(x).s).collect();
                Ex::atom(format!("{ty}({})", parts.join(", ")))
            }
            ExprKind::Ctor(t) => {
                let ty = self.ty(t);
                Ex::atom(format!("{ty}()"))
            }
            ExprKind::Call(name, args) => self.call(e, name, args),
            ExprKind::Method(r, name, args) => self.method(e, r, name, args),
        }
    }

    fn call(&mut self, e: &Expr, name: &str, args: &[Expr]) -> Ex {
        let callee = self.info.slots[e.id as usize];
        if callee != u32::MAX {
            let a: Vec<String> = args.iter().map(|x| self.expr(x).s).collect();
            return Ex::atom(format!("{}({})", ident(name), a.join(", ")));
        }
        let a: Vec<Ex> = args.iter().map(|x| self.expr(x)).collect();
        let at = |i: usize| a[i].s.clone();
        let t0 = args.first().map(|x| self.ety(x));
        match name {
            "len" => Ex::new(format!("(long long){}.size()", a[0].at(20)), P_UNARY),
            "min" | "max" => {
                self.inc.insert("algorithm");
                if args.len() == 2 {
                    let ty = self.ty(&t0.unwrap_or(Ty::Int));
                    Ex::atom(format!("std::{name}<{ty}>({}, {})", at(0), at(1)))
                } else {
                    Ex::new(format!("*std::{name}_element({0}.begin(), {0}.end())", a[0].at(20)), P_UNARY)
                }
            }
            "abs" => {
                self.inc.insert("cstdlib");
                Ex::atom(format!("std::abs({})", at(0)))
            }
            "sum" => {
                self.inc.insert("numeric");
                Ex::atom(format!("std::accumulate({0}.begin(), {0}.end(), 0LL)", a[0].at(20)))
            }
            "sorted" => {
                self.uses.insert("sorted");
                Ex::atom(format!("ac::sorted({})", at(0)))
            }
            "reversed" => {
                self.uses.insert("reversed");
                Ex::atom(format!("ac::reversed({})", at(0)))
            }
            "to_string" => {
                self.inc.insert("string");
                if matches!(t0, Some(Ty::Char)) { Ex::atom(format!("std::string(1, {})", at(0))) } else { Ex::atom(format!("std::to_string({})", at(0))) }
            }
            "to_int" => Ex::atom(format!("std::stoll({})", at(0))),
            "ord" => Ex::new(format!("(long long){}", a[0].at(P_UNARY)), P_UNARY),
            "chr" => Ex::new(format!("(char){}", a[0].at(P_UNARY)), P_UNARY),
            "join" => {
                self.uses.insert("join");
                Ex::atom(format!("ac::join({}, {})", at(0), at(1)))
            }
            "from_chars" => Ex::atom(format!("std::string({0}.begin(), {0}.end())", a[0].at(20))),
            "some" => {
                let t = self.ety(e);
                let ty = self.ty(&t);
                Ex::atom(format!("{ty}({})", at(0)))
            }
            "lnode" => Ex::atom(format!("new ListNode{{{}, {}}}", at(0), at(1))),
            "tnode" => Ex::atom(format!("new TreeNode{{{}, {}, {}}}", at(0), at(1), at(2))),
            _ => Ex::atom(format!("{}({})", ident(name), a.iter().map(|x| x.s.clone()).collect::<Vec<_>>().join(", "))),
        }
    }

    fn method(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr]) -> Ex {
        let r = self.expr(recv);
        let a: Vec<String> = args.iter().map(|x| self.expr(x).s).collect();
        let rt = self.ety(recv);
        let rs = r.at(20);
        let atom = Ex::atom;
        match (&rt, name) {
            (Ty::Vec(_), "push") => atom(format!("{rs}.push_back({})", a[0])),
            (Ty::Vec(_), "pop") | (Ty::Stack(_), "pop") | (Ty::Queue(_), "pop") | (Ty::Heap(_), "pop") => {
                self.uses.insert("pop");
                atom(format!("ac::pop({})", r.s))
            }
            (Ty::Deque(_), "pop" | "pop_front") | (Ty::Deque(_), "pop_back") => {
                self.uses.insert("pop_dq");
                atom(format!("ac::{}({})", if name == "pop_back" { "pop_back" } else { "pop_front" }, r.s))
            }
            (Ty::Vec(_), "insert") => atom(format!("{rs}.insert({rs}.begin() + {}, {})", a[0], a[1])),
            (Ty::Vec(_), "remove") => {
                self.uses.insert("erase_at");
                atom(format!("ac::erase_at({}, {})", r.s, a[0]))
            }
            (Ty::Vec(_), "contains") => {
                self.inc.insert("algorithm");
                atom(format!("(std::find({rs}.begin(), {rs}.end(), {}) != {rs}.end())", a[0]))
            }
            (Ty::Vec(_), "index_of") => {
                self.uses.insert("index_of");
                atom(format!("ac::index_of({}, {})", r.s, a[0]))
            }
            (Ty::Vec(_), "count") | (Ty::Str, "count") => {
                self.inc.insert("algorithm");
                atom(format!("(long long)std::count({rs}.begin(), {rs}.end(), {})", a[0]))
            }
            (Ty::Vec(_), "reverse") => {
                self.inc.insert("algorithm");
                atom(format!("std::reverse({rs}.begin(), {rs}.end())"))
            }
            (Ty::Vec(_), "sort") => {
                self.inc.insert("algorithm");
                atom(format!("std::sort({rs}.begin(), {rs}.end())"))
            }
            (Ty::Vec(_), "swap") => {
                self.inc.insert("algorithm");
                atom(format!("std::swap({rs}[{}], {rs}[{}])", a[0], a[1]))
            }
            (Ty::Vec(_), "extend") => atom(format!("{rs}.insert({rs}.end(), {0}.begin(), {0}.end())", Ex::atom(a[0].clone()).at(20))),
            (Ty::Vec(_) | Ty::Str | Ty::Set(_) | Ty::Map(..) | Ty::Stack(_) | Ty::Queue(_) | Ty::Deque(_) | Ty::Heap(_), "is_empty") => atom(format!("{rs}.empty()")),
            (Ty::Vec(_) | Ty::Set(_) | Ty::Map(..) | Ty::Deque(_), "clear") => atom(format!("{rs}.clear()")),
            (Ty::Stack(_) | Ty::Queue(_) | Ty::Heap(_), "clear") => {
                self.uses.insert("clear");
                atom(format!("ac::clear_all({})", r.s))
            }
            (Ty::Str, "upper" | "lower") => {
                self.uses.insert("upper");
                atom(format!("ac::{name}({})", r.s))
            }
            (Ty::Str, "trim") => {
                self.uses.insert("trim");
                atom(format!("ac::trim({})", r.s))
            }
            (Ty::Str, "find") => {
                self.uses.insert("find");
                atom(format!("ac::find({}, {})", r.s, a[0]))
            }
            (Ty::Str, "contains") => atom(format!("({rs}.find({}) != std::string::npos)", a[0])),
            (Ty::Str, "starts_with") => atom(format!("({rs}.rfind({}, 0) == 0)", a[0])),
            (Ty::Str, "ends_with") => {
                self.uses.insert("ends_with");
                atom(format!("ac::ends_with({}, {})", r.s, a[0]))
            }
            (Ty::Str, "split") => {
                self.uses.insert("split");
                atom(format!("ac::split({}, {})", r.s, a[0]))
            }
            (Ty::Str, "chars") => atom(format!("std::vector<char>({rs}.begin(), {rs}.end())")),
            (Ty::Str, "push") => atom(format!("{rs}.push_back({})", a[0])),
            (Ty::Str, "replace") => {
                self.uses.insert("replace");
                atom(format!("ac::replace({}, {}, {})", r.s, a[0], a[1]))
            }
            (Ty::Str, "repeat") => {
                self.uses.insert("repeat");
                atom(format!("ac::repeat({}, {})", r.s, a[0]))
            }
            (Ty::Char, "is_digit" | "is_alpha" | "is_alnum" | "is_upper" | "is_lower" | "is_space") => {
                self.inc.insert("cctype");
                let f = match name {
                    "is_digit" => "isdigit",
                    "is_alpha" => "isalpha",
                    "is_alnum" => "isalnum",
                    "is_upper" => "isupper",
                    "is_lower" => "islower",
                    _ => "isspace",
                };
                atom(format!("(std::{f}((unsigned char){}) != 0)", r.at(P_UNARY)))
            }
            (Ty::Char, "upper" | "lower") => {
                self.inc.insert("cctype");
                atom(format!("(char)std::to{name}((unsigned char){})", r.at(P_UNARY)))
            }
            (Ty::Set(_), "add") => atom(format!("{rs}.insert({})", a[0])),
            (Ty::Set(_) | Ty::Map(..), "remove") => atom(format!("{rs}.erase({})", a[0])),
            (Ty::Set(_) | Ty::Map(..), "contains") => atom(format!("({rs}.count({}) > 0)", a[0])),
            (Ty::Set(_), "union" | "intersection" | "difference") => {
                self.uses.insert("sets");
                atom(format!("ac::set_{name}({}, {})", r.s, a[0]))
            }
            (Ty::Map(..), "get") => {
                self.uses.insert("maps");
                self.inc.insert("optional");
                if a.len() == 2 { atom(format!("ac::get_or({}, {}, {})", r.s, a[0], a[1])) } else { atom(format!("ac::get({}, {})", r.s, a[0])) }
            }
            (Ty::Map(..), "keys" | "values" | "items") => {
                self.uses.insert("maps");
                self.inc.insert("algorithm");
                atom(format!("ac::{name}({})", r.s))
            }
            (Ty::Option(_), "is_some") => atom(format!("{rs}.has_value()")),
            (Ty::Option(_), "is_none") => Ex::new(format!("!{rs}.has_value()"), P_UNARY),
            (Ty::Option(_), "unwrap") => atom(format!("{rs}.value()")),
            (Ty::Option(_), "unwrap_or") => atom(format!("{rs}.value_or({})", a[0])),
            (Ty::Stack(_) | Ty::Queue(_), "push") | (Ty::Heap(_), "push") => atom(format!("{rs}.push({})", a[0])),
            (Ty::Stack(_) | Ty::Heap(_), "peek") => atom(format!("{rs}.top()")),
            (Ty::Queue(_) | Ty::Deque(_), "front") => atom(format!("{rs}.front()")),
            (Ty::Deque(_), "back") => atom(format!("{rs}.back()")),
            (Ty::Deque(_), "push_front") => atom(format!("{rs}.push_front({})", a[0])),
            (Ty::Deque(_), "push_back") => atom(format!("{rs}.push_back({})", a[0])),
            _ => {
                let _ = e;
                atom(format!("{rs}.{}({})", name, a.join(", ")))
            }
        }
    }

    // ----- statements -----
    fn block(&mut self, b: &Block) {
        self.ind += 1;
        for s in &b.stmts {
            self.stmt(s);
        }
        self.ind -= 1;
    }

    fn pat_names(p: &Pat) -> String {
        match p {
            Pat::Var(v) => ident(v),
            Pat::Wild => "_".into(),
            Pat::Tuple(ps) => ps.iter().map(Self::pat_names).collect::<Vec<_>>().join(", "),
        }
    }

    fn pat_flat(p: &Pat) -> bool {
        matches!(p, Pat::Tuple(ps) if ps.iter().all(|x| !matches!(x, Pat::Tuple(_))))
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Let { pat, ty, init } => {
                let v = self.expr(init).s;
                match pat {
                    Pat::Var(n) => {
                        let t = self.info.ty(s.id).clone();
                        let _ = ty;
                        let tys = self.ty(&t);
                        self.line(&format!("{tys} {} = {v};", ident(n)));
                    }
                    Pat::Wild => self.line(&format!("{v};")),
                    Pat::Tuple(_) => self.line(&format!("auto [{}] = {v};", Self::pat_names(pat))),
                }
            }
            StmtKind::Assign { target, op, value } => {
                let t = self.expr(target).s;
                let v = self.expr(value);
                match op {
                    None => self.line(&format!("{t} = {};", v.s)),
                    Some(BinOp::Add) if matches!(self.ety(target), Ty::Vec(_)) => {
                        self.line(&format!("{t}.insert({t}.end(), {0}.begin(), {0}.end());", v.at(20)))
                    }
                    Some(o) => self.line(&format!("{t} {}= {};", o.text(), v.s)),
                }
            }
            StmtKind::If { .. } => self.if_chain(s, false),
            StmtKind::While { cond, body } => {
                let c = self.expr(cond).s;
                self.line(&format!("while ({c}) {{"));
                self.block(body);
                self.line("}");
            }
            StmtKind::ForRange { var, start, end, step, body } => {
                let v = ident(var);
                let a = start.as_ref().map_or("0".to_string(), |x| self.expr(x).s);
                let b = self.expr(end).s;
                let k = match step.as_ref().map(|x| &x.kind) {
                    Some(ExprKind::Int(k)) => *k,
                    _ => 1,
                };
                let head = if k == 1 {
                    format!("for (long long {v} = {a}; {v} < {b}; ++{v}) {{")
                } else if k > 0 {
                    format!("for (long long {v} = {a}; {v} < {b}; {v} += {k}) {{")
                } else if k == -1 {
                    format!("for (long long {v} = {a}; {v} > {b}; --{v}) {{")
                } else {
                    format!("for (long long {v} = {a}; {v} > {b}; {v} -= {}) {{", -k)
                };
                self.line(&head);
                self.block(body);
                self.line("}");
            }
            StmtKind::ForEach { pat, iter, body } => self.for_each(s, pat, iter, body),
            StmtKind::Break => self.line("break;"),
            StmtKind::Continue => self.line("continue;"),
            StmtKind::Return(None) => self.line("return;"),
            StmtKind::Return(Some(e)) => {
                let v = self.expr(e).s;
                self.line(&format!("return {v};"));
            }
            StmtKind::Expr(e) => {
                let v = self.expr(e).s;
                self.line(&format!("{v};"));
            }
        }
    }

    fn if_chain(&mut self, s: &Stmt, continued: bool) {
        let StmtKind::If { cond, then, els } = &s.kind else { return };
        let c = self.expr(cond).s;
        let head = if continued { format!("}} else if ({c}) {{") } else { format!("if ({c}) {{") };
        self.line(&head);
        self.block(then);
        match els {
            None => self.line("}"),
            Some(b) => {
                if let [only] = b.stmts.as_slice()
                    && matches!(only.kind, StmtKind::If { .. })
                {
                    self.if_chain(only, true);
                } else {
                    self.line("} else {");
                    self.block(b);
                    self.line("}");
                }
            }
        }
    }

    fn pat_mutated(&self, s: &Stmt, pat: &Pat) -> bool {
        let _ = pat;
        self.info.pat_slots.get(&s.id).is_some_and(|v| v.iter().any(|&x| self.slot_mutated(x)))
    }

    fn for_each(&mut self, s: &Stmt, pat: &Pat, iter: &Expr, body: &Block) {
        let mutated = self.pat_mutated(s, pat);
        if let Some((kind, args)) = iter_special(iter) {
            let Pat::Tuple(ps) = pat else { return };
            self.tmp += 1;
            let i = if kind == "enumerate" { Self::pat_names(&ps[0]) } else { format!("i{}", self.tmp) };
            let a0 = self.expr(&args[0]).at(20);
            let (bound, names): (String, Vec<(String, String)>) = if kind == "enumerate" {
                (format!("(long long){a0}.size()"), vec![(Self::pat_names(&ps[1]), format!("{a0}[{i}]"))])
            } else {
                let a1 = self.expr(&args[1]).at(20);
                self.inc.insert("algorithm");
                (
                    format!("(long long)std::min({a0}.size(), {a1}.size())"),
                    vec![(Self::pat_names(&ps[0]), format!("{a0}[{i}]")), (Self::pat_names(&ps[1]), format!("{a1}[{i}]"))],
                )
            };
            self.line(&format!("for (long long {i} = 0; {i} < {bound}; ++{i}) {{"));
            self.ind += 1;
            for (n, src) in names {
                if n != "_" {
                    self.line(&format!("{} {n} = {src};", if mutated { "auto" } else { "const auto&" }));
                }
            }
            self.ind -= 1;
            self.block(body);
            self.line("}");
            return;
        }
        let it = self.expr(iter).s;
        let elem = match self.ety(iter) {
            Ty::Vec(t) => *t,
            _ => Ty::Char,
        };
        let q = if mutated || is_copy(&elem) { "auto" } else { "const auto&" };
        match pat {
            Pat::Var(v) => {
                let tn = if is_copy(&elem) { self.ty(&elem) } else { q.to_string() };
                self.line(&format!("for ({tn} {} : {it}) {{", ident(v)));
                self.block(body);
                self.line("}");
            }
            Pat::Wild => {
                self.line(&format!("for ([[maybe_unused]] auto _it : {it}) {{"));
                self.block(body);
                self.line("}");
            }
            Pat::Tuple(_) if Self::pat_flat(pat) => {
                self.line(&format!("for ({q} [{}] : {it}) {{", Self::pat_names(pat)));
                self.block(body);
                self.line("}");
            }
            Pat::Tuple(ps) => {
                self.tmp += 1;
                let item = format!("item{}", self.tmp);
                self.line(&format!("for (const auto& {item} : {it}) {{"));
                self.ind += 1;
                for (k, p) in ps.iter().enumerate() {
                    let acc = if ps.len() == 2 { if k == 0 { format!("{item}.first") } else { format!("{item}.second") } } else { format!("std::get<{k}>({item})") };
                    match p {
                        Pat::Var(v) => self.line(&format!("auto {} = {acc};", ident(v))),
                        Pat::Tuple(_) => self.line(&format!("auto [{}] = {acc};", Self::pat_names(p))),
                        Pat::Wild => {}
                    }
                }
                self.ind -= 1;
                self.block(body);
                self.line("}");
            }
        }
    }

    fn function(&mut self, idx: usize) -> String {
        self.f = idx;
        let f = &self.prog.fns[idx];
        let ret = self.ty(&f.ret);
        let mut params = Vec::new();
        for (i, p) in f.params.iter().enumerate() {
            let t = self.ty(&p.ty);
            let n = ident(&p.name);
            let mutated = self.info.fns[idx].mutated.get(i).copied().unwrap_or(false);
            params.push(if p.by_ref {
                format!("{t}& {n}")
            } else if is_copy(&p.ty) || mutated {
                format!("{t} {n}")
            } else {
                format!("const {t}& {n}")
            });
        }
        format!("{ret} {}({})", ident(&f.name), params.join(", "))
    }
}

fn prelude(g: &Gen<'_>, nodes: (bool, bool)) -> String {
    let mut inc: BTreeSet<&str> = g.inc.clone();
    for u in &g.uses {
        for i in helper_includes(u) {
            inc.insert(i);
        }
    }
    let mut s = String::new();
    for i in &inc {
        s.push_str(&format!("#include <{i}>\n"));
    }
    if g.uses.contains("literals") {
        s.push_str("\nusing namespace std::string_literals;\n");
    }
    if nodes.0 {
        s.push_str("\nstruct ListNode {\n    long long val;\n    ListNode* next;\n};\n");
    }
    if nodes.1 {
        s.push_str("\nstruct TreeNode {\n    long long val;\n    TreeNode* left;\n    TreeNode* right;\n};\n");
    }
    let order = ["hash", "pop", "pop_dq", "clear", "erase_at", "index_of", "slice", "concat", "sorted", "reversed", "join", "upper", "trim", "find", "ends_with", "split", "replace", "repeat", "sets", "maps"];
    let helpers: Vec<&str> = order.iter().copied().filter(|n| g.uses.contains(n)).collect();
    if !helpers.is_empty() {
        s.push_str("\n// Small helpers so the solution below reads like the original program.\nnamespace ac {\n");
        for h in helpers {
            s.push_str(helper(h));
            s.push('\n');
        }
        s.push_str("}  // namespace ac\n");
    }
    s
}

/// Translate to C++17.
pub fn generate(prog: &Program, info: &TypeInfo) -> Generated {
    let mut g = Gen { prog, info, f: 0, out: String::new(), ind: 0, uses: BTreeSet::new(), inc: BTreeSet::new(), tmp: 0 };
    let used = used_types(prog, info);
    let nodes = (used.contains(&Ty::LNode), used.contains(&Ty::TNode));
    let mut protos = Vec::new();
    let mut bodies = String::new();
    for i in 0..prog.fns.len() {
        let head = g.function(i);
        protos.push(format!("{head};"));
        g.out.clear();
        g.ind = 0;
        g.line(&format!("{head} {{"));
        g.block(&prog.fns[i].body);
        g.line("}");
        if !bodies.is_empty() {
            bodies.push('\n');
        }
        bodies.push_str(&g.out);
    }
    let mut code = String::new();
    if prog.fns.len() > 1 {
        for p in &protos[..protos.len() - 1] {
            code.push_str(p);
            code.push('\n');
        }
        code.push('\n');
    }
    code.push_str(&bodies);
    // generate types for prototypes too (already inserted into inc/uses by g.function)
    let pre = prelude(&g, nodes);
    Generated { lang: Lang::Cpp, prelude: pre, code }
}

/// The C++ spelling of a type (used by the test harness).
pub fn ty_name(t: &Ty) -> String {
    let p = Program::default();
    let i = TypeInfo::default();
    let mut g = Gen { prog: &p, info: &i, f: 0, out: String::new(), ind: 0, uses: BTreeSet::new(), inc: BTreeSet::new(), tmp: 0 };
    g.ty(t)
}
