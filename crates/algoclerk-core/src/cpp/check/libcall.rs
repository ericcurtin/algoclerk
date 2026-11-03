// SPDX-License-Identifier: Apache-2.0
//! Matching call arguments against a library signature: template deduction with C++ rules (a deduced
//! parameter must agree exactly, so `std::max(int, long long)` fails; a parameter fixed by the receiver
//! only needs an implicit conversion).

use super::ctx::Checker;
use super::sigpat::{PParam, PT, Sig, parse_sig};
use super::val::*;
use crate::cpp::ast::*;
use crate::cpp::stdlib::Member;
use std::collections::HashMap;

/// Why a signature did not accept the arguments: a diagnostic code and a friendly message.
#[derive(Clone, Debug)]
pub(super) struct Fail {
    pub code: &'static str,
    pub msg: String,
}

fn fail<T>(code: &'static str, msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail {
        code,
        msg: msg.into(),
    })
}

/// Template bindings of one match attempt.
#[derive(Clone, Debug, Default)]
pub(super) struct Bind {
    /// Fixed by the receiver (`T` of `vector<T>`): arguments only need to convert.
    pub fixed: HashMap<String, Ty>,
    /// Fixed packs (`A...` of `function<R(A...)>`).
    pub packs: HashMap<String, Vec<Ty>>,
    /// Deduced from arguments: later uses must agree exactly.
    pub deduced: HashMap<String, Val>,
    /// Packs collected from arguments.
    pub got: HashMap<String, Vec<Ty>>,
    /// Arguments whose type matched the parameter exactly (overload ranking).
    pub exact: usize,
}

/// Element type an iterator over `container` yields.
pub(super) fn elem_of(container: &Ty) -> Option<Ty> {
    Some(match container {
        Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) | Ty::Array(t, _) => (**t).clone(),
        Ty::Set(t) | Ty::UnorderedSet(t) => (**t).clone(),
        Ty::Str | Ty::StrView => Ty::Char,
        Ty::Map(k, v) | Ty::UnorderedMap(k, v) => Ty::Pair(k.clone(), v.clone()),
        _ => return None,
    })
}

/// Do iterators of this container support `+`, `-` and `<`?
pub(super) fn random_access(container: &Ty) -> bool {
    matches!(
        container,
        Ty::Vector(_) | Ty::Array(..) | Ty::Deque(_) | Ty::Str | Ty::StrView
    )
}

/// `*it` for an iterator or pointer value: an lvalue of the element (const for set elements and
/// const iterators; a map element's `first` is const).
pub(super) fn deref_val(it: &Val) -> Option<Val> {
    match &it.ty {
        Ty::Iter { container, .. } => {
            let e = elem_of(container)?;
            let set = matches!(**container, Ty::Set(_) | Ty::UnorderedSet(_) | Ty::StrView);
            let map = matches!(**container, Ty::Map(..) | Ty::UnorderedMap(..));
            Some(Val::lv(e, it.ic || set).with_ic(map))
        }
        Ty::Ptr(t) => Some(Val::lv((**t).clone(), it.ic)),
        _ => None,
    }
}

/// Bindings fixed by the receiver's type.
pub(super) fn bind_recv(t: &Ty) -> Bind {
    let mut b = Bind::default();
    let mut put = |k: &str, v: &Ty| {
        b.fixed.insert(k.to_string(), v.clone());
    };
    match t {
        Ty::Vector(x) | Ty::Deque(x) | Ty::List(x) | Ty::Set(x) | Ty::UnorderedSet(x) => {
            put("T", x)
        }
        Ty::Stack(x) | Ty::Queue(x) | Ty::Optional(x) | Ty::Array(x, _) => put("T", x),
        Ty::PriorityQueue(x, c) => {
            put("T", x);
            put("Cmp", c);
        }
        Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
            put("K", k);
            put("V", v);
        }
        Ty::Pair(a, c) => {
            put("A", a);
            put("B", c);
        }
        Ty::Function(args, r) => {
            put("R", r);
            b.packs.insert("A".into(), args.clone());
        }
        Ty::Tuple(ts) => {
            b.packs.insert("T".into(), ts.clone());
        }
        Ty::Iter { container, .. } => {
            if let Some(e) = elem_of(container) {
                put("T", &e);
            }
        }
        _ => {}
    }
    b
}

