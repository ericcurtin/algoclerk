// SPDX-License-Identifier: Apache-2.0
//! The static type checker with local inference and clerk-flavoured messages.

use super::ast::*;
use super::diag::Diag;
use super::infer::{Infer, hashable, orderable};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Which types and built-ins a level unlocks. `None` means "everything".
#[derive(Clone, Debug, Default)]
pub struct Allowed {
    /// Unlocked type families (`vec`, `map`, ...); `int`, `bool` and `unit` are always available.
    pub types: Option<BTreeSet<String>>,
    /// Unlocked built-in names (`len`, `push`, ...).
    pub builtins: Option<BTreeSet<String>>,
    /// Helper functions allowed besides the entry function.
    pub helpers: bool,
    /// Recursion allowed.
    pub recursion: bool,
}

impl Allowed {
    /// Everything unlocked.
    pub fn all() -> Allowed {
        Allowed { types: None, builtins: None, helpers: true, recursion: true }
    }
}

/// The fixed header of the function the player writes.
#[derive(Clone, Debug, PartialEq)]
pub struct EntrySig {
    /// Function name.
    pub name: String,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type.
    pub ret: Ty,
}

impl EntrySig {
    /// The header as text.
    pub fn text(&self) -> String {
        let f = FnDecl { id: 0, name: self.name.clone(), params: self.params.clone(), ret: self.ret.clone(), body: Block::default() };
        super::print::signature_text(&f)
    }
}

/// Options for [`check`].
#[derive(Clone, Debug, Default)]
pub struct CheckOpts {
    /// Unlock lists.
    pub allowed: Allowed,
    /// The required entry function (levels); `None` for sandbox programs.
    pub entry: Option<EntrySig>,
}

/// Per-function facts for the interpreter and the code generators.
#[derive(Clone, Debug, Default)]
pub struct FnInfo {
    /// Number of local slots (parameters first).
    pub nslots: u32,
    /// Name and type of every slot.
    pub locals: Vec<(String, Ty)>,
    /// Slots that are assigned or mutated after their declaration.
    pub mutated: Vec<bool>,
    /// The function (indirectly) calls itself.
    pub recursive: bool,
}

/// Results of checking: types and slots by node id.
#[derive(Clone, Debug, Default)]
pub struct TypeInfo {
    /// Resolved type of each expression node (and the initialiser type for `let`).
    pub types: Vec<Ty>,
    /// Variable slot of `Var` nodes, callee index of `Call` nodes (`u32::MAX` for built-ins), loop var slot of `ForRange`.
    pub slots: Vec<u32>,
    /// Pattern slots of `Let` / `ForEach` statements (`u32::MAX` for `_`).
    pub pat_slots: HashMap<NodeId, Vec<u32>>,
    /// One entry per function.
    pub fns: Vec<FnInfo>,
}

impl TypeInfo {
    /// Type of a node.
    pub fn ty(&self, id: NodeId) -> &Ty {
        &self.types[id as usize]
    }
}

/// Describe a type in clerk words.
pub fn plural(t: &Ty) -> String {
    match t {
        Ty::Int => "numbers (int)".into(),
        Ty::Str => "words (string)".into(),
        Ty::Char => "letters (char)".into(),
        Ty::Bool => "yes/no answers (bool)".into(),
        Ty::Unit => "nothing".into(),
        Ty::Var(_) => "something I cannot name yet".into(),
        other => format!("a {} drawer", other),
    }
}

fn did_you_mean<'a>(name: &str, options: impl Iterator<Item = &'a str>) -> Option<String> {
    fn dist(a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        for i in 1..=a.len() {
            let mut cur = vec![i];
            for j in 1..=b.len() {
                let c = usize::from(a[i - 1] != b[j - 1]);
                cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + c));
            }
            prev = cur;
        }
        prev[b.len()]
    }
    options
        .map(|o| (dist(name, o), o))
        .filter(|(d, _)| *d <= 2 && *d < name.len())
        .min_by_key(|(d, _)| *d)
        .map(|(_, o)| o.to_string())
}

struct Scope {
    names: HashMap<String, (Ty, u32)>,
}

struct FnCx {
    scopes: Vec<Scope>,
    locals: Vec<(String, Ty)>,
    mutated: Vec<bool>,
    ret: Ty,
    loops: u32,
    mut_stack: Vec<HashSet<u32>>,
    calls: HashSet<String>,
}

struct Checker<'a> {
    prog: &'a Program,
    opts: &'a CheckOpts,
    inf: Infer,
    types: Vec<Ty>,
    slots: Vec<u32>,
    pat_slots: HashMap<NodeId, Vec<u32>>,
    sigs: HashMap<String, (usize, Vec<Param>, Ty)>,
    cx: FnCx,
    errors: Vec<Diag>,
    need_ord: Vec<(NodeId, Ty)>,
    need_hash: Vec<(NodeId, Ty)>,
    null_vars: Vec<(NodeId, Ty)>,
    node_tys: Vec<NodeId>,
}

type R<T> = Result<T, ()>;

const MUTATING_VEC: &[&str] = &["push", "pop", "insert", "remove", "reverse", "sort", "swap", "clear", "extend"];

/// Does calling this method on a value of this type change the value?
pub fn is_mutating(recv: &Ty, name: &str) -> bool {
    match recv {
        Ty::Vec(_) => MUTATING_VEC.contains(&name),
        Ty::Str => name == "push",
        Ty::Set(_) => matches!(name, "add" | "remove" | "clear"),
        Ty::Map(..) => matches!(name, "remove" | "clear"),
        Ty::Stack(_) | Ty::Queue(_) | Ty::Heap(_) => matches!(name, "push" | "pop" | "clear"),
        Ty::Deque(_) => matches!(name, "push_front" | "push_back" | "pop_front" | "pop_back" | "clear"),
        _ => false,
    }
}

