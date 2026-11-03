// SPDX-License-Identifier: Apache-2.0
//! Iterators: creation, dereference, arithmetic, comparison, and range-for iteration.

use super::rval::*;
use super::{Machine, R, RtKind, invalid_msg};
use crate::cpp::ast::BinOp;
use crate::cpp::interp::UnorderedOrder;
use std::ops::Bound;

/// How a container is walked.
#[derive(Clone, Copy, Debug)]
pub(super) enum CKind {
    Seq,
    Ord,
    Un,
    List,
}

/// State of a range-for loop.
pub(super) enum RangeIt {
    Seq {
        i: usize,
        ver: u32,
    },
    Keys {
        keys: Vec<Key>,
        i: usize,
    },
    Ents {
        i: Option<usize>,
        ver: u32,
        back: bool,
        prev: Option<usize>,
    },
    Nodes {
        cur: u32,
        prev: u32,
    },
}

/// A resolved range of a random access container.
#[derive(Clone, Debug)]
pub(super) struct SeqRange {
    pub cont: Place,
    pub lo: usize,
    pub hi: usize,
    pub rev: bool,
    pub ver: u32,
    /// Iterator position of the first element (`Pos::Idx` payload).
    pub p0: usize,
}

impl<'p> Machine<'p> {
    fn ckind(&self, cont: &Place) -> R<(CKind, usize, u32)> {
        let v = self.get(cont).map_err(|f| self.fail(f))?;
        Ok(match v {
            Value::Vector(c) if c.kind != VKind::Stack => (CKind::Seq, c.v.len(), c.ver),
            Value::Deque(c) if c.kind == DKind::Deque => (CKind::Seq, c.v.len(), c.ver),
            Value::Str(s) => (CKind::Seq, s.s.len(), s.ver),
            Value::View(b) => (CKind::Seq, b.len(), 0),
            Value::Map(m) | Value::Set(m) => (CKind::Ord, m.m.len(), 0),
            Value::UMap(u) | Value::USet(u) => (CKind::Un, u.live, u.ver),
            Value::List(l) => (CKind::List, l.len, 0),
            _ => {
                return Err(self.err(
                    RtKind::BadArg,
                    "this container has no iterators (stack, queue and priority_queue cannot be walked)",
                ));
            }
        })
    }

    fn un_back(&self) -> bool {
        self.lim.unordered == UnorderedOrder::Reverse
    }

    /// `begin()`, `end()`, `rbegin()`, `rend()`.
    pub(super) fn make_iter(&self, cont: &Place, end: bool, rev: bool) -> R<IterV> {
        let (k, len, ver) = self.ckind(cont)?;
        let pos = match k {
            CKind::Seq => Pos::Idx(if end { len } else { 0 }),
            CKind::Ord => {
                if end || len == 0 {
                    Pos::End
                } else {
                    let m = match self.get(cont).map_err(|f| self.fail(f))? {
                        Value::Map(m) | Value::Set(m) => m,
                        _ => return Err(self.internal("not ordered")),
                    };
                    let key = if rev {
                        m.m.keys().next_back()
                    } else {
                        m.m.keys().next()
                    };
                    key.cloned().map_or(Pos::End, Pos::Key)
                }
            }
            CKind::Un => {
                if end || len == 0 || rev {
                    Pos::End
                } else {
                    let u = match self.get(cont).map_err(|f| self.fail(f))? {
                        Value::UMap(u) | Value::USet(u) => u,
                        _ => return Err(self.internal("not unordered")),
                    };
                    let back = self.un_back();
                    let start = if back {
                        u.ents.len().saturating_sub(1)
                    } else {
                        0
                    };
                    u.next_live(start, back).map_or(Pos::End, Pos::Ent)
                }
            }
            CKind::List => {
                if end {
                    Pos::End
                } else {
                    match self.get(cont).map_err(|f| self.fail(f))? {
                        Value::List(l) => {
                            let n = if rev { l.tail } else { l.head };
                            if n == NIL { Pos::End } else { Pos::Node(n) }
                        }
                        _ => return Err(self.internal("not a list")),
                    }
                }
            }
        };
        Ok(IterV {
            cont: cont.clone(),
            pos,
            rev,
            ver,
        })
    }

