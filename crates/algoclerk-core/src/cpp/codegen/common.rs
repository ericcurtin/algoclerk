// SPDX-License-Identifier: Apache-2.0
//! Pieces shared by the Rust and C translators: expression text with precedence, friendly errors, and
//! small syntactic analyses of the checked tree (which variables are mutated, loop shape tests).

use crate::cpp::ast::*;
use crate::cpp::check::{Resolved, TypeInfo};
use std::collections::HashSet;

/// Translation result.
pub type R<T> = Result<T, String>;

/// The friendly "cannot translate" error.
pub fn no<T>(lang: &str, what: impl std::fmt::Display) -> R<T> {
    Err(format!("this clerk cannot translate {what} to {lang} yet"))
}

/// What a piece of generated expression text denotes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An owned temporary value.
    Val,
    /// An assignable place of the value's type (a variable, `v[i]`, a field).
    Place,
    /// A reference (`&T` / `&mut T`) to a value of the type.
    Ref,
}

/// A piece of generated expression text with its binding strength.
#[derive(Clone, Debug)]
pub struct Ex {
    /// Text.
    pub s: String,
    /// Binding strength of the outermost operator (20 = atom).
    pub p: u8,
    /// Value, place or reference.
    pub kind: Kind,
    /// The text is an integer literal with this value (so it can be re-typed).
    pub lit: Option<i128>,
    /// The text ends in an `as` cast (a following `<` would be read as generics).
    pub cast: bool,
    /// The text has type `&str` (a literal or a `&str` parameter).
    pub slice: bool,
}

impl Ex {
    /// An atom value.
    pub fn val(s: impl Into<String>) -> Ex {
        Ex::new(s, 20, Kind::Val)
    }
    /// With precedence and kind.
    pub fn new(s: impl Into<String>, p: u8, kind: Kind) -> Ex {
        Ex {
            s: s.into(),
            p,
            kind,
            lit: None,
            cast: false,
            slice: false,
        }
    }
    /// Text, parenthesised when binding weaker than `min`.
    pub fn at(&self, min: u8) -> String {
        if self.p < min {
            format!("({})", self.s)
        } else {
            self.s.clone()
        }
    }
}

/// The root variable of an lvalue-like expression (`a[i].x`, `*p`, `v.back()`).
pub fn root(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Var(n) => Some(n),
        ExprKind::Index(a, _) | ExprKind::Unary(UnOp::Deref, a) => root(a),
        ExprKind::Member { obj, .. } => root(obj),
        ExprKind::Binary(BinOp::Add | BinOp::Sub, a, _) => root(a),
        ExprKind::Call { callee, .. } => match &callee.kind {
            ExprKind::Member { obj, name, .. }
                if matches!(
                    name.as_str(),
                    "back" | "front" | "at" | "top" | "begin" | "end" | "rbegin" | "rend" | "find"
                ) =>
            {
                root(obj)
            }
            _ => None,
        },
        _ => None,
    }
}

const MUTATING: &[&str] = &[
    "push_back",
    "pop_back",
    "push",
    "pop",
    "emplace",
    "emplace_back",
    "emplace_front",
    "insert",
    "erase",
    "clear",
    "resize",
    "assign",
    "swap",
    "push_front",
    "pop_front",
    "reset",
    "append",
    "fill",
    "reserve",
    "shrink_to_fit",
];

const MUTATING_FREE: &[&str] = &[
    "sort",
    "stable_sort",
    "reverse",
    "fill",
    "iota",
    "next_permutation",
    "prev_permutation",
    "rotate",
    "unique",
    "remove",
    "remove_if",
    "swap",
    "copy",
    "partial_sum",
    "for_each",
];