/// Names of every built-in function and method (for the unlock lists, the picker and tests).
pub const ALL_BUILTINS: &[&str] = &[
    "len", "min", "max", "abs", "sum", "sorted", "reversed", "to_string", "to_int", "ord", "chr", "join", "from_chars",
    "enumerate", "zip", "some", "lnode", "tnode", "push", "pop", "insert", "remove", "contains", "index_of", "reverse",
    "sort", "swap", "clear", "is_empty", "extend", "count", "upper", "lower", "find", "starts_with", "ends_with",
    "split", "chars", "trim", "replace", "repeat", "is_digit", "is_alpha", "is_alnum", "is_upper", "is_lower",
    "is_space", "add", "union", "intersection", "difference", "get", "keys", "values", "items", "is_some", "is_none",
    "unwrap", "unwrap_or", "peek", "front", "back", "push_front", "push_back", "pop_front", "pop_back",
];

impl<'a> Checker<'a> {
    fn err<T>(&mut self, node: NodeId, code: &'static str, msg: impl Into<String>) -> R<T> {
        self.errors.push(Diag::node(node, code, msg));
        Err(())
    }

    fn set_ty(&mut self, id: NodeId, t: Ty) {
        let i = id as usize;
        if self.types.len() <= i {
            self.types.resize(i + 1, Ty::Unit);
            self.slots.resize(i + 1, u32::MAX);
        }
        self.types[i] = t;
        self.node_tys.push(id);
    }

    fn unify_at(&mut self, node: NodeId, expected: &Ty, found: &Ty) -> R<()> {
        if self.inf.unify(expected, found) {
            Ok(())
        } else {
            let e = self.inf.resolve(expected);
            let f = self.inf.resolve(found);
            self.err(
                node,
                "type.mismatch",
                format!("Mix-up! I need {} here, but this holds {}.", plural(&e), plural(&f)),
            )
        }
    }

    fn builtin_ok(&mut self, node: NodeId, name: &str) -> R<()> {
        if let Some(b) = &self.opts.allowed.builtins
            && !b.contains(name)
        {
            return self.err(
                node,
                "locked.builtin",
                format!("The `{name}` tool is locked on this floor. Part of the challenge is doing without it!"),
            );
        }
        Ok(())
    }

    fn declare(&mut self, node: NodeId, name: &str, ty: Ty) -> R<u32> {
        if name == "_" {
            return Ok(u32::MAX);
        }
        let slot = self.cx.locals.len() as u32;
        if self.cx.scopes.last().is_some_and(|s| s.names.contains_key(name)) {
            return self.err(node, "name.dup", format!("You already have a drawer labelled `{name}` here. Use a new name."));
        }
        self.cx.locals.push((name.to_string(), ty.clone()));
        self.cx.mutated.push(false);
        if let Some(s) = self.cx.scopes.last_mut() {
            s.names.insert(name.to_string(), (ty, slot));
        }
        Ok(slot)
    }

    fn lookup(&self, name: &str) -> Option<(Ty, u32)> {
        self.cx.scopes.iter().rev().find_map(|s| s.names.get(name).cloned())
    }

    fn bind_pat(&mut self, node: NodeId, pat: &Pat, ty: &Ty, out: &mut Vec<u32>) -> R<()> {
        match pat {
            Pat::Wild => {
                out.push(u32::MAX);
                Ok(())
            }
            Pat::Var(n) => {
                let s = self.declare(node, n, ty.clone())?;
                out.push(s);
                Ok(())
            }
            Pat::Tuple(ps) => {
                let parts: Vec<Ty> = ps.iter().map(|_| self.inf.fresh()).collect();
                self.unify_at(node, &Ty::Tuple(parts.clone()), ty)?;
                for (p, t) in ps.iter().zip(parts.iter()) {
                    self.bind_pat(node, p, t, out)?;
                }
                Ok(())
            }
        }
    }

    /// Slot of the variable a place expression is rooted in (through index and tuple steps only).
    fn root_slot(&self, e: &Expr) -> Option<u32> {
        match &e.kind {
            ExprKind::Var(_) => Some(self.slots[e.id as usize]).filter(|s| *s != u32::MAX),
            ExprKind::Index(a, _) | ExprKind::TupleIdx(a, _) => self.root_slot(a),
            _ => None,
        }
    }

    fn mark_mutated(&mut self, e: &Expr) {
        if let Some(s) = self.root_slot(e) {
            if let Some(m) = self.cx.mutated.get_mut(s as usize) {
                *m = true;
            }
            for set in &mut self.cx.mut_stack {
                set.insert(s);
            }
        }
    }

