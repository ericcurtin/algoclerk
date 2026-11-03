// SPDX-License-Identifier: Apache-2.0
//! Member functions of vector, array, stack, deque, queue, list and priority_queue.

use super::expr::lg;
use super::rval::*;
use super::{Machine, R, RtKind, Rv, invalid_msg};
use crate::cpp::ast::*;

pub(super) fn is_scalar_like(t: &Ty) -> bool {
    Sk::of_ty(t).is_some() || matches!(t, Ty::Str | Ty::Ptr(_))
}

impl<'p> Machine<'p> {
    fn empty_err(&self, what: &str, name: &str) -> Box<super::RtError> {
        self.err(
            RtKind::Empty,
            format!("{name}() on an empty {what}: there is no element to look at"),
        )
    }

    fn at_err(&self, i: i128, len: usize) -> Box<super::RtError> {
        self.err(
            RtKind::Index,
            format!("at({i}) is out of range for a container of size {len}: this would throw out_of_range"),
        )
    }

    /// Position an iterator argument designates in a vector-like container.
    pub(super) fn iter_idx(&self, rp: &Place, it: &IterV, len: usize, ver: u32) -> R<usize> {
        if it.cont != *rp {
            return Err(self.err(
                RtKind::BadArg,
                "this iterator belongs to a different container",
            ));
        }
        if it.ver != ver {
            return Err(self.fail(invalid_msg()));
        }
        match it.pos {
            Pos::Idx(i) if !it.rev && i <= len => Ok(i),
            _ => Err(self.err(RtKind::Index, "this iterator is outside the container")),
        }
    }

    fn new_iter_at(&self, rp: &Place, i: usize) -> R<Value> {
        let mut it = self.make_iter(rp, false, false)?;
        it.pos = Pos::Idx(i);
        Ok(Value::Iter(Box::new(it)))
    }

