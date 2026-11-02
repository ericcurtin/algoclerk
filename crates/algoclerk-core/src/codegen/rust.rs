// SPDX-License-Identifier: Apache-2.0
//! Rust code generator: `Vec`, `String`, `HashMap`, `HashSet`, `VecDeque`, `BinaryHeap`, iterators.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Owned,
    Ref,
    RefMut,
}

struct Gen<'a> {
    prog: &'a Program,
    info: &'a TypeInfo,
    entry: String,
    f: usize,
    out: String,
    ind: usize,
}

fn rprec(op: BinOp) -> u8 {
    match op {
        BinOp::Mul | BinOp::Div | BinOp::Rem => 13,
        BinOp::Add | BinOp::Sub => 12,
        BinOp::Shl | BinOp::Shr => 11,
        BinOp::BitAnd => 10,
        BinOp::BitXor => 9,
        BinOp::BitOr => 8,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 7,
        BinOp::And => 5,
        BinOp::Or => 4,
    }
}

const P_UNARY: u8 = 15;
const P_CAST: u8 = 14;

/// Node arena prelude for the test harness (without a second `use` when one exists).
pub fn nodes_prelude_pub(list: bool, tree: bool, has_use: bool) -> String {
    let s = nodes_prelude(list, tree);
    if has_use { s.replacen("use std::cell::RefCell;\n", "", 1) } else { s }
}

fn nodes_prelude(list: bool, tree: bool) -> String {
    let mut s = String::from("use std::cell::RefCell;\n");
    if list {
        s.push_str(
            r#"
// Linked-list nodes live in an arena; a `ListRef` is a handle that may be null.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ListRef(Option<usize>);

thread_local! {
    static LISTS: RefCell<Vec<(i64, ListRef)>> = RefCell::new(Vec::new());
}

impl ListRef {
    const NULL: ListRef = ListRef(None);
    fn new(val: i64, next: ListRef) -> ListRef {
        LISTS.with(|a| {
            let mut a = a.borrow_mut();
            a.push((val, next));
            ListRef(Some(a.len() - 1))
        })
    }
    fn val(self) -> i64 {
        LISTS.with(|a| a.borrow()[self.0.unwrap()].0)
    }
    fn next(self) -> ListRef {
        LISTS.with(|a| a.borrow()[self.0.unwrap()].1)
    }
    fn set_val(self, v: i64) {
        LISTS.with(|a| a.borrow_mut()[self.0.unwrap()].0 = v);
    }
    fn set_next(self, n: ListRef) {
        LISTS.with(|a| a.borrow_mut()[self.0.unwrap()].1 = n);
    }
}
"#,
        );
    }
    if tree {
        s.push_str(
            r#"
// Tree nodes live in an arena; a `TreeRef` is a handle that may be null.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TreeRef(Option<usize>);

thread_local! {
    static TREES: RefCell<Vec<(i64, TreeRef, TreeRef)>> = RefCell::new(Vec::new());
}

impl TreeRef {
    const NULL: TreeRef = TreeRef(None);
    fn new(val: i64, left: TreeRef, right: TreeRef) -> TreeRef {
        TREES.with(|a| {
            let mut a = a.borrow_mut();
            a.push((val, left, right));
            TreeRef(Some(a.len() - 1))
        })
    }
    fn val(self) -> i64 {
        TREES.with(|a| a.borrow()[self.0.unwrap()].0)
    }
    fn left(self) -> TreeRef {
        TREES.with(|a| a.borrow()[self.0.unwrap()].1)
    }
    fn right(self) -> TreeRef {
        TREES.with(|a| a.borrow()[self.0.unwrap()].2)
    }
    fn set_val(self, v: i64) {
        TREES.with(|a| a.borrow_mut()[self.0.unwrap()].0 = v);
    }
    fn set_left(self, n: TreeRef) {
        TREES.with(|a| a.borrow_mut()[self.0.unwrap()].1 = n);
    }
    fn set_right(self, n: TreeRef) {
        TREES.with(|a| a.borrow_mut()[self.0.unwrap()].2 = n);
    }
}
"#,
        );
    }
    s
}