/// Names of variables that are assigned, incremented, mutated through a method, bound to a mutable
/// reference or have their address taken anywhere in `b` (lambda bodies included). Name based: an
/// over-approximation is harmless (an unneeded `mut`), a miss would not compile.
pub fn mutated_names(prog: &Program, info: &TypeInfo, b: &Block) -> HashSet<String> {
    let mut out = HashSet::new();
    scan_block(prog, info, b, &mut out);
    out
}

/// [`mutated_names`] for one statement.
pub fn mutated_in_stmt(prog: &Program, info: &TypeInfo, s: &Stmt) -> HashSet<String> {
    let mut out = HashSet::new();
    walk_stmt(s, &mut |st| scan_stmt(prog, info, st, &mut out));
    out
}

fn scan_block(prog: &Program, info: &TypeInfo, b: &Block, out: &mut HashSet<String>) {
    walk_stmts(b, &mut |s| scan_stmt(prog, info, s, out));
}

fn scan_stmt(prog: &Program, info: &TypeInfo, s: &Stmt, out: &mut HashSet<String>) {
    match &s.kind {
        StmtKind::RangeFor { spec, range, .. } if spec.is_ref && !spec.konst => {
            out.extend(root(range).map(str::to_string));
        }
        StmtKind::Structured { spec, init, .. } if spec.is_ref && !spec.konst => {
            out.extend(root(init).map(str::to_string));
        }
        StmtKind::Decl { spec, vars } if spec.is_ref && !spec.konst => {
            for v in vars {
                if let Init::Copy(e) = &v.init {
                    out.extend(root(e).map(str::to_string));
                }
            }
        }
        _ => {}
    }
    stmt_exprs(s, &mut |e| {
        walk_expr(e, &mut |x| scan_expr(prog, info, x, out))
    });
}