    fn check_ver(&self, it: &IterV) -> R<(CKind, usize)> {
        let (k, len, ver) = self.ckind(&it.cont)?;
        if matches!(k, CKind::Seq | CKind::Un) && ver != it.ver {
            return Err(self.fail(invalid_msg()));
        }
        Ok((k, len))
    }

    /// The element an iterator points at.
    pub(super) fn iter_deref(&self, it: &IterV) -> R<Place> {
        let (k, len) = self.check_ver(it)?;
        let end_err = || {
            self.err(
                RtKind::Empty,
                "this iterator is at end(): there is no element to read there",
            )
        };
        match (&it.pos, k) {
            (Pos::Idx(i), CKind::Seq) => {
                if *i >= len {
                    return Err(end_err());
                }
                let idx = if it.rev { len - 1 - i } else { *i };
                Ok(it.cont.with(Step::Idx(idx, it.ver)))
            }
            (Pos::Key(key), CKind::Ord) => Ok(it.cont.with(Step::Key(key.clone()))),
            (Pos::Ent(i), CKind::Un) => match self.get(&it.cont).map_err(|f| self.fail(f))? {
                Value::UMap(u) | Value::USet(u) => match u.ents.get(*i).and_then(Option::as_ref) {
                    Some(e) => Ok(it.cont.with(Step::Key(u.key_of(e)))),
                    None => Err(self.fail(invalid_msg())),
                },
                _ => Err(self.internal("not unordered")),
            },
            (Pos::Node(n), CKind::List) => match self.get(&it.cont).map_err(|f| self.fail(f))? {
                Value::List(l) => match l.nodes.get(*n as usize) {
                    Some(nd) if nd.val.is_some() => Ok(it.cont.with(Step::Node(*n, nd.gn))),
                    _ => Err(self.fail(invalid_msg())),
                },
                _ => Err(self.internal("not a list")),
            },
            (Pos::End, _) => Err(end_err()),
            _ => Err(self.internal("iterator does not match its container")),
        }
    }

