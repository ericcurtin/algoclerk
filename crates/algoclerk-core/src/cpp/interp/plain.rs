// SPDX-License-Identifier: Apache-2.0
//! Conversion between plain (public) values and runtime values; entry point; trace snapshots.

use super::rval::*;
use super::{Event, Machine, R, RtError, RtKind, Slot, UnorderedOrder, VarSnap};
use crate::cpp::ast::*;
use crate::cpp::value::{Heap, Plain};
use std::collections::HashMap;
use std::rc::Rc;

struct InCtx {
    map: HashMap<u32, (u32, u32)>,
    pending: Vec<(u32, u32, String)>,
}

/// Output heap under construction.
struct HeapOut {
    nodes: Vec<Vec<Plain>>,
    map: HashMap<(u32, u32), u32>,
    pending: Vec<(u32, u32, u32)>,
}

impl HeapOut {
    fn new() -> HeapOut {
        HeapOut {
            nodes: vec![Vec::new()],
            map: HashMap::new(),
            pending: Vec::new(),
        }
    }
}

impl<'p> Machine<'p> {
    fn plain_to_value(&mut self, p: &Plain, ty: &Ty, heap: &Heap, cx: &mut InCtx) -> R<Value> {
        let bad = |m: &Self| {
            m.err(
                RtKind::BadArg,
                format!("the test value does not fit the type {ty}"),
            )
        };
        if let Some(sk) = Sk::of_ty(ty) {
            return match p {
                Plain::Int(x) => Ok(sk.make(*x)),
                Plain::Uninit => Ok(Value::Uninit(Box::new(ty.clone()))),
                _ => Err(bad(self)),
            };
        }
        match (ty, p) {
            (Ty::Str, Plain::Str(b)) => Ok(Value::string(b.clone())),
            (Ty::StrView, Plain::Str(b)) => Ok(Value::View(Rc::new(b.clone()))),
            (Ty::Pair(a, b), Plain::Seq(xs)) if xs.len() == 2 => {
                let x = self.plain_to_value(&xs[0], a, heap, cx)?;
                let y = self.plain_to_value(&xs[1], b, heap, cx)?;
                Ok(Value::pair(x, y))
            }
            (Ty::Tuple(ts), Plain::Seq(xs)) if xs.len() == ts.len() => {
                let mut out = Vec::with_capacity(xs.len());
                for (x, t) in xs.iter().zip(ts) {
                    out.push(self.plain_to_value(x, t, heap, cx)?);
                }
                Ok(Value::Tuple(Box::new(out)))
            }
            (Ty::Optional(t), Plain::Opt(o)) => {
                let v = match o {
                    Some(x) => Some(self.plain_to_value(x, t, heap, cx)?),
                    None => None,
                };
                Ok(Value::Opt(Box::new(OptV {
                    ety: Rc::new((**t).clone()),
                    v,
                })))
            }
            (Ty::Ptr(inner), Plain::Ptr(i)) => {
                if *i == 0 {
                    return Ok(Value::PNull);
                }
                let Ty::Struct(name) = &**inner else {
                    return Err(bad(self));
                };
                if let Some(&(id, gn)) = cx.map.get(i) {
                    return Ok(Value::PHeap(id, gn));
                }
                if heap.nodes.get(*i as usize).is_none() {
                    return Err(bad(self));
                }
                let sid = self.struct_id(name)?;
                let decl = self.rs.structs[sid as usize].decl;
                let ph = Value::Struct(Box::new(StructV {
                    sid,
                    name: Rc::from(decl.name.as_str()),
                    f: Vec::new(),
                }));
                let h = self.alloc_heap(ph);
                let Value::PHeap(id, gn) = h else {
                    return Err(self.internal("alloc"));
                };
                cx.map.insert(*i, (id, gn));
                cx.pending.push((*i, id, name.clone()));
                Ok(Value::PHeap(id, gn))
            }
            (Ty::Struct(name), Plain::Struct(fs)) => {
                let sid = self.struct_id(name)?;
                let decl = self.rs.structs[sid as usize].decl;
                if fs.len() != decl.fields.len() {
                    return Err(bad(self));
                }
                let mut out = Vec::with_capacity(fs.len());
                for (x, f) in fs.iter().zip(&decl.fields) {
                    out.push(self.plain_to_value(x, &f.spec.ty, heap, cx)?);
                }
                Ok(Value::Struct(Box::new(StructV {
                    sid,
                    name: Rc::from(decl.name.as_str()),
                    f: out,
                })))
            }
            (Ty::Map(k, v) | Ty::UnorderedMap(k, v), Plain::Map(es)) => {
                let mut c = super::conv::empty_container(ty).unwrap_or(Value::Void);
                for (a, b) in es {
                    let kk = self.plain_to_value(a, k, heap, cx)?;
                    let vv = self.plain_to_value(b, v, heap, cx)?;
                    let pair = Value::pair(kk.clone(), vv);
                    match &mut c {
                        Value::Map(m) => {
                            m.m.entry(Key(kk)).or_insert(pair);
                        }
                        Value::UMap(u) => {
                            let key = Key(kk);
                            if !u.idx.contains_key(&key) {
                                u.push(key, pair);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(c)
            }
            (Ty::Set(t) | Ty::UnorderedSet(t), Plain::Seq(xs)) => {
                let mut c = super::conv::empty_container(ty).unwrap_or(Value::Void);
                for x in xs {
                    let v = self.plain_to_value(x, t, heap, cx)?;
                    match &mut c {
                        Value::Set(m) => {
                            m.m.entry(Key(v)).or_insert(Value::Void);
                        }
                        Value::USet(u) => {
                            let key = Key(v.clone());
                            if !u.idx.contains_key(&key) {
                                u.push(key, v);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(c)
            }
            (
                Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) | Ty::Stack(t) | Ty::Queue(t),
                Plain::Seq(xs),
            ) => {
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(self.plain_to_value(x, t, heap, cx)?);
                }
                self.seq_from_vec(ty, out)
            }
            (Ty::Array(t, n), Plain::Seq(xs)) => {
                if xs.len() != *n {
                    return Err(bad(self));
                }
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(self.plain_to_value(x, t, heap, cx)?);
                }
                Ok(Value::Vector(Box::new(VecV {
                    ety: Rc::new((**t).clone()),
                    v: out,
                    ver: 0,
                    kind: VKind::Array,
                })))
            }
            (Ty::PriorityQueue(t, _), Plain::Seq(xs)) => {
                let mut pq = self.zero_value(ty)?;
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(self.plain_to_value(x, t, heap, cx)?);
                }
                if let Value::Pq(p) = &mut pq {
                    p.v = out;
                }
                let pl = self.temp(pq);
                self.pq_make_heap(&pl)?;
                let v = self.load(&pl).map_err(|f| self.fail(f))?;
                Ok(v)
            }
            _ => Err(bad(self)),
        }
    }

    fn drain_pending(&mut self, heap: &Heap, cx: &mut InCtx) -> R<()> {
        while let Some((pidx, hid, name)) = cx.pending.pop() {
            let sid = self.struct_id(&name)?;
            let decl = self.rs.structs[sid as usize].decl;
            let fs = heap.nodes.get(pidx as usize).cloned().unwrap_or_default();
            if fs.len() != decl.fields.len() {
                return Err(self.err(
                    RtKind::BadArg,
                    "a node of the test data has the wrong number of fields",
                ));
            }
            let mut out = Vec::with_capacity(fs.len());
            for (x, f) in fs.iter().zip(&decl.fields) {
                out.push(self.plain_to_value(x, &f.spec.ty, heap, cx)?);
            }
            if let Some(h) = self.heap.get_mut(hid as usize) {
                h.v = Some(Value::Struct(Box::new(StructV {
                    sid,
                    name: Rc::from(decl.name.as_str()),
                    f: out,
                })));
            }
        }
        Ok(())
    }

    pub(super) fn run_entry(
        &mut self,
        entry: &str,
        args: Vec<Plain>,
        heap: &Heap,
    ) -> R<(Plain, Heap)> {
        // global constants
        let prog = self.prog;
        for g in 0..self.rs.globals.len() {
            let (spec, decl) = (self.rs.globals[g].spec, self.rs.globals[g].decl);
            self.cur = decl.id;
            let v = self.decl_value(spec, decl, 0, 0)?;
            self.globals.push(Slot { gn: 1, v });
        }
        let Some(g) = self.rs.group_names.iter().position(|n| *n == entry) else {
            return Err(self.err(
                RtKind::Internal,
                format!("the program defines no function called `{entry}`"),
            ));
        };
        let cands: Vec<u32> = self.rs.groups[g]
            .iter()
            .copied()
            .filter(|&f| self.rs.fns[f as usize].decl.params.len() == args.len())
            .collect();
        let Some(&fi) = cands.first() else {
            return Err(self.err(
                RtKind::Internal,
                format!("`{entry}` does not take {} arguments", args.len()),
            ));
        };
        let decl = self.rs.fns[fi as usize].decl;
        self.cur = decl.id;
        let mut cx = InCtx {
            map: HashMap::new(),
            pending: Vec::new(),
        };
        let mut vals = Vec::with_capacity(args.len());
        for (p, a) in decl.params.iter().zip(&args) {
            let v = self.plain_to_value(a, &p.spec.ty, heap, &mut cx)?;
            vals.push(v);
        }
        self.drain_pending(heap, &mut cx)?;
        let mut input = 0u64;
        for v in &vals {
            input += cells(v);
        }
        for h in &self.heap {
            if let Some(v) = &h.v {
                input += cells(v);
            }
        }
        self.input_cells = input;
        self.peak = input;
        self.live_hint = input;
        let mut bound = Vec::with_capacity(vals.len());
        for (p, v) in decl.params.iter().zip(vals) {
            if p.spec.is_ref {
                let t = self.temp(v);
                bound.push(Value::Ref(Box::new(t)));
            } else {
                self.charge(leaves(&v))?;
                bound.push(v);
            }
        }
        self.charge(1)?;
        let _ = prog;
        let rv = self.call_fn(fi, bound)?;
        let v = self.rv_value_uncharged(rv)?;
        let mut ho = HeapOut::new();
        let mut budget = 50_000_000i64;
        let plain = self.to_plain(&v, &mut ho, &mut budget, 0);
        self.flush_heap(&mut ho, &mut budget);
        Ok((plain, Heap { nodes: ho.nodes }))
    }

    fn rv_value_uncharged(&mut self, rv: super::Rv) -> R<Value> {
        match rv {
            super::Rv::Val(v) => Ok(v),
            super::Rv::Place(p) => self.load(&p).map_err(|f| self.fail(f)),
        }
    }

    fn to_plain(&self, v: &Value, ho: &mut HeapOut, budget: &mut i64, depth: u32) -> Plain {
        *budget -= 1;
        if *budget < 0 || depth > 200 {
            return Plain::Void;
        }
        if let Some(x) = v.as_i128() {
            return Plain::Int(x);
        }
        let seq =
            |m: &Self, it: &mut dyn Iterator<Item = &Value>, ho: &mut HeapOut, budget: &mut i64| {
                let mut out = Vec::new();
                for x in it {
                    if *budget < 0 {
                        break;
                    }
                    out.push(m.to_plain(x, ho, budget, depth + 1));
                }
                Plain::Seq(out)
            };
        match v {
            Value::Void => Plain::Void,
            Value::Uninit(_) => Plain::Uninit,
            Value::Str(s) => Plain::Str(s.s.clone()),
            Value::View(b) => Plain::Str((**b).clone()),
            Value::Vector(c) => seq(self, &mut c.v.iter(), ho, budget),
            Value::Deque(c) => seq(self, &mut c.v.iter(), ho, budget),
            Value::List(l) => seq(self, &mut l.values().into_iter(), ho, budget),
            Value::Set(m) => seq(self, &mut m.m.keys().map(|k| &k.0), ho, budget),
            Value::USet(u) => {
                let items: Vec<&Value> = self.un_order(u);
                seq(self, &mut items.into_iter(), ho, budget)
            }
            Value::Map(m) => {
                let mut out = Vec::new();
                for (k, e) in &m.m {
                    if *budget < 0 {
                        break;
                    }
                    let val = match e {
                        Value::Pair(p) => &p[1],
                        o => o,
                    };
                    out.push((
                        self.to_plain(&k.0, ho, budget, depth + 1),
                        self.to_plain(val, ho, budget, depth + 1),
                    ));
                }
                Plain::Map(out)
            }
            Value::UMap(u) => {
                let mut out = Vec::new();
                for e in self.un_order(u) {
                    if *budget < 0 {
                        break;
                    }
                    if let Value::Pair(p) = e {
                        out.push((
                            self.to_plain(&p[0], ho, budget, depth + 1),
                            self.to_plain(&p[1], ho, budget, depth + 1),
                        ));
                    }
                }
                Plain::Map(out)
            }
            Value::Pq(p) => {
                let mut items: Vec<&Value> = p.v.iter().collect();
                match &p.cmp {
                    PqCmp::Less => items.sort_by(|a, b| cmp_val(b, a)),
                    PqCmp::Greater => items.sort_by(|a, b| cmp_val(a, b)),
                    PqCmp::Fn(_) => {}
                }
                seq(self, &mut items.into_iter(), ho, budget)
            }
            Value::Pair(p) => Plain::Seq(vec![
                self.to_plain(&p[0], ho, budget, depth + 1),
                self.to_plain(&p[1], ho, budget, depth + 1),
            ]),
            Value::Tuple(t) => seq(self, &mut t.iter(), ho, budget),
            Value::Opt(o) => Plain::Opt(
                o.v.as_ref()
                    .map(|x| Box::new(self.to_plain(x, ho, budget, depth + 1))),
            ),
            Value::Struct(s) => seq_struct(self, &s.f, ho, budget, depth),
            Value::PNull => Plain::Ptr(0),
            Value::PHeap(id, gn) => {
                match self.heap.get(*id as usize) {
                    Some(h) if h.gn == *gn && h.v.is_some() => {}
                    _ => return Plain::Ptr(0),
                }
                if let Some(&n) = ho.map.get(&(*id, *gn)) {
                    return Plain::Ptr(n);
                }
                ho.nodes.push(Vec::new());
                let n = (ho.nodes.len() - 1) as u32;
                ho.map.insert((*id, *gn), n);
                ho.pending.push((*id, *gn, n));
                Plain::Ptr(n)
            }
            Value::PPlace(p) => match self.get(p) {
                Ok(Value::Struct(s)) => {
                    ho.nodes.push(Vec::new());
                    let n = (ho.nodes.len() - 1) as u32;
                    let fields: Vec<Plain> =
                        s.f.iter()
                            .map(|x| self.to_plain(x, ho, budget, depth + 8))
                            .collect();
                    ho.nodes[n as usize] = fields;
                    Plain::Ptr(n)
                }
                _ => Plain::Ptr(0),
            },
            _ => Plain::Void,
        }
    }

    fn un_order<'a>(&self, u: &'a UnV) -> Vec<&'a Value> {
        let mut items: Vec<&Value> = u.ents.iter().flatten().collect();
        if self.lim.unordered == UnorderedOrder::Reverse {
            items.reverse();
        }
        items
    }

    fn flush_heap(&self, ho: &mut HeapOut, budget: &mut i64) {
        while let Some((id, gn, n)) = ho.pending.pop() {
            let Some(h) = self.heap.get(id as usize) else {
                continue;
            };
            if h.gn != gn {
                continue;
            }
            let fields: Vec<Plain> = match &h.v {
                Some(Value::Struct(s)) => {
                    s.f.iter()
                        .map(|x| self.to_plain(x, ho, budget, 0))
                        .collect()
                }
                Some(v) => vec![self.to_plain(v, ho, budget, 0)],
                None => Vec::new(),
            };
            ho.nodes[n as usize] = fields;
        }
    }

    pub(super) fn snapshot(&self, node: NodeId, returned: Option<&Value>) -> Event {
        let mut ho = HeapOut::new();
        let mut budget = 4000i64;
        let mut vars = Vec::new();
        for (name, slot) in &self.vis[self.vis_base.min(self.vis.len())..] {
            let abs = self.base + *slot as usize;
            let Some(sl) = self.stack.get(abs) else {
                continue;
            };
            if sl.gn == 0 {
                continue;
            }
            let v: Option<&Value> = match &sl.v {
                Value::Ref(p) => self.get(p).ok(),
                v => Some(v),
            };
            let Some(v) = v else { continue };
            vars.push(VarSnap {
                name: (*name).to_string(),
                ty: v.ty(),
                value: self.to_plain(v, &mut ho, &mut budget, 0),
            });
        }
        let ret = returned.map(|v| self.to_plain(v, &mut ho, &mut budget, 0));
        self.flush_heap(&mut ho, &mut budget);
        Event {
            node,
            func: self.func,
            depth: self.depth,
            cost: self.cost,
            vars,
            returned: ret,
            heap: Heap { nodes: ho.nodes },
        }
    }
}

fn seq_struct(
    m: &Machine<'_>,
    f: &[Value],
    ho: &mut HeapOut,
    budget: &mut i64,
    depth: u32,
) -> Plain {
    Plain::Struct(
        f.iter()
            .map(|x| m.to_plain(x, ho, budget, depth + 1))
            .collect(),
    )
}

#[allow(dead_code)]
fn unused(_: RtError) {}
