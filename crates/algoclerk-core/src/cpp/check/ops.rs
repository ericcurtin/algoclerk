// SPDX-License-Identifier: Apache-2.0
//! Unary and binary operators.

use super::ctx::Checker;
use super::libcall::{deref_val, random_access};
use super::val::*;
use crate::cpp::ast::*;

/// "adding", "subtracting", ... for messages.
fn verb(op: BinOp) -> String {
    match op {
        BinOp::Add => "adding".into(),
        BinOp::Sub => "subtracting".into(),
        BinOp::Mul => "multiplying".into(),
        BinOp::Div => "dividing".into(),
        BinOp::Rem => "taking the remainder of".into(),
        op => format!("combining with `{}`", op.text()),
    }
}

/// Short description of an expression for messages (`nums`, `p.x`, `v[...]`).
pub(super) fn describe(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Var(n) => n.clone(),
        ExprKind::Member { obj, name, arrow } => {
            format!("{}{}{name}", describe(obj), if *arrow { "->" } else { "." })
        }
        ExprKind::Index(a, _) => format!("{}[...]", describe(a)),
        ExprKind::Unary(UnOp::Deref, a) => format!("*{}", describe(a)),
        _ => "this value".to_string(),
    }
}

impl<'a> Checker<'a> {
    /// `a op b` for non-assignment binary operators.
    pub fn binary(&mut self, e: &Expr, op: BinOp, a: &'a Expr, b: &'a Expr) -> Val {
        let av = self.expr(a);
        let bv = self.expr(b);
        if av.is_unknown() || bv.is_unknown() {
            return if op == BinOp::Comma {
                bv
            } else {
                Val::unknown()
            };
        }
        match op {
            BinOp::Comma => bv,
            BinOp::And | BinOp::Or => {
                for (x, ex) in [(&av, a), (&bv, b)] {
                    if !self.truthy(&x.ty) {
                        self.err(
                            ex.id,
                            "type.condition",
                            format!(
                                "`{}` needs yes/no values on both sides, but this is a {}",
                                op.text(),
                                show(x)
                            ),
                        );
                        return Val::unknown();
                    }
                }
                let c = match (av.cval, bv.cval) {
                    (Some(x), Some(y)) => fold(op, x, y, &Ty::Bool),
                    _ => None,
                };
                Val {
                    cval: c,
                    ..Val::pr(Ty::Bool)
                }
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.compare(e, op, &av, &bv)
            }
            _ => self.arith(e, op, &av, &bv),
        }
    }

    fn bad_operands(&mut self, e: &Expr, op: BinOp, a: &Val, b: &Val) -> Val {
        let hint = match (&a.ty, &b.ty, op) {
            (Ty::Str, t, BinOp::Add) | (t, Ty::Str, BinOp::Add)
                if t.is_integral() && *t != Ty::Char =>
            {
                "; use `std::to_string(...)` to turn the number into text first"
            }
            (x, y, BinOp::Add) if super::val::is_cstr(x) && super::val::is_cstr(y) => {
                "; two string literals cannot be added, wrap one in `std::string(...)`"
            }
            _ => "",
        };
        self.err(
            e.id,
            "type.binary",
            format!(
                "you are {} a {} and a {}: `{}` does not work between those{hint}",
                verb(op),
                show(a),
                show(b),
                op.text()
            ),
        );
        Val::unknown()
    }

    fn arith(&mut self, e: &Expr, op: BinOp, a: &Val, b: &Val) -> Val {
        let shift = matches!(op, BinOp::Shl | BinOp::Shr);
        if a.ty.is_integral() && b.ty.is_integral() {
            let ty = if shift {
                promote(&a.ty)
            } else {
                uac(&a.ty, &b.ty)
            };
            let c = match (a.cval, b.cval) {
                (Some(x), Some(y)) if shift => fold(op, wrap(&ty, x), y, &ty),
                (Some(x), Some(y)) => fold(op, wrap(&ty, x), wrap(&ty, y), &ty),
                _ => None,
            };
            return Val {
                cval: c,
                ..Val::pr(ty)
            };
        }
        if op == BinOp::Add {
            let strlike = |t: &Ty| *t == Ty::Str || super::val::is_cstr(t) || *t == Ty::Char;
            if (a.ty == Ty::Str && strlike(&b.ty)) || (b.ty == Ty::Str && strlike(&a.ty)) {
                return Val::pr(Ty::Str);
            }
        }
        if let (Ty::Array(t, _), true, true) = (
            &a.ty,
            matches!(op, BinOp::Add | BinOp::Sub),
            b.ty.is_integral(),
        ) {
            return Val::pr(Ty::Ptr(t.clone()));
        }
        if let (Ty::Iter { container, .. }, true) = (&a.ty, matches!(op, BinOp::Add | BinOp::Sub)) {
            if b.ty.is_integral() {
                if random_access(container) {
                    return Val::pr(a.ty.clone()).with_ic(a.ic);
                }
                self.err(e.id, "type.binary", format!("iterators of {container} cannot jump by a number; step with `++` or use `std::next(it, n)`"));
                return Val::unknown();
            }
            if op == BinOp::Sub && a.ty == b.ty && random_access(container) {
                return Val::pr(Ty::LongLong);
            }
        }
        if let (Ty::Iter { container, .. }, BinOp::Add, true) = (&b.ty, op, a.ty.is_integral())
            && random_access(container)
        {
            return Val::pr(b.ty.clone()).with_ic(b.ic);
        }
        if matches!(a.ty, Ty::Ptr(_)) && !super::val::is_cstr(&a.ty)
            || matches!(b.ty, Ty::Ptr(_)) && !super::val::is_cstr(&b.ty)
        {
            self.err(
                e.id,
                "unsupported.pointer",
                "pointer arithmetic is not in the subset: use `->` to follow pointers",
            );
            return Val::unknown();
        }
        self.bad_operands(e, op, a, b)
    }

