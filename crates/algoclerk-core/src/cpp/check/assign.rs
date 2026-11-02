// SPDX-License-Identifier: Apache-2.0
//! Assignment, the conditional operator and subscripting.

use super::ctx::Checker;
use super::libcall::random_access;
use super::ops::describe;
use super::val::*;
use crate::cpp::ast::*;

impl<'a> Checker<'a> {
    /// `lhs = rhs` and `lhs op= rhs`.
    pub fn assign(&mut self, e: &Expr, op: Option<BinOp>, lhs: &'a Expr, rhs: &'a Expr) -> Val {
        let lv = self.expr(lhs);
        let rv = self.expr(rhs);
        if lv.is_unknown() || rv.is_unknown() {
            return Val::unknown();
        }
        let scalar = lv.ty.is_integral() || matches!(lv.ty, Ty::Ptr(_) | Ty::Iter { .. });
        if (scalar || lv.lval) && !self.writable(lhs, &lv, e.id) {
            return Val::unknown();
        }
        let what = format!("assigning to `{}`", describe(lhs));
        match op {
            None => {
                if matches!(lv.ty, Ty::Lambda(_)) {
                    self.err(e.id, "type.assign", "a lambda variable cannot be assigned a new value; use `std::function` if you need that");
                    return Val::unknown();
                }
                self.convert_to(rhs.id, &rv, &lv.ty, &what);
            }
            Some(op) => {
                let ok = if lv.ty.is_integral() && rv.ty.is_integral() {
                    true
                } else if lv.ty == Ty::Str && op == BinOp::Add {
                    rv.ty == Ty::Str || rv.ty == Ty::Char || is_cstr(&rv.ty)
                } else if let Ty::Iter { container, .. } = &lv.ty {
                    matches!(op, BinOp::Add | BinOp::Sub)
                        && rv.ty.is_integral()
                        && random_access(container)
                } else {
                    false
                };
                if !ok {
                    let hint = if lv.ty == Ty::Str && rv.ty.is_integral() && rv.ty != Ty::Char {
                        "; use `std::to_string(...)` for numbers"
                    } else {
                        ""
                    };
                    self.err(
                        e.id,
                        "type.binary",
                        format!(
                            "`{}=` does not work between a {} and a {}{hint}",
                            op.text(),
                            show(&lv),
                            show(&rv)
                        ),
                    );
                    return Val::unknown();
                }
            }
        }
        Val::lv(lv.ty.clone(), false).with_ic(lv.ic)
    }

    /// `c ? a : b`.
    pub fn ternary(&mut self, e: &Expr, c: &'a Expr, a: &'a Expr, b: &'a Expr) -> Val {
        let cv = self.expr(c);
        let av = self.expr(a);
        let bv = self.expr(b);
        if !cv.is_unknown() && !self.truthy(&cv.ty) {
            self.err(
                c.id,
                "type.condition",
                format!(
                    "the condition of `?:` must be a yes/no value, not a {}",
                    show(&cv)
                ),
            );
        }
        if av.is_unknown() || bv.is_unknown() {
            return Val::unknown();
        }
        if av.ty == bv.ty && av.ty != Ty::InitList {
            let both = av.lval && bv.lval;
            let mut v = if both {
                Val::lv(av.ty.clone(), av.konst || bv.konst)
            } else {
                Val::pr(av.ty.clone())
            };
            v.ic = av.ic || bv.ic;
            return v;
        }
        if av.ty.is_integral() && bv.ty.is_integral() {
            return Val::pr(uac(&av.ty, &bv.ty));
        }
        let pick = |x: &Val, y: &Val| -> Option<Ty> {
            if y.ty == Ty::NullPtr && matches!(x.ty, Ty::Ptr(_))
                || (x.ty == Ty::Str && is_cstr(&y.ty))
            {
                Some(x.ty.clone())
            } else {
                None
            }
        };
        if let Some(t) = pick(&av, &bv).or_else(|| pick(&bv, &av)) {
            return Val::pr(t);
        }
        for (x, y) in [(&av, &bv), (&bv, &av)] {
            if matches!(x.ty, Ty::Ptr(_)) && y.cval == Some(0) {
                return Val::pr(x.ty.clone());
            }
        }
        self.err(
            e.id,
            "type.ternary",
            format!("the two answers of `?:` have different types ({} and {}); make them the same type, for example with a cast", show(&av), show(&bv)),
        );
        Val::unknown()
    }

    /// `a[i]`.
    pub fn index(&mut self, e: &Expr, a: &'a Expr, i: &'a Expr) -> Val {
        let av = self.expr(a);
        let iv = self.expr(i);
        if av.is_unknown() || iv.is_unknown() {
            return Val::unknown();
        }
        let seq_index = |s: &mut Self, elem: &Ty, konst: bool| {
            if !iv.ty.is_integral() {
                s.err(
                    i.id,
                    "type.index",
                    format!(
                        "an index must be a whole number, but this is a {}",
                        show(&iv)
                    ),
                );
                return Val::unknown();
            }
            Val::lv(elem.clone(), konst)
        };
        match &av.ty {
            Ty::Vector(t) | Ty::Array(t, _) | Ty::Deque(t) => seq_index(self, t, av.konst),
            Ty::Str => seq_index(self, &Ty::Char, av.konst),
            Ty::StrView => seq_index(self, &Ty::Char, true),
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
                if av.konst {
                    self.err(e.id, "const.method", format!("`{}` is const, and `[]` on a map may insert a new entry, so it needs a non-const map; use `.at(key)` or `.find(key)` instead", describe(a)));
                    return Val::unknown();
                }
                self.convert_to(i.id, &iv, k, &format!("the key of `{}[...]`", describe(a)));
                Val::lv((**v).clone(), false)
            }
            t => {
                self.err(
                    e.id,
                    "type.index",
                    format!("a {t} cannot be indexed with `[]`"),
                );
                Val::unknown()
            }
        }
    }
}