    /// Move an iterator by `n` steps (negative moves backwards).
    pub(super) fn iter_advance(&self, it: &IterV, n: i64) -> R<IterV> {
        let (k, len) = self.check_ver(it)?;
        let mut out = it.clone();
        match (&it.pos, k) {
            (Pos::Idx(i), CKind::Seq) => {
                let j = *i as i128 + i128::from(n);
                if j < 0 || j > len as i128 {
                    return Err(self.err(
                        RtKind::Index,
                        "this iterator moved outside the container (before begin() or past end())",
                    ));
                }
                out.pos = Pos::Idx(j as usize);
            }
            (_, CKind::Seq) => return Err(self.internal("bad iterator position")),
            (_, CKind::Ord) => {
                let m = match self.get(&it.cont).map_err(|f| self.fail(f))? {
                    Value::Map(m) | Value::Set(m) => m,
                    _ => return Err(self.internal("not ordered")),
                };
                if let Pos::Key(k) = &it.pos
                    && !m.m.contains_key(k)
                {
                    // the element this iterator pointed at was erased: moving it is undefined behaviour
                    return Err(self.fail(invalid_msg()));
                }
                let mut pos = it.pos.clone();
                let mut steps = n;
                while steps != 0 {
                    let logical_fwd = steps > 0;
                    let phys_fwd = logical_fwd != it.rev;
                    pos = match (&pos, phys_fwd) {
                        (Pos::Key(k), true) => {
                            m.m.range((Bound::Excluded(k.clone()), Bound::Unbounded))
                                .next()
                                .map_or(Pos::End, |(k, _)| Pos::Key(k.clone()))
                        }
                        (Pos::Key(k), false) => match m.m.range(..k.clone()).next_back() {
                            Some((k, _)) => Pos::Key(k.clone()),
                            None if it.rev && logical_fwd => Pos::End,
                            None => {
                                return Err(self.err(
                                    RtKind::Index,
                                    "this iterator moved before begin(): undefined behaviour",
                                ));
                            }
                        },
                        (Pos::End, false) if !it.rev => match m.m.keys().next_back() {
                            Some(k) => Pos::Key(k.clone()),
                            None => {
                                return Err(self.err(
                                    RtKind::Index,
                                    "decrementing begin() of an empty container",
                                ));
                            }
                        },
                        (Pos::End, true) if it.rev => match m.m.keys().next() {
                            Some(k) => Pos::Key(k.clone()),
                            None => {
                                return Err(self.err(
                                    RtKind::Index,
                                    "decrementing rend() of an empty container",
                                ));
                            }
                        },
                        _ => {
                            return Err(self.err(
                                RtKind::Index,
                                "this iterator moved past end(): undefined behaviour",
                            ));
                        }
                    };
                    steps += if steps > 0 { -1 } else { 1 };
                }
                out.pos = pos;
            }
            (Pos::Ent(i), CKind::Un) => {
                if n != 1 {
                    return Err(self.err(
                        RtKind::BadArg,
                        "unordered iterators only move forward by one",
                    ));
                }
                let u = match self.get(&it.cont).map_err(|f| self.fail(f))? {
                    Value::UMap(u) | Value::USet(u) => u,
                    _ => return Err(self.internal("not unordered")),
                };
                if u.ents.get(*i).and_then(Option::as_ref).is_none() {
                    return Err(self.fail(invalid_msg()));
                }
                let back = self.un_back();
                let nxt = if back {
                    if *i == 0 {
                        None
                    } else {
                        u.next_live(*i - 1, true)
                    }
                } else {
                    u.next_live(*i + 1, false)
                };
                out.pos = nxt.map_or(Pos::End, Pos::Ent);
            }
            (Pos::End, CKind::Un) => {
                return Err(self.err(RtKind::Index, "this iterator is already at end()"));
            }
            (_, CKind::List) => {
                let l = match self.get(&it.cont).map_err(|f| self.fail(f))? {
                    Value::List(l) => l,
                    _ => return Err(self.internal("not a list")),
                };
                if let Pos::Node(x) = &it.pos
                    && !l.nodes.get(*x as usize).is_some_and(|nd| nd.val.is_some())
                {
                    return Err(self.fail(invalid_msg()));
                }
                let mut pos = it.pos.clone();
                let mut steps = n;
                while steps != 0 {
                    let logical_fwd = steps > 0;
                    let phys_fwd = logical_fwd != it.rev;
                    pos = match (&pos, phys_fwd) {
                        (Pos::Node(x), true) => {
                            let nx = l.nodes[*x as usize].next;
                            if nx == NIL { Pos::End } else { Pos::Node(nx) }
                        }
                        (Pos::Node(x), false) => {
                            let pv = l.nodes[*x as usize].prev;
                            if pv != NIL {
                                Pos::Node(pv)
                            } else if it.rev && logical_fwd {
                                Pos::End
                            } else {
                                return Err(
                                    self.err(RtKind::Index, "moved before begin() of a list")
                                );
                            }
                        }
                        (Pos::End, false) if !it.rev && l.tail != NIL => Pos::Node(l.tail),
                        (Pos::End, true) if it.rev && l.head != NIL => Pos::Node(l.head),
                        _ => return Err(self.err(RtKind::Index, "moved past end() of a list")),
                    };
                    steps += if steps > 0 { -1 } else { 1 };
                }
                out.pos = pos;
            }
            _ => return Err(self.internal("bad iterator")),
        }
        Ok(out)
    }

    /// Operators with an iterator operand.
    pub(super) fn iter_binary(&self, op: BinOp, x: &Value, y: &Value) -> R<Value> {
        match (op, x, y) {
            (BinOp::Add | BinOp::Sub, Value::Iter(it), n) if n.as_i128().is_some() => {
                let n = n.as_i128().unwrap_or(0) as i64;
                let n = if op == BinOp::Sub { n.saturating_neg() } else { n };
                Ok(Value::Iter(Box::new(self.iter_advance(it, n)?)))
            }
            (BinOp::Add, n, Value::Iter(it)) if n.as_i128().is_some() => {
                let n = n.as_i128().unwrap_or(0) as i64;
                Ok(Value::Iter(Box::new(self.iter_advance(it, n)?)))
            }
            (BinOp::Sub, Value::Iter(a), Value::Iter(b)) => {
                self.check_ver(a)?;
                self.check_ver(b)?;
                match (&a.pos, &b.pos) {
                    (Pos::Idx(i), Pos::Idx(j)) => Ok(Value::LL(*i as i64 - *j as i64)),
                    _ => Err(self.err(
                        RtKind::BadArg,
                        "only random access iterators can be subtracted",
                    )),
                }
            }
            (BinOp::Eq | BinOp::Ne, Value::Iter(a), Value::Iter(b)) => {
                self.check_ver(a)?;
                self.check_ver(b)?;
                let eq = a.pos == b.pos;
                Ok(Value::Bool(if op == BinOp::Eq { eq } else { !eq }))
            }
            (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Value::Iter(a), Value::Iter(b)) => {
                self.check_ver(a)?;
                self.check_ver(b)?;
                let (Pos::Idx(i), Pos::Idx(j)) = (&a.pos, &b.pos) else {
                    return Err(self.err(
                        RtKind::BadArg,
                        "only random access iterators can be ordered",
                    ));
                };
                Ok(Value::Bool(match op {
                    BinOp::Lt => i < j,
                    BinOp::Le => i <= j,
                    BinOp::Gt => i > j,
                    _ => i >= j,
                }))
            }
            _ => Err(self.err(RtKind::BadArg, "this operator does not work on iterators")),
        }
    }

