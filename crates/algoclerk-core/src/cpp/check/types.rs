// SPDX-License-Identifier: Apache-2.0
//! Types as spelled by the player (aliases, `decltype`, headers) and the implicit conversion rules.

use super::ctx::{Checker, ParamTy};
use super::val::*;
use crate::cpp::ast::*;

/// Rebuild a type with every direct child type mapped through `f`.
pub(super) fn map_children(t: &Ty, f: &mut dyn FnMut(&Ty) -> Ty) -> Ty {
    let mut b = |x: &Ty| Box::new(f(x));
    match t {
        Ty::Vector(x) => Ty::Vector(b(x)),
        Ty::Array(x, n) => Ty::Array(b(x), *n),
        Ty::Optional(x) => Ty::Optional(b(x)),
        Ty::UnorderedSet(x) => Ty::UnorderedSet(b(x)),
        Ty::Set(x) => Ty::Set(b(x)),
        Ty::Stack(x) => Ty::Stack(b(x)),
        Ty::Queue(x) => Ty::Queue(b(x)),
        Ty::Deque(x) => Ty::Deque(b(x)),
        Ty::List(x) => Ty::List(b(x)),
        Ty::Ptr(x) => Ty::Ptr(b(x)),
        Ty::Functor(k, x) => Ty::Functor(*k, b(x)),
        Ty::Pair(x, y) => Ty::Pair(b(x), b(y)),
        Ty::UnorderedMap(x, y) => Ty::UnorderedMap(b(x), b(y)),
        Ty::Map(x, y) => Ty::Map(b(x), b(y)),
        Ty::PriorityQueue(x, y) => Ty::PriorityQueue(b(x), b(y)),
        Ty::Tuple(xs) => Ty::Tuple(xs.iter().map(&mut *f).collect()),
        Ty::Function(xs, r) => Ty::Function(xs.iter().map(&mut *f).collect(), Box::new(f(r))),
        Ty::Iter { container, reverse } => Ty::Iter {
            container: Box::new(f(container)),
            reverse: *reverse,
        },
        t => t.clone(),
    }
}