/// Names for which `std::string` / `std::string_view` accept a single `char` as well.
const STR_CHAR_OK: &[&str] = &[
    "find",
    "rfind",
    "find_first_of",
    "find_last_of",
    "find_first_not_of",
    "find_last_not_of",
    "operator+=",
];

impl Checker<'_> {
    /// The concrete type a pattern stands for under the bindings (`None` when not known yet).
    pub fn subst(&self, pt: &PT, b: &Bind, recv: Option<&Val>) -> Option<Ty> {
        Some(match pt {
            PT::Tpl(n) => b
                .fixed
                .get(n)
                .cloned()
                .or_else(|| b.deduced.get(n).map(|v| v.ty.clone()))?,
            PT::Ty(t) => t.clone(),
            PT::SelfTy => recv?.ty.clone(),
            PT::Pair(x, y) => Ty::Pair(
                Box::new(self.subst(x, b, recv)?),
                Box::new(self.subst(y, b, recv)?),
            ),
            PT::RecvIter { reverse, .. } => Ty::Iter {
                container: Box::new(recv?.ty.clone()),
                reverse: *reverse,
            },
            PT::Diff => Ty::LongLong,
            PT::Iter(n) => b.deduced.get(n)?.ty.clone(),
            PT::Ptr(x) => Ty::Ptr(Box::new(self.subst(x, b, recv)?)),
            PT::Nullopt => Ty::Struct("nullopt_t".into()),
            _ => return None,
        })
    }

    /// Match one argument against one parameter pattern.
    fn match_param(
        &self,
        b: &mut Bind,
        p: &PParam,
        a: &Val,
        recv: Option<&Val>,
        m: &Member,
    ) -> Result<(), Fail> {
        if a.is_unknown() {
            return Ok(());
        }
        // a C array decays to a pointer where an iterator is wanted (`std::sort(a, a + n)`)
        let decayed;
        let a = match (&a.ty, &p.pt) {
            (Ty::Array(t, _), PT::Iter(_)) => {
                decayed = Val::pr(Ty::Ptr(t.clone())).with_ic(a.konst);
                &decayed
            }
            _ => a,
        };
        if p.is_ref && !p.konst && !matches!(p.pt, PT::Pack(_)) {
            if !a.lval {
                return fail(
                    "type.temporary",
                    "it needs a variable it can change, but this is a temporary value",
                );
            }
            if a.konst {
                return fail(
                    "const.ref",
                    "it changes its argument, but this one is const",
                );
            }
        }
        let name = m.name;
        match &p.pt {
            PT::Tpl(n) => {
                if let Some(t) = b.fixed.get(n) {
                    if !self.convertible(a, t) {
                        return fail(
                            "type.mismatch",
                            format!("a {} cannot be turned into a {t}", show(a)),
                        );
                    }
                    b.exact += usize::from(a.ty == *t);
                } else if let Some(d) = b.deduced.get(n) {
                    if d.ty != a.ty {
                        return fail(
                            "type.deduce",
                            format!(
                                "it wants both arguments to have one and the same type, but they are {} and {}; cast one of them",
                                show(d),
                                show(a)
                            ),
                        );
                    }
                } else if a.ty == Ty::InitList {
                    return fail(
                        "type.deduce",
                        "it cannot work out a type from a braced list `{...}`",
                    );
                } else {
                    b.deduced
                        .insert(n.clone(), Val::pr(a.ty.clone()).with_ic(a.ic));
                    b.exact += 1;
                }
            }
            PT::Iter(n) => {
                if !matches!(a.ty, Ty::Iter { .. } | Ty::Ptr(_)) {
                    return fail(
                        "type.mismatch",
                        format!("it wants an iterator here, not a {}", show(a)),
                    );
                }
                if let (true, Ty::Iter { container, .. }) = (n.starts_with("Random"), &a.ty)
                    && !random_access(container)
                {
                    return fail(
                        "type.mismatch",
                        format!(
                            "it needs random-access iterators, and {container} does not have them"
                        ),
                    );
                }
                if let Some(d) = b.deduced.get(n) {
                    if d.ty != a.ty {
                        return fail(
                            "type.deduce",
                            "the two iterators must come from the same kind of container",
                        );
                    }
                } else {
                    b.deduced
                        .insert(n.clone(), Val::pr(a.ty.clone()).with_ic(a.ic));
                }
                b.exact += 1;
            }
            PT::Callable(n) => {
                if !matches!(a.ty, Ty::Lambda(_) | Ty::Function(..) | Ty::Functor(..)) {
                    return fail(
                        "type.mismatch",
                        format!("it wants a function or lambda here, not a {}", show(a)),
                    );
                }
                b.deduced.insert(n.clone(), a.clone());
            }
            PT::IList(x) => self.match_ilist(b, x, a)?,
            PT::Pack(n) => {
                if let Some(fixed) = b.packs.get(n) {
                    let i = b.got.get(n).map_or(0, Vec::len);
                    match fixed.get(i) {
                        Some(t) if self.convertible(a, t) => {}
                        Some(t) => {
                            return fail(
                                "type.mismatch",
                                format!("argument {} must be a {t}, not a {}", i + 1, show(a)),
                            );
                        }
                        None => return fail("type.arity", "too many arguments"),
                    }
                }
                b.got.entry(n.clone()).or_default().push(a.ty.clone());
            }
            PT::Pair(x, y) => {
                if let Some(t) = self.subst(&p.pt, b, recv) {
                    if !self.convertible(a, &t) {
                        return fail(
                            "type.mismatch",
                            format!("a {} cannot be turned into a {t}", show(a)),
                        );
                    }
                } else if let (Ty::Pair(l, r), PT::Tpl(xn), PT::Tpl(yn)) = (&a.ty, &**x, &**y) {
                    b.deduced.insert(xn.clone(), Val::pr((**l).clone()));
                    b.deduced.insert(yn.clone(), Val::pr((**r).clone()));
                } else {
                    return fail("type.mismatch", "it wants a std::pair here");
                }
            }
            PT::Tuple => {
                if let Ty::Tuple(ts) = &a.ty {
                    b.packs.insert("T".into(), ts.clone());
                } else if !matches!(a.ty, Ty::Pair(..)) {
                    return fail("type.mismatch", "it wants a std::tuple");
                }
            }
            PT::Ty(t) => {
                let char_ok = a.ty == Ty::Char
                    && STR_CHAR_OK.contains(&name)
                    && matches!(t, Ty::Str | Ty::StrView);
                if p.is_ref && !p.konst && a.ty != *t {
                    return fail(
                        "type.mismatch",
                        format!("it needs a {t} variable, not a {}", show(a)),
                    );
                }
                if !char_ok && !self.convertible(a, t) {
                    return fail(
                        "type.mismatch",
                        format!("a {} cannot be turned into a {t}", show(a)),
                    );
                }
                b.exact += usize::from(a.ty == *t);
            }
            pt @ (PT::SelfTy | PT::RecvIter { .. } | PT::Diff) => {
                let Some(t) = self.subst(pt, b, recv) else {
                    return Ok(());
                };
                if !self.convertible(a, &t) {
                    return fail(
                        "type.mismatch",
                        format!("a {} cannot be turned into a {t}", show(a)),
                    );
                }
                b.exact += usize::from(a.ty == t);
            }
            _ => {}
        }
        Ok(())
    }

    fn match_ilist(&self, b: &mut Bind, x: &PT, a: &Val) -> Result<(), Fail> {
        let (PT::Tpl(n), Some(items)) = (x, &a.list) else {
            return fail("type.mismatch", "it wants a braced list `{a, b, c}` here");
        };
        for it in items {
            match b.deduced.get(n) {
                Some(d) if d.ty != it.ty => {
                    return fail(
                        "type.deduce",
                        format!(
                            "the values in the braces must all have the same type, but they are {} and {}",
                            show(d),
                            show(it)
                        ),
                    );
                }
                Some(_) => {}
                None => {
                    b.deduced.insert(n.clone(), Val::pr(it.ty.clone()));
                }
            }
        }
        Ok(())
    }

    /// Try one signature: the matched return value or why not.
    pub fn try_sig(
        &self,
        m: &Member,
        recv: Option<&Val>,
        args: &[Val],
        explicit: &[Ty],
    ) -> Result<(Val, Bind), Fail> {
        let sig: Sig = parse_sig(m.sig);
        let mut b = recv.map_or_else(Bind::default, |r| bind_recv(&r.ty));
        // explicit template arguments (`std::max<long long>(a, b)`) fix the parameters in order
        let mut names: Vec<String> = Vec::new();
        for p in &sig.params {
            let mut add = |pt: &PT| {
                if let PT::Tpl(n) = pt
                    && !names.contains(n)
                {
                    names.push(n.clone());
                }
            };
            match &p.pt {
                PT::Pair(x, y) => {
                    add(x);
                    add(y);
                }
                pt => add(pt),
            }
        }
        for (n, t) in names.iter().zip(explicit) {
            b.fixed.insert(n.clone(), t.clone());
        }
        let has_pack = sig.params.iter().any(|p| matches!(p.pt, PT::Pack(_)));
        let plain = sig
            .params
            .iter()
            .filter(|p| !matches!(p.pt, PT::Pack(_)))
            .count();
        let required = sig
            .params
            .iter()
            .filter(|p| !p.has_default && !matches!(p.pt, PT::Pack(_)))
            .count();
        if args.len() < required || (!has_pack && args.len() > plain) {
            let want = if required == plain {
                plain.to_string()
            } else {
                format!("{required} to {plain}")
            };
            return fail(
                "type.arity",
                format!("it takes {want} argument(s) but you gave {}", args.len()),
            );
        }
        for (i, a) in args.iter().enumerate() {
            let Some(p) = sig
                .params
                .get(i)
                .or_else(|| sig.params.last().filter(|p| matches!(p.pt, PT::Pack(_))))
            else {
                break;
            };
            self.match_param(&mut b, p, a, recv, m)?;
        }
        let exact = b.exact;
        let ret = self.ret_val(&sig, &b, recv);
        b.exact = exact;
        Ok((ret, b))
    }

    /// The value a successful match produces.
    fn ret_val(&self, sig: &Sig, b: &Bind, recv: Option<&Val>) -> Val {
        let rk = recv.is_some_and(|r| r.konst);
        let ty = match &sig.ret {
            PT::Void => Ty::Void,
            PT::Tuple => Ty::Tuple(b.got.get("T").cloned().unwrap_or_default()),
            PT::Any | PT::Pack(_) | PT::IList(_) | PT::Callable(_) => Ty::Unknown,
            pt => self.subst(pt, b, recv).unwrap_or(Ty::Unknown),
        };
        let ic = match &sig.ret {
            PT::RecvIter { konst, .. } => *konst || rk,
            PT::Iter(n) => b.deduced.get(n).is_some_and(|d| d.ic),
            _ => false,
        };
        if sig.ret_ref {
            let mut v = Val::lv(ty, sig.ret_konst || (rk && !sig.konst_method));
            v.konst |= sig.ret_konst;
            return v.with_ic(false);
        }
        Val::pr(ty).with_ic(ic)
    }
}