fn scan_expr(prog: &Program, info: &TypeInfo, e: &Expr, out: &mut HashSet<String>) {
    let mut add = |x: &Expr| out.extend(root(x).map(str::to_string));
    match &e.kind {
        ExprKind::Assign { lhs, .. } => add(lhs),
        ExprKind::Unary(UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec, a) => add(a),
        ExprKind::Unary(UnOp::AddrOf, a) if !matches!(info.ty(a.id), Ty::Struct(_)) => add(a),
        ExprKind::Index(a, _) if matches!(info.ty(a.id), Ty::Map(..) | Ty::UnorderedMap(..)) => {
            add(a);
        }
        ExprKind::Call { callee, args } => {
            match &callee.kind {
                ExprKind::Member { obj, name, .. } if MUTATING.contains(&name.as_str()) => add(obj),
                ExprKind::Path(p)
                    if p.last()
                        .is_some_and(|s| MUTATING_FREE.contains(&s.name.as_str())) =>
                {
                    args.iter().for_each(&mut add);
                }
                _ => {}
            }
            if let Some(Resolved::Func(id)) = info.resolved(callee.id)
                && let Some(f) = prog.fns().find(|f| f.id == *id)
            {
                for (p, a) in f.params.iter().zip(args) {
                    if p.spec.is_ref && !p.spec.konst {
                        add(a);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Does the body contain a `continue` that belongs to the loop around it (not to an inner loop)?
pub fn has_continue(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::Continue => true,
        StmtKind::If { then, els, .. } => {
            has_continue(then) || els.as_ref().is_some_and(|e| has_continue(e))
        }
        StmtKind::Block(b) | StmtKind::Switch { body: b, .. } => b.stmts.iter().any(has_continue),
        _ => false,
    }
}

/// Does the body contain a `break` that belongs to the loop around it?
pub fn has_break(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::Break => true,
        StmtKind::If { then, els, .. } => {
            has_break(then) || els.as_ref().is_some_and(|e| has_break(e))
        }
        StmtKind::Block(b) => b.stmts.iter().any(has_break),
        _ => false,
    }
}

/// Names read anywhere in an expression, and whether it calls anything other than `size` / `length`.
pub fn names_and_calls(e: &Expr) -> (HashSet<String>, bool) {
    let mut names = HashSet::new();
    let mut calls = false;
    walk_expr(e, &mut |x| match &x.kind {
        ExprKind::Var(n) => {
            names.insert(n.clone());
        }
        ExprKind::Call { callee, .. } => match &callee.kind {
            ExprKind::Member { name, .. } if name == "size" || name == "length" => {}
            _ => calls = true,
        },
        ExprKind::Assign { .. }
        | ExprKind::Unary(UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec, _) => {
            calls = true
        }
        _ => {}
    });
    (names, calls)
}

/// Is `e` free of side effects and cheap to evaluate twice?
pub fn is_pure(e: &Expr) -> bool {
    let mut pure = true;
    walk_expr(e, &mut |x| {
        if matches!(
            &x.kind,
            ExprKind::Call { .. }
                | ExprKind::Assign { .. }
                | ExprKind::Unary(
                    UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec,
                    _
                )
                | ExprKind::New { .. }
                | ExprKind::Lambda(_)
        ) {
            pure = false;
        }
    });
    pure
}

/// Integer promotion followed by the usual arithmetic conversions of C++ for two integral types.
pub fn common_ty(a: &Ty, b: &Ty) -> Ty {
    let pa = promote(a);
    let pb = promote(b);
    if pa == pb {
        return pa;
    }
    let (wa, wb) = (pa.bits(), pb.bits());
    let (ua, ub) = (pa.is_unsigned(), pb.is_unsigned());
    if ua == ub {
        return if wa >= wb { pa } else { pb };
    }
    let (u, s) = if ua { (pa, pb) } else { (pb, pa) };
    if u.bits() >= s.bits() { u } else { s }
}

/// Integer promotion: `bool` and `char` become `int`.
pub fn promote(t: &Ty) -> Ty {
    match t {
        Ty::Bool | Ty::Char => Ty::Int,
        t => t.clone(),
    }
}

/// Make a name safe as an identifier in the target language (appends `_` to reserved words).
pub fn ident(name: &str, reserved: &[&str]) -> String {
    if reserved.contains(&name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

/// Where an iterator expression points: the beginning or the end of a container, plus an offset.
pub enum Pos<'e> {
    Begin(&'e Expr, Option<&'e Expr>, bool),
    End(&'e Expr, Option<&'e Expr>, bool),
}

pub fn iter_pos(e: &Expr) -> Option<Pos<'_>> {
    match &e.kind {
        ExprKind::Call { callee, args } if args.is_empty() => {
            let ExprKind::Member { obj, name, .. } = &callee.kind else {
                return None;
            };
            match name.as_str() {
                "begin" | "cbegin" => Some(Pos::Begin(obj, None, false)),
                "end" | "cend" => Some(Pos::End(obj, None, false)),
                "rbegin" => Some(Pos::Begin(obj, None, true)),
                "rend" => Some(Pos::End(obj, None, true)),
                _ => None,
            }
        }
        ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), a, k) => match iter_pos(a)? {
            Pos::Begin(o, None, r) if *op == BinOp::Add => Some(Pos::Begin(o, Some(k), r)),
            Pos::End(o, None, r) if *op == BinOp::Sub => Some(Pos::End(o, Some(k), r)),
            _ => None,
        },
        _ => None,
    }
}

/// The types of a function's parameters and its return type.
pub fn sig_types(f: &FnDecl) -> Vec<Ty> {
    let mut v: Vec<Ty> = f.params.iter().map(|p| p.spec.ty.clone()).collect();
    v.push(f.ret.ty.clone());
    v
}

/// Does any of these types mention a pointer to the struct `name`?
pub fn uses_struct(tys: &[Ty], name: &str) -> bool {
    let mut found = false;
    for t in tys {
        t.walk(&mut |x| {
            if matches!(x, Ty::Ptr(inner) if matches!(&**inner, Ty::Struct(n) if n == name)) {
                found = true;
            }
        });
    }
    found
}