/// The Rust spelling of a type.
pub fn ty_name(t: &Ty) -> String {
    match t {
        Ty::Int => "i64".into(),
        Ty::Bool => "bool".into(),
        Ty::Char => "char".into(),
        Ty::Str => "String".into(),
        Ty::Unit => "()".into(),
        Ty::Vec(a) | Ty::Stack(a) => format!("Vec<{}>", ty_name(a)),
        Ty::Set(a) => format!("HashSet<{}>", ty_name(a)),
        Ty::Map(k, v) => format!("HashMap<{}, {}>", ty_name(k), ty_name(v)),
        Ty::Tuple(ts) => format!("({})", ts.iter().map(ty_name).collect::<Vec<_>>().join(", ")),
        Ty::Option(a) => format!("Option<{}>", ty_name(a)),
        Ty::Queue(a) | Ty::Deque(a) => format!("VecDeque<{}>", ty_name(a)),
        Ty::Heap(a) => format!("BinaryHeap<{}>", ty_name(a)),
        Ty::LNode => "ListRef".into(),
        Ty::TNode => "TreeRef".into(),
        Ty::Null | Ty::Var(_) => "_".into(),
    }
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

    /// How a parameter is passed: by value, by shared reference or by mutable reference.
    fn param_kind(&self, fidx: usize, i: usize) -> Kind {
        let p = &self.prog.fns[fidx].params[i];
        if p.by_ref {
            Kind::RefMut
        } else if is_copy(&p.ty) || self.info.fns[fidx].mutated.get(i).copied().unwrap_or(false) || self.prog.fns[fidx].name == self.entry {
            Kind::Owned
        } else {
            Kind::Ref
        }
    }

    fn var_kind(&self, e: &Expr) -> Kind {
        if let ExprKind::Var(_) = e.kind {
            let slot = self.info.slots[e.id as usize] as usize;
            if slot < self.prog.fns[self.f].params.len() {
                return self.param_kind(self.f, slot);
            }
        }
        Kind::Owned
    }

    // ----- expressions -----
    fn raw_or_owned_str(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => format!("\"{}\"", escape_str(s.as_bytes())),
            _ => self.borrowed(e),
        }
    }

    /// `&expr` unless it is already a reference or a literal.
    fn borrowed(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => format!("\"{}\"", escape_str(s.as_bytes())),
            ExprKind::Var(_) if self.var_kind(e) != Kind::Owned => self.expr(e).s,
            _ => format!("&{}", self.expr(e).at(P_UNARY)),
        }
    }

    /// Evaluate in an owning position: places of non-Copy type are cloned.
    fn owned(&mut self, e: &Expr) -> String {
        let t = self.ety(e);
        let x = self.expr(e);
        if is_place(e) && !is_copy(&t) {
            // moving a plain owned local into its last use is fine, but cloning is always correct
            format!("{}.clone()", x.at(20))
        } else if let ExprKind::Str(_) = e.kind {
            format!("{}.to_string()", x.s)
        } else {
            x.s
        }
    }

    fn idx(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) if *n >= 0 => n.to_string(),
            _ => {
                let x = self.expr(e);
                format!("{} as usize", if x.p >= 20 { x.s } else { format!("({})", x.s) })
            }
        }
    }

    fn key_ref(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => format!("\"{}\"", escape_str(s.as_bytes())),
            ExprKind::Var(_) if self.var_kind(e) != Kind::Owned => self.expr(e).s,
            _ => format!("&{}", self.expr(e).at(P_UNARY)),
        }
    }

    fn operand(&mut self, e: &Expr) -> Ex {
        // comparison operands: reference parameters are dereferenced so both sides have the same type
        if let ExprKind::Var(_) = e.kind
            && self.var_kind(e) != Kind::Owned
        {
            return Ex::new(format!("*{}", self.expr(e).s), P_UNARY);
        }
        self.expr(e)
    }

    fn expr(&mut self, e: &Expr) -> Ex {
        match &e.kind {
            ExprKind::Int(n) => {
                if *n < 0 { Ex::new(n.to_string(), P_UNARY) } else { Ex::atom(n.to_string()) }
            }
            ExprKind::Bool(b) => Ex::atom(b.to_string()),
            ExprKind::Char(c) => Ex::atom(format!("'{}'", escape_char(*c))),
            ExprKind::Str(s) => Ex::atom(format!("\"{}\"", escape_str(s.as_bytes()))),
            ExprKind::Null => Ex::atom(format!("{}::NULL", ty_name(&self.ety(e)))),
            ExprKind::NoneLit => Ex::atom("None"),
            ExprKind::Var(n) => Ex::atom(ident(n)),
            ExprKind::Unary(op, a) => {
                let x = self.expr(a);
                let s = match op {
                    UnOp::Neg => "-",
                    UnOp::Not | UnOp::BitNot => "!",
                };
                let inner = if x.p >= 20 && !x.s.starts_with('-') { x.s } else { format!("({})", x.s) };
                Ex::new(format!("{s}{inner}"), P_UNARY)
            }
            ExprKind::Binary(op, a, b) => self.binary(*op, a, b),
            ExprKind::Index(a, i) => {
                let t = self.ety(a);
                let x = self.expr(a);
                match t {
                    Ty::Map(..) => {
                        let k = self.key_ref(i);
                        Ex::atom(format!("{}[{}]", x.at(20), k))
                    }
                    Ty::Str => {
                        let ix = self.idx(i);
                        Ex::new(format!("{}.as_bytes()[{ix}] as char", x.at(20)), P_CAST)
                    }
                    _ => {
                        let ix = self.idx(i);
                        Ex::atom(format!("{}[{ix}]", x.at(20)))
                    }
                }
            }
            ExprKind::Slice(a, lo, hi) => {
                let t = self.ety(a);
                let x = self.expr(a);
                let l = lo.as_ref().map(|l| self.idx(l)).unwrap_or_default();
                let h = hi.as_ref().map(|h| self.idx(h)).unwrap_or_default();
                let conv = if matches!(t, Ty::Str) { "to_string()" } else { "to_vec()" };
                Ex::atom(format!("{}[{l}..{h}].{conv}", x.at(20)))
            }
            ExprKind::Field(a, f) => {
                let x = self.expr(a);
                Ex::atom(format!("{}.{}()", x.at(20), f))
            }
            ExprKind::TupleIdx(a, n) => {
                let x = self.expr(a);
                Ex::atom(format!("{}.{n}", x.at(20)))
            }
            ExprKind::VecLit(items) => {
                if items.is_empty() {
                    return Ex::atom("Vec::new()");
                }
                let parts: Vec<String> = items.iter().map(|x| self.owned(x)).collect();
                Ex::atom(format!("vec![{}]", parts.join(", ")))
            }
            ExprKind::VecRepeat(x, n) => {
                let xs = self.owned(x);
                let ns = self.idx(n);
                Ex::atom(format!("vec![{xs}; {ns}]"))
            }
            ExprKind::TupleLit(items) => {
                let parts: Vec<String> = items.iter().map(|x| self.owned(x)).collect();
                Ex::atom(format!("({})", parts.join(", ")))
            }
            ExprKind::Ctor(t) => Ex::atom(match t {
                Ty::Vec(_) | Ty::Stack(_) => "Vec::new()",
                Ty::Set(_) => "HashSet::new()",
                Ty::Map(..) => "HashMap::new()",
                Ty::Queue(_) | Ty::Deque(_) => "VecDeque::new()",
                Ty::Heap(_) => "BinaryHeap::new()",
                _ => "None",
            }),
            ExprKind::Call(name, args) => self.call(e, name, args),
            ExprKind::Method(r, name, args) => self.method(e, r, name, args),
        }
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Ex {
        let p = rprec(op);
        let cmp = p == 7;
        if op == BinOp::Add {
            match self.ety(a) {
                Ty::Str => {
                    return if matches!(self.ety(b), Ty::Char) {
                        let (l, r) = (self.expr(a).s, self.expr(b).s);
                        Ex::atom(format!("format!(\"{{}}{{}}\", {l}, {r})"))
                    } else {
                        let l = self.owned(a);
                        let r = self.raw_or_owned_str(b);
                        Ex::new(format!("{l} + {r}"), p)
                    };
                }
                Ty::Vec(_) => {
                    let (l, r) = (self.owned(a), self.owned(b));
                    return Ex::atom(format!("[{l}, {r}].concat()"));
                }
                _ => {}
            }
        }
        let (l, r) = if cmp { (self.operand(a), self.operand(b)) } else { (self.expr(a), self.expr(b)) };
        // comparing a String with a literal: keep the literal borrowed
        let ls = if (op == BinOp::Lt || op == BinOp::Shl) && l.p == P_CAST { format!("({})", l.s) } else if cmp && l.p <= p { format!("({})", l.s) } else { l.at(p) };
        let rs = if cmp && r.p <= p { format!("({})", r.s) } else { r.at(p + 1) };
        let sym = match op {
            BinOp::And => "&&",
            BinOp::Or => "||",
            o => o.text(),
        };
        Ex::new(format!("{ls} {sym} {rs}"), p)
    }

    fn call(&mut self, e: &Expr, name: &str, args: &[Expr]) -> Ex {
        let callee = self.info.slots[e.id as usize];
        if callee != u32::MAX {
            let idx = callee as usize;
            let mut a = Vec::new();
            for (i, x) in args.iter().enumerate() {
                a.push(match self.param_kind(idx, i) {
                    Kind::RefMut => {
                        if self.var_kind(x) == Kind::RefMut { self.expr(x).s } else { format!("&mut {}", self.expr(x).s) }
                    }
                    Kind::Ref => self.borrowed(x),
                    Kind::Owned => self.owned(x),
                });
            }
            return Ex::atom(format!("{}({})", ident(name), a.join(", ")));
        }
        let t0 = args.first().map(|x| self.ety(x));
        match name {
            "len" => {
                let x = self.expr(&args[0]);
                Ex::new(format!("{}.len() as i64", x.at(20)), P_CAST)
            }
            "min" | "max" => {
                if args.len() == 2 {
                    let (a, b) = (self.owned(&args[0]), self.owned(&args[1]));
                    Ex::atom(format!("std::cmp::{name}({a}, {b})"))
                } else {
                    let x = self.expr(&args[0]);
                    let deref = if is_copy(match &t0 { Some(Ty::Vec(t)) => t, _ => &Ty::Int }) { "*" } else { "" };
                    let tail = if deref.is_empty() { ".clone()" } else { "" };
                    Ex::new(format!("{deref}{}.iter().{name}().unwrap(){tail}", x.at(20)), if deref.is_empty() { 20 } else { P_UNARY })
                }
            }
            "abs" => {
                let x = self.expr(&args[0]);
                Ex::atom(format!("i64::abs({})", x.s))
            }
            "sum" => {
                let x = self.expr(&args[0]);
                Ex::atom(format!("{}.iter().sum::<i64>()", x.at(20)))
            }
            "sorted" => {
                let x = self.owned(&args[0]);
                match t0 {
                    Some(Ty::Str) => Ex::atom(format!("{{ let mut t: Vec<char> = {x}.chars().collect(); t.sort(); t.into_iter().collect::<String>() }}")),
                    Some(Ty::Set(_)) => Ex::atom(format!("{{ let mut t: Vec<_> = {x}.into_iter().collect(); t.sort(); t }}")),
                    _ => Ex::atom(format!("{{ let mut t = {x}; t.sort(); t }}")),
                }
            }
            "reversed" => {
                let x = self.expr(&args[0]);
                if matches!(t0, Some(Ty::Str)) {
                    Ex::atom(format!("{}.chars().rev().collect::<String>()", x.at(20)))
                } else {
                    Ex::atom(format!("{}.iter().rev().cloned().collect::<Vec<_>>()", x.at(20)))
                }
            }
            "to_string" => {
                let x = self.expr(&args[0]);
                Ex::atom(format!("{}.to_string()", x.at(20)))
            }
            "to_int" => {
                let x = self.expr(&args[0]);
                Ex::atom(format!("{}.parse::<i64>().unwrap()", x.at(20)))
            }
            "ord" => {
                let x = self.expr(&args[0]);
                Ex::new(format!("{} as i64", x.at(P_CAST + 1)), P_CAST)
            }
            "chr" => {
                let x = self.expr(&args[0]);
                Ex::new(format!("({} as u8) as char", x.at(P_CAST + 1)), P_CAST)
            }
            "join" => {
                let x = self.expr(&args[0]);
                let s = self.raw_or_owned_str(&args[1]);
                Ex::atom(format!("{}.join({s})", x.at(20)))
            }
            "from_chars" => {
                let x = self.expr(&args[0]);
                Ex::atom(format!("{}.iter().collect::<String>()", x.at(20)))
            }
            "some" => Ex::atom(format!("Some({})", self.owned(&args[0]))),
            "lnode" => Ex::atom(format!("ListRef::new({}, {})", self.owned(&args[0]), self.owned(&args[1]))),
            "tnode" => Ex::atom(format!("TreeRef::new({}, {}, {})", self.owned(&args[0]), self.owned(&args[1]), self.owned(&args[2]))),
            _ => Ex::atom(format!("{}()", ident(name))),
        }
    }

    fn method(&mut self, _e: &Expr, recv: &Expr, name: &str, args: &[Expr]) -> Ex {
        let rt = self.ety(recv);
        let r = if crate::lang::check::is_mutating(&rt, name) { Ex::atom(self.place(recv, false)) } else { self.expr(recv) };
        let rs = r.at(20);
        let atom = Ex::atom;
        let copy_elem = |t: &Ty| match t {
            Ty::Vec(x) | Ty::Stack(x) | Ty::Queue(x) | Ty::Deque(x) | Ty::Heap(x) => is_copy(x),
            _ => false,
        };
        let one = |g: &mut Self| g.owned(&args[0]);
        match (&rt, name) {
            (Ty::Vec(_) | Ty::Stack(_) | Ty::Heap(_), "push") | (Ty::Set(_), "add") => {
                let m = if matches!(rt, Ty::Set(_)) { "insert" } else { "push" };
                let a = one(self);
                atom(format!("{rs}.{m}({a})"))
            }
            (Ty::Vec(_) | Ty::Stack(_) | Ty::Heap(_), "pop") => atom(format!("{rs}.pop().unwrap()")),
            (Ty::Queue(_), "push") | (Ty::Deque(_), "push_back") => atom(format!("{rs}.push_back({})", one(self))),
            (Ty::Deque(_), "push_front") => atom(format!("{rs}.push_front({})", one(self))),
            (Ty::Queue(_), "pop") | (Ty::Deque(_), "pop_front") => atom(format!("{rs}.pop_front().unwrap()")),
            (Ty::Deque(_), "pop_back") => atom(format!("{rs}.pop_back().unwrap()")),
            (Ty::Stack(_), "peek") => {
                let c = if copy_elem(&rt) { ".copied()" } else { ".cloned()" };
                atom(format!("{rs}.last(){c}.unwrap()"))
            }
            (Ty::Queue(_) | Ty::Deque(_), "front") | (Ty::Heap(_), "peek") | (Ty::Deque(_), "back") => {
                let m = match name {
                    "peek" => "peek",
                    "front" => "front",
                    _ => "back",
                };
                let c = if copy_elem(&rt) { ".copied()" } else { ".cloned()" };
                atom(format!("{rs}.{m}(){c}.unwrap()"))
            }
            (Ty::Vec(_), "insert") => {
                let i = self.idx(&args[0]);
                let x = self.owned(&args[1]);
                atom(format!("{rs}.insert({i}, {x})"))
            }
            (Ty::Vec(_), "remove") => {
                let i = self.idx(&args[0]);
                atom(format!("{rs}.remove({i})"))
            }
            (Ty::Vec(_), "contains") => {
                let k = self.key_ref(&args[0]);
                atom(format!("{rs}.contains({k})"))
            }
            (Ty::Vec(_), "index_of") => {
                let x = self.expr(&args[0]).s;
                Ex::new(format!("{rs}.iter().position(|y| *y == {x}).map_or(-1, |p| p as i64)"), 20)
            }
            (Ty::Vec(_), "count") => {
                let x = self.expr(&args[0]).s;
                Ex::new(format!("{rs}.iter().filter(|&y| *y == {x}).count() as i64"), P_CAST)
            }
            (Ty::Vec(_), "reverse" | "sort" | "clear") | (Ty::Set(_) | Ty::Map(..) | Ty::Stack(_) | Ty::Queue(_) | Ty::Deque(_) | Ty::Heap(_), "clear") => {
                atom(format!("{rs}.{name}()"))
            }
            (Ty::Vec(_), "swap") => {
                let (i, j) = (self.idx(&args[0]), self.idx(&args[1]));
                atom(format!("{rs}.swap({i}, {j})"))
            }
            (Ty::Vec(_), "extend") => {
                let w = self.borrowed(&args[0]);
                atom(format!("{rs}.extend_from_slice({w})"))
            }
            (_, "is_empty") => atom(format!("{rs}.is_empty()")),
            (Ty::Str, "upper") => atom(format!("{rs}.to_ascii_uppercase()")),
            (Ty::Str, "lower") => atom(format!("{rs}.to_ascii_lowercase()")),
            (Ty::Str, "trim") => atom(format!("{rs}.trim().to_string()")),
            (Ty::Str, "find") => {
                let p = self.pattern(&args[0]);
                atom(format!("{rs}.find({p}).map_or(-1, |p| p as i64)"))
            }
            (Ty::Str, "contains" | "starts_with" | "ends_with") => {
                let p = self.pattern(&args[0]);
                atom(format!("{rs}.{name}({p})"))
            }
            (Ty::Str, "split") => {
                let p = self.pattern(&args[0]);
                atom(format!("{rs}.split({p}).map(|p| p.to_string()).collect::<Vec<String>>()"))
            }
            (Ty::Str, "chars") => atom(format!("{rs}.chars().collect::<Vec<char>>()")),
            (Ty::Str, "push") => atom(format!("{rs}.push({})", self.expr(&args[0]).s)),
            (Ty::Str, "count") => {
                let x = self.expr(&args[0]).s;
                Ex::new(format!("{rs}.chars().filter(|&y| y == {x}).count() as i64"), P_CAST)
            }
            (Ty::Str, "replace") => {
                let (a, b) = (self.raw_or_owned_str(&args[0]), self.raw_or_owned_str(&args[1]));
                atom(format!("{rs}.replace({a}, {b})"))
            }
            (Ty::Str, "repeat") => {
                let k = self.idx(&args[0]);
                atom(format!("{rs}.repeat({k})"))
            }
            (Ty::Char, "is_digit") => atom(format!("{rs}.is_ascii_digit()")),
            (Ty::Char, "is_alpha") => atom(format!("{rs}.is_ascii_alphabetic()")),
            (Ty::Char, "is_alnum") => atom(format!("{rs}.is_ascii_alphanumeric()")),
            (Ty::Char, "is_upper") => atom(format!("{rs}.is_ascii_uppercase()")),
            (Ty::Char, "is_lower") => atom(format!("{rs}.is_ascii_lowercase()")),
            (Ty::Char, "is_space") => atom(format!("{rs}.is_ascii_whitespace()")),
            (Ty::Char, "upper") => atom(format!("{rs}.to_ascii_uppercase()")),
            (Ty::Char, "lower") => atom(format!("{rs}.to_ascii_lowercase()")),
            (Ty::Set(_) | Ty::Map(..), "remove") => {
                let k = self.key_ref(&args[0]);
                atom(format!("{rs}.remove({k})"))
            }
            (Ty::Set(_), "contains") => {
                let k = self.key_ref(&args[0]);
                atom(format!("{rs}.contains({k})"))
            }
            (Ty::Map(..), "contains") => {
                let k = self.key_ref(&args[0]);
                atom(format!("{rs}.contains_key({k})"))
            }
            (Ty::Set(_), "union" | "intersection" | "difference") => {
                let o = self.borrowed(&args[0]);
                atom(format!("{rs}.{name}({o}).cloned().collect::<HashSet<_>>()"))
            }
            (Ty::Map(..), "get") => {
                let k = self.key_ref(&args[0]);
                if args.len() == 2 {
                    let d = self.owned(&args[1]);
                    atom(format!("{rs}.get({k}).cloned().unwrap_or({d})"))
                } else {
                    atom(format!("{rs}.get({k}).cloned()"))
                }
            }
            (Ty::Map(..), "keys") => atom(format!("{{ let mut t: Vec<_> = {rs}.keys().cloned().collect(); t.sort(); t }}")),
            (Ty::Map(..), "values" | "items") => {
                let mapper = if name == "values" { "v.clone()" } else { "(k.clone(), v.clone())" };
                atom(format!("{{ let mut t: Vec<_> = {rs}.iter().collect(); t.sort_by(|a, b| a.0.cmp(b.0)); t.into_iter().map(|(k, v)| {mapper}).collect::<Vec<_>>() }}"))
            }
            (Ty::Option(_), "is_some" | "is_none" | "unwrap") => atom(format!("{rs}.{name}()")),
            (Ty::Option(_), "unwrap_or") => {
                let d = one(self);
                atom(format!("{rs}.unwrap_or({d})"))
            }
            _ => atom(format!("{rs}.{name}()")),
        }
    }

    fn pattern(&mut self, e: &Expr) -> String {
        match (&e.kind, self.ety(e)) {
            (ExprKind::Str(s), _) => format!("\"{}\"", escape_str(s.as_bytes())),
            (_, Ty::Char) => self.expr(e).s,
            _ => self.borrowed(e),
        }
    }

    // ----- places and statements -----
    fn place(&mut self, e: &Expr, deref_root: bool) -> String {
        match &e.kind {
            ExprKind::Var(n) => {
                if deref_root && self.var_kind(e) == Kind::RefMut {
                    format!("*{}", ident(n))
                } else {
                    ident(n)
                }
            }
            ExprKind::Index(a, i) => {
                let at = self.ety(a);
                match at {
                    Ty::Map(..) => {
                        let base = self.place(a, false);
                        let k = self.key_ref(i);
                        format!("{base}.get_mut({k}).unwrap()")
                    }
                    _ => {
                        let base = self.place(a, false);
                        let ix = self.idx(i);
                        format!("{base}[{ix}]")
                    }
                }
            }
            ExprKind::TupleIdx(a, n) => format!("{}.{n}", self.place(a, false)),
            _ => self.expr(e).s,
        }
    }

    fn block(&mut self, b: &Block) {
        self.ind += 1;
        for s in &b.stmts {
            self.stmt(s);
        }
        self.ind -= 1;
    }

    fn pat_text(p: &Pat, mutable: bool) -> String {
        match p {
            Pat::Var(v) => format!("{}{}", if mutable { "mut " } else { "" }, ident(v)),
            Pat::Wild => "_".into(),
            Pat::Tuple(ps) => format!("({})", ps.iter().map(|x| Self::pat_text(x, mutable)).collect::<Vec<_>>().join(", ")),
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Let { pat, ty, init } => {
                let v = self.owned(init);
                let t = self.info.ty(s.id).clone();
                let any_mut = self.info.pat_slots.get(&s.id).is_some_and(|v| v.iter().any(|&x| self.slot_mutated(x)));
                let needs_ty = ty.is_some()
                    || matches!(&init.kind, ExprKind::Ctor(_) | ExprKind::NoneLit | ExprKind::Null)
                    || matches!(&init.kind, ExprKind::VecLit(items) if items.is_empty())
                    || matches!(&init.kind, ExprKind::Int(_));
                let ann = if needs_ty && !matches!(pat, Pat::Tuple(_)) { format!(": {}", ty_name(&t)) } else { String::new() };
                let name = match pat {
                    Pat::Tuple(_) => Self::pat_text(pat, any_mut),
                    Pat::Wild => "_".into(),
                    Pat::Var(_) => Self::pat_text(pat, any_mut),
                };
                self.line(&format!("let {name}{ann} = {v};"));
            }
            StmtKind::Assign { target, op, value } => self.assign(target, *op, value),
            StmtKind::If { .. } => self.if_chain(s, false),
            StmtKind::While { cond, body } => {
                let c = self.expr(cond).s;
                self.line(&format!("while {c} {{"));
                self.block(body);
                self.line("}");
            }
            StmtKind::ForRange { var, start, end, step, body } => {
                let v = ident(var);
                let a = start.as_ref().map_or("0".to_string(), |x| self.expr(x).at(12));
                let b = self.expr(end).at(12);
                let k = match step.as_ref().map(|x| &x.kind) {
                    Some(ExprKind::Int(k)) => *k,
                    _ => 1,
                };
                let range = if k > 0 {
                    let r = format!("{a}..{b}");
                    if k == 1 { r } else { format!("({r}).step_by({k})") }
                } else {
                    let r = format!("({b} + 1)..={a}");
                    let rev = format!("({r}).rev()");
                    if k == -1 { rev } else { format!("{rev}.step_by({})", -k) }
                };
                self.line(&format!("for {v} in {range} {{"));
                self.block(body);
                self.line("}");
            }
            StmtKind::ForEach { pat, iter, body } => self.for_each(s, pat, iter, body),
            StmtKind::Break => self.line("break;"),
            StmtKind::Continue => self.line("continue;"),
            StmtKind::Return(None) => self.line("return;"),
            StmtKind::Return(Some(e)) => {
                let v = if let ExprKind::Var(_) = e.kind
                    && self.var_kind(e) == Kind::Owned
                {
                    self.expr(e).s
                } else {
                    self.owned(e)
                };
                self.line(&format!("return {v};"));
            }
            StmtKind::Expr(e) => {
                let v = self.expr(e).s;
                self.line(&format!("{v};"));
            }
        }
    }

    fn assign(&mut self, target: &Expr, op: Option<BinOp>, value: &Expr) {
        if let ExprKind::Field(a, f) = &target.kind {
            let recv = self.expr(a).at(20);
            let setter = format!("set_{f}");
            let newv = match op {
                None => self.owned(value),
                Some(o) => {
                    let rhs = self.expr(value);
                    format!("{recv}.{f}() {} {}", o.text(), rhs.at(rprec(o) + 1))
                }
            };
            self.line(&format!("{recv}.{setter}({newv});"));
            return;
        }
        let tt = self.ety(target);
        if let (ExprKind::Index(a, i), None) = (&target.kind, op)
            && matches!(self.ety(a), Ty::Map(..))
        {
            let base = self.place(a, false);
            let k = self.owned(i);
            let v = self.owned(value);
            self.line(&format!("{base}.insert({k}, {v});"));
            return;
        }
        let t = if let ExprKind::Index(a, _) = &target.kind
            && matches!(self.ety(a), Ty::Map(..))
        {
            format!("*{}", self.place(target, true))
        } else {
            self.place(target, true)
        };
        match op {
            None => {
                let v = self.owned(value);
                self.line(&format!("{t} = {v};"));
            }
            Some(BinOp::Add) if matches!(tt, Ty::Str) => {
                if matches!(self.ety(value), Ty::Char) {
                    let v = self.expr(value).s;
                    self.line(&format!("{}.push({v});", t.trim_start_matches('*')));
                } else {
                    let v = self.raw_or_owned_str(value);
                    self.line(&format!("{}.push_str({v});", t.trim_start_matches('*')));
                }
            }
            Some(BinOp::Add) if matches!(tt, Ty::Vec(_)) => {
                let w = self.borrowed(value);
                self.line(&format!("{}.extend_from_slice({w});", t.trim_start_matches('*')));
            }
            Some(o) => {
                let v = self.expr(value).s;
                self.line(&format!("{t} {}= {v};", o.text()));
            }
        }
    }

    fn if_chain(&mut self, s: &Stmt, continued: bool) {
        let StmtKind::If { cond, then, els } = &s.kind else { return };
        let c = self.expr(cond).s;
        let head = if continued { format!("}} else if {c} {{") } else { format!("if {c} {{") };
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

    fn for_each(&mut self, s: &Stmt, pat: &Pat, iter: &Expr, body: &Block) {
        let any_mut = self.info.pat_slots.get(&s.id).is_some_and(|v| v.iter().any(|&x| self.slot_mutated(x)));
        if let Some((kind, args)) = iter_special(iter) {
            let Pat::Tuple(ps) = pat else { return };
            let a0 = self.expr(&args[0]).at(20);
            let (first, second) = (Self::pat_text(&ps[0], any_mut), Self::pat_text(&ps[1], any_mut));
            if kind == "enumerate" {
                let elem = match self.ety(&args[0]) {
                    Ty::Vec(t) => *t,
                    _ => Ty::Int,
                };
                let conv = if is_copy(&elem) { "copied" } else { "cloned" };
                self.line(&format!("for ({}, {second}) in {a0}.iter().{conv}().enumerate().map(|(i, x)| (i as i64, x)) {{", first));
            } else {
                let a1 = self.expr(&args[1]).at(20);
                self.line(&format!("for ({first}, {second}) in {a0}.iter().cloned().zip({a1}.iter().cloned()) {{"));
            }
            self.block(body);
            self.line("}");
            return;
        }
        let elem = match self.ety(iter) {
            Ty::Vec(t) => *t,
            _ => Ty::Char,
        };
        let p = Self::pat_text(pat, any_mut);
        let it = self.expr(iter);
        let src = if matches!(self.ety(iter), Ty::Str) {
            format!("{}.chars()", it.at(20))
        } else if is_place(iter) {
            let conv = if is_copy(&elem) { "copied" } else { "cloned" };
            format!("{}.iter().{conv}()", it.at(20))
        } else {
            it.s
        };
        self.line(&format!("for {p} in {src} {{"));
        self.block(body);
        self.line("}");
    }

    fn function(&mut self, idx: usize) -> String {
        self.f = idx;
        let f = &self.prog.fns[idx];
        let mut params = Vec::new();
        for (i, p) in f.params.iter().enumerate() {
            let t = ty_name(&p.ty);
            let n = ident(&p.name);
            let mutated = self.info.fns[idx].mutated.get(i).copied().unwrap_or(false);
            params.push(match self.param_kind(idx, i) {
                Kind::RefMut => format!("{n}: &mut {t}"),
                Kind::Ref => format!("{n}: &{t}"),
                Kind::Owned => format!("{}{n}: {t}", if mutated { "mut " } else { "" }),
            });
        }
        let ret = if f.ret == Ty::Unit { String::new() } else { format!(" -> {}", ty_name(&f.ret)) };
        format!("fn {}({}){ret}", ident(&f.name), params.join(", "))
    }
}

/// Translate to Rust.
pub fn generate(prog: &Program, info: &TypeInfo, entry: &str) -> Generated {
    let mut g = Gen { prog, info, entry: entry.to_string(), f: 0, out: String::new(), ind: 0 };
    let used = used_types(prog, info);
    let (list, tree) = (used.contains(&Ty::LNode), used.contains(&Ty::TNode));
    let mut code = String::new();
    for i in 0..prog.fns.len() {
        let head = g.function(i);
        g.out.clear();
        g.ind = 0;
        g.line(&format!("{head} {{"));
        g.block(&prog.fns[i].body);
        g.line("}");
        if !code.is_empty() {
            code.push('\n');
        }
        code.push_str(&g.out);
    }
    let mut pre = String::new();
    let mut std_use = Vec::new();
    for (pred, name) in [
        (used.iter().any(|t| matches!(t, Ty::Map(..))), "HashMap"),
        (used.iter().any(|t| matches!(t, Ty::Set(_))), "HashSet"),
        (used.iter().any(|t| matches!(t, Ty::Queue(_) | Ty::Deque(_))), "VecDeque"),
        (used.iter().any(|t| matches!(t, Ty::Heap(_))), "BinaryHeap"),
    ] {
        if pred {
            std_use.push(name);
        }
    }
    std_use.sort();
    if !std_use.is_empty() {
        pre.push_str(&format!("use std::collections::{{{}}};\n", std_use.join(", ")));
    }
    if list || tree {
        pre.push_str(&nodes_prelude(list, tree));
    }
    Generated { lang: Lang::Rust, prelude: pre, code }
}
