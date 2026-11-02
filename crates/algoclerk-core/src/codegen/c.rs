// SPDX-License-Identifier: Apache-2.0
//! C11 code generator. The prelude instantiates small typed containers (vectors, hash sets and maps,
//! deques, heaps, tuples, options) and the solution is written plainly against them.

use super::c_templates as t;
use super::*;
use crate::lang::infer::{hashable, orderable};
use std::collections::BTreeSet;

struct Gen<'a> {
    prog: &'a Program,
    info: &'a TypeInfo,
    f: usize,
    out: String,
    ind: usize,
    tmp: usize,
    has: BTreeSet<String>,
}

const P_UNARY: u8 = 15;

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

/// Name fragment used to build instance names.
pub fn mangle(t: &Ty) -> String {
    match t {
        Ty::Int => "int".into(),
        Ty::Bool => "bool".into(),
        Ty::Char => "char".into(),
        Ty::Str => "str".into(),
        Ty::LNode => "lnode".into(),
        Ty::TNode => "tnode".into(),
        Ty::Unit => "unit".into(),
        other => sname(other),
    }
}

/// The C struct name of a composite type.
pub fn sname(t: &Ty) -> String {
    match t {
        Ty::Vec(a) | Ty::Stack(a) => format!("Vec_{}", mangle(a)),
        Ty::Set(a) => format!("Set_{}", mangle(a)),
        Ty::Map(k, v) => format!("Map_{}_{}", mangle(k), mangle(v)),
        Ty::Tuple(ts) => format!("Tup_{}", ts.iter().map(mangle).collect::<Vec<_>>().join("_")),
        Ty::Option(a) => format!("Opt_{}", mangle(a)),
        Ty::Queue(a) | Ty::Deque(a) => format!("Deq_{}", mangle(a)),
        Ty::Heap(a) => format!("Heap_{}", mangle(a)),
        Ty::Str => "Vec_char".into(),
        other => mangle(other),
    }
}

/// The C spelling of a type.
pub fn cty(t: &Ty) -> String {
    match t {
        Ty::Int => "long long".into(),
        Ty::Bool => "bool".into(),
        Ty::Char => "char".into(),
        Ty::Str => "Str".into(),
        Ty::Unit => "void".into(),
        Ty::LNode => "ListNode *".into(),
        Ty::TNode => "TreeNode *".into(),
        other => sname(other),
    }
}

/// Prefix of the `_eq`, `_cmp`, `_hash` and `_clone` functions of a type.
pub fn ops(t: &Ty) -> String {
    match t {
        Ty::Str => "Vec_char".into(),
        other => mangle(other),
    }
}

fn fill(tpl: &str, pairs: &[(&str, String)]) -> String {
    let mut s = tpl.to_string();
    let order = ["$KV", "$VV", "$IV", "$KP", "$VP", "$FIELDS", "$PARAMS", "$INIT", "$CLONE", "$EQ", "$CMP", "$HASH", "$K", "$V", "$T", "$O", "$E", "$P", "$S"];
    for key in order {
        if let Some((_, v)) = pairs.iter().find(|(k, _)| *k == key) {
            s = s.replace(key, v);
        }
    }
    s
}