    fn is_place(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Var(_) => true,
            ExprKind::Index(a, _) | ExprKind::TupleIdx(a, _) => Self::is_place(a),
            _ => false,
        }
    }

    // ----- expressions -----
    fn expr(&mut self, e: &Expr) -> R<Ty> {
        let t = self.expr_inner(e)?;
        self.set_ty(e.id, t.clone());
        Ok(t)
    }

    fn expect(&mut self, e: &Expr, want: &Ty) -> R<()> {
        let t = self.expr(e)?;
        self.unify_at(e.id, want, &t)
    }

    fn args_n(&mut self, node: NodeId, name: &str, args: &[Expr], n: usize) -> R<()> {
        if args.len() != n {
            return self.err(
                node,
                "call.arity",
                format!(
                    "`{name}` needs {n} thing{} but you gave it {}.",
                    if n == 1 { "" } else { "s" },
                    args.len()
                ),
            );
        }
        Ok(())
    }

    fn expr_inner(&mut self, e: &Expr) -> R<Ty> {
        match &e.kind {
            ExprKind::Int(_) => Ok(Ty::Int),
            ExprKind::Bool(_) => Ok(Ty::Bool),
            ExprKind::Char(_) => Ok(Ty::Char),
            ExprKind::Str(_) => Ok(Ty::Str),
            ExprKind::Null => {
                let v = self.inf.fresh();
                self.null_vars.push((e.id, v.clone()));
                Ok(v)
            }
            ExprKind::NoneLit => Ok(Ty::Option(Box::new(self.inf.fresh()))),
            ExprKind::Var(n) => match self.lookup(n) {
                Some((t, s)) => {
                    self.slots.resize(self.slots.len().max(e.id as usize + 1), u32::MAX);
                    self.slots[e.id as usize] = s;
                    Ok(t)
                }
                None => {
                    let names: Vec<String> = self.cx.scopes.iter().flat_map(|s| s.names.keys().cloned()).collect();
                    let hint = did_you_mean(n, names.iter().map(String::as_str))
                        .map(|h| format!(" Did you mean `{h}`?"))
                        .unwrap_or_default();
                    self.err(e.id, "name.unknown", format!("I cannot find a drawer labelled `{n}`.{hint}"))
                }
            },
            ExprKind::Unary(op, a) => {
                let t = match op {
                    UnOp::Neg | UnOp::BitNot => Ty::Int,
                    UnOp::Not => Ty::Bool,
                };
                self.expect(a, &t)?;
                Ok(t)
            }
            ExprKind::Binary(op, a, b) => self.binary(e.id, *op, a, b),
            ExprKind::Index(a, i) => {
                let at = self.expr(a)?;
                match self.inf.shallow(&at) {
                    Ty::Vec(t) => {
                        self.expect(i, &Ty::Int)?;
                        Ok(*t)
                    }
                    Ty::Str => {
                        self.expect(i, &Ty::Int)?;
                        Ok(Ty::Char)
                    }
                    Ty::Map(k, v) => {
                        self.expect(i, &k)?;
                        Ok(*v)
                    }
                    other => self.err(
                        e.id,
                        "index.type",
                        format!("You cannot look inside {} with [..].", plural(&self.inf.resolve(&other))),
                    ),
                }
            }
            ExprKind::Slice(a, lo, hi) => {
                let at = self.expr(a)?;
                for x in [lo, hi].into_iter().flatten() {
                    self.expect(x, &Ty::Int)?;
                }
                match self.inf.shallow(&at) {
                    t @ (Ty::Vec(_) | Ty::Str) => Ok(t),
                    other => self.err(
                        e.id,
                        "slice.type",
                        format!("Only shelves and words can be sliced, not {}.", plural(&self.inf.resolve(&other))),
                    ),
                }
            }
            ExprKind::Field(a, f) => {
                let at = self.expr(a)?;
                match (self.inf.shallow(&at), f.as_str()) {
                    (Ty::LNode, "val") | (Ty::TNode, "val") => Ok(Ty::Int),
                    (Ty::LNode, "next") => Ok(Ty::LNode),
                    (Ty::TNode, "left" | "right") => Ok(Ty::TNode),
                    (Ty::LNode, _) => self.err(e.id, "field.unknown", format!("A list tag has `val` and `next`, not `{f}`.")),
                    (Ty::TNode, _) => {
                        self.err(e.id, "field.unknown", format!("A tree node has `val`, `left` and `right`, not `{f}`."))
                    }
                    (other, _) => self.err(
                        e.id,
                        "field.type",
                        format!("Only list and tree nodes have fields; this is {}.", plural(&self.inf.resolve(&other))),
                    ),
                }
            }
            ExprKind::TupleIdx(a, n) => {
                let at = self.expr(a)?;
                match self.inf.shallow(&at) {
                    Ty::Tuple(ts) if *n < ts.len() => Ok(ts[*n].clone()),
                    Ty::Tuple(ts) => {
                        self.err(e.id, "tuple.index", format!("That tuple only has {} parts (.0 to .{}).", ts.len(), ts.len() - 1))
                    }
                    other => self.err(
                        e.id,
                        "tuple.type",
                        format!("`.{n}` only works on tuples, not {}.", plural(&self.inf.resolve(&other))),
                    ),
                }
            }
            ExprKind::VecLit(items) => {
                let el = self.inf.fresh();
                for it in items {
                    self.expect(it, &el)?;
                }
                Ok(Ty::Vec(Box::new(el)))
            }
            ExprKind::VecRepeat(x, n) => {
                let t = self.expr(x)?;
                self.expect(n, &Ty::Int)?;
                Ok(Ty::Vec(Box::new(t)))
            }
            ExprKind::TupleLit(items) => {
                let mut ts = Vec::new();
                for it in items {
                    ts.push(self.expr(it)?);
                }
                Ok(Ty::Tuple(ts))
            }
            ExprKind::Ctor(t) => {
                self.need_ty_checks(e.id, t);
                Ok(t.clone())
            }
            ExprKind::Call(name, args) => self.call(e, name, args),
            ExprKind::Method(recv, name, args) => self.method(e, recv, name, args),
        }
    }

    fn need_ty_checks(&mut self, node: NodeId, t: &Ty) {
        match t {
            Ty::Set(x) => self.need_hash.push((node, (**x).clone())),
            Ty::Map(k, _) => self.need_hash.push((node, (**k).clone())),
            Ty::Heap(x) => self.need_ord.push((node, (**x).clone())),
            _ => {}
        }
        match t {
            Ty::Vec(a) | Ty::Set(a) | Ty::Option(a) | Ty::Stack(a) | Ty::Queue(a) | Ty::Deque(a) | Ty::Heap(a) => {
                self.need_ty_checks(node, a)
            }
            Ty::Map(k, v) => {
                self.need_ty_checks(node, k);
                self.need_ty_checks(node, v);
            }
            Ty::Tuple(ts) => ts.iter().for_each(|x| self.need_ty_checks(node, x)),
            _ => {}
        }
    }

    fn binary(&mut self, id: NodeId, op: BinOp, a: &Expr, b: &Expr) -> R<Ty> {
        match op {
            BinOp::And | BinOp::Or => {
                self.expect(a, &Ty::Bool)?;
                self.expect(b, &Ty::Bool)?;
                Ok(Ty::Bool)
            }
            BinOp::Eq | BinOp::Ne => {
                let ta = self.expr(a)?;
                self.expect(b, &ta)?;
                Ok(Ty::Bool)
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let ta = self.expr(a)?;
                self.expect(b, &ta)?;
                self.need_ord.push((id, ta));
                Ok(Ty::Bool)
            }
            BinOp::Add => {
                let ta = self.expr(a)?;
                let tb = self.expr(b)?;
                let sa = self.inf.shallow(&ta);
                let sb = self.inf.shallow(&tb);
                match (&sa, &sb) {
                    (Ty::Str, Ty::Char) => return Ok(Ty::Str),
                    (Ty::Str, Ty::Str) => return Ok(Ty::Str),
                    (Ty::Vec(_), Ty::Vec(_)) => {
                        self.unify_at(b.id, &ta, &tb)?;
                        return Ok(ta);
                    }
                    _ => {}
                }
                self.unify_at(b.id, &ta, &tb)?;
                match self.inf.shallow(&ta) {
                    Ty::Var(_) => {
                        self.unify_at(a.id, &Ty::Int, &ta)?;
                        Ok(Ty::Int)
                    }
                    Ty::Int => Ok(Ty::Int),
                    other => self.err(
                        id,
                        "op.type",
                        format!("`+` adds numbers, words or shelves, not {}.", plural(&self.inf.resolve(&other))),
                    ),
                }
            }
            _ => {
                self.expect(a, &Ty::Int)?;
                self.expect(b, &Ty::Int)?;
                Ok(Ty::Int)
            }
        }
    }

    fn call(&mut self, e: &Expr, name: &str, args: &[Expr]) -> R<Ty> {
        let id = e.id;
        // user functions first
        if let Some((idx, params, ret)) = self.sigs.get(name).cloned() {
            if !self.opts.allowed.helpers && self.opts.entry.as_ref().is_some_and(|s| s.name != name) {
                return self.err(id, "locked.helpers", "Helper functions are locked on this floor; do it all in one function.");
            }
            self.args_n(id, name, args, params.len())?;
            self.cx.calls.insert(name.to_string());
            for (a, p) in args.iter().zip(params.iter()) {
                if p.by_ref {
                    if !matches!(a.kind, ExprKind::Var(_)) {
                        return self.err(a.id, "call.ref", format!("`{}` changes its `{}`, so pass a plain variable here.", name, p.name));
                    }
                    self.expect(a, &p.ty)?;
                    self.mark_mutated(a);
                } else {
                    self.expect(a, &p.ty)?;
                }
            }
            self.slots.resize(self.slots.len().max(id as usize + 1), u32::MAX);
            self.slots[id as usize] = idx as u32;
            return Ok(ret);
        }
        if name == "range" {
            return self.err(id, "range.place", "`range` only works right after `for x in`.");
        }
        if !ALL_BUILTINS.contains(&name) {
            let names: Vec<&str> = self.sigs.keys().map(String::as_str).chain(ALL_BUILTINS.iter().copied()).collect();
            let hint = did_you_mean(name, names.into_iter()).map(|h| format!(" Did you mean `{h}`?")).unwrap_or_default();
            return self.err(id, "call.unknown", format!("I do not know a tool called `{name}`.{hint}"));
        }
        self.builtin_ok(id, name)?;
        match name {
            "len" => {
                self.args_n(id, name, args, 1)?;
                let t = self.expr(&args[0])?;
                match self.inf.shallow(&t) {
                    Ty::Str | Ty::Vec(_) | Ty::Set(_) | Ty::Map(..) | Ty::Stack(_) | Ty::Queue(_) | Ty::Deque(_) | Ty::Heap(_) => Ok(Ty::Int),
                    other => self.err(id, "len.type", format!("`len` counts the items of a drawer, not {}.", plural(&self.inf.resolve(&other)))),
                }
            }
            "min" | "max" => {
                if args.len() == 2 {
                    let t = self.expr(&args[0])?;
                    self.expect(&args[1], &t)?;
                    self.need_ord.push((id, t.clone()));
                    Ok(t)
                } else {
                    self.args_n(id, name, args, 1)?;
                    let t = self.expr(&args[0])?;
                    let el = self.inf.fresh();
                    self.unify_at(args[0].id, &Ty::Vec(Box::new(el.clone())), &t)?;
                    self.need_ord.push((id, el.clone()));
                    Ok(el)
                }
            }
            "abs" => {
                self.args_n(id, name, args, 1)?;
                self.expect(&args[0], &Ty::Int)?;
                Ok(Ty::Int)
            }
            "sum" => {
                self.args_n(id, name, args, 1)?;
                self.expect(&args[0], &Ty::vec(Ty::Int))?;
                Ok(Ty::Int)
            }
            "sorted" | "reversed" => {
                self.args_n(id, name, args, 1)?;
                let t = self.expr(&args[0])?;
                match (self.inf.shallow(&t), name) {
                    (Ty::Str, _) => Ok(Ty::Str),
                    (v @ Ty::Vec(_), _) => {
                        if name == "sorted"
                            && let Ty::Vec(el) = &v
                        {
                            self.need_ord.push((id, (**el).clone()));
                        }
                        Ok(v)
                    }
                    (Ty::Set(el), "sorted") => {
                        self.need_ord.push((id, (*el).clone()));
                        Ok(Ty::Vec(el))
                    }
                    (other, _) => self.err(
                        id,
                        "sort.type",
                        format!("`{name}` works on shelves{}, not {}.", if name == "sorted" { ", words and bins" } else { " and words" }, plural(&self.inf.resolve(&other))),
                    ),
                }
            }
            "to_string" => {
                self.args_n(id, name, args, 1)?;
                let t = self.expr(&args[0])?;
                match self.inf.shallow(&t) {
                    Ty::Int | Ty::Char => Ok(Ty::Str),
                    Ty::Var(_) => {
                        self.unify_at(args[0].id, &Ty::Int, &t)?;
                        Ok(Ty::Str)
                    }
                    other => self.err(id, "tostring.type", format!("`to_string` turns a number or a letter into a word, not {}.", plural(&self.inf.resolve(&other)))),
                }
            }
            "to_int" => {
                self.args_n(id, name, args, 1)?;
                self.expect(&args[0], &Ty::Str)?;
                Ok(Ty::Int)
            }
            "ord" => {
                self.args_n(id, name, args, 1)?;
                self.expect(&args[0], &Ty::Char)?;
                Ok(Ty::Int)
            }
            "chr" => {
                self.args_n(id, name, args, 1)?;
                self.expect(&args[0], &Ty::Int)?;
                Ok(Ty::Char)
            }
            "join" => {
                self.args_n(id, name, args, 2)?;
                self.expect(&args[0], &Ty::vec(Ty::Str))?;
                self.expect(&args[1], &Ty::Str)?;
                Ok(Ty::Str)
            }
            "from_chars" => {
                self.args_n(id, name, args, 1)?;
                self.expect(&args[0], &Ty::vec(Ty::Char))?;
                Ok(Ty::Str)
            }
            "enumerate" | "zip" => self.err(
                id,
                "iter.place",
                format!("`{name}` only works right after `for ... in`, like: for (i, x) in {name}(..)."),
            ),
            "some" => {
                self.args_n(id, name, args, 1)?;
                let t = self.expr(&args[0])?;
                Ok(Ty::Option(Box::new(t)))
            }
            "lnode" => {
                self.args_n(id, name, args, 2)?;
                self.expect(&args[0], &Ty::Int)?;
                self.expect(&args[1], &Ty::LNode)?;
                Ok(Ty::LNode)
            }
            "tnode" => {
                self.args_n(id, name, args, 3)?;
                self.expect(&args[0], &Ty::Int)?;
                self.expect(&args[1], &Ty::TNode)?;
                self.expect(&args[2], &Ty::TNode)?;
                Ok(Ty::TNode)
            }
            _ => self.err(id, "call.method", format!("`{name}` is used on something, like x.{name}(..), not on its own.")),
        }
    }

    fn method(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr]) -> R<Ty> {
        let id = e.id;
        let rt = self.expr(recv)?;
        let rt = self.inf.shallow(&rt);
        if matches!(rt, Ty::Var(_)) {
            return self.err(
                id,
                "method.unknown_type",
                format!("I cannot tell what kind of drawer this is yet, so I cannot use `{name}` on it. Add a type, like `let x: vec<int> = [];`."),
            );
        }
        let known = ALL_BUILTINS.contains(&name);
        if !known {
            return self.err(id, "method.unknown", format!("I do not know a tool called `{name}`."));
        }
        let sig: Option<Vec<(Vec<Ty>, Ty)>> = method_sigs(&rt, name);
        let Some(sigs) = sig else {
            return self.err(
                id,
                "method.type",
                format!("{} has no `{name}` tool.", capital(&plural(&self.inf.resolve(&rt)))),
            );
        };
        self.builtin_ok(id, name)?;
        let chosen = sigs.iter().find(|(ps, _)| ps.len() == args.len());
        let Some((params, ret)) = chosen.cloned() else {
            let n = sigs[0].0.len();
            return self.err(id, "call.arity", format!("`{name}` needs {n} thing{} but you gave it {}.", if n == 1 { "" } else { "s" }, args.len()));
        };
        if is_mutating(&rt, name) {
            if !Self::is_place(recv) {
                return self.err(
                    recv.id,
                    "mutate.temp",
                    format!("`{name}` rearranges its drawer, so it must sit in a variable, not be a temporary result."),
                );
            }
            self.mark_mutated(recv);
        }
        // overloads whose argument may be char or string
        for (a, p) in args.iter().zip(params.iter()) {
            if matches!(p, Ty::Var(u32::MAX)) {
                // "char or string"
                let t = self.expr(a)?;
                match self.inf.shallow(&t) {
                    Ty::Char | Ty::Str => {}
                    other => {
                        return self.err(a.id, "type.mismatch", format!("I need a letter or a word here, but this holds {}.", plural(&self.inf.resolve(&other))))
                    }
                }
            } else {
                self.expect(a, p)?;
            }
        }
        match (&rt, name) {
            (Ty::Vec(el), "sort") => self.need_ord.push((id, (**el).clone())),
            (Ty::Heap(el), _) => self.need_ord.push((id, (**el).clone())),
            (Ty::Vec(el), "contains" | "index_of" | "count") => {
                let _ = el;
            }
            _ => {}
        }
        Ok(ret)
    }

    // ----- statements -----
    fn block(&mut self, b: &Block, scope: bool) -> R<()> {
        if scope {
            self.cx.scopes.push(Scope { names: HashMap::new() });
        }
        let mut ok = true;
        for s in &b.stmts {
            if self.stmt(s).is_err() {
                ok = false;
                if self.errors.len() > 8 {
                    break;
                }
            }
        }
        if scope {
            self.cx.scopes.pop();
        }
        if ok { Ok(()) } else { Err(()) }
    }

    fn stmt(&mut self, s: &Stmt) -> R<()> {
        match &s.kind {
            StmtKind::Let { pat, ty, init } => {
                let it = self.expr(init)?;
                if let Some(t) = ty {
                    self.need_ty_checks(s.id, t);
                    self.unify_at(init.id, t, &it)?;
                }
                self.set_ty(s.id, it.clone());
                let mut slots = Vec::new();
                self.bind_pat(s.id, pat, &it, &mut slots)?;
                self.pat_slots.insert(s.id, slots);
                Ok(())
            }
            StmtKind::Assign { target, op, value } => {
                let tt = self.expr(target)?;
                if let ExprKind::Index(a, _) = &target.kind {
                    let at = self.types[a.id as usize].clone();
                    if matches!(self.inf.shallow(&at), Ty::Str) {
                        return self.err(target.id, "assign.str", "Words cannot be edited letter by letter. Build a new word instead.");
                    }
                }
                match &target.kind {
                    ExprKind::Var(_) | ExprKind::Index(..) | ExprKind::Field(..) => {}
                    _ => return self.err(target.id, "assign.place", "You can only store into a variable, a shelf slot or a node field."),
                }
                match op {
                    None => self.expect(value, &tt)?,
                    Some(BinOp::Add) => {
                        let vt = self.expr(value)?;
                        match (self.inf.shallow(&tt), self.inf.shallow(&vt)) {
                            (Ty::Str, Ty::Char | Ty::Str) => {}
                            (Ty::Vec(_), _) => self.unify_at(value.id, &tt, &vt)?,
                            _ => {
                                self.unify_at(target.id, &Ty::Int, &tt)?;
                                self.unify_at(value.id, &Ty::Int, &vt)?;
                            }
                        }
                    }
                    Some(_) => {
                        self.unify_at(target.id, &Ty::Int, &tt)?;
                        self.expect(value, &Ty::Int)?;
                    }
                }
                if let ExprKind::Field(a, f) = &target.kind {
                    let at = self.types[a.id as usize].clone();
                    let _ = (at, f);
                }
                self.mark_mutated(target);
                Ok(())
            }
            StmtKind::If { cond, then, els } => {
                self.expect(cond, &Ty::Bool)?;
                let a = self.block(then, true);
                let b = match els {
                    Some(b) => self.block(b, true),
                    None => Ok(()),
                };
                a.and(b)
            }
            StmtKind::While { cond, body } => {
                self.expect(cond, &Ty::Bool)?;
                self.cx.loops += 1;
                let r = self.block(body, true);
                self.cx.loops -= 1;
                r
            }
            StmtKind::ForRange { var, start, end, step, body } => {
                for x in [start, step].into_iter().flatten() {
                    self.expect(x, &Ty::Int)?;
                }
                self.expect(end, &Ty::Int)?;
                self.cx.scopes.push(Scope { names: HashMap::new() });
                let slot = self.declare(s.id, var, Ty::Int);
                self.cx.loops += 1;
                let r = match slot {
                    Ok(slot) => {
                        self.slots.resize(self.slots.len().max(s.id as usize + 1), u32::MAX);
                        self.slots[s.id as usize] = slot;
                        self.block(body, false)
                    }
                    Err(()) => Err(()),
                };
                self.cx.loops -= 1;
                self.cx.scopes.pop();
                r
            }
            StmtKind::ForEach { pat, iter, body } => self.for_each(s, pat, iter, body),
            StmtKind::Break | StmtKind::Continue => {
                if self.cx.loops == 0 {
                    let w = if matches!(s.kind, StmtKind::Break) { "break" } else { "continue" };
                    return self.err(s.id, "loop.outside", format!("`{w}` only makes sense inside a loop."));
                }
                Ok(())
            }
            StmtKind::Return(v) => {
                let ret = self.cx.ret.clone();
                match v {
                    Some(e) => {
                        if ret == Ty::Unit {
                            return self.err(s.id, "return.unit", "This function hands back nothing, so `return` takes no value.");
                        }
                        self.expect(e, &ret)
                    }
                    None => {
                        if ret != Ty::Unit {
                            return self.err(s.id, "return.missing", format!("This function must hand back {}.", plural(&ret)));
                        }
                        Ok(())
                    }
                }
            }
            StmtKind::Expr(e) => {
                self.expr(e)?;
                Ok(())
            }
        }
    }

    fn for_each(&mut self, s: &Stmt, pat: &Pat, iter: &Expr, body: &Block) -> R<()> {
        let elem: Ty;
        let mut iter_root: Option<u32> = None;
        match &iter.kind {
            ExprKind::Call(n, args) if (n == "enumerate" || n == "zip") && !self.sigs.contains_key(n) => {
                self.builtin_ok(iter.id, n)?;
                if n == "enumerate" {
                    self.args_n(iter.id, n, args, 1)?;
                    let t = self.expr(&args[0])?;
                    let el = self.inf.fresh();
                    self.unify_at(args[0].id, &Ty::Vec(Box::new(el.clone())), &t)?;
                    elem = Ty::Tuple(vec![Ty::Int, el]);
                    iter_root = self.root_slot(&args[0]);
                } else {
                    self.args_n(iter.id, n, args, 2)?;
                    let a = self.inf.fresh();
                    let b = self.inf.fresh();
                    self.expect(&args[0], &Ty::Vec(Box::new(a.clone())))?;
                    self.expect(&args[1], &Ty::Vec(Box::new(b.clone())))?;
                    elem = Ty::Tuple(vec![a, b]);
                }
                if !matches!(pat, Pat::Tuple(ps) if ps.len() == 2) {
                    return self.err(s.id, "iter.pattern", format!("`{n}` hands out pairs, so write `for (a, b) in {n}(..)`."));
                }
                self.set_ty(iter.id, Ty::Vec(Box::new(elem.clone())));
            }
            _ => {
                let t = self.expr(iter)?;
                iter_root = self.root_slot(iter);
                match self.inf.shallow(&t) {
                    Ty::Vec(el) => elem = *el,
                    Ty::Str => elem = Ty::Char,
                    other @ (Ty::Set(_) | Ty::Map(..)) => {
                        let what = if matches!(other, Ty::Set(_)) { "sorted(bin)" } else { "m.keys(), m.values() or m.items()" };
                        return self.err(
                            iter.id,
                            "iter.unordered",
                            format!("Bins and cabinets have no order to walk in. Use {what} to get a shelf first."),
                        );
                    }
                    other => {
                        return self.err(
                            iter.id,
                            "iter.type",
                            format!("I can walk along a shelf or a word, but not {}.", plural(&self.inf.resolve(&other))),
                        )
                    }
                }
            }
        }
        self.cx.scopes.push(Scope { names: HashMap::new() });
        let mut slots = Vec::new();
        let b = self.bind_pat(s.id, pat, &elem, &mut slots);
        self.pat_slots.insert(s.id, slots);
        self.cx.loops += 1;
        self.cx.mut_stack.push(HashSet::new());
        let r = b.and_then(|()| self.block(body, false));
        let muts = self.cx.mut_stack.pop().unwrap_or_default();
        self.cx.loops -= 1;
        self.cx.scopes.pop();
        if let Some(root) = iter_root
            && muts.contains(&root)
        {
            return self.err(
                s.id,
                "iter.mutate",
                "Do not rearrange a shelf while you are walking along it! Walk a copy, or build a new shelf.",
            );
        }
        r
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// Signatures of a method for a receiver type: `(param types, return type)` per arity.
/// `Ty::Var(u32::MAX)` as a parameter means "a char or a string".
pub fn method_sigs(recv: &Ty, name: &str) -> Option<std::vec::Vec<(std::vec::Vec<Ty>, Ty)>> {
    use Ty::*;
    let b = |t: &Ty| Box::new(t.clone());
    let one = |p: std::vec::Vec<Ty>, r: Ty| Some(vec![(p, r)]);
    let any = Var(u32::MAX);
    match recv {
        Vec(t) => match name {
            "push" => one(vec![(**t).clone()], Unit),
            "pop" => one(vec![], (**t).clone()),
            "insert" => one(vec![Int, (**t).clone()], Unit),
            "remove" => one(vec![Int], (**t).clone()),
            "contains" => one(vec![(**t).clone()], Bool),
            "index_of" => one(vec![(**t).clone()], Int),
            "count" => one(vec![(**t).clone()], Int),
            "reverse" | "sort" | "clear" => one(vec![], Unit),
            "swap" => one(vec![Int, Int], Unit),
            "is_empty" => one(vec![], Bool),
            "extend" => one(vec![Vec(t.clone())], Unit),
            _ => None,
        },
        Str => match name {
            "upper" | "lower" | "trim" => one(vec![], Str),
            "find" => one(vec![any], Int),
            "contains" => one(vec![any], Bool),
            "starts_with" | "ends_with" => one(vec![Str], Bool),
            "split" => one(vec![any], Vec(b(&Str))),
            "chars" => one(vec![], Vec(b(&Char))),
            "push" => one(vec![Char], Unit),
            "is_empty" => one(vec![], Bool),
            "count" => one(vec![Char], Int),
            "replace" => one(vec![Str, Str], Str),
            "repeat" => one(vec![Int], Str),
            _ => None,
        },
        Char => match name {
            "is_digit" | "is_alpha" | "is_alnum" | "is_upper" | "is_lower" | "is_space" => one(vec![], Bool),
            "upper" | "lower" => one(vec![], Char),
            _ => None,
        },
        Set(t) => match name {
            "add" | "remove" => one(vec![(**t).clone()], Unit),
            "contains" => one(vec![(**t).clone()], Bool),
            "is_empty" => one(vec![], Bool),
            "clear" => one(vec![], Unit),
            "union" | "intersection" | "difference" => one(vec![recv.clone()], recv.clone()),
            _ => None,
        },
        Map(k, v) => match name {
            "get" => Some(vec![(vec![(**k).clone(), (**v).clone()], (**v).clone()), (vec![(**k).clone()], Option(v.clone()))]),
            "contains" => one(vec![(**k).clone()], Bool),
            "remove" => one(vec![(**k).clone()], Unit),
            "keys" => one(vec![], Vec(k.clone())),
            "values" => one(vec![], Vec(v.clone())),
            "items" => one(vec![], Vec(b(&Tuple(vec![(**k).clone(), (**v).clone()])))),
            "is_empty" => one(vec![], Bool),
            "clear" => one(vec![], Unit),
            _ => None,
        },
        Option(t) => match name {
            "is_some" | "is_none" => one(vec![], Bool),
            "unwrap" => one(vec![], (**t).clone()),
            "unwrap_or" => one(vec![(**t).clone()], (**t).clone()),
            _ => None,
        },
        Stack(t) => match name {
            "push" => one(vec![(**t).clone()], Unit),
            "pop" | "peek" => one(vec![], (**t).clone()),
            "is_empty" => one(vec![], Bool),
            "clear" => one(vec![], Unit),
            _ => None,
        },
        Queue(t) => match name {
            "push" => one(vec![(**t).clone()], Unit),
            "pop" | "front" => one(vec![], (**t).clone()),
            "is_empty" => one(vec![], Bool),
            "clear" => one(vec![], Unit),
            _ => None,
        },
        Deque(t) => match name {
            "push_front" | "push_back" => one(vec![(**t).clone()], Unit),
            "pop_front" | "pop_back" | "front" | "back" => one(vec![], (**t).clone()),
            "is_empty" => one(vec![], Bool),
            "clear" => one(vec![], Unit),
            _ => None,
        },
        Heap(t) => match name {
            "push" => one(vec![(**t).clone()], Unit),
            "pop" | "peek" => one(vec![], (**t).clone()),
            "is_empty" => one(vec![], Bool),
            "clear" => one(vec![], Unit),
            _ => None,
        },
        _ => None,
    }
}

fn always_returns(b: &Block) -> bool {
    match b.stmts.last().map(|s| &s.kind) {
        Some(StmtKind::Return(_)) => true,
        Some(StmtKind::If { then, els: Some(e), .. }) => always_returns(then) && always_returns(e),
        _ => false,
    }
}

/// Check a program. On success returns the type information; otherwise the diagnostics (never panics).
pub fn check(prog: &Program, opts: &CheckOpts) -> Result<TypeInfo, Vec<Diag>> {
    let mut sigs: HashMap<String, (usize, Vec<Param>, Ty)> = HashMap::new();
    let mut errors = Vec::new();
    for (i, f) in prog.fns.iter().enumerate() {
        if ALL_BUILTINS.contains(&f.name.as_str()) || f.name == "range" {
            errors.push(Diag::node(f.id, "fn.builtin", format!("`{}` is already the name of a built-in tool. Pick another name.", f.name)));
        }
        if sigs.insert(f.name.clone(), (i, f.params.clone(), f.ret.clone())).is_some() {
            errors.push(Diag::node(f.id, "fn.dup", format!("There are two functions called `{}`.", f.name)));
        }
    }
    if let Some(entry) = &opts.entry {
        match prog.find(&entry.name) {
            None => errors.push(Diag::plain("fn.missing", format!("Your answer needs a function with this header: {}", entry.text()))),
            Some(f) => {
                let got = EntrySig { name: f.name.clone(), params: f.params.clone(), ret: f.ret.clone() };
                if got != *entry {
                    errors.push(Diag::node(
                        f.id,
                        "fn.header",
                        format!("The header must read exactly: {}", entry.text()),
                    ));
                }
            }
        }
        if !opts.allowed.helpers && prog.fns.len() > 1 {
            let extra = prog.fns.iter().find(|f| f.name != entry.name);
            if let Some(f) = extra {
                errors.push(Diag::node(f.id, "locked.helpers", "Helper functions are locked on this floor; do it all in one function."));
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut ck = Checker {
        prog,
        opts,
        inf: Infer::default(),
        types: vec![Ty::Unit; prog.next_id as usize],
        slots: vec![u32::MAX; prog.next_id as usize],
        pat_slots: HashMap::new(),
        sigs,
        cx: FnCx { scopes: vec![], locals: vec![], mutated: vec![], ret: Ty::Unit, loops: 0, mut_stack: vec![], calls: HashSet::new() },
        errors: Vec::new(),
        need_ord: vec![],
        need_hash: vec![],
        null_vars: vec![],
        node_tys: vec![],
    };
    let mut fn_infos = Vec::new();
    let mut call_graph: Vec<HashSet<String>> = Vec::new();
    for f in &prog.fns {
        ck.cx = FnCx {
            scopes: vec![Scope { names: HashMap::new() }],
            locals: vec![],
            mutated: vec![],
            ret: f.ret.clone(),
            loops: 0,
            mut_stack: vec![],
            calls: HashSet::new(),
        };
        for p in &f.params {
            ck.need_ty_checks(f.id, &p.ty);
            if ck.declare(f.id, &p.name, p.ty.clone()).is_err() {
                continue;
            }
        }
        let _ = ck.block(&f.body, false);
        if f.ret != Ty::Unit && !always_returns(&f.body) && ck.errors.is_empty() {
            ck.errors.push(Diag::node(f.id, "return.path", format!("`{}` must hand back {} on every path. Finish with a `return`.", f.name, plural(&f.ret))));
        }
        let cx = std::mem::replace(&mut ck.cx, FnCx { scopes: vec![], locals: vec![], mutated: vec![], ret: Ty::Unit, loops: 0, mut_stack: vec![], calls: HashSet::new() });
        call_graph.push(cx.calls.clone());
        fn_infos.push(FnInfo { nslots: cx.locals.len() as u32, locals: cx.locals, mutated: cx.mutated, recursive: false });
    }
    if !ck.errors.is_empty() {
        return Err(std::mem::take(&mut ck.errors));
    }
    // final resolution
    let mut errs: Vec<Diag> = Vec::new();
    let allowed_types = |t: &Ty| -> Option<&'static str> {
        let Some(allowed) = &opts.allowed.types else { return None };
        let mut fams = Vec::new();
        t.families(&mut fams);
        fams.into_iter().find(|f| !matches!(*f, "int" | "bool" | "unit" | "null" | "?") && !allowed.contains(*f))
    };
    for &id in &ck.node_tys.clone() {
        let t = ck.inf.resolve(&ck.types[id as usize]);
        if ck.inf.has_var(&t) {
            // `null`/`none` and empty literals that were never pinned down
            let msg = "I cannot tell what this holds. Add a type, like `let x: vec<int> = [];`.";
            if !errs.iter().any(|e| e.code == "type.unknown") {
                errs.push(Diag::node(id, "type.unknown", msg));
            }
            continue;
        }
        if let Some(fam) = allowed_types(&t)
            && !errs.iter().any(|e| e.code == "locked.type")
        {
            errs.push(Diag::node(id, "locked.type", format!("The `{fam}` drawer is locked on this floor. Part of the challenge is doing without it!")));
        }
        ck.types[id as usize] = t;
    }
    for (id, t) in &ck.null_vars {
        let t = ck.inf.resolve(t);
        if !matches!(t, Ty::LNode | Ty::TNode) {
            errs.push(Diag::node(*id, "null.type", "`null` can only stand for a missing list node or tree node."));
        }
    }
    for (id, t) in ck.need_ord.clone() {
        let t = ck.inf.resolve(&t);
        if !orderable(&t) {
            errs.push(Diag::node(id, "type.order", format!("I cannot put {} in order. I can order numbers, letters, words and tuples or shelves of those.", plural(&t))));
        }
    }
    for (id, t) in ck.need_hash.clone() {
        let t = ck.inf.resolve(&t);
        if !hashable(&t) {
            errs.push(Diag::node(id, "type.hash", format!("{} cannot label a drawer or sit in a bin. Use numbers, letters, words, or tuples of those.", capital(&plural(&t)))));
        }
    }
    // helpers/recursion unlocks
    let names: Vec<String> = prog.fns.iter().map(|f| f.name.clone()).collect();
    for (i, calls) in call_graph.iter().enumerate() {
        // reachability of i from itself
        let mut seen: HashSet<usize> = HashSet::new();
        let mut stack: Vec<usize> = calls.iter().filter_map(|c| names.iter().position(|n| n == c)).collect();
        while let Some(j) = stack.pop() {
            if j == i {
                fn_infos[i].recursive = true;
            }
            if seen.insert(j) {
                stack.extend(call_graph[j].iter().filter_map(|c| names.iter().position(|n| n == c)));
            }
        }
        if fn_infos[i].recursive && !opts.allowed.recursion {
            errs.push(Diag::node(prog.fns[i].id, "locked.recursion", "Calling a function from itself is locked on this floor. Use a loop!"));
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let _ = ck.prog;
    for fi in &mut fn_infos {
        for (_, t) in &mut fi.locals {
            *t = ck.inf.resolve(t);
        }
    }
    Ok(TypeInfo { types: ck.types, slots: ck.slots, pat_slots: ck.pat_slots, fns: fn_infos })
}