    // ----- range-for -----

    pub(super) fn range_iter(&mut self, cont: &Place) -> R<RangeIt> {
        let (k, len, ver) = self.ckind(cont)?;
        Ok(match k {
            CKind::Seq => RangeIt::Seq { i: 0, ver },
            CKind::Ord => {
                self.charge(len as u64)?;
                let keys: Vec<Key> = match self.get(cont).map_err(|f| self.fail(f))? {
                    Value::Map(m) | Value::Set(m) => m.m.keys().cloned().collect(),
                    _ => Vec::new(),
                };
                RangeIt::Keys { keys, i: 0 }
            }
            CKind::Un => {
                let back = self.un_back();
                let start = match self.get(cont).map_err(|f| self.fail(f))? {
                    Value::UMap(u) | Value::USet(u) => {
                        let s = if back {
                            u.ents.len().saturating_sub(1)
                        } else {
                            0
                        };
                        u.next_live(s, back)
                    }
                    _ => None,
                };
                RangeIt::Ents {
                    i: start,
                    ver,
                    back,
                    prev: None,
                }
            }
            CKind::List => {
                let head = match self.get(cont).map_err(|f| self.fail(f))? {
                    Value::List(l) => l.head,
                    _ => NIL,
                };
                RangeIt::Nodes {
                    cur: head,
                    prev: NIL,
                }
            }
        })
    }

    pub(super) fn range_next(&mut self, it: &mut RangeIt, cont: &Place) -> R<Option<Place>> {
        match it {
            RangeIt::Seq { i, ver } => {
                let (_, len, cur_ver) = self.ckind(cont)?;
                if cur_ver != *ver {
                    return Err(self.fail(invalid_msg()));
                }
                if *i >= len {
                    return Ok(None);
                }
                let p = cont.with(Step::Idx(*i, *ver));
                *i += 1;
                Ok(Some(p))
            }
            RangeIt::Keys { keys, i } => {
                if *i > 0 {
                    // the element we just left must still exist (erasing it is undefined behaviour)
                    let prev = &keys[*i - 1];
                    let ok = match self.get(cont).map_err(|f| self.fail(f))? {
                        Value::Map(m) | Value::Set(m) => m.m.contains_key(prev),
                        _ => true,
                    };
                    if !ok {
                        return Err(self.fail(invalid_msg()));
                    }
                }
                match keys.get(*i) {
                    None => Ok(None),
                    Some(k) => {
                        *i += 1;
                        Ok(Some(cont.with(Step::Key(k.clone()))))
                    }
                }
            }
            RangeIt::Ents { i, ver, back, prev } => {
                let u = match self.get(cont).map_err(|f| self.fail(f))? {
                    Value::UMap(u) | Value::USet(u) => u,
                    _ => return Err(self.internal("not unordered")),
                };
                if u.ver != *ver {
                    return Err(self.fail(invalid_msg()));
                }
                if let Some(p) = prev
                    && u.ents.get(*p).is_none_or(Option::is_none)
                {
                    return Err(self.fail(invalid_msg()));
                }
                let Some(cur) = *i else { return Ok(None) };
                let Some(e) = u.ents.get(cur).and_then(Option::as_ref) else {
                    return Err(self.fail(invalid_msg()));
                };
                let place = cont.with(Step::Key(u.key_of(e)));
                *prev = Some(cur);
                *i = if *back {
                    if cur == 0 {
                        None
                    } else {
                        u.next_live(cur - 1, true)
                    }
                } else {
                    u.next_live(cur + 1, false)
                };
                Ok(Some(place))
            }
            RangeIt::Nodes { cur, prev } => {
                let l = match self.get(cont).map_err(|f| self.fail(f))? {
                    Value::List(l) => l,
                    _ => return Err(self.internal("not a list")),
                };
                if *prev != NIL && l.nodes[*prev as usize].val.is_none() {
                    return Err(self.fail(invalid_msg()));
                }
                if *cur == NIL {
                    return Ok(None);
                }
                let nd = &l.nodes[*cur as usize];
                if nd.val.is_none() {
                    return Err(self.fail(invalid_msg()));
                }
                let p = cont.with(Step::Node(*cur, nd.gn));
                *prev = *cur;
                *cur = nd.next;
                Ok(Some(p))
            }
        }
    }