    fn vec_ref(&self, rp: &Place) -> R<&VecV> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::Vector(c) => Ok(c),
            _ => Err(self.internal("not a vector")),
        }
    }

    fn vec_mut(&mut self, rp: &Place) -> R<&mut VecV> {
        let e = self.internal("not a vector");
        match self.get_mut(rp)? {
            Value::Vector(c) => Ok(c),
            _ => Err(e),
        }
    }

    pub(super) fn m_vec(&mut self, rp: Place, name: &'p str, args: &'p [Expr]) -> R<Rv> {
        let (ety, ver, len, kind) = {
            let c = self.vec_ref(&rp)?;
            (c.ety.clone(), c.ver, c.v.len(), c.kind)
        };
        let what = match kind {
            VKind::Vector => "vector",
            VKind::Array => "array",
            VKind::Stack => "stack",
        };
        let growable = kind != VKind::Array;
        match name {
            "size" => Self::ret_size(len),
            "empty" => Ok(Rv::Val(Value::Bool(len == 0))),
            "push_back" | "push" | "emplace_back" | "emplace"
                if growable && (name != "emplace" || kind == VKind::Stack) =>
            {
                let v = if matches!(name, "push_back" | "push") {
                    self.nargs(name, args, 1, 1)?;
                    self.eval_conv(&args[0], &ety)?
                } else {
                    self.construct_ty(&ety, args, false)?
                };
                self.reserve(1)?;
                let c = self.vec_mut(&rp)?;
                c.v.push(v);
                c.ver = c.ver.wrapping_add(1);
                let nver = c.ver;
                if name.starts_with("emplace") {
                    return Ok(Rv::Place(rp.with(Step::Idx(len, nver))));
                }
                Ok(Rv::Val(Value::Void))
            }
            "pop_back" | "pop" if growable => {
                if len == 0 {
                    return Err(self.empty_err(what, name));
                }
                let c = self.vec_mut(&rp)?;
                c.v.pop();
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "back" | "top" | "front" => {
                if len == 0 {
                    return Err(self.empty_err(what, name));
                }
                let i = if name == "front" { 0 } else { len - 1 };
                Ok(Rv::Place(rp.with(Step::Idx(i, ver))))
            }
            "at" => {
                self.nargs(name, args, 1, 1)?;
                let i = self.arg_int(&args[0])?;
                if i < 0 || i >= len as i128 {
                    return Err(self.at_err(i, len));
                }
                Ok(Rv::Place(rp.with(Step::Idx(i as usize, ver))))
            }
            "clear" if growable => {
                self.charge(leaves_of(self.vec_ref(&rp)?))?;
                let c = self.vec_mut(&rp)?;
                c.v.clear();
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "reserve" => {
                self.nargs(name, args, 1, 1)?;
                let n = self.arg_int(&args[0])?;
                let n = self.count_arg(n)?;
                self.reserve(n)?;
                Ok(Rv::Val(Value::Void))
            }
            "shrink_to_fit" => Ok(Rv::Val(Value::Void)),
            "swap" => {
                self.nargs(name, args, 1, 1)?;
                let other = self.eval_place(&args[0])?;
                self.swap_places(&rp, &other)?;
                Ok(Rv::Val(Value::Void))
            }
            "fill" if kind == VKind::Array => {
                self.nargs(name, args, 1, 1)?;
                let x = self.eval_conv(&args[0], &ety)?;
                self.charge(len as u64 * leaves(&x).max(1))?;
                let c = self.vec_mut(&rp)?;
                for e in &mut c.v {
                    *e = x.clone();
                }
                Ok(Rv::Val(Value::Void))
            }
            "insert" | "emplace" if growable && kind != VKind::Stack => self.vec_insert(&rp, &ety, ver, len, name, args),
            "erase" if growable => {
                self.nargs(name, args, 1, 2)?;
                let a = self.arg_iter(&args[0])?;
                let i = self.iter_idx(&rp, &a, len, ver)?;
                let j = if args.len() == 2 {
                    let b = self.arg_iter(&args[1])?;
                    self.iter_idx(&rp, &b, len, ver)?
                } else {
                    if i >= len {
                        return Err(
                            self.err(RtKind::Invalidated, "erase(end()) is undefined behaviour")
                        );
                    }
                    i + 1
                };
                if i > j {
                    return Err(self.err(RtKind::BadArg, "the erase range is backwards"));
                }
                self.charge((len - i) as u64)?;
                let c = self.vec_mut(&rp)?;
                c.v.drain(i..j);
                c.ver = c.ver.wrapping_add(1);
                self.new_iter_at(&rp, i).map(Rv::Val)
            }
            "resize" if growable => {
                self.nargs(name, args, 1, 2)?;
                let n = self.arg_int(&args[0])?;
                let n = self.count_arg(n)?;
                let x = if args.len() == 2 {
                    Some(self.eval_conv(&args[1], &ety)?)
                } else {
                    None
                };
                if n as usize > len {
                    let per = x.as_ref().map_or(1, |x| leaves(x).max(1));
                    self.reserve((n - len as u64).saturating_mul(per))?;
                    self.charge(n.saturating_mul(per))?;
                    let mut add = Vec::with_capacity(n as usize - len);
                    for _ in len..n as usize {
                        add.push(match &x {
                            Some(x) => x.clone(),
                            None => self.zero_value(&ety)?,
                        });
                    }
                    self.vec_mut(&rp)?.v.extend(add);
                } else {
                    self.charge(n)?;
                    self.vec_mut(&rp)?.v.truncate(n as usize);
                }
                let c = self.vec_mut(&rp)?;
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "assign" if growable => {
                self.nargs(name, args, 1, 2)?;
                let new = self.assign_items(&ety, args)?;
                let c = self.vec_mut(&rp)?;
                c.v = new;
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::{what} has no member function `{name}` in this shop"),
            )),
        }
    }

    /// The new contents of `assign(...)`: `n, x`, an iterator range or `{...}`.
    pub(super) fn assign_items(&mut self, ety: &TyRc, args: &'p [Expr]) -> R<Vec<Value>> {
        self.nargs("assign", args, 1, 2)?;
        let new: Vec<Value> = if args.len() == 2 {
                    let first = self.eval(&args[0])?;
                    if let Some(n) = first.as_i128() {
                        let x = self.eval_conv(&args[1], ety)?;
                        let n = self.count_arg(n)?;
                        let per = leaves(&x).max(1);
                        self.reserve(n.saturating_mul(per))?;
                        self.charge(n.saturating_mul(per))?;
                        (0..n).map(|_| x.clone()).collect()
                    } else {
                        let second = self.eval(&args[1])?;
                        let vals = self.range_values(&first, &second)?;
                        let mut out = Vec::with_capacity(vals.len());
                        for v in vals {
                            out.push(self.convert(v, ety)?);
                        }
                        out
                    }
                } else if let ExprKind::InitList(elems) = &args[0].kind {
                    let mut out = Vec::new();
                    for e in elems {
                        out.push(self.eval_conv(e, ety)?);
                    }
                    self.charge(out.len() as u64)?;
                    out
                } else {
                    return Err(self.err(RtKind::BadArg, "unsupported assign overload"));
                };
        Ok(new)
    }

    /// The values `insert(pos, ...)` / `emplace(pos, ...)` add, from the arguments after the position:
    /// one value, `{...}`, `n, x`, or an iterator range.
    pub(super) fn insert_items(
        &mut self,
        ety: &TyRc,
        name: &str,
        args: &'p [Expr],
    ) -> R<Vec<Value>> {
        Ok(if name == "emplace" {
            vec![self.construct_ty(ety, &args[1..], false)?]
        } else if args.len() == 2 {
            if let ExprKind::InitList(elems) = &args[1].kind {
                if is_scalar_like(ety) {
                    let mut out = Vec::new();
                    for e in elems {
                        out.push(self.eval_conv(e, ety)?);
                    }
                    out
                } else {
                    vec![self.eval_conv(&args[1], ety)?]
                }
            } else {
                vec![self.eval_conv(&args[1], ety)?]
            }
        } else if args.len() == 3 {
            let first = self.eval(&args[1])?;
            if let Some(n) = first.as_i128() {
                let x = self.eval_conv(&args[2], ety)?;
                let n = self.count_arg(n)?;
                let per = leaves(&x).max(1);
                self.reserve(n.saturating_mul(per))?;
                self.charge(n.saturating_mul(per))?;
                (0..n).map(|_| x.clone()).collect()
            } else {
                let second = self.eval(&args[2])?;
                let vals = self.range_values(&first, &second)?;
                let mut out = Vec::with_capacity(vals.len());
                for v in vals {
                    out.push(self.convert(v, ety)?);
                }
                out
            }
        } else {
            return Err(self.err(RtKind::BadArg, "unsupported insert overload"));
        })
    }

    fn vec_insert(
        &mut self,
        rp: &Place,
        ety: &TyRc,
        ver: u32,
        len: usize,
        name: &'p str,
        args: &'p [Expr],
    ) -> R<Rv> {
        self.nargs(name, args, 2, 100)?;
        let a = self.arg_iter(&args[0])?;
        let i = self.iter_idx(rp, &a, len, ver)?;
        let items = self.insert_items(ety, name, args)?;
        self.reserve(items.len() as u64)?;
        self.charge((len - i) as u64)?;
        let c = self.vec_mut(rp)?;
        c.v.splice(i..i, items);
        c.ver = c.ver.wrapping_add(1);
        let nver = c.ver;
        if name == "emplace" {
            let mut it = self.make_iter(rp, false, false)?;
            it.pos = Pos::Idx(i);
            it.ver = nver;
            return Ok(Rv::Val(Value::Iter(Box::new(it))));
        }
        let mut it = self.make_iter(rp, false, false)?;
        it.pos = Pos::Idx(i);
        Ok(Rv::Val(Value::Iter(Box::new(it))))
    }

    // ----- deque and queue -----

    fn dq_ref(&self, rp: &Place) -> R<&DqV> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::Deque(c) => Ok(c),
            _ => Err(self.internal("not a deque")),
        }
    }

    fn dq_mut(&mut self, rp: &Place) -> R<&mut DqV> {
        let e = self.internal("not a deque");
        match self.get_mut(rp)? {
            Value::Deque(c) => Ok(c),
            _ => Err(e),
        }
    }

    pub(super) fn m_deque(&mut self, rp: Place, name: &'p str, args: &'p [Expr]) -> R<Rv> {
        let (ety, ver, len, kind) = {
            let c = self.dq_ref(&rp)?;
            (c.ety.clone(), c.ver, c.v.len(), c.kind)
        };
        let queue = kind == DKind::Queue;
        let what = if queue { "queue" } else { "deque" };
        let name: &str = if queue {
            match name {
                "push" => "push_back",
                "emplace" => "emplace_back",
                "pop" => "pop_front",
                n => n,
            }
        } else {
            name
        };
        match name {
            "size" => Self::ret_size(len),
            "empty" => Ok(Rv::Val(Value::Bool(len == 0))),
            "push_back" | "emplace_back" | "push_front" | "emplace_front" => {
                let front = name.ends_with("front");
                let v = if name.starts_with("push") {
                    self.nargs(name, args, 1, 1)?;
                    self.eval_conv(&args[0], &ety)?
                } else {
                    self.construct_ty(&ety, args, false)?
                };
                self.reserve(1)?;
                let c = self.dq_mut(&rp)?;
                if front {
                    c.v.push_front(v);
                } else {
                    c.v.push_back(v);
                }
                c.ver = c.ver.wrapping_add(1);
                let nver = c.ver;
                if name.starts_with("emplace") {
                    let i = if front { 0 } else { len };
                    return Ok(Rv::Place(rp.with(Step::Idx(i, nver))));
                }
                Ok(Rv::Val(Value::Void))
            }
            "pop_back" | "pop_front" => {
                if len == 0 {
                    return Err(self.empty_err(what, name));
                }
                let c = self.dq_mut(&rp)?;
                if name == "pop_back" {
                    c.v.pop_back();
                } else {
                    c.v.pop_front();
                }
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "front" | "back" => {
                if len == 0 {
                    return Err(self.empty_err(what, name));
                }
                let i = if name == "front" { 0 } else { len - 1 };
                Ok(Rv::Place(rp.with(Step::Idx(i, ver))))
            }
            "at" if !queue => {
                self.nargs(name, args, 1, 1)?;
                let i = self.arg_int(&args[0])?;
                if i < 0 || i >= len as i128 {
                    return Err(self.at_err(i, len));
                }
                Ok(Rv::Place(rp.with(Step::Idx(i as usize, ver))))
            }
            "clear" if !queue => {
                self.charge(len as u64)?;
                let c = self.dq_mut(&rp)?;
                c.v.clear();
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "swap" => {
                let other = self.eval_place(&args[0])?;
                self.swap_places(&rp, &other)?;
                Ok(Rv::Val(Value::Void))
            }
            "resize" if !queue => {
                let n = self.arg_int(&args[0])?;
                let n = self.count_arg(n)?;
                self.reserve(n)?;
                self.charge(n)?;
                let x = if args.len() == 2 {
                    Some(self.eval_conv(&args[1], &ety)?)
                } else {
                    None
                };
                if n as usize > len {
                    let mut add = Vec::new();
                    for _ in len..n as usize {
                        add.push(match &x {
                            Some(x) => x.clone(),
                            None => self.zero_value(&ety)?,
                        });
                    }
                    self.dq_mut(&rp)?.v.extend(add);
                } else {
                    self.dq_mut(&rp)?.v.truncate(n as usize);
                }
                let c = self.dq_mut(&rp)?;
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "insert" | "emplace" if !queue => {
                self.nargs(name, args, 2, 100)?;
                let a = self.arg_iter(&args[0])?;
                let i = self.iter_idx(&rp, &a, len, ver)?;
                let items = self.insert_items(&ety, name, args)?;
                self.reserve(items.len() as u64)?;
                self.charge(i.min(len - i.min(len)) as u64 + items.len() as u64)?;
                let c = self.dq_mut(&rp)?;
                let tail = c.v.split_off(i);
                c.v.extend(items);
                c.v.extend(tail);
                c.ver = c.ver.wrapping_add(1);
                self.new_iter_at(&rp, i).map(Rv::Val)
            }
            "assign" if !queue => {
                let new = self.assign_items(&ety, args)?;
                let c = self.dq_mut(&rp)?;
                c.v = new.into();
                c.ver = c.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "erase" if !queue => {
                self.nargs(name, args, 1, 2)?;
                let a = self.arg_iter(&args[0])?;
                let i = self.iter_idx(&rp, &a, len, ver)?;
                let j = if args.len() == 2 {
                    let b = self.arg_iter(&args[1])?;
                    self.iter_idx(&rp, &b, len, ver)?
                } else {
                    if i >= len {
                        return Err(
                            self.err(RtKind::Invalidated, "erase(end()) is undefined behaviour")
                        );
                    }
                    i + 1
                };
                if i > j {
                    return Err(self.err(RtKind::BadArg, "the erase range is backwards"));
                }
                self.charge(i.min(len.saturating_sub(i)) as u64)?;
                let c = self.dq_mut(&rp)?;
                c.v.drain(i..j);
                c.ver = c.ver.wrapping_add(1);
                self.new_iter_at(&rp, i).map(Rv::Val)
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::{what} has no member function `{name}` in this shop"),
            )),
        }
    }

    // ----- list -----

    fn list_ref(&self, rp: &Place) -> R<&ListV> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::List(l) => Ok(l),
            _ => Err(self.internal("not a list")),
        }
    }

    fn list_mut(&mut self, rp: &Place) -> R<&mut ListV> {
        let e = self.internal("not a list");
        match self.get_mut(rp)? {
            Value::List(l) => Ok(l),
            _ => Err(e),
        }
    }

    fn list_iter_at(&self, rp: &Place, n: u32) -> R<Value> {
        let mut it = self.make_iter(rp, true, false)?;
        it.pos = if n == NIL { Pos::End } else { Pos::Node(n) };
        Ok(Value::Iter(Box::new(it)))
    }

    fn list_node_arg(&mut self, rp: &Place, a: &'p Expr) -> R<u32> {
        let it = self.arg_iter(a)?;
        if it.cont != *rp {
            return Err(self.err(RtKind::BadArg, "this iterator belongs to a different list"));
        }
        match it.pos {
            Pos::End => Ok(NIL),
            Pos::Node(n) => {
                if self
                    .list_ref(rp)?
                    .nodes
                    .get(n as usize)
                    .is_none_or(|x| x.val.is_none())
                {
                    return Err(self.fail(invalid_msg()));
                }
                Ok(n)
            }
            _ => Err(self.err(RtKind::BadArg, "bad list iterator")),
        }
    }

    pub(super) fn m_list(&mut self, rp: Place, name: &'p str, args: &'p [Expr]) -> R<Rv> {
        let (ety, len) = {
            let l = self.list_ref(&rp)?;
            (l.ety.clone(), l.len)
        };
        match name {
            "size" => Self::ret_size(len),
            "empty" => Ok(Rv::Val(Value::Bool(len == 0))),
            "push_back" | "emplace_back" | "push_front" | "emplace_front" => {
                let v = if name.starts_with("push") {
                    self.nargs(name, args, 1, 1)?;
                    self.eval_conv(&args[0], &ety)?
                } else {
                    self.construct_ty(&ety, args, false)?
                };
                self.reserve(1)?;
                let l = self.list_mut(&rp)?;
                if name.ends_with("front") {
                    let h = l.head;
                    l.insert_before(h, v);
                } else {
                    l.insert_before(NIL, v);
                }
                Ok(Rv::Val(Value::Void))
            }
            "pop_back" | "pop_front" => {
                if len == 0 {
                    return Err(self.empty_err("list", name));
                }
                let l = self.list_mut(&rp)?;
                let id = if name == "pop_back" { l.tail } else { l.head };
                l.remove(id);
                Ok(Rv::Val(Value::Void))
            }
            "front" | "back" => {
                if len == 0 {
                    return Err(self.empty_err("list", name));
                }
                let l = self.list_ref(&rp)?;
                let id = if name == "front" { l.head } else { l.tail };
                let gn = l.nodes[id as usize].gn;
                Ok(Rv::Place(rp.with(Step::Node(id, gn))))
            }
            "clear" => {
                self.charge(len as u64)?;
                let l = self.list_mut(&rp)?;
                *l = ListV::new(l.ety.clone());
                Ok(Rv::Val(Value::Void))
            }
            "insert" | "emplace" => {
                self.nargs(name, args, 2, 100)?;
                let at = self.list_node_arg(&rp, &args[0])?;
                let items = self.insert_items(&ety, name, args)?;
                self.reserve(items.len() as u64)?;
                self.charge(items.len() as u64)?;
                let mut first = at;
                for (k, x) in items.into_iter().enumerate() {
                    let id = self.list_mut(&rp)?.insert_before(at, x);
                    if k == 0 {
                        first = id;
                    }
                }
                self.list_iter_at(&rp, first).map(Rv::Val)
            }
            "assign" => {
                let new = self.assign_items(&ety, args)?;
                let l = self.list_mut(&rp)?;
                *l = ListV::new(l.ety.clone());
                for x in new {
                    l.insert_before(NIL, x);
                }
                Ok(Rv::Val(Value::Void))
            }
            "erase" => {
                self.nargs(name, args, 1, 2)?;
                let a = self.list_node_arg(&rp, &args[0])?;
                if a == NIL {
                    return Err(
                        self.err(RtKind::Invalidated, "erase(end()) is undefined behaviour")
                    );
                }
                let stop = if args.len() == 2 {
                    self.list_node_arg(&rp, &args[1])?
                } else {
                    self.list_ref(&rp)?.nodes[a as usize].next
                };
                let mut cur = a;
                let mut guard = 0u64;
                while cur != stop {
                    if cur == NIL {
                        return Err(self.err(RtKind::BadArg, "the erase range is invalid"));
                    }
                    let nxt = self.list_ref(&rp)?.nodes[cur as usize].next;
                    self.list_mut(&rp)?.remove(cur);
                    cur = nxt;
                    guard += 1;
                    self.charge(0)?;
                    if guard > 1 << 40 {
                        break;
                    }
                }
                self.charge(guard.saturating_sub(1))?;
                self.list_iter_at(&rp, stop).map(Rv::Val)
            }
            "reverse" => {
                self.charge(len as u64)?;
                let l = self.list_mut(&rp)?;
                let mut cur = l.head;
                while cur != NIL {
                    let n = &mut l.nodes[cur as usize];
                    std::mem::swap(&mut n.prev, &mut n.next);
                    cur = n.prev;
                }
                std::mem::swap(&mut l.head, &mut l.tail);
                Ok(Rv::Val(Value::Void))
            }
            "sort" => {
                self.nargs(name, args, 0, 1)?;
                let cmp = match args.first() {
                    Some(a) => Some(self.eval(a)?),
                    None => None,
                };
                let order = self.list_ref(&rp)?.order();
                let vals: Vec<Value> = order
                    .iter()
                    .filter_map(|&i| self.list_ref(&rp).ok()?.nodes[i as usize].val.clone())
                    .collect();
                let sorted = self.sort_owned(vals, cmp.as_ref())?;
                let l = self.list_mut(&rp)?;
                for (id, v) in order.into_iter().zip(sorted) {
                    l.nodes[id as usize].val = Some(v);
                }
                Ok(Rv::Val(Value::Void))
            }
            "remove" | "remove_if" | "unique" => {
                self.nargs(name, args, 0, 1)?;
                let arg = match args.first() {
                    Some(a) if name == "remove" => Some(self.eval_conv(a, &ety)?),
                    Some(a) => Some(self.eval(a)?),
                    None => None,
                };
                self.charge(len as u64)?;
                let order = self.list_ref(&rp)?.order();
                let mut removed = 0usize;
                let mut prev: Option<u32> = None;
                for id in order {
                    let kill = {
                        let cur = self.list_ref(&rp)?.nodes[id as usize]
                            .val
                            .clone()
                            .unwrap_or(Value::Void);
                        match name {
                            "remove" => arg.as_ref().is_some_and(|x| eq_val(&cur, x)),
                            "remove_if" => {
                                let f = arg.clone().unwrap_or(Value::Void);
                                let r = self.call_value(&f, vec![Rv::Val(cur)])?;
                                let r = self.rv_val(r)?;
                                self.truthy(&r)?
                            }
                            _ => match prev {
                                Some(p) => {
                                    let pv = self.list_ref(&rp)?.nodes[p as usize]
                                        .val
                                        .clone()
                                        .unwrap_or(Value::Void);
                                    match &arg {
                                        Some(f) => {
                                            let r = self
                                                .call_value(f, vec![Rv::Val(pv), Rv::Val(cur)])?;
                                            let r = self.rv_val(r)?;
                                            self.truthy(&r)?
                                        }
                                        None => eq_val(&pv, &cur),
                                    }
                                }
                                None => false,
                            },
                        }
                    };
                    if kill {
                        self.list_mut(&rp)?.remove(id);
                        removed += 1;
                    } else {
                        prev = Some(id);
                    }
                }
                let _ = removed;
                Ok(Rv::Val(Value::Void))
            }
            "swap" => {
                let other = self.eval_place(&args[0])?;
                self.swap_places(&rp, &other)?;
                Ok(Rv::Val(Value::Void))
            }
            "resize" => {
                self.nargs(name, args, 1, 2)?;
                let n = self.arg_int(&args[0])?;
                let n = self.count_arg(n)?;
                self.reserve(n)?;
                self.charge(n)?;
                let x = if args.len() == 2 {
                    Some(self.eval_conv(&args[1], &ety)?)
                } else {
                    None
                };
                while self.list_ref(&rp)?.len > n as usize {
                    let t = self.list_ref(&rp)?.tail;
                    self.list_mut(&rp)?.remove(t);
                }
                while self.list_ref(&rp)?.len < n as usize {
                    let v = match &x {
                        Some(x) => x.clone(),
                        None => self.zero_value(&ety)?,
                    };
                    self.list_mut(&rp)?.insert_before(NIL, v);
                }
                Ok(Rv::Val(Value::Void))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::list has no member function `{name}` in this shop"),
            )),
        }
    }

    // ----- priority_queue -----

    fn pq_ref(&self, rp: &Place) -> R<&PqV> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::Pq(p) => Ok(p),
            _ => Err(self.internal("not a priority_queue")),
        }
    }

    fn pq_mut(&mut self, rp: &Place) -> R<&mut PqV> {
        let e = self.internal("not a priority_queue");
        match self.get_mut(rp)? {
            Value::Pq(p) => Ok(p),
            _ => Err(e),
        }
    }

    /// `comp(heap[i], heap[j])`
    fn pq_less(&mut self, rp: &Place, i: usize, j: usize) -> R<bool> {
        let (cmp, ver) = {
            let p = self.pq_ref(rp)?;
            (p.cmp.clone(), p.ver)
        };
        match cmp {
            PqCmp::Less => {
                let p = self.pq_ref(rp)?;
                Ok(cmp_val(&p.v[i], &p.v[j]) == std::cmp::Ordering::Less)
            }
            PqCmp::Greater => {
                let p = self.pq_ref(rp)?;
                Ok(cmp_val(&p.v[i], &p.v[j]) == std::cmp::Ordering::Greater)
            }
            PqCmp::Fn(f) => {
                let a = rp.with(Step::Idx(i, ver));
                let b = rp.with(Step::Idx(j, ver));
                let r = self.call_value(&f, vec![Rv::Place(a), Rv::Place(b)])?;
                let r = self.rv_val(r)?;
                self.truthy(&r)
            }
        }
    }

    /// `comp(heap[i], value)` with a value held outside the heap.
    fn pq_less_val(&mut self, rp: &Place, i: usize, val: &Value) -> R<bool> {
        let (cmp, ver) = {
            let p = self.pq_ref(rp)?;
            (p.cmp.clone(), p.ver)
        };
        match cmp {
            PqCmp::Less => {
                let p = self.pq_ref(rp)?;
                Ok(cmp_val(&p.v[i], val) == std::cmp::Ordering::Less)
            }
            PqCmp::Greater => {
                let p = self.pq_ref(rp)?;
                Ok(cmp_val(&p.v[i], val) == std::cmp::Ordering::Greater)
            }
            PqCmp::Fn(f) => {
                let a = rp.with(Step::Idx(i, ver));
                let t = self.temp(val.clone());
                let r = self.call_value(&f, vec![Rv::Place(a), Rv::Place(t)])?;
                let r = self.rv_val(r)?;
                self.truthy(&r)
            }
        }
    }

    /// `std::push_heap` as in libstdc++.
    pub(super) fn pq_push(&mut self, rp: &Place, v: Value) -> R<()> {
        self.reserve(1)?;
        let n = {
            let p = self.pq_mut(rp)?;
            p.v.push(Value::Void);
            p.v.len()
        };
        let mut hole = n - 1;
        // sift up
        while hole > 0 {
            let parent = (hole - 1) / 2;
            if self.pq_less_val(rp, parent, &v)? {
                let p = self.pq_mut(rp)?;
                p.v[hole] = std::mem::replace(&mut p.v[parent], Value::Void);
                hole = parent;
            } else {
                break;
            }
        }
        self.pq_mut(rp)?.v[hole] = v;
        Ok(())
    }

    /// libstdc++ `__adjust_heap(first, hole, len, value)`.
    fn pq_adjust(&mut self, rp: &Place, mut hole: usize, len: usize, v: Value) -> R<()> {
        let top = hole;
        let mut child = hole;
        while child < len.saturating_sub(1) / 2 && len >= 2 {
            child = 2 * (child + 1);
            if self.pq_less(rp, child, child - 1)? {
                child -= 1;
            }
            let p = self.pq_mut(rp)?;
            p.v[hole] = std::mem::replace(&mut p.v[child], Value::Void);
            hole = child;
        }
        if len.is_multiple_of(2) && len >= 2 && child == (len - 2) / 2 {
            child = 2 * (child + 1);
            let p = self.pq_mut(rp)?;
            p.v[hole] = std::mem::replace(&mut p.v[child - 1], Value::Void);
            hole = child - 1;
        }
        // push_heap from the hole up to `top`
        while hole > top {
            let parent = (hole - 1) / 2;
            if self.pq_less_val(rp, parent, &v)? {
                let p = self.pq_mut(rp)?;
                p.v[hole] = std::mem::replace(&mut p.v[parent], Value::Void);
                hole = parent;
            } else {
                break;
            }
        }
        self.pq_mut(rp)?.v[hole] = v;
        Ok(())
    }

    fn pq_pop(&mut self, rp: &Place) -> R<()> {
        let len = self.pq_ref(rp)?.v.len();
        if len == 1 {
            self.pq_mut(rp)?.v.pop();
            return Ok(());
        }
        // pop_heap: take the last as the value, the old top goes to the last slot, which is then dropped
        let last = self.pq_mut(rp)?.v.pop().unwrap_or(Value::Void);
        let n = len - 1;
        self.pq_adjust(rp, 0, n, last)?;
        Ok(())
    }

    /// `std::make_heap` over the whole queue.
    pub(super) fn pq_make_heap(&mut self, rp: &Place) -> R<()> {
        let len = self.pq_ref(rp)?.v.len();
        if len < 2 {
            return Ok(());
        }
        let mut parent = (len - 2) / 2;
        loop {
            let v = std::mem::replace(&mut self.pq_mut(rp)?.v[parent], Value::Void);
            self.pq_adjust(rp, parent, len, v)?;
            if parent == 0 {
                break;
            }
            parent -= 1;
        }
        Ok(())
    }

    pub(super) fn m_pq(&mut self, rp: Place, name: &'p str, args: &'p [Expr]) -> R<Rv> {
        let (ety, ver, len) = {
            let p = self.pq_ref(&rp)?;
            (p.ety.clone(), p.ver, p.v.len())
        };
        match name {
            "size" => Self::ret_size(len),
            "empty" => Ok(Rv::Val(Value::Bool(len == 0))),
            "top" => {
                if len == 0 {
                    return Err(self.empty_err("priority_queue", name));
                }
                Ok(Rv::Place(rp.with(Step::Idx(0, ver))))
            }
            "push" | "emplace" => {
                let v = if name == "push" {
                    self.nargs(name, args, 1, 1)?;
                    self.eval_conv(&args[0], &ety)?
                } else {
                    self.construct_ty(&ety, args, false)?
                };
                self.charge(lg(len as u64 + 1))?;
                self.pq_push(&rp, v)?;
                let p = self.pq_mut(&rp)?;
                p.ver = p.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "pop" => {
                if len == 0 {
                    return Err(self.empty_err("priority_queue", name));
                }
                self.charge(lg(len as u64 + 1))?;
                self.pq_pop(&rp)?;
                let p = self.pq_mut(&rp)?;
                p.ver = p.ver.wrapping_add(1);
                Ok(Rv::Val(Value::Void))
            }
            "swap" => {
                let other = self.eval_place(&args[0])?;
                self.swap_places(&rp, &other)?;
                Ok(Rv::Val(Value::Void))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::priority_queue has no member function `{name}`"),
            )),
        }
    }
}

fn leaves_of(c: &VecV) -> u64 {
    let t: &Ty = &c.ety;
    if is_scalar_like(t) && !matches!(t, Ty::Str) {
        c.v.len() as u64
    } else {
        c.v.iter().map(leaves).sum()
    }
}