fn instance(ty: &Ty, has: &BTreeSet<String>) -> Option<String> {
    let hv = |t: &Ty| has.contains(&sname(t));
    Some(match ty {
        Ty::Vec(a) | Ty::Stack(a) => {
            let p = [("$S", sname(ty)), ("$E", cty(a)), ("$P", ops(a))];
            let mut s = fill(t::VEC, &p);
            if orderable(a) {
                s.push_str(&fill(t::VEC_ORD, &p));
            }
            if hashable(ty) {
                s.push_str(&fill(t::VEC_HASH, &p));
            }
            if **a == Ty::Int {
                s.push_str(&fill(t::VEC_INT, &p));
            }
            s
        }
        Ty::Set(a) => {
            let p = [("$S", sname(ty)), ("$E", cty(a)), ("$P", ops(a)), ("$V", sname(&Ty::Vec(a.clone())))];
            let mut s = fill(t::SET, &p);
            if hv(&Ty::Vec(a.clone())) {
                s.push_str(&fill(t::SET_SORTED, &p));
            }
            s
        }
        Ty::Map(k, v) => {
            let kv = Ty::Vec(k.clone());
            let vv = Ty::Vec(v.clone());
            let iv = Ty::Vec(Box::new(Ty::Tuple(vec![(**k).clone(), (**v).clone()])));
            let p = [
                ("$S", sname(ty)),
                ("$K", cty(k)),
                ("$V", cty(v)),
                ("$KP", ops(k)),
                ("$VP", ops(v)),
                ("$KV", sname(&kv)),
                ("$VV", sname(&vv)),
                ("$IV", sname(&iv)),
                ("$T", sname(&Ty::Tuple(vec![(**k).clone(), (**v).clone()]))),
                ("$O", sname(&Ty::Option(v.clone()))),
            ];
            let mut s = fill(t::MAP, &p);
            if hv(&kv) {
                s.push_str(&fill(t::MAP_KEYS, &p));
                if hv(&vv) {
                    s.push_str(&fill(t::MAP_VALUES, &p));
                }
                if hv(&iv) {
                    s.push_str(&fill(t::MAP_ITEMS, &p));
                }
            }
            if hv(&Ty::Option(v.clone())) {
                s.push_str(&fill(t::MAP_GET, &p));
            }
            s
        }
        Ty::Tuple(ts) => {
            let n = ts.len();
            let mut fields = String::new();
            let mut params = Vec::new();
            let (mut init, mut clone, mut eq, mut cmp, mut hash) = (String::new(), String::new(), Vec::new(), String::new(), String::new());
            for (i, e) in ts.iter().enumerate() {
                fields.push_str(&format!("{} f{i}; ", cty(e)));
                params.push(format!("{} a{i}", cty(e)));
                init.push_str(&format!("t.f{i} = a{i}; "));
                clone.push_str(&format!("c.f{i} = {}_clone(t.f{i}); ", ops(e)));
                eq.push(format!("{}_eq(a.f{i}, b.f{i})", ops(e)));
                cmp.push_str(&format!("c = {}_cmp(a.f{i}, b.f{i}); if (c) return c; ", ops(e)));
                hash.push_str(&format!("h = hash_mix(h, {}_hash(t.f{i})); ", ops(e)));
            }
            let _ = n;
            let p = [
                ("$S", sname(ty)),
                ("$FIELDS", fields),
                ("$PARAMS", params.join(", ")),
                ("$INIT", init),
                ("$CLONE", clone),
                ("$EQ", eq.join(" && ")),
                ("$CMP", cmp),
                ("$HASH", hash),
            ];
            let mut s = fill(t::TUPLE, &p);
            if ts.iter().all(orderable) {
                s.push_str(&fill(t::TUPLE_ORD, &p));
            }
            if ts.iter().all(hashable) {
                s.push_str(&fill(t::TUPLE_HASH, &p));
            }
            s
        }
        Ty::Option(a) => fill(t::OPT, &[("$S", sname(ty)), ("$E", cty(a)), ("$P", ops(a))]),
        Ty::Queue(a) | Ty::Deque(a) => fill(t::DEQ, &[("$S", sname(ty)), ("$E", cty(a)), ("$P", ops(a))]),
        Ty::Heap(a) => fill(t::HEAP, &[("$S", sname(ty)), ("$E", cty(a)), ("$P", ops(a))]),
        _ => return None,
    })
}

impl<'a> Gen<'a> {
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
    fn is_ref_param(&self, e: &Expr) -> bool {
        if let ExprKind::Var(_) = e.kind {
            let slot = self.info.slots[e.id as usize] as usize;
            return self.prog.fns[self.f].params.get(slot).is_some_and(|p| p.by_ref);
        }
        false
    }
    fn is_param(&self, e: &Expr) -> bool {
        matches!(e.kind, ExprKind::Var(_)) && (self.info.slots[e.id as usize] as usize) < self.prog.fns[self.f].params.len()
    }