    // ----- ranges between two iterators -----

    /// The element values between two iterators of the same container (cloned, in iteration order).
    pub(super) fn range_values(&mut self, a: &Value, b: &Value) -> R<Vec<Value>> {
        let (Value::Iter(x), Value::Iter(y)) = (a, b) else {
            return Err(self.err(RtKind::BadArg, "expected a pair of iterators"));
        };
        if let Some(r) = self.try_seq_range(x, y)? {
            self.charge(r.hi.saturating_sub(r.lo) as u64)?;
            return self.seq_values(&r);
        }
        // walk with iterator stepping
        let mut out = Vec::new();
        let mut cur = (**x).clone();
        let mut guard = 0u64;
        while cur.pos != y.pos {
            let p = self.iter_deref(&cur)?;
            self.charge(1)?;
            out.push(self.load(&p).map_err(|f| self.fail(f))?);
            cur = self.iter_advance(&cur, 1)?;
            guard += 1;
            if guard > 100_000_000 {
                return Err(self.err(RtKind::BadArg, "iterators of different containers"));
            }
        }
        Ok(out)
    }

    /// A resolved index range for random access iterators.
    pub(super) fn try_seq_range(&self, x: &IterV, y: &IterV) -> R<Option<SeqRange>> {
        let (k, len) = self.check_ver(x)?;
        self.check_ver(y)?;
        if !matches!(k, CKind::Seq) {
            return Ok(None);
        }
        let (Pos::Idx(i), Pos::Idx(j)) = (&x.pos, &y.pos) else {
            return Ok(None);
        };
        if x.cont != y.cont || x.rev != y.rev {
            return Err(self.err(
                RtKind::BadArg,
                "the two iterators belong to different containers",
            ));
        }
        if i > j {
            return Err(self.err(
                RtKind::BadArg,
                "the first iterator is after the second: this range is invalid",
            ));
        }
        if *j > len {
            return Err(self.err(
                RtKind::Index,
                "this range ends past the end of the container",
            ));
        }
        let (lo, hi) = if x.rev { (len - j, len - i) } else { (*i, *j) };
        Ok(Some(SeqRange {
            cont: x.cont.clone(),
            lo,
            hi,
            rev: x.rev,
            ver: x.ver,
            p0: *i,
        }))
    }

    pub(super) fn seq_range(&self, a: &Value, b: &Value) -> R<SeqRange> {
        match (a, b) {
            (Value::Iter(x), Value::Iter(y)) => self.try_seq_range(x, y)?.ok_or_else(|| {
                self.err(
                    RtKind::BadArg,
                    "this algorithm needs random access iterators (vector, string, array, deque)",
                )
            }),
            _ => Err(self.err(RtKind::BadArg, "expected a pair of iterators")),
        }
    }

    fn seq_values(&self, r: &SeqRange) -> R<Vec<Value>> {
        let v = self.get(&r.cont).map_err(|f| self.fail(f))?;
        let mut out: Vec<Value> = match v {
            Value::Vector(c) => c.v[r.lo..r.hi].to_vec(),
            Value::Deque(c) => c.v.range(r.lo..r.hi).cloned().collect(),
            Value::Str(s) => s.s[r.lo..r.hi]
                .iter()
                .map(|&b| Value::Char(b as i8))
                .collect(),
            Value::View(b) => b[r.lo..r.hi]
                .iter()
                .map(|&b| Value::Char(b as i8))
                .collect(),
            _ => return Err(self.internal("not a sequence")),
        };
        if r.rev {
            out.reverse();
        }
        Ok(out)
    }
}
