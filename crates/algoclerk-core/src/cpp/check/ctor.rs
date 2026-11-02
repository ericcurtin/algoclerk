// SPDX-License-Identifier: Apache-2.0
//! Conversions with diagnostics, reference binding and construction of library and user types.

use super::ctx::{Checker, ParamTy};
use super::libcall::elem_of;
use super::val::*;
use crate::cpp::ast::*;

/// A clerk's hint for a conversion that does not exist.
fn conv_hint(from: &Ty, to: &Ty) -> &'static str {
    match (from, to) {
        (f, Ty::Str) if f.is_integral() && *f != Ty::Char => {
            "; `std::to_string(...)` turns a number into text"
        }
        (Ty::Str, t) if t.is_integral() => "; `std::stoi(...)` turns text into a number",
        (Ty::Str | Ty::StrView, Ty::Char) => "; use `s[0]` to take one character",
        (Ty::Char, Ty::Str) => "; write `std::string(1, c)` to make a string from a character",
        (Ty::StrView, Ty::Str) => "; write `std::string(sv)` to copy it",
        (f, Ty::Ptr(_)) if f.is_integral() => {
            "; only the literal `0` or `nullptr` is a null pointer"
        }
        _ => "",
    }
}

impl Checker<'_> {
    /// Copy-initialise / assign: `v` must convert to `to` (narrowing is fine outside braces).
    pub fn convert_to(&mut self, node: NodeId, v: &Val, to: &Ty, what: &str) -> bool {
        if v.is_unknown() || has_unknown(to) {
            return true;
        }
        if v.ty == Ty::Void {
            self.err(
                node,
                "type.void",
                format!("{what}: this call returns nothing (`void`), so there is no value to use"),
            );
            return false;
        }
        if v.ty == Ty::InitList {
            let list = v.list.as_deref().unwrap_or(&[]);
            return match self.list_convertible(list, to) {
                Ok(()) => true,
                Err(msg) => {
                    let code = if msg.starts_with("narrowing") {
                        "type.narrowing"
                    } else {
                        "type.mismatch"
                    };
                    self.err(
                        node,
                        code,
                        format!("{what}: the braces `{{...}}` cannot build a {to}: {msg}"),
                    );
                    false
                }
            };
        }
        if self.convertible(v, to) {
            return true;
        }
        let hint = conv_hint(&v.ty, to);
        self.err(
            node,
            "type.mismatch",
            format!("{what}: a {} cannot be turned into a {to}{hint}", show(v)),
        );
        false
    }

    /// Bind an argument or initialiser to a declared parameter or variable type (`T`, `T&`, `const T&`).
    pub fn bind_to(&mut self, node: NodeId, v: &Val, p: &ParamTy, what: &str) -> bool {
        if !p.is_ref {
            return self.convert_to(node, v, &p.ty, what);
        }
        if v.is_unknown() || has_unknown(&p.ty) {
            return true;
        }
        if p.konst {
            return self.convert_to(node, v, &p.ty, what);
        }
        if v.ty != p.ty && !(v.ty == Ty::InitList) {
            self.err(
                node,
                "type.mismatch",
                format!("{what}: a reference to {} cannot hold a {}", p.ty, show(v)),
            );
            return false;
        }
        if !v.lval {
            self.err(
                node,
                "type.temporary",
                format!("{what}: this is a temporary value, and a non-const reference cannot be bound to one; use `const {}&` or copy it into a variable first", p.ty),
            );
            return false;
        }
        if v.konst {
            self.err(
                node,
                "const.ref",
                format!("{what}: this is const, so it cannot be held by a non-const reference; use `const {}&`", p.ty),
            );
            return false;
        }
        true
    }

    /// Check `T(args)` / `T{args}` / `T x(args)`; reports and returns false when no constructor fits.
    pub fn construct(&mut self, node: NodeId, ty: &Ty, args: &[Val], brace: bool) -> bool {
        if has_unknown(ty) || args.iter().any(Val::is_unknown) {
            return true;
        }
        let ok = if brace {
            self.brace_ok(node, ty, args)
        } else {
            Some(self.paren_ok(ty, args))
        };
        match ok {
            Some(true) => true,
            Some(false) => {
                let shown: Vec<String> = args.iter().map(show).collect();
                self.err(
                    node,
                    "type.construct",
                    format!("a {ty} cannot be built from ({}); check the number and the types of the values", shown.join(", ")),
                );
                false
            }
            None => false,
        }
    }

    /// Brace construction; `None` when an error was already reported.
    fn brace_ok(&mut self, node: NodeId, ty: &Ty, args: &[Val]) -> Option<bool> {
        let braced = matches!(
            ty,
            Ty::Vector(_)
                | Ty::Deque(_)
                | Ty::List(_)
                | Ty::Set(_)
                | Ty::UnorderedSet(_)
                | Ty::Array(..)
                | Ty::Map(..)
                | Ty::UnorderedMap(..)
                | Ty::Pair(..)
                | Ty::Tuple(_)
                | Ty::Optional(_)
                | Ty::Struct(_)
        ) || ty.is_integral()
            || matches!(ty, Ty::Ptr(_));
        if args.len() == 1 && args[0].ty == *ty {
            return Some(true);
        }
        if !braced {
            return Some(self.paren_ok(ty, args));
        }
        if *ty == Ty::Str {
            return Some(self.paren_ok(ty, args));
        }
        match self.list_convertible(args, ty) {
            Ok(()) => Some(true),
            Err(msg) => {
                let code = if msg.starts_with("narrowing") {
                    "type.narrowing"
                } else {
                    "type.construct"
                };
                self.err(
                    node,
                    code,
                    format!("the braces `{{...}}` cannot build a {ty}: {msg}"),
                );
                None
            }
        }
    }

    fn paren_ok(&self, ty: &Ty, args: &[Val]) -> bool {
        let n = args.len();
        let conv = |v: &Val, t: &Ty| self.convertible(v, t);
        let same_iters = |a: &Val, b: &Val| matches!(a.ty, Ty::Iter { .. }) && a.ty == b.ty;
        let iter_elems = |a: &Val, t: &Ty| match &a.ty {
            Ty::Iter { container, .. } => elem_of(container).is_some_and(|e| conv(&Val::pr(e), t)),
            _ => false,
        };
        if n == 0 {
            return match ty {
                Ty::Struct(s) => self
                    .structs
                    .get(s)
                    .is_none_or(|d| d.ctors.is_empty() || d.ctors.iter().any(|c| c.is_empty())),
                Ty::Lambda(_) => false,
                _ => true,
            };
        }
        if n == 1 && conv(&args[0], ty) && !matches!(args[0].ty, Ty::InitList) {
            return true;
        }
        match ty {
            Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) => match n {
                1 => args[0].ty.is_integral(),
                2 => {
                    (args[0].ty.is_integral() && conv(&args[1], t))
                        || (same_iters(&args[0], &args[1]) && iter_elems(&args[0], t))
                }
                _ => false,
            },
            Ty::Str => match n {
                1 => matches!(args[0].ty, Ty::Ptr(_) | Ty::Str | Ty::StrView),
                2 => {
                    (args[0].ty.is_integral() && args[1].ty == Ty::Char)
                        || same_iters(&args[0], &args[1])
                        || (args[0].ty == Ty::Str && args[1].ty.is_integral())
                }
                3 => args[0].ty == Ty::Str && args[1].ty.is_integral() && args[2].ty.is_integral(),
                _ => false,
            },
            Ty::StrView => n == 1 && matches!(args[0].ty, Ty::Ptr(_) | Ty::Str),
            Ty::Set(t) | Ty::UnorderedSet(t) => {
                n == 2 && same_iters(&args[0], &args[1]) && iter_elems(&args[0], t)
            }
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
                n == 2
                    && same_iters(&args[0], &args[1])
                    && iter_elems(&args[0], &Ty::Pair(k.clone(), v.clone()))
            }
            Ty::PriorityQueue(t, c) => match n {
                1 => conv(&args[0], c),
                2 => same_iters(&args[0], &args[1]) && iter_elems(&args[0], t),
                3 => same_iters(&args[0], &args[1]) && iter_elems(&args[0], t) && conv(&args[2], c),
                _ => false,
            },
            Ty::Pair(a, b) => n == 2 && conv(&args[0], a) && conv(&args[1], b),
            Ty::Tuple(ts) => n == ts.len() && args.iter().zip(ts).all(|(v, t)| conv(v, t)),
            Ty::Struct(s) => self.structs.get(s).is_none_or(|d| {
                d.ctors
                    .iter()
                    .any(|c| c.len() == n && c.iter().zip(args).all(|(p, v)| self.arg_fits(v, p)))
            }),
            t if t.is_integral() || matches!(t, Ty::Ptr(_)) => n == 1 && conv(&args[0], t),
            _ => false,
        }
    }

    /// Does an argument fit a user parameter (by value, `const&`, or non-const `&` with an lvalue)?
    pub fn arg_fits(&self, v: &Val, p: &ParamTy) -> bool {
        if v.is_unknown() {
            return true;
        }
        if p.is_ref && !p.konst {
            return v.lval && !v.konst && v.ty == p.ty;
        }
        self.convertible(v, &p.ty)
    }

    /// `emplace_back(args)` and friends build the element in place: check that it can be built.
    pub fn emplace_check(&mut self, node: NodeId, recv: &Ty, args: &[Val]) {
        let target = match recv {
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => Ty::Pair(k.clone(), v.clone()),
            Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) | Ty::Set(t) | Ty::UnorderedSet(t) => {
                (**t).clone()
            }
            Ty::Stack(t) | Ty::Queue(t) | Ty::PriorityQueue(t, _) => (**t).clone(),
            _ => return,
        };
        self.construct(node, &target, args, false);
    }
}