    fn stable(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Char(_) => true,
            ExprKind::Var(_) => !self.slot_mutated(self.info.slots[e.id as usize]),
            ExprKind::Binary(_, a, b) => self.stable(a) && self.stable(b),
            ExprKind::Unary(_, a) => self.stable(a),
            ExprKind::Call(n, a) if n == "len" => self.stable(&a[0]),
            _ => false,
        }
    }

    /// Lvalue text of a place expression.
    fn lval(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Var(n) => {
                if self.is_ref_param(e) { format!("(*{})", ident(n)) } else { ident(n) }
            }
            ExprKind::Index(a, i) => {
                let at = self.ety(a);
                let base = self.lval(a);
                let idx = self.expr(i).s;
                match at {
                    Ty::Map(..) => format!("(*{}_at(&{base}, {idx}))", sname(&at)),
                    _ => format!("{base}.data[{idx}]"),
                }
            }
            ExprKind::TupleIdx(a, n) => format!("{}.f{n}", self.lval(a)),
            _ => self.expr(e).at(20),
        }
    }

    fn owned(&mut self, e: &Expr) -> String {
        let t = self.ety(e);
        let x = self.expr(e);
        if is_place(e) && !is_copy(&t) { format!("{}_clone({})", ops(&t), x.s) } else { x.s }
    }

    fn expr(&mut self, e: &Expr) -> Ex {
        match &e.kind {
            ExprKind::Int(n) => {
                if *n == i64::MIN {
                    Ex::atom("(-9223372036854775807LL - 1)")
                } else if *n < 0 {
                    Ex::new(format!("{n}"), P_UNARY)
                } else if *n > i64::from(i32::MAX) {
                    Ex::atom(format!("{n}LL"))
                } else {
                    Ex::atom(n.to_string())
                }
            }
            ExprKind::Bool(b) => Ex::atom(b.to_string()),
            ExprKind::Char(c) => Ex::atom(format!("'{}'", escape_char(*c))),
            ExprKind::Str(s) => Ex::atom(format!("str_from(\"{}\", {})", escape_str(s.as_bytes()), s.len())),
            ExprKind::Null => Ex::atom("NULL"),
            ExprKind::NoneLit => Ex::atom(format!("{}_none()", sname(&self.ety(e)))),
            ExprKind::Var(_) => Ex::atom(self.lval(e)),
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
            ExprKind::Binary(op, a, b) => self.binary(*op, a, b),
            ExprKind::Index(..) | ExprKind::TupleIdx(..) => Ex::atom(self.lval(e)),
            ExprKind::Slice(a, lo, hi) => {
                let at = self.ety(a);
                let x = self.expr(a).s;
                let l = lo.as_ref().map_or("0".to_string(), |l| self.expr(l).s);
                let h = match hi {
                    Some(h) => self.expr(h).s,
                    None => format!("(long long)({x}).len"),
                };
                Ex::atom(format!("{}_slice({x}, {l}, {h})", ops(&at)))
            }
            ExprKind::Field(a, f) => {
                let x = self.expr(a);
                Ex::atom(format!("{}->{}", x.at(20), f))
            }
            ExprKind::VecLit(items) => {
                let ty = self.ety(e);
                let s = sname(&ty);
                if items.is_empty() {
                    return Ex::atom(format!("{s}_new()"));
                }
                let el = match &ty {
                    Ty::Vec(a) => cty(a),
                    _ => "long long".into(),
                };
                let parts: Vec<String> = items.iter().map(|x| self.owned(x)).collect();
                Ex::atom(format!("{s}_from(({el}[]){{{}}}, {})", parts.join(", "), parts.len()))
            }
            ExprKind::VecRepeat(x, n) => {
                let s = sname(&self.ety(e));
                let (xs, ns) = (self.owned(x), self.expr(n).s);
                Ex::atom(format!("{s}_repeat({xs}, {ns})"))
            }
            ExprKind::TupleLit(items) => {
                let s = sname(&self.ety(e));
                let parts: Vec<String> = items.iter().map(|x| self.owned(x)).collect();
                Ex::atom(format!("{s}_make({})", parts.join(", ")))
            }
            ExprKind::Ctor(ty) => Ex::atom(match ty {
                Ty::Option(_) => format!("{}_none()", sname(ty)),
                _ => format!("{}_new()", sname(ty)),
            }),
            ExprKind::Call(name, args) => self.call(e, name, args),
            ExprKind::Method(r, name, args) => self.method(e, r, name, args),
        }
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Ex {
        let ta = self.ety(a);
        let (l, r) = (self.expr(a), self.expr(b));
        let p = cprec(op);
        if op == BinOp::Add {
            match &ta {
                Ty::Str => {
                    return if matches!(self.ety(b), Ty::Char) {
                        Ex::atom(format!("str_concat({}, str_from_char({}))", l.s, r.s))
                    } else {
                        Ex::atom(format!("str_concat({}, {})", l.s, r.s))
                    };
                }
                Ty::Vec(_) => return Ex::atom(format!("{}_concat({}, {})", sname(&ta), l.s, r.s)),
                _ => {}
            }
        }
        if !matches!(op, BinOp::And | BinOp::Or) && !is_scalar_ty(&ta) {
            let o = ops(&ta);
            return match op {
                BinOp::Eq => Ex::atom(format!("{o}_eq({}, {})", l.s, r.s)),
                BinOp::Ne => Ex::new(format!("!{o}_eq({}, {})", l.s, r.s), P_UNARY),
                _ => Ex::new(format!("{o}_cmp({}, {}) {} 0", l.s, r.s, op.text()), 10),
            };
        }
        let sym = match op {
            BinOp::And => "&&",
            BinOp::Or => "||",
            o => o.text(),
        };
        Ex::new(format!("{} {} {}", l.at(p), sym, r.at(p + 1)), p)
    }

    fn call(&mut self, e: &Expr, name: &str, args: &[Expr]) -> Ex {
        let callee = self.info.slots[e.id as usize];
        if callee != u32::MAX {
            let idx = callee as usize;
            let mut a = Vec::new();
            for (i, x) in args.iter().enumerate() {
                let p = &self.prog.fns[idx].params[i];
                if p.by_ref {
                    a.push(if self.is_ref_param(x) { self.lval(x).trim_start_matches("(*").trim_end_matches(')').to_string() } else { format!("&{}", self.lval(x)) });
                } else if self.info.fns[idx].mutated.get(i).copied().unwrap_or(false) {
                    a.push(self.owned(x));
                } else {
                    a.push(self.expr(x).s);
                }
            }
            return Ex::atom(format!("{}({})", ident(name), a.join(", ")));
        }
        let t0 = args.first().map(|x| self.ety(x));
        let a: Vec<Ex> = args.iter().map(|x| self.expr(x)).collect();
        let atom = |s: String| Ex::atom(s);
        match name {
            "len" => Ex::new(format!("(long long){}.len", a[0].at(20)), P_UNARY),
            "min" | "max" => {
                if args.len() == 2 {
                    let ty = t0.unwrap_or(Ty::Int);
                    atom(format!("{}_{name}({}, {})", ops(&ty), a[0].s, a[1].s))
                } else {
                    let el = match &t0 { Some(Ty::Vec(x)) => (**x).clone(), _ => Ty::Int };
                    let _ = el;
                    atom(format!("{}_v{name}({})", sname(&t0.unwrap_or(Ty::Int)), a[0].s))
                }
            }
            "abs" => atom(format!("llabs({})", a[0].s)),
            "sum" => atom(format!("Vec_int_sum({})", a[0].s)),
            "sorted" => match t0 {
                Some(Ty::Set(_)) => atom(format!("{}_sorted({})", sname(&t0.unwrap()), a[0].s)),
                Some(ty) => atom(format!("{}_sorted({})", sname(&ty), a[0].s)),
                None => atom("0".to_string()),
            },
            "reversed" => atom(format!("{}_reversed({})", sname(&t0.unwrap_or(Ty::Str)), a[0].s)),
            "to_string" => {
                if matches!(t0, Some(Ty::Char)) { atom(format!("str_from_char({})", a[0].s)) } else { atom(format!("str_from_int({})", a[0].s)) }
            }
            "to_int" => atom(format!("str_to_int({})", a[0].s)),
            "ord" => Ex::new(format!("(long long)(unsigned char){}", a[0].at(P_UNARY)), P_UNARY),
            "chr" => Ex::new(format!("(char){}", a[0].at(P_UNARY)), P_UNARY),
            "join" => atom(format!("str_join({}, {})", a[0].s, a[1].s)),
            "from_chars" => atom(format!("Vec_char_clone({})", a[0].s)),
            "some" => {
                let s = sname(&self.ety(e));
                atom(format!("{s}_some({})", self.owned(&args[0])))
            }
            "lnode" => atom(format!("ListNode_new({}, {})", a[0].s, a[1].s)),
            "tnode" => atom(format!("TreeNode_new({}, {}, {})", a[0].s, a[1].s, a[2].s)),
            _ => atom(format!("{}()", ident(name))),
        }
    }

    fn method(&mut self, _e: &Expr, recv: &Expr, name: &str, args: &[Expr]) -> Ex {
        let rt = self.ety(recv);
        let s = sname(&rt);
        let r = self.expr(recv).s;
        let mutating = crate::lang::check::is_mutating(&rt, name);
        let ptr = if mutating { format!("&{}", self.lval(recv)) } else { String::new() };
        let atom = |s: String| Ex::atom(s);
        let mut a: Vec<String> = Vec::new();
        for (i, x) in args.iter().enumerate() {
            let owned = mutating || matches!(rt, Ty::Option(_)) && i == 0;
            a.push(if owned { self.owned(x) } else { self.expr(x).s });
        }
        let a0 = a.first().cloned().unwrap_or_default();
        let a1 = a.get(1).cloned().unwrap_or_default();
        let arg_is_char = args.first().is_some_and(|x| matches!(self.ety(x), Ty::Char));
        match (&rt, name) {
            (Ty::Vec(_) | Ty::Stack(_), "push") | (Ty::Str, "push") | (Ty::Heap(_), "push") | (Ty::Set(_), "add") => {
                let m = if matches!(rt, Ty::Set(_)) { "add" } else { "push" };
                atom(format!("{s}_{m}({ptr}, {a0})"))
            }
            (Ty::Vec(_) | Ty::Stack(_) | Ty::Heap(_), "pop") => atom(format!("{s}_pop({ptr})")),
            (Ty::Queue(_), "push") | (Ty::Deque(_), "push_back") => atom(format!("{s}_push_back({ptr}, {a0})")),
            (Ty::Deque(_), "push_front") => atom(format!("{s}_push_front({ptr}, {a0})")),
            (Ty::Queue(_), "pop") | (Ty::Deque(_), "pop_front") => atom(format!("{s}_pop_front({ptr})")),
            (Ty::Deque(_), "pop_back") => atom(format!("{s}_pop_back({ptr})")),
            (Ty::Queue(_) | Ty::Deque(_), "front") => atom(format!("{s}_front({r})")),
            (Ty::Deque(_), "back") => atom(format!("{s}_back({r})")),
            (Ty::Stack(_), "peek") | (Ty::Heap(_), "peek") => atom(format!("{s}_peek({r})")),
            (Ty::Vec(_), "insert") => atom(format!("{s}_insert({ptr}, {a0}, {a1})")),
            (Ty::Vec(_), "remove") => atom(format!("{s}_remove({ptr}, {a0})")),
            (Ty::Vec(_), "contains") => atom(format!("{s}_contains({r}, {a0})")),
            (Ty::Vec(_), "index_of") => atom(format!("{s}_index_of({r}, {a0})")),
            (Ty::Vec(_) | Ty::Str, "count") => atom(format!("{s}_count({r}, {a0})")),
            (Ty::Vec(_) | Ty::Set(_) | Ty::Map(..) | Ty::Stack(_) | Ty::Queue(_) | Ty::Deque(_) | Ty::Heap(_) | Ty::Str, "clear") => atom(format!("{s}_clear({ptr})")),
            (Ty::Vec(_), "reverse" | "sort") => atom(format!("{s}_{name}({ptr})")),
            (Ty::Vec(_), "swap") => atom(format!("{s}_swap({ptr}, {a0}, {a1})")),
            (Ty::Vec(_), "extend") => atom(format!("{s}_extend({ptr}, {a0})")),
            (_, "is_empty") => Ex::new(format!("({}.len == 0)", Ex::atom(r).at(20)), 20),
            (Ty::Str, "upper" | "lower" | "trim") => atom(format!("str_{name}({r})")),
            (Ty::Str, "find") => atom(if arg_is_char { format!("str_find_char({r}, {a0})") } else { format!("str_find({r}, {a0})") }),
            (Ty::Str, "contains") => atom(if arg_is_char { format!("(str_find_char({r}, {a0}) >= 0)") } else { format!("str_contains({r}, {a0})") }),
            (Ty::Str, "starts_with" | "ends_with") => atom(format!("str_{name}({r}, {a0})")),
            (Ty::Str, "split") => atom(if arg_is_char { format!("str_split({r}, str_from_char({a0}))") } else { format!("str_split({r}, {a0})") }),
            (Ty::Str, "chars") => atom(format!("Vec_char_clone({r})")),
            (Ty::Str, "replace") => atom(format!("str_replace({r}, {a0}, {a1})")),
            (Ty::Str, "repeat") => atom(format!("str_repeat({r}, {a0})")),
            (Ty::Char, "is_digit" | "is_alpha" | "is_alnum" | "is_upper" | "is_lower" | "is_space") => {
                let f = match name {
                    "is_digit" => "isdigit",
                    "is_alpha" => "isalpha",
                    "is_alnum" => "isalnum",
                    "is_upper" => "isupper",
                    "is_lower" => "islower",
                    _ => "isspace",
                };
                atom(format!("({f}((unsigned char){}) != 0)", Ex::new(r, 20).at(P_UNARY)))
            }
            (Ty::Char, "upper" | "lower") => atom(format!("(char)to{name}((unsigned char){})", Ex::new(r, 20).at(P_UNARY))),
            (Ty::Set(_), "remove") | (Ty::Map(..), "remove") => atom(format!("{s}_remove({ptr}, {a0})")),
            (Ty::Set(_) | Ty::Map(..), "contains") => atom(format!("{s}_contains({r}, {a0})")),
            (Ty::Set(_), "union" | "intersection" | "difference") => atom(format!("{s}_{name}({r}, {a0})")),
            (Ty::Map(..), "get") => atom(if args.len() == 2 { format!("{s}_get_or({r}, {a0}, {a1})") } else { format!("{s}_get({r}, {a0})") }),
            (Ty::Map(..), "keys" | "values" | "items") => atom(format!("{s}_{name}({r})")),
            (Ty::Option(_), "is_some") => atom(format!("{}.has", Ex::new(r, 20).at(20))),
            (Ty::Option(_), "is_none") => Ex::new(format!("!{}.has", Ex::new(r, 20).at(20)), P_UNARY),
            (Ty::Option(_), "unwrap") => atom(format!("{}.v", Ex::new(r, 20).at(20))),
            (Ty::Option(_), "unwrap_or") => atom(format!("{s}_unwrap_or({r}, {a0})")),
            _ => atom(format!("{s}_{name}({r})")),
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

    fn destructure(&mut self, p: &Pat, ty: &Ty, src: &str) {
        match (p, ty) {
            (Pat::Var(v), t) => self.line(&format!("{} {} = {src};", cty(t), ident(v))),
            (Pat::Wild, _) => {}
            (Pat::Tuple(ps), Ty::Tuple(ts)) => {
                for (i, (q, t)) in ps.iter().zip(ts.iter()).enumerate() {
                    self.destructure(q, t, &format!("{src}.f{i}"));
                }
            }
            _ => {}
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Let { pat, init, .. } => {
                let v = self.owned(init);
                let t = self.info.ty(s.id).clone();
                match pat {
                    Pat::Var(n) => self.line(&format!("{} {} = {v};", cty(&t), ident(n))),
                    Pat::Wild => self.line(&format!("(void)({v});")),
                    Pat::Tuple(_) => {
                        self.tmp += 1;
                        let tmpn = format!("t{}_", self.tmp);
                        self.line(&format!("{} {tmpn} = {v};", cty(&t)));
                        self.destructure(pat, &t, &tmpn);
                    }
                }
            }
            StmtKind::Assign { target, op, value } => self.assign(target, *op, value),
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
                let k = match step.as_ref().map(|x| &x.kind) {
                    Some(ExprKind::Int(k)) => *k,
                    _ => 1,
                };
                let (bound, init_extra) = if self.stable(end) {
                    (self.expr(end).at(11), String::new())
                } else {
                    self.tmp += 1;
                    let n = format!("end{}_", self.tmp);
                    (n.clone(), format!(", {n} = {}", self.expr(end).s))
                };
                let (cmp, incr) = match k {
                    1 => ("<", format!("++{v}")),
                    k if k > 0 => ("<", format!("{v} += {k}")),
                    -1 => (">", format!("--{v}")),
                    k => (">", format!("{v} -= {}", -k)),
                };
                self.line(&format!("for (long long {v} = {a}{init_extra}; {v} {cmp} {bound}; {incr}) {{"));
                self.block(body);
                self.line("}");
            }
            StmtKind::ForEach { pat, iter, body } => self.for_each(s, pat, iter, body),
            StmtKind::Break => self.line("break;"),
            StmtKind::Continue => self.line("continue;"),
            StmtKind::Return(None) => self.line("return;"),
            StmtKind::Return(Some(e)) => {
                let t = self.ety(e);
                let v = if matches!(e.kind, ExprKind::Var(_)) && !self.is_param(e) { self.expr(e).s } else { self.owned(e) };
                let _ = t;
                self.line(&format!("return {v};"));
            }
            StmtKind::Expr(e) => {
                let v = self.expr(e).s;
                self.line(&format!("{v};"));
            }
        }
    }

    fn assign(&mut self, target: &Expr, op: Option<BinOp>, value: &Expr) {
        let tt = self.ety(target);
        if let ExprKind::Field(a, f) = &target.kind {
            let recv = self.expr(a).at(20);
            let v = match op {
                None => self.owned(value),
                Some(_) => self.expr(value).s,
            };
            match op {
                None => self.line(&format!("{recv}->{f} = {v};")),
                Some(o) => self.line(&format!("{recv}->{f} {}= {v};", o.text())),
            }
            return;
        }
        if let (ExprKind::Index(a, i), None) = (&target.kind, op)
            && let at @ Ty::Map(..) = self.ety(a)
        {
            let base = self.lval(a);
            let k = self.owned(i);
            let v = self.owned(value);
            self.line(&format!("{}_set(&{base}, {k}, {v});", sname(&at)));
            return;
        }
        let lv = self.lval(target);
        match op {
            None => {
                let v = self.owned(value);
                self.line(&format!("{lv} = {v};"));
            }
            Some(BinOp::Add) if matches!(tt, Ty::Str) => {
                if matches!(self.ety(value), Ty::Char) {
                    let v = self.expr(value).s;
                    self.line(&format!("Vec_char_push(&{lv}, {v});"));
                } else {
                    let v = self.expr(value).s;
                    self.line(&format!("str_push_str(&{lv}, {v});"));
                }
            }
            Some(BinOp::Add) if matches!(tt, Ty::Vec(_)) => {
                let v = self.expr(value).s;
                self.line(&format!("{}_extend(&{lv}, {v});", sname(&tt)));
            }
            Some(o) => {
                let v = self.expr(value).s;
                self.line(&format!("{lv} {}= {v};", o.text()));
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

    /// A name for a collection that is indexed repeatedly: places are used directly, temporaries are bound first.
    fn hoist(&mut self, e: &Expr) -> String {
        if is_place(e) {
            return self.lval(e);
        }
        self.tmp += 1;
        let name = format!("src{}_", self.tmp);
        let v = self.expr(e).s;
        let t = self.ety(e);
        self.line(&format!("{} {name} = {v};", cty(&t)));
        name
    }

    fn for_each(&mut self, _s: &Stmt, pat: &Pat, iter: &Expr, body: &Block) {
        self.tmp += 1;
        let i = format!("i{}_", self.tmp);
        if let Some((kind, args)) = iter_special(iter) {
            let Pat::Tuple(ps) = pat else { return };
            let (t0, t1) = match self.ety(iter) {
                Ty::Vec(t) => match *t {
                    Ty::Tuple(ts) => (ts[0].clone(), ts[1].clone()),
                    _ => (Ty::Int, Ty::Int),
                },
                _ => (Ty::Int, Ty::Int),
            };
            let a0 = self.hoist(&args[0]);
            if kind == "enumerate" {
                self.line(&format!("for (size_t {i} = 0; {i} < {a0}.len; {i}++) {{"));
                self.ind += 1;
                self.destructure(&ps[0], &Ty::Int, &format!("(long long){i}"));
                self.destructure(&ps[1], &t1, &format!("{a0}.data[{i}]"));
                self.ind -= 1;
            } else {
                let a1 = self.hoist(&args[1]);
                self.line(&format!("for (size_t {i} = 0; {i} < {a0}.len && {i} < {a1}.len; {i}++) {{"));
                self.ind += 1;
                self.destructure(&ps[0], &t0, &format!("{a0}.data[{i}]"));
                self.destructure(&ps[1], &t1, &format!("{a1}.data[{i}]"));
                self.ind -= 1;
            }
            self.block(body);
            self.line("}");
            return;
        }
        let it_ty = self.ety(iter);
        let elem = match &it_ty {
            Ty::Vec(t) => (**t).clone(),
            _ => Ty::Char,
        };
        let base = if is_place(iter) {
            self.lval(iter)
        } else {
            let b = format!("it{}_", self.tmp);
            let v = self.expr(iter).s;
            self.line(&format!("{} {b} = {v};", cty(&it_ty)));
            b
        };
        self.line(&format!("for (size_t {i} = 0; {i} < {base}.len; {i}++) {{"));
        self.ind += 1;
        match pat {
            Pat::Var(v) => self.line(&format!("{} {} = {base}.data[{i}];", cty(&elem), ident(v))),
            Pat::Wild => {}
            Pat::Tuple(_) => self.destructure(pat, &elem, &format!("{base}.data[{i}]")),
        }
        self.ind -= 1;
        self.block(body);
        self.line("}");
    }

    fn function(&self, idx: usize) -> String {
        let f = &self.prog.fns[idx];
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{}{}{}", cty(&p.ty), if p.by_ref { " *" } else { " " }, ident(&p.name)))
            .collect();
        let ret = cty(&f.ret);
        format!("{ret} {}({})", ident(&f.name), if params.is_empty() { "void".to_string() } else { params.join(", ") })
    }
}

fn is_scalar_ty(t: &Ty) -> bool {
    matches!(t, Ty::Int | Ty::Bool | Ty::Char | Ty::LNode | Ty::TNode)
}

/// Types that must be instantiated for a program (and a harness).
pub fn needed_types(used: &[Ty]) -> Vec<Ty> {
    let mut v: Vec<Ty> = Vec::new();
    let add = |t: &Ty, v: &mut Vec<Ty>| {
        if !v.iter().any(|x| sname(x) == sname(t) && !is_scalar_ty(t)) && !is_scalar_ty(t) && !matches!(t, Ty::Unit | Ty::Str) {
            v.push(t.clone());
        }
    };
    // strings are vec<char>
    add(&Ty::Vec(Box::new(Ty::Char)), &mut v);
    for t in used {
        add(t, &mut v);
    }
    v
}

/// Build the prelude text for a set of used types.
pub fn build_prelude(used: &[Ty]) -> String {
    let need = needed_types(used);
    let mut has: BTreeSet<String> = need.iter().map(sname).collect();
    // Vec<str> is needed for split/join; add when strings and vec<str> are both around
    let mut s = String::from(t::BASE);
    let (list, tree) = (used.contains(&Ty::LNode), used.contains(&Ty::TNode));
    if list {
        s.push_str(t::LIST_NODE);
    }
    if tree {
        s.push_str(t::TREE_NODE);
    }
    s.push_str("\n/* ---- typed containers, instantiated for the types this program uses ---- */\n");
    has.insert("Vec_char".into());
    let mut emitted: BTreeSet<String> = BTreeSet::new();
    let names: BTreeSet<String> = need.iter().map(sname).collect();
    fn deps(t: &Ty) -> Vec<Ty> {
        match t {
            Ty::Vec(a) | Ty::Stack(a) | Ty::Queue(a) | Ty::Deque(a) | Ty::Heap(a) | Ty::Option(a) => vec![(**a).clone()],
            Ty::Set(a) => vec![(**a).clone(), Ty::Vec(a.clone())],
            Ty::Map(k, v) => {
                let tup = Ty::Tuple(vec![(**k).clone(), (**v).clone()]);
                vec![(**k).clone(), (**v).clone(), Ty::Vec(k.clone()), Ty::Vec(v.clone()), tup.clone(), Ty::Vec(Box::new(tup)), Ty::Option(v.clone())]
            }
            Ty::Tuple(ts) => ts.clone(),
            _ => vec![],
        }
    }
    fn emit(ty: &Ty, need: &[Ty], names: &BTreeSet<String>, has: &BTreeSet<String>, emitted: &mut BTreeSet<String>, s: &mut String, visiting: &mut Vec<String>) {
        let n = sname(ty);
        if emitted.contains(&n) || visiting.contains(&n) || !names.contains(&n) {
            return;
        }
        visiting.push(n.clone());
        for d in deps(ty) {
            if let Some(real) = need.iter().find(|x| sname(x) == sname(&d)) {
                emit(real, need, names, has, emitted, s, visiting);
            }
        }
        visiting.pop();
        if !emitted.insert(n) {
            return;
        }
        if let Some(code) = instance(ty, has) {
            s.push_str(&code);
        }
        if *ty == Ty::Vec(Box::new(Ty::Char)) {
            s.push_str(t::STR_HELPERS);
        }
    }
    for ty in &need {
        emit(ty, &need, &names, &has, &mut emitted, &mut s, &mut Vec::new());
    }
    if has.contains("Vec_str") {
        s.push_str(t::STR_SPLIT_JOIN);
    }
    s
}

/// Translate to C.
pub fn generate(prog: &Program, info: &TypeInfo) -> Generated {
    let mut g = Gen { prog, info, f: 0, out: String::new(), ind: 0, tmp: 0, has: BTreeSet::new() };
    let _ = &g.has;
    let used = used_types(prog, info);
    let mut protos = Vec::new();
    let mut bodies = String::new();
    for i in 0..prog.fns.len() {
        g.f = i;
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
    Generated { lang: Lang::C, prelude: build_prelude(&used), code }
}