impl Checker<'_> {
    /// Resolve aliases and `decltype(name)`, check that every library type's header is included.
    pub fn fix_ty(&mut self, t: &Ty, node: NodeId) -> Ty {
        if let Ty::Struct(n) = t {
            return self.fix_named(n, node);
        }
        if *t == Ty::Size {
            self.need_header(node, "<cstddef>", "size_t");
            return Ty::Size;
        }
        let r = match t {
            // the default comparator `std::less` is implied by `std::priority_queue<T>`: no <functional> needed
            Ty::PriorityQueue(e, c) if matches!(**c, Ty::Functor(Functor::Less, _)) => {
                Ty::PriorityQueue(
                    Box::new(self.fix_ty(e, node)),
                    Box::new(Ty::Functor(Functor::Less, Box::new(self.fix_ty(e, node)))),
                )
            }
            _ => map_children(t, &mut |c| self.fix_ty(c, node)),
        };
        if let Some(h) = r.header() {
            let name = r.to_string();
            let name = name.split('<').next().unwrap_or("").to_string();
            if self.need_header(node, h, &name) {
                self.check_locked(node, &[h.to_string(), name]);
            }
        }
        r
    }

    fn fix_named(&mut self, n: &str, node: NodeId) -> Ty {
        if let Some(rest) = n.strip_prefix("decltype(") {
            let var = rest.trim_end_matches(')');
            let found = self.scopes.iter().rev().find_map(|sc| {
                sc.iter()
                    .rev()
                    .find(|(m, _)| m == var)
                    .map(|(_, s)| s.ty.clone())
            });
            return found.unwrap_or_else(|| {
                self.err(
                    node,
                    "name.undeclared",
                    format!("`decltype({var})` names `{var}`, which is not declared yet"),
                );
                Ty::Unknown
            });
        }
        if self.structs.contains_key(n) {
            return Ty::Struct(n.to_string());
        }
        if let Some(t) = self.aliases.get(n) {
            return t.clone();
        }
        self.err(
            node,
            "name.type",
            format!("I do not know a type called `{n}`; is it declared above this line?"),
        );
        Ty::Unknown
    }

    /// A declared type with `const` and `&`.
    pub fn spec_param(&mut self, s: &TypeSpec, node: NodeId) -> ParamTy {
        let ty = if s.ty == Ty::Auto {
            Ty::Auto
        } else {
            self.fix_ty(&s.ty, node)
        };
        ParamTy {
            ty,
            konst: s.konst,
            is_ref: s.is_ref,
        }
    }

    /// Does a lambda accept arguments of these types and produce something convertible to `ret`?
    pub fn lambda_fits(&self, id: NodeId, args: &[Ty], ret: &Ty) -> bool {
        let Some(l) = self.lams.get(&id) else {
            return true;
        };
        if l.params.len() != args.len() {
            return false;
        }
        let args_ok = l
            .params
            .iter()
            .zip(args)
            .all(|(p, a)| p.ty == Ty::Auto || self.convertible(&Val::pr(a.clone()), &p.ty));
        let ret_ok = match &l.ret {
            Some(r) if *ret != Ty::Void => self.convertible(&Val::pr(r.clone()), ret),
            Some(_) => true,
            None => true,
        };
        args_ok && ret_ok
    }

    /// Implicit conversion (copy-initialisation) of `from` to `to`; narrowing is allowed.
    pub fn convertible(&self, from: &Val, to: &Ty) -> bool {
        let f = &from.ty;
        if has_unknown(f) || has_unknown(to) || f == to {
            return true;
        }
        let pr = |t: &Ty| Val::pr(t.clone());
        match (f, to) {
            (Ty::InitList, t) => self
                .list_convertible(from.list.as_deref().unwrap_or(&[]), t)
                .is_ok(),
            (a, b) if a.is_integral() && b.is_integral() => true,
            (Ty::NullPtr, Ty::Ptr(_)) => true,
            (a, Ty::Ptr(_)) if a.is_integral() && from.cval == Some(0) => true,
            (Ty::Ptr(_) | Ty::NullPtr, Ty::Bool) => true,
            (Ty::Ptr(c), Ty::Str | Ty::StrView) => **c == Ty::Char,
            (Ty::Str, Ty::StrView) => true,
            (Ty::Optional(a), Ty::Optional(b)) => self.convertible(&pr(a), b),
            (Ty::Struct(n), Ty::Optional(_)) if n == "nullopt_t" => true,
            (_, Ty::Optional(t)) => self.convertible(from, t),
            (Ty::Pair(a, b), Ty::Pair(c, d)) => {
                self.convertible(&pr(a), c) && self.convertible(&pr(b), d)
            }
            (Ty::Pair(a, b), Ty::Tuple(ts)) if ts.len() == 2 => {
                self.convertible(&pr(a), &ts[0]) && self.convertible(&pr(b), &ts[1])
            }
            (Ty::Tuple(a), Ty::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| self.convertible(&pr(x), y))
            }
            (Ty::Lambda(id), Ty::Function(args, ret)) => self.lambda_fits(*id, args, ret),
            (Ty::Function(a, r), Ty::Function(b, s)) => {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(x, y)| self.convertible(&pr(y), x))
                    && (**s == Ty::Void || self.convertible(&pr(r), s))
            }
            _ => false,
        }
    }

    /// Can a braced list initialise `to`? The `Err` says why not (a narrowing or a mismatch).
    pub fn list_convertible(&self, list: &[Val], to: &Ty) -> Result<(), String> {
        if has_unknown(to) || list.iter().any(Val::is_unknown) {
            return Ok(());
        }
        let each = |elem: &Ty, strict: bool| -> Result<(), String> {
            for v in list {
                self.elem_ok(v, elem, strict)?;
            }
            Ok(())
        };
        let n = list.len();
        match to {
            Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) | Ty::Set(t) | Ty::UnorderedSet(t) => {
                each(t, true)
            }
            Ty::Array(t, len) if n <= *len => each(t, true),
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
                let pair = Ty::Pair(k.clone(), v.clone());
                each(&pair, false)
            }
            Ty::Pair(a, b) if n == 2 => {
                self.elem_ok(&list[0], a, false)?;
                self.elem_ok(&list[1], b, false)
            }
            Ty::Tuple(ts) if ts.len() == n => {
                for (v, t) in list.iter().zip(ts) {
                    self.elem_ok(v, t, false)?;
                }
                Ok(())
            }
            Ty::Optional(t) if n <= 1 => each(t, false),
            Ty::Str if n == 1 => each(&Ty::Str, false),
            t if t.is_integral() || matches!(t, Ty::Ptr(_)) => match n {
                0 => Ok(()),
                1 => self.elem_ok(&list[0], t, true),
                _ => Err("a single value".into()),
            },
            Ty::Struct(s) => self.struct_list_ok(s, list),
            _ => Err(format!("{to} cannot be built from a braced list")),
        }
    }

    fn struct_list_ok(&self, name: &str, list: &[Val]) -> Result<(), String> {
        let Some(sd) = self.structs.get(name) else {
            return Ok(());
        };
        if !sd.ctors.is_empty() {
            let ok = sd.ctors.iter().any(|c| {
                c.len() == list.len()
                    && c.iter()
                        .zip(list)
                        .all(|(p, v)| self.elem_ok(v, &p.ty, false).is_ok())
            });
            return if ok {
                Ok(())
            } else {
                Err(format!("no constructor of {name} takes these"))
            };
        }
        if list.len() > sd.fields.len() {
            return Err(format!("{name} has only {} fields", sd.fields.len()));
        }
        for (v, (_, t, _)) in list.iter().zip(&sd.fields) {
            self.elem_ok(v, t, true)?;
        }
        Ok(())
    }

    /// One element of a braced list converting to `to`; `strict` forbids narrowing.
    fn elem_ok(&self, v: &Val, to: &Ty, strict: bool) -> Result<(), String> {
        if v.ty == Ty::InitList {
            return self.list_convertible(v.list.as_deref().unwrap_or(&[]), to);
        }
        if !self.convertible(v, to) {
            return Err(format!("{} cannot become {to}", show(v)));
        }
        if strict && narrowing(v, to) {
            return Err(format!(
                "narrowing: a {} does not always fit in a {to} inside braces",
                show(v)
            ));
        }
        Ok(())
    }

    /// Can values of this type be compared with `==` (and `<` when `ordered`)?
    pub fn comparable(&self, t: &Ty, ordered: bool) -> bool {
        match t {
            Ty::Vector(e)
            | Ty::Array(e, _)
            | Ty::Deque(e)
            | Ty::List(e)
            | Ty::Set(e)
            | Ty::Optional(e)
            | Ty::Stack(e)
            | Ty::Queue(e) => self.comparable(e, ordered),
            Ty::Map(a, b) | Ty::Pair(a, b) => {
                self.comparable(a, ordered) && self.comparable(b, ordered)
            }
            Ty::UnorderedMap(a, b) => {
                !ordered && self.comparable(a, false) && self.comparable(b, false)
            }
            Ty::UnorderedSet(e) => !ordered && self.comparable(e, false),
            Ty::Tuple(ts) => ts.iter().all(|e| self.comparable(e, ordered)),
            Ty::Iter { .. } | Ty::Str | Ty::StrView | Ty::Ptr(_) | Ty::NullPtr => true,
            t => t.is_integral() || has_unknown(t),
        }
    }

    /// Usable as a condition (`if (x)`, `!x`, `x && y`)?
    pub fn truthy(&self, t: &Ty) -> bool {
        t.is_integral() || matches!(t, Ty::Ptr(_) | Ty::NullPtr | Ty::Optional(_)) || has_unknown(t)
    }
}