    fn compare(&mut self, e: &Expr, op: BinOp, a: &Val, b: &Val) -> Val {
        let ordered = !matches!(op, BinOp::Eq | BinOp::Ne);
        let strlike = |t: &Ty| matches!(t, Ty::Str | Ty::StrView) || super::val::is_cstr(t);
        let ptrlike = |v: &Val| matches!(v.ty, Ty::Ptr(_) | Ty::NullPtr);
        let ok = if a.ty.is_integral() && b.ty.is_integral() {
            let ty = uac(&a.ty, &b.ty);
            let c = match (a.cval, b.cval) {
                (Some(x), Some(y)) => fold(op, wrap(&ty, x), wrap(&ty, y), &ty),
                _ => None,
            };
            return Val {
                cval: c,
                ..Val::pr(Ty::Bool)
            };
        } else if a.ty == b.ty {
            self.comparable(&a.ty, ordered)
        } else if strlike(&a.ty) && strlike(&b.ty) {
            !(super::val::is_cstr(&a.ty) && super::val::is_cstr(&b.ty))
        } else if (ptrlike(a) && ptrlike(b))
            || (ptrlike(a) && b.cval == Some(0))
            || (ptrlike(b) && a.cval == Some(0))
        {
            !ordered
        } else if let (Ty::Optional(t), _) | (_, Ty::Optional(t)) = (&a.ty, &b.ty) {
            let other = if matches!(a.ty, Ty::Optional(_)) {
                b
            } else {
                a
            };
            (matches!(&other.ty, Ty::Struct(n) if n == "nullopt_t") && !ordered)
                || (self.convertible(other, t) && !matches!(other.ty, Ty::Optional(_)))
        } else {
            false
        };
        if !ok {
            let what = if matches!(a.ty, Ty::Struct(_)) || matches!(b.ty, Ty::Struct(_)) {
                "; compare their members instead (`a.x == b.x`)"
            } else {
                ""
            };
            self.err(
                e.id,
                "type.compare",
                format!(
                    "you are comparing a {} with a {}: `{}` does not work between those{what}",
                    show(a),
                    show(b),
                    op.text()
                ),
            );
            return Val::unknown();
        }
        Val::pr(Ty::Bool)
    }

    /// Unary operators.
    pub fn unary(&mut self, e: &Expr, op: UnOp, a: &'a Expr) -> Val {
        let v = self.expr(a);
        if v.is_unknown() {
            return Val::unknown();
        }
        let bad = |s: &mut Self, why: &str| {
            s.err(
                e.id,
                "type.unary",
                format!("{why}, but this is a {}", show(&v)),
            );
            Val::unknown()
        };
        match op {
            UnOp::Neg | UnOp::Plus | UnOp::BitNot => {
                if !v.ty.is_integral() {
                    return bad(self, "the sign/bit operator needs a number");
                }
                let ty = promote(&v.ty);
                let c = v.cval.map(|c| match op {
                    UnOp::Neg => wrap(&ty, -c),
                    UnOp::BitNot => wrap(&ty, !c),
                    _ => c,
                });
                Val {
                    cval: c,
                    ..Val::pr(ty)
                }
            }
            UnOp::Not => {
                if !self.truthy(&v.ty) {
                    return bad(self, "`!` needs a yes/no value");
                }
                Val {
                    cval: v.cval.map(|c| i128::from(c == 0)),
                    ..Val::pr(Ty::Bool)
                }
            }
            UnOp::Deref => match deref_val(&v) {
                Some(d) => d,
                None => match &v.ty {
                    Ty::Array(t, _) => Val::lv((**t).clone(), v.konst),
                    Ty::Optional(t) => Val::lv((**t).clone(), v.konst),
                    _ => bad(self, "`*` needs a pointer, an iterator or an optional"),
                },
            },
            UnOp::AddrOf => {
                if !v.lval {
                    return bad(self, "`&` needs a variable (something with an address)");
                }
                Val::pr(Ty::Ptr(Box::new(v.ty.clone()))).with_ic(v.konst)
            }
            UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => {
                let int_ok = v.ty.is_integral() && v.ty != Ty::Bool;
                if !(int_ok || matches!(v.ty, Ty::Iter { .. })) {
                    return bad(self, "`++`/`--` need a number or an iterator");
                }
                if !self.writable(a, &v, e.id) {
                    return Val::unknown();
                }
                if matches!(op, UnOp::PreInc | UnOp::PreDec) {
                    Val::lv(v.ty.clone(), false).with_ic(v.ic)
                } else {
                    Val::pr(v.ty.clone()).with_ic(v.ic)
                }
            }
        }
    }

    /// Is `v` (the value of `a`) an lvalue that may be changed? Reports `type.lvalue` / `const.assign`.
    pub fn writable(&mut self, a: &Expr, v: &Val, at: NodeId) -> bool {
        if !v.lval {
            self.err(
                at,
                "type.lvalue",
                format!(
                    "you cannot change {}: it is a temporary value, not a variable",
                    describe(a)
                ),
            );
            return false;
        }
        if v.konst {
            self.err(
                at,
                "const.assign",
                format!("`{}` is const here, so it cannot be changed", describe(a)),
            );
            return false;
        }
        true
    }
}
