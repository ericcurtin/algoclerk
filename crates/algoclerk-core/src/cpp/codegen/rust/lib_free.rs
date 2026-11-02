// SPDX-License-Identifier: Apache-2.0
//! Rust translation of iterator ranges and the library's free functions (`std::sort`, `std::max`, ...).
//! Iterators are never translated as values: a pair `(c.begin() + a, c.end() - b)` becomes a slice, and
//! `find(...) != c.end()` becomes `contains` / `is_some`.

use super::Rs;
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;
use crate::cpp::print::print_expr;

/// A `[first, last)` pair of iterators into one container.
pub struct Rg {
    pub obj: String,
    pub ty: Ty,
    pub lo: Option<String>,
    pub hi: Option<String>,
    pub rev: bool,
}

/// Where an iterator expression points: the beginning or the end of a container, plus an offset.
pub enum Pos<'e> {
    Begin(&'e Expr, Option<&'e Expr>, bool),
    End(&'e Expr, Option<&'e Expr>, bool),
}

pub fn iter_pos_pub(e: &Expr) -> Option<Pos<'_>> {
    iter_pos(e)
}

fn iter_pos(e: &Expr) -> Option<Pos<'_>> {
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

impl Rs<'_> {
    /// Translate a `[first, last)` pair into a range of one container.
    pub(super) fn range(&mut self, a: &Expr, b: &Expr) -> R<Option<Rg>> {
        let (Some(pa), Some(pb)) = (iter_pos(a), iter_pos(b)) else {
            return Ok(None);
        };
        let first_end = matches!(pa, Pos::End(..));
        let (
            Pos::Begin(oa, ka, ra) | Pos::End(oa, ka, ra),
            Pos::End(ob, kb, rb) | Pos::Begin(ob, kb, rb),
        ) = (pa, pb);
        let end_side = matches!(iter_pos(b), Some(Pos::End(..)));
        if print_expr(oa) != print_expr(ob) || ra != rb {
            return Ok(None);
        }
        let ty = self.ty_of(oa);
        let base = self.ex(oa, true)?;
        let mut obj = base.at(19);
        if matches!(ty, Ty::Str | Ty::StrView) {
            obj = format!("{obj}.as_bytes()");
        }
        let off = |this: &mut Self, k: Option<&Expr>| -> R<Option<String>> {
            match k {
                None => Ok(None),
                Some(k) => {
                    let t = this.ty_of(k);
                    Ok(Some(this.index_text(k, &t)?))
                }
            }
        };
        let lo = match (first_end, off(self, ka)?) {
            (false, k) => k,
            (true, Some(k)) => Some(format!("{obj}.len() - {k}")),
            (true, None) => Some(format!("{obj}.len()")),
        };
        let kb_t = off(self, kb)?;
        let hi = match (end_side, kb_t) {
            (true, None) => None,
            (true, Some(k)) => Some(format!("{obj}.len() - {k}")),
            (false, Some(k)) => Some(k),
            (false, None) => Some("0".to_string()),
        };
        Ok(Some(Rg {
            obj,
            ty,
            lo,
            hi,
            rev: ra,
        }))
    }

    /// The slice (or whole container) of a range, usable as a method receiver.
    pub(super) fn range_slice(&self, r: &Rg) -> String {
        match (&r.lo, &r.hi) {
            (None, None) => r.obj.clone(),
            (lo, hi) => format!(
                "{}[{}..{}]",
                r.obj,
                lo.clone().unwrap_or_default(),
                hi.clone().unwrap_or_default()
            ),
        }
    }

    /// An iterator over cloned elements of the range.
    pub(super) fn range_iter(&self, r: &Rg) -> String {
        let s = self.range_slice(r);
        let it = match &r.ty {
            Ty::Map(..) | Ty::UnorderedMap(..) => {
                format!("{s}.iter().map(|(k, v)| (k.clone(), v.clone()))")
            }
            _ => format!("{s}.iter().cloned()"),
        };
        if r.rev { format!("{it}.rev()") } else { it }
    }

    /// `Vec::from_iter`-style construction of `ty` from a range.
    pub(super) fn range_collect(&mut self, r: &Rg, ty: &Ty) -> R<String> {
        if matches!(ty, Ty::Stack(_) | Ty::Queue(_) | Ty::PriorityQueue(..)) {
            return no("Rust", "building this container from a range");
        }
        Ok(format!(
            "{}.collect::<{}>()",
            self.range_iter(r),
            self.rty(ty)?
        ))
    }

    fn end_call(e: &Expr) -> bool {
        matches!(iter_pos(e), Some(Pos::End(_, None, false)))
    }

    /// `it != c.end()`, `c.find(k) == c.end()`, `std::find(a, b, x) != b`.
    pub(super) fn iter_compare(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Option<R<Ex>> {
        if !matches!(op, BinOp::Eq | BinOp::Ne) {
            return None;
        }
        let (other, _) = if Self::end_call(b) {
            (a, b)
        } else if Self::end_call(a) {
            (b, a)
        } else {
            return None;
        };
        let found = match self.found_text(other) {
            Ok(Some(t)) => t,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };
        let eq = op == BinOp::Eq;
        Some(
            Ok(Ex::val(if eq { format!("!{found}") } else { found })).map(|x| {
                let mut x = x;
                if eq {
                    x.p = 17;
                } else {
                    x.p = 19;
                }
                x
            }),
        )
    }

    /// Boolean text "the element was found" for a find-like call or a `find` iterator variable.
    fn found_text(&mut self, e: &Expr) -> R<Option<String>> {
        if let ExprKind::Var(v) = &e.kind {
            if matches!(self.lookup(v).map(|x| x.rk), Some(super::Rk::Found(_))) {
                return Ok(Some(format!("{}.is_some()", self.name(v))));
            }
            return Ok(None);
        }
        let ExprKind::Call { callee, args } = &e.kind else {
            return Ok(None);
        };
        match &callee.kind {
            ExprKind::Member { obj, name, .. } if name == "find" && args.len() == 1 => {
                let ot = self.ty_of(obj);
                let o = self.ex(obj, false)?;
                Ok(match &ot {
                    Ty::Map(k, _) | Ty::UnorderedMap(k, _) => {
                        let key = self.conv_val(&args[0], k)?;
                        Some(format!("{}.contains_key(&{})", o.at(19), key.at(17)))
                    }
                    Ty::Set(k) | Ty::UnorderedSet(k) => {
                        let key = self.conv_val(&args[0], k)?;
                        Some(format!("{}.contains(&{})", o.at(19), key.at(17)))
                    }
                    _ => None,
                })
            }
            ExprKind::Path(_) | ExprKind::Var(_) if args.len() == 3 => {
                let Some(crate::cpp::check::Resolved::Lib(mem)) = self.info.resolved(callee.id)
                else {
                    return Ok(None);
                };
                if mem.name != "std::find" {
                    return Ok(None);
                }
                let Some(r) = self.range(&args[0], &args[1])? else {
                    return Ok(None);
                };
                let et = self.elem_of(&r.ty);
                let x = self.conv_val(&args[2], &et)?;
                Ok(Some(format!(
                    "{}.contains(&{})",
                    self.range_slice(&r),
                    x.at(17)
                )))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn elem_of(&self, t: &Ty) -> Ty {
        match t {
            Ty::Vector(e)
            | Ty::Array(e, _)
            | Ty::Deque(e)
            | Ty::List(e)
            | Ty::Set(e)
            | Ty::UnorderedSet(e) => (**e).clone(),
            Ty::Str | Ty::StrView => Ty::Char,
            _ => Ty::Unknown,
        }
    }

    /// `*it` for the iterator-valued calls we understand.
    pub(super) fn deref_iter(&mut self, a: &Expr, m: bool) -> R<Ex> {
        if let ExprKind::Var(v) = &a.kind
            && matches!(self.lookup(v).map(|x| x.rk), Some(super::Rk::Found(_)))
        {
            return Ok(Ex::new(format!("{}.unwrap()", self.name(v)), 19, Kind::Ref));
        }
        let ExprKind::Call { callee, args } = &a.kind else {
            return no("Rust", "dereferencing this iterator");
        };
        if let ExprKind::Member { obj, name, .. } = &callee.kind {
            let ot = self.ty_of(obj);
            let o = self.ex(obj, m)?;
            let t = match (name.as_str(), &ot) {
                ("begin", Ty::Vector(_) | Ty::Array(..)) => {
                    format!("{}.first().unwrap()", o.at(19))
                }
                ("begin", Ty::Set(_)) => format!("{}.first().unwrap()", o.at(19)),
                ("rbegin", Ty::Set(_)) => format!("{}.last().unwrap()", o.at(19)),
                ("begin", Ty::UnorderedSet(_)) => format!("{}.iter().next().unwrap()", o.at(19)),
                _ => return no("Rust", "dereferencing this iterator"),
            };
            return Ok(Ex::new(t, 19, Kind::Ref));
        }
        let Some(crate::cpp::check::Resolved::Lib(mem)) = self.info.resolved(callee.id) else {
            return no("Rust", "dereferencing this iterator");
        };
        match (mem.name, args.as_slice()) {
            ("std::min_element" | "std::max_element", [a, b]) => {
                let Some(r) = self.range(a, b)? else {
                    return no("Rust", "this range");
                };
                let f = if mem.name == "std::min_element" {
                    "min"
                } else {
                    "max"
                };
                Ok(Ex::new(
                    format!("{}.iter().{f}().unwrap()", self.range_slice(&r)),
                    19,
                    Kind::Ref,
                ))
            }
            _ => no("Rust", "dereferencing this iterator"),
        }
    }
}
