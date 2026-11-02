// SPDX-License-Identifier: Apache-2.0
//! Member functions of map, set, unordered_map and unordered_set.

use super::expr::lg;
use super::rval::*;
use super::{Machine, R, RtKind, Rv};
use crate::cpp::ast::*;
use std::ops::Bound;

impl<'p> Machine<'p> {
    fn ord_ref(&self, rp: &Place) -> R<&OrdV> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::Map(m) | Value::Set(m) => Ok(m),
            _ => Err(self.internal("not an ordered container")),
        }
    }

    fn ord_mut(&mut self, rp: &Place) -> R<&mut OrdV> {
        let e = self.internal("not an ordered container");
        match self.get_mut(rp)? {
            Value::Map(m) | Value::Set(m) => Ok(m),
            _ => Err(e),
        }
    }

    fn un_ref(&self, rp: &Place) -> R<&UnV> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::UMap(m) | Value::USet(m) => Ok(m),
            _ => Err(self.internal("not an unordered container")),
        }
    }

    fn un_mut(&mut self, rp: &Place) -> R<&mut UnV> {
        let e = self.internal("not an unordered container");
        match self.get_mut(rp)? {
            Value::UMap(m) | Value::USet(m) => Ok(m),
            _ => Err(e),
        }
    }

    fn ord_iter(&self, rp: &Place, k: Option<Key>) -> Value {
        Value::Iter(Box::new(IterV {
            cont: rp.clone(),
            pos: k.map_or(Pos::End, Pos::Key),
            rev: false,
            ver: 0,
        }))
    }

    fn un_iter(&self, rp: &Place, ent: Option<usize>) -> R<Value> {
        let ver = self.un_ref(rp)?.ver;
        Ok(Value::Iter(Box::new(IterV {
            cont: rp.clone(),
            pos: ent.map_or(Pos::End, Pos::Ent),
            rev: false,
            ver,
        })))
    }

    /// The value part of a map insertion argument list as a stored pair (maps) or element (sets).
    fn assoc_entry(
        &mut self,
        is_map: bool,
        kty: &Ty,
        vty: &Option<TyRc>,
        args: &'p [Expr],
        construct: bool,
    ) -> R<Value> {
        if !is_map {
            return if construct {
                self.construct_ty(kty, args, false)
            } else {
                self.eval_conv(&args[0], kty)
            };
        }
        let vt = vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone());
        let pt = Ty::Pair(Box::new(kty.clone()), Box::new(vt.clone()));
        if args.len() == 2 && construct {
            let k = self.eval_conv(&args[0], kty)?;
            let v = self.eval_conv(&args[1], &vt)?;
            return Ok(Value::pair(k, v));
        }
        if args.len() == 1 {
            return self.eval_conv(&args[0], &pt);
        }
        Err(self.err(RtKind::BadArg, "unsupported map insertion"))
    }

    pub(super) fn m_ord(
        &mut self,
        rp: Place,
        is_map: bool,
        name: &'p str,
        args: &'p [Expr],
    ) -> R<Rv> {
        let (kty, vty, n) = {
            let m = self.ord_ref(&rp)?;
            (m.kty.clone(), m.vty.clone(), m.m.len())
        };
        let what = if is_map { "map" } else { "set" };
        match name {
            "size" => Self::ret_size(n),
            "empty" => Ok(Rv::Val(Value::Bool(n == 0))),
            "clear" => {
                self.charge(n as u64)?;
                self.ord_mut(&rp)?.m.clear();
                Ok(Rv::Val(Value::Void))
            }
            "find" | "count" | "lower_bound" | "upper_bound" | "equal_range" => {
                self.nargs(name, args, 1, 1)?;
                let k = Key(self.eval_conv(&args[0], &kty)?);
                self.charge(lg(n as u64))?;
                let m = self.ord_ref(&rp)?;
                match name {
                    "find" => {
                        let r = m.m.contains_key(&k).then(|| k.clone());
                        Ok(Rv::Val(self.ord_iter(&rp, r)))
                    }
                    "count" => Ok(Rv::Val(Value::Size(u64::from(m.m.contains_key(&k))))),
                    "lower_bound" => {
                        let r = m.m.range(k..).next().map(|(k, _)| k.clone());
                        Ok(Rv::Val(self.ord_iter(&rp, r)))
                    }
                    "upper_bound" => {
                        let r =
                            m.m.range((Bound::Excluded(k), Bound::Unbounded))
                                .next()
                                .map(|(k, _)| k.clone());
                        Ok(Rv::Val(self.ord_iter(&rp, r)))
                    }
                    _ => {
                        let lo = m.m.range(k.clone()..).next().map(|(k, _)| k.clone());
                        let hi =
                            m.m.range((Bound::Excluded(k), Bound::Unbounded))
                                .next()
                                .map(|(k, _)| k.clone());
                        let (a, b) = (self.ord_iter(&rp, lo), self.ord_iter(&rp, hi));
                        Ok(Rv::Val(Value::pair(a, b)))
                    }
                }
            }
            "at" if is_map => {
                self.nargs(name, args, 1, 1)?;
                let k = Key(self.eval_conv(&args[0], &kty)?);
                self.charge(lg(n as u64))?;
                if !self.ord_ref(&rp)?.m.contains_key(&k) {
                    return Err(self.err(
                        RtKind::Index,
                        "at(): this key is not in the map (this would throw out_of_range)",
                    ));
                }
                Ok(Rv::Place(rp.with(Step::Key(k)).with(Step::Field(1))))
            }
            "insert" | "emplace" | "try_emplace" | "insert_or_assign" | "emplace_hint" => {
                self.charge(lg(n as u64))?;
                // insert(first, last) / insert(hint, value)
                let mut args = args;
                if name == "insert" && args.len() == 2 {
                    let a = self.eval(&args[0])?;
                    if matches!(a, Value::Iter(_)) {
                        let b = self.eval(&args[1])?;
                        if matches!(b, Value::Iter(_)) {
                            let vals = self.range_values(&a, &b)?;
                            for v in vals {
                                let e = self.convert_entry(is_map, &kty, &vty, v)?;
                                self.ord_put(&rp, e, false)?;
                            }
                            return Ok(Rv::Val(Value::Void));
                        }
                        args = &args[1..];
                    }
                }
                if name == "emplace_hint" {
                    args = &args[1..];
                }
                if let (
                    true,
                    [
                        Expr {
                            kind: ExprKind::InitList(items),
                            ..
                        },
                    ],
                ) = (name == "insert", args)
                {
                    // insert({a, b, c}) for sets, insert({{k,v},{k,v}}) for maps
                    let all_lists = is_map
                        && items
                            .iter()
                            .all(|i| matches!(i.kind, ExprKind::InitList(_)));
                    if (!is_map && super::seqs::is_scalar_like(&kty)) || all_lists {
                        let mut ents = Vec::new();
                        for it in items {
                            let e = if is_map {
                                let vt = vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone());
                                self.eval_conv(
                                    it,
                                    &Ty::Pair(Box::new((*kty).clone()), Box::new(vt)),
                                )?
                            } else {
                                self.eval_conv(it, &kty)?
                            };
                            ents.push(e);
                        }
                        for e in ents {
                            self.ord_put(&rp, e, false)?;
                        }
                        return Ok(Rv::Val(Value::Void));
                    }
                }
                match name {
                    "try_emplace" | "insert_or_assign" => {
                        let k = self.eval_conv(&args[0], &kty)?;
                        let key = Key(k.clone());
                        let exists = self.ord_ref(&rp)?.m.contains_key(&key);
                        let vt = vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone());
                        if exists && name == "try_emplace" {
                            let it = self.ord_iter(&rp, Some(key));
                            return Ok(Rv::Val(Value::pair(it, Value::Bool(false))));
                        }
                        let v = if name == "insert_or_assign" {
                            self.eval_conv(&args[1], &vt)?
                        } else {
                            self.construct_ty(&vt, &args[1..], false)?
                        };
                        if exists {
                            self.store(&rp.with(Step::Key(key.clone())).with(Step::Field(1)), v)
                                .map_err(|f| self.fail(f))?;
                            let it = self.ord_iter(&rp, Some(key));
                            return Ok(Rv::Val(Value::pair(it, Value::Bool(false))));
                        }
                        let (it, _) = self.ord_put(&rp, Value::pair(k, v), true)?;
                        Ok(Rv::Val(Value::pair(it, Value::Bool(true))))
                    }
                    _ => {
                        let e = self.assoc_entry(is_map, &kty, &vty, args, name != "insert")?;
                        let (it, ok) = self.ord_put(&rp, e, true)?;
                        Ok(Rv::Val(Value::pair(it, Value::Bool(ok))))
                    }
                }
            }
            "erase" => {
                self.nargs(name, args, 1, 2)?;
                self.charge(lg(n as u64))?;
                if args.len() == 2 {
                    let a = self.arg_iter(&args[0])?;
                    let b = self.arg_iter(&args[1])?;
                    let mut cur = a;
                    let mut count = 0u64;
                    while cur.pos != b.pos {
                        let Pos::Key(k) = &cur.pos else {
                            return Err(self.err(RtKind::BadArg, "the erase range is invalid"));
                        };
                        let k = k.clone();
                        let nxt = self.iter_advance(&cur, 1)?;
                        self.ord_mut(&rp)?.m.remove(&k);
                        cur = nxt;
                        count += 1;
                    }
                    self.charge(count)?;
                    return Ok(Rv::Val(Value::Iter(Box::new(cur))));
                }
                let first = self.eval(&args[0])?;
                if let Value::Iter(it) = first {
                    let Pos::Key(k) = &it.pos else {
                        return Err(
                            self.err(RtKind::Invalidated, "erase(end()) is undefined behaviour")
                        );
                    };
                    if !self.ord_ref(&rp)?.m.contains_key(k) {
                        return Err(self.err(
                            RtKind::Invalidated,
                            "this iterator points at an erased element",
                        ));
                    }
                    let k = k.clone();
                    let nxt = self.iter_advance(&it, 1)?;
                    self.ord_mut(&rp)?.m.remove(&k);
                    return Ok(Rv::Val(Value::Iter(Box::new(nxt))));
                }
                let k = Key(self.convert(first, &kty)?);
                let removed = self.ord_mut(&rp)?.m.remove(&k).is_some();
                Ok(Rv::Val(Value::Size(u64::from(removed))))
            }
            "swap" => {
                let other = self.eval_place(&args[0])?;
                self.swap_places(&rp, &other)?;
                Ok(Rv::Val(Value::Void))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::{what} has no member function `{name}` in this shop"),
            )),
        }
    }

    fn convert_entry(
        &mut self,
        is_map: bool,
        kty: &TyRc,
        vty: &Option<TyRc>,
        v: Value,
    ) -> R<Value> {
        if is_map {
            let vt = vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone());
            self.convert(v, &Ty::Pair(Box::new((**kty).clone()), Box::new(vt)))
        } else {
            self.convert(v, kty)
        }
    }

    /// Insert an entry if the key is absent; returns the iterator and whether it was inserted.
    fn ord_put(&mut self, rp: &Place, e: Value, _want: bool) -> R<(Value, bool)> {
        let key = match &e {
            Value::Pair(p) if matches!(self.get(rp).map_err(|f| self.fail(f))?, Value::Map(_)) => {
                Key(p[0].clone())
            }
            v => Key(v.clone()),
        };
        let exists = self.ord_ref(rp)?.m.contains_key(&key);
        if !exists {
            self.reserve(1)?;
            let is_set = matches!(self.get(rp).map_err(|f| self.fail(f))?, Value::Set(_));
            let stored = if is_set { Value::Void } else { e };
            self.ord_mut(rp)?.m.insert(key.clone(), stored);
        }
        Ok((self.ord_iter(rp, Some(key)), !exists))
    }

    // ----- unordered -----

    fn un_put(&mut self, rp: &Place, e: Value) -> R<(Value, bool)> {
        let is_map = matches!(self.get(rp).map_err(|f| self.fail(f))?, Value::UMap(_));
        let key = match &e {
            Value::Pair(p) if is_map => Key(p[0].clone()),
            v => Key(v.clone()),
        };
        if let Some(&i) = self.un_ref(rp)?.idx.get(&key) {
            return Ok((self.un_iter(rp, Some(i))?, false));
        }
        self.reserve(1)?;
        let u = self.un_mut(rp)?;
        u.push(key, e);
        let i = u.ents.len() - 1;
        Ok((self.un_iter(rp, Some(i))?, true))
    }

    pub(super) fn m_unord(
        &mut self,
        rp: Place,
        is_map: bool,
        name: &'p str,
        args: &'p [Expr],
    ) -> R<Rv> {
        let (kty, vty, n) = {
            let m = self.un_ref(&rp)?;
            (m.kty.clone(), m.vty.clone(), m.live)
        };
        let what = if is_map {
            "unordered_map"
        } else {
            "unordered_set"
        };
        match name {
            "size" => Self::ret_size(n),
            "empty" => Ok(Rv::Val(Value::Bool(n == 0))),
            "clear" => {
                self.charge(n as u64)?;
                let u = self.un_mut(&rp)?;
                u.idx.clear();
                u.ents.clear();
                u.live = 0;
                u.ver = u.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "reserve" | "rehash" => {
                let c = self.arg_int(&args[0])?;
                let c = self.count_arg(c)?;
                self.reserve(c / 4)?;
                let u = self.un_mut(&rp)?;
                u.ver = u.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "find" | "count" => {
                self.nargs(name, args, 1, 1)?;
                let k = Key(self.eval_conv(&args[0], &kty)?);
                let r = self.un_ref(&rp)?.idx.get(&k).copied();
                if name == "count" {
                    return Ok(Rv::Val(Value::Size(u64::from(r.is_some()))));
                }
                Ok(Rv::Val(self.un_iter(&rp, r)?))
            }
            "at" if is_map => {
                self.nargs(name, args, 1, 1)?;
                let k = Key(self.eval_conv(&args[0], &kty)?);
                if !self.un_ref(&rp)?.idx.contains_key(&k) {
                    return Err(self.err(
                        RtKind::Index,
                        "at(): this key is not in the map (this would throw out_of_range)",
                    ));
                }
                Ok(Rv::Place(rp.with(Step::Key(k)).with(Step::Field(1))))
            }
            "insert" | "emplace" | "try_emplace" | "insert_or_assign" | "emplace_hint" => {
                let mut args = args;
                if name == "insert" && args.len() == 2 {
                    let a = self.eval(&args[0])?;
                    if let Value::Iter(_) = a {
                        let b = self.eval(&args[1])?;
                        if matches!(b, Value::Iter(_)) {
                            let vals = self.range_values(&a, &b)?;
                            for v in vals {
                                let e = self.convert_entry_un(is_map, &kty, &vty, v)?;
                                self.un_put(&rp, e)?;
                            }
                            return Ok(Rv::Val(Value::Void));
                        }
                        args = &args[1..];
                    }
                }
                if name == "emplace_hint" {
                    args = &args[1..];
                }
                if let (
                    true,
                    [
                        Expr {
                            kind: ExprKind::InitList(items),
                            ..
                        },
                    ],
                ) = (name == "insert", args)
                {
                    let all_lists = is_map
                        && items
                            .iter()
                            .all(|i| matches!(i.kind, ExprKind::InitList(_)));
                    if (!is_map && super::seqs::is_scalar_like(&kty)) || all_lists {
                        let mut ents = Vec::new();
                        for it in items {
                            let e = if is_map {
                                let vt = vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone());
                                self.eval_conv(
                                    it,
                                    &Ty::Pair(Box::new((*kty).clone()), Box::new(vt)),
                                )?
                            } else {
                                self.eval_conv(it, &kty)?
                            };
                            ents.push(e);
                        }
                        for e in ents {
                            self.un_put(&rp, e)?;
                        }
                        return Ok(Rv::Val(Value::Void));
                    }
                }
                match name {
                    "try_emplace" | "insert_or_assign" => {
                        let k = self.eval_conv(&args[0], &kty)?;
                        let key = Key(k.clone());
                        let existing = self.un_ref(&rp)?.idx.get(&key).copied();
                        let vt = vty.as_ref().map_or(Ty::Unknown, |t| (**t).clone());
                        if let (Some(i), "try_emplace") = (existing, name) {
                            return Ok(Rv::Val(Value::pair(
                                self.un_iter(&rp, Some(i))?,
                                Value::Bool(false),
                            )));
                        }
                        let v = if name == "insert_or_assign" {
                            self.eval_conv(&args[1], &vt)?
                        } else {
                            self.construct_ty(&vt, &args[1..], false)?
                        };
                        if let Some(i) = existing {
                            self.store(&rp.with(Step::Key(key)).with(Step::Field(1)), v)
                                .map_err(|f| self.fail(f))?;
                            return Ok(Rv::Val(Value::pair(
                                self.un_iter(&rp, Some(i))?,
                                Value::Bool(false),
                            )));
                        }
                        let (it, _) = self.un_put(&rp, Value::pair(k, v))?;
                        Ok(Rv::Val(Value::pair(it, Value::Bool(true))))
                    }
                    _ => {
                        let e = self.assoc_entry(is_map, &kty, &vty, args, name != "insert")?;
                        let (it, ok) = self.un_put(&rp, e)?;
                        Ok(Rv::Val(Value::pair(it, Value::Bool(ok))))
                    }
                }
            }
            "erase" => {
                self.nargs(name, args, 1, 2)?;
                if args.len() == 2 {
                    let a = self.arg_iter(&args[0])?;
                    let b = self.arg_iter(&args[1])?;
                    let mut cur = a;
                    let mut count = 0u64;
                    while cur.pos != b.pos {
                        let Pos::Ent(i) = cur.pos else {
                            return Err(self.err(RtKind::BadArg, "the erase range is invalid"));
                        };
                        let nxt = self.iter_advance(&cur, 1)?;
                        let key = {
                            let u = self.un_ref(&rp)?;
                            u.ents.get(i).and_then(Option::as_ref).map(|e| u.key_of(e))
                        };
                        if let Some(k) = key {
                            self.un_mut(&rp)?.remove(&k);
                        }
                        cur = nxt;
                        count += 1;
                    }
                    self.charge(count)?;
                    return Ok(Rv::Val(Value::Iter(Box::new(cur))));
                }
                let first = self.eval(&args[0])?;
                if let Value::Iter(it) = first {
                    let Pos::Ent(i) = it.pos else {
                        return Err(
                            self.err(RtKind::Invalidated, "erase(end()) is undefined behaviour")
                        );
                    };
                    let key = {
                        let u = self.un_ref(&rp)?;
                        if u.ver != it.ver {
                            return Err(self.fail(super::invalid_msg()));
                        }
                        u.ents.get(i).and_then(Option::as_ref).map(|e| u.key_of(e))
                    };
                    let Some(k) = key else {
                        return Err(self.fail(super::invalid_msg()));
                    };
                    let nxt = self.iter_advance(&it, 1)?;
                    self.un_mut(&rp)?.remove(&k);
                    return Ok(Rv::Val(Value::Iter(Box::new(nxt))));
                }
                let k = Key(self.convert(first, &kty)?);
                let removed = self.un_mut(&rp)?.remove(&k).is_some();
                Ok(Rv::Val(Value::Size(u64::from(removed))))
            }
            "swap" => {
                let other = self.eval_place(&args[0])?;
                self.swap_places(&rp, &other)?;
                Ok(Rv::Val(Value::Void))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::{what} has no member function `{name}` in this shop"),
            )),
        }
    }

    fn convert_entry_un(
        &mut self,
        is_map: bool,
        kty: &TyRc,
        vty: &Option<TyRc>,
        v: Value,
    ) -> R<Value> {
        self.convert_entry(is_map, kty, vty, v)
    }
}
