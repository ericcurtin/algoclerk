// SPDX-License-Identifier: Apache-2.0
//! Conversions, default and value initialisation, brace lists and constructors.

use super::rval::*;
use super::{Machine, R, RtKind, Rv, Slot, ops};
use crate::cpp::ast::*;
use std::rc::Rc;

/// Does the value already have (the shape of) the type?
pub(super) fn matches_ty(v: &Value, t: &Ty) -> bool {
    if let Some(sk) = Sk::of_ty(t) {
        return v.sk() == Some(sk);
    }
    match (v, t) {
        (_, Ty::Auto | Ty::Unknown | Ty::InitList) => true,
        (Value::Str(_), Ty::Str) | (Value::View(_), Ty::StrView) => true,
        (Value::Vector(c), Ty::Vector(_)) => c.kind == VKind::Vector,
        (Value::Vector(c), Ty::Array(..)) => c.kind == VKind::Array,
        (Value::Vector(c), Ty::Stack(_)) => c.kind == VKind::Stack,
        (Value::Deque(c), Ty::Deque(_)) => c.kind == DKind::Deque,
        (Value::Deque(c), Ty::Queue(_)) => c.kind == DKind::Queue,
        (Value::List(_), Ty::List(_))
        | (Value::Pq(_), Ty::PriorityQueue(..))
        | (Value::Map(_), Ty::Map(..))
        | (Value::Set(_), Ty::Set(_))
        | (Value::UMap(_), Ty::UnorderedMap(..))
        | (Value::USet(_), Ty::UnorderedSet(_))
        | (Value::Pair(_), Ty::Pair(..))
        | (Value::Tuple(_), Ty::Tuple(_))
        | (Value::Opt(_), Ty::Optional(_))
        | (Value::Closure(_) | Value::FnRef(_), Ty::Function(..) | Ty::Lambda(_))
        | (Value::Iter(_), Ty::Iter { .. })
        | (Value::Functor(..), Ty::Functor(..)) => true,
        (Value::Struct(s), Ty::Struct(n)) => &*s.name == n.as_str(),
        (v, Ty::Ptr(_)) => v.is_ptr(),
        _ => false,
    }
}

/// How well an argument value fits a parameter type (bigger is better).
pub(super) fn match_score(v: &Value, t: &Ty) -> i32 {
    if matches_ty(v, t) {
        10
    } else if v.sk().is_some() && Sk::of_ty(t).is_some() {
        5
    } else if matches!(
        (v, t),
        (Value::Str(_) | Value::View(_), Ty::Str | Ty::StrView)
    ) {
        6
    } else if v.is_ptr() && matches!(t, Ty::Ptr(_)) {
        8
    } else {
        0
    }
}

/// An empty container of the given type, when the type is a container.
pub(super) fn empty_container(ty: &Ty) -> Option<Value> {
    let rc = |t: &Ty| -> TyRc { Rc::new(t.clone()) };
    Some(match ty {
        Ty::Str => Value::string(Vec::new()),
        Ty::StrView => Value::View(Rc::new(Vec::new())),
        Ty::Vector(t) => Value::vector(rc(t), Vec::new()),
        Ty::Stack(t) => Value::Vector(Box::new(VecV {
            ety: rc(t),
            v: Vec::new(),
            ver: 0,
            kind: VKind::Stack,
        })),
        Ty::Deque(t) | Ty::Queue(t) => Value::Deque(Box::new(DqV {
            ety: rc(t),
            v: Default::default(),
            ver: 0,
            kind: if matches!(ty, Ty::Queue(_)) {
                DKind::Queue
            } else {
                DKind::Deque
            },
        })),
        Ty::List(t) => Value::List(Box::new(ListV::new(rc(t)))),
        Ty::Map(k, v) => Value::Map(Box::new(OrdV {
            kty: rc(k),
            vty: Some(rc(v)),
            m: Default::default(),
        })),
        Ty::Set(k) => Value::Set(Box::new(OrdV {
            kty: rc(k),
            vty: None,
            m: Default::default(),
        })),
        Ty::UnorderedMap(k, v) => Value::UMap(Box::new(UnV::new(rc(k), Some(rc(v))))),
        Ty::UnorderedSet(k) => Value::USet(Box::new(UnV::new(rc(k), None))),
        Ty::PriorityQueue(t, c) => Value::Pq(Box::new(PqV {
            ety: rc(t),
            v: Vec::new(),
            cmp: match &**c {
                Ty::Functor(Functor::Greater, _) => PqCmp::Greater,
                Ty::Functor(Functor::Less, _) => PqCmp::Less,
                _ => PqCmp::Fn(Value::Void),
            },
            ver: 0,
        })),
        Ty::Optional(t) => Value::Opt(Box::new(OptV {
            ety: rc(t),
            v: None,
        })),
        Ty::Functor(f, t) => Value::Functor(*f, rc(t)),
        _ => return None,
    })
}

impl<'p> Machine<'p> {
    /// Implicit conversion of a value to a declared type.
    pub(super) fn convert(&mut self, v: Value, ty: &Ty) -> R<Value> {
        if let Some(sk) = Sk::of_ty(ty) {
            return match &v {
                _ if v.sk().is_some() => Ok(ops::convert(&v, sk).unwrap_or(v)),
                Value::Uninit(t) => Err(self.err(
                    RtKind::Uninit,
                    format!("this {t} value was never set, so using it is undefined behaviour"),
                )),
                Value::PNull => Ok(sk.make(0)),
                Value::PHeap(..) | Value::PPlace(_) if sk == Sk::Bool => Ok(Value::Bool(true)),
                Value::Opt(o) if sk == Sk::Bool => Ok(Value::Bool(o.v.is_some())),
                _ => Err(self.err(
                    RtKind::BadArg,
                    format!("cannot convert this value to {}", ops::name(sk)),
                )),
            };
        }
        match (ty, v) {
            (Ty::Auto | Ty::Unknown | Ty::InitList | Ty::Void, v) => Ok(v),
            (Ty::Str, Value::View(b)) => {
                self.charge(b.len() as u64)?;
                Ok(Value::string((*b).clone()))
            }
            (Ty::StrView, Value::Str(s)) => Ok(Value::View(Rc::new(s.s))),
            (Ty::Ptr(_), Value::Int(0)) => Ok(Value::PNull),
            (Ty::Pair(a, b), Value::Pair(p)) => {
                let [x, y] = *p;
                let x = self.convert(x, a)?;
                let y = self.convert(y, b)?;
                Ok(Value::pair(x, y))
            }
            (Ty::Tuple(ts), Value::Tuple(t)) if ts.len() == t.len() => {
                let mut out = Vec::with_capacity(t.len());
                for (x, ty) in t.into_iter().zip(ts) {
                    out.push(self.convert(x, ty)?);
                }
                Ok(Value::Tuple(Box::new(out)))
            }
            (Ty::Tuple(ts), Value::Pair(p)) if ts.len() == 2 => {
                let [x, y] = *p;
                let x = self.convert(x, &ts[0])?;
                let y = self.convert(y, &ts[1])?;
                Ok(Value::Tuple(Box::new(vec![x, y])))
            }
            (Ty::Optional(t), Value::Opt(mut o)) => {
                if o.v.is_none() {
                    o.ety = Rc::new((**t).clone());
                    return Ok(Value::Opt(o));
                }
                match o.v.take() {
                    Some(x) => {
                        let x = self.convert(x, t)?;
                        Ok(Value::Opt(Box::new(OptV {
                            ety: Rc::new((**t).clone()),
                            v: Some(x),
                        })))
                    }
                    None => Ok(Value::Opt(o)),
                }
            }
            (Ty::Optional(t), v) => {
                let x = self.convert(v, t)?;
                Ok(Value::Opt(Box::new(OptV {
                    ety: Rc::new((**t).clone()),
                    v: Some(x),
                })))
            }
            (Ty::Vector(t), Value::Vector(mut c)) if c.kind == VKind::Vector => {
                if *c.ety != **t && c.v.is_empty() {
                    c.ety = Rc::new((**t).clone());
                }
                Ok(Value::Vector(c))
            }
            (Ty::Function(..), v @ (Value::Closure(_) | Value::FnRef(_) | Value::Uninit(_))) => {
                Ok(v)
            }
            (_, v) => Ok(v),
        }
    }

    /// `T x;` : scalars stay uninitialised.
    pub(super) fn default_value(&mut self, ty: &Ty) -> R<Value> {
        self.init_value(ty, false)
    }

    /// `T()` / `T{}` : value initialisation.
    pub(super) fn zero_value(&mut self, ty: &Ty) -> R<Value> {
        self.init_value(ty, true)
    }

    fn init_value(&mut self, ty: &Ty, zero: bool) -> R<Value> {
        if let Some(sk) = Sk::of_ty(ty) {
            return Ok(if zero {
                sk.make(0)
            } else {
                Value::Uninit(Box::new(ty.clone()))
            });
        }
        if let Some(c) = empty_container(ty) {
            return Ok(c);
        }
        match ty {
            Ty::Void => Ok(Value::Void),
            Ty::Ptr(_) => Ok(if zero {
                Value::PNull
            } else {
                Value::Uninit(Box::new(ty.clone()))
            }),
            Ty::NullPtr => Ok(Value::PNull),
            Ty::Array(t, n) => {
                self.reserve(*n as u64)?;
                self.charge(*n as u64)?;
                let mut v = Vec::with_capacity(*n);
                for _ in 0..*n {
                    v.push(self.init_value(t, zero)?);
                }
                Ok(Value::Vector(Box::new(VecV {
                    ety: Rc::new((**t).clone()),
                    v,
                    ver: 0,
                    kind: VKind::Array,
                })))
            }
            Ty::Pair(a, b) => {
                let x = self.init_value(a, true)?;
                let y = self.init_value(b, true)?;
                Ok(Value::pair(x, y))
            }
            Ty::Tuple(ts) => {
                let mut v = Vec::with_capacity(ts.len());
                for t in ts {
                    v.push(self.init_value(t, true)?);
                }
                Ok(Value::Tuple(Box::new(v)))
            }
            Ty::Struct(name) => {
                let sid = self.struct_id(name)?;
                self.struct_default(sid, zero)
            }
            Ty::Function(..) | Ty::Lambda(_) | Ty::Iter { .. } => {
                Ok(Value::Uninit(Box::new(ty.clone())))
            }
            _ => Err(self.err(
                RtKind::Internal,
                format!("cannot create a value of type {ty}"),
            )),
        }
    }

    pub(super) fn struct_id(&self, name: &str) -> R<u32> {
        self.struct_ix.get(name).copied().ok_or_else(|| {
            self.err(
                RtKind::BadArg,
                format!("this program never defines the struct `{name}`"),
            )
        })
    }

    fn struct_default(&mut self, sid: u32, zero: bool) -> R<Value> {
        let info = &self.rs.structs[sid as usize];
        if let Some(k) = info.ctors.iter().position(|c| c.ctor.params.is_empty()) {
            return self.run_ctor(sid, k, Vec::new());
        }
        self.aggregate(sid, Vec::new(), zero)
    }

    fn aggregate(&mut self, sid: u32, vals: Vec<Value>, zero: bool) -> R<Value> {
        let decl = self.rs.structs[sid as usize].decl;
        if vals.len() > decl.fields.len() {
            return Err(self.err(RtKind::BadArg, "too many values for this struct"));
        }
        let given = vals.len();
        let mut out = Vec::with_capacity(decl.fields.len());
        let mut it = vals.into_iter();
        for f in &decl.fields {
            match it.next() {
                Some(v) => out.push(self.convert(v, &f.spec.ty)?),
                None => match &f.default {
                    Some(d) => out.push(self.eval_conv(d, &f.spec.ty)?),
                    None => out.push(self.init_value(&f.spec.ty, zero || given > 0)?),
                },
            }
        }
        Ok(Value::Struct(Box::new(StructV {
            sid,
            name: Rc::from(decl.name.as_str()),
            f: out,
        })))
    }

    /// Construct a struct from already evaluated arguments.
    pub(super) fn construct_struct(&mut self, sid: u32, args: Vec<Value>) -> R<Value> {
        let info = &self.rs.structs[sid as usize];
        if info.ctors.is_empty() {
            return self.aggregate(sid, args, false);
        }
        let mut best: Option<(i32, usize)> = None;
        for (k, c) in info.ctors.iter().enumerate() {
            if c.ctor.params.len() != args.len() {
                continue;
            }
            let score: i32 = c
                .ctor
                .params
                .iter()
                .zip(&args)
                .map(|(p, a)| conv_score(a, &p.spec.ty))
                .sum();
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, k));
            }
        }
        match best {
            Some((_, k)) => self.run_ctor(sid, k, args),
            None => Err(self.err(
                RtKind::BadArg,
                format!(
                    "no constructor of `{}` takes {} arguments",
                    info.decl.name,
                    args.len()
                ),
            )),
        }
    }

    fn run_ctor(&mut self, sid: u32, k: usize, args: Vec<Value>) -> R<Value> {
        if self.depth >= self.lim.depth || self.deep() {
            return Err(self.too_deep());
        }
        let (ctor, nslots, decl) = {
            let s = &self.rs.structs[sid as usize];
            (s.ctors[k].ctor, s.ctors[k].nslots as usize, s.decl)
        };
        let saved = (
            self.base,
            self.cap_off,
            self.ret_spec,
            self.vis_base,
            self.cur,
        );
        let new_base = self.stack.len();
        self.stack
            .resize_with(new_base + nslots.max(args.len() + 1), || Slot {
                gn: 0,
                v: Value::Void,
            });
        self.base = new_base;
        self.cap_off = 0;
        self.ret_spec = None;
        self.depth += 1;
        self.vis_base = self.vis.len();
        let placeholder = Value::Struct(Box::new(StructV {
            sid,
            name: Rc::from(decl.name.as_str()),
            f: decl.fields.iter().map(|_| Value::Void).collect(),
        }));
        let gn = self.declare(0, placeholder);
        let mut result = Ok(());
        for (i, (v, p)) in args.into_iter().zip(&ctor.params).enumerate() {
            match self.convert(v, &p.spec.ty) {
                Ok(v) => {
                    self.declare(1 + i as u32, v);
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        let this_place = Place::root(Root::Local(new_base as u32, gn));
        if result.is_ok() {
            result = self.run_ctor_body(ctor, decl, &this_place);
        }
        let out = match result {
            Ok(()) => {
                let v = self
                    .stack
                    .get_mut(new_base)
                    .map(|s| std::mem::replace(&mut s.v, Value::Void))
                    .unwrap_or(Value::Void);
                Ok(v)
            }
            Err(e) => Err(e),
        };
        self.depth -= 1;
        self.stack.truncate(new_base);
        self.vis.truncate(self.vis_base);
        (
            self.base,
            self.cap_off,
            self.ret_spec,
            self.vis_base,
            self.cur,
        ) = saved;
        out
    }

    fn run_ctor_body(&mut self, ctor: &'p Ctor, decl: &'p StructDecl, this: &Place) -> R<()> {
        for (i, f) in decl.fields.iter().enumerate() {
            let v = if let Some((_, args)) = ctor.inits.iter().find(|(n, _)| *n == f.name) {
                self.construct_ty(&f.spec.ty, args, false)?
            } else if let Some(d) = &f.default {
                self.eval_conv(d, &f.spec.ty)?
            } else {
                self.default_value(&f.spec.ty)?
            };
            self.store(&this.with(Step::Field(i as u16)), v)
                .map_err(|f| self.fail(f))?;
        }
        let tm = self.temps.len();
        let r = self.exec_stmts(&ctor.body.stmts)?;
        self.temps.truncate(tm);
        let _ = r;
        Ok(())
    }

    /// Value of a braced list (or `{}`) of type `ty`.
    pub(super) fn list_value(&mut self, elems: &'p [Expr], ty: &Ty) -> R<Value> {
        if Sk::of_ty(ty).is_some() || matches!(ty, Ty::Ptr(_)) {
            return match elems {
                [] => self.zero_value(ty),
                [e] => self.eval_conv(e, ty),
                _ => Err(self.err(
                    RtKind::BadArg,
                    "a single value needs at most one initialiser",
                )),
            };
        }
        self.charge(elems.len() as u64)?;
        match ty {
            Ty::Auto | Ty::Unknown | Ty::InitList => {
                let mut vs = Vec::with_capacity(elems.len());
                for e in elems {
                    vs.push(self.eval(e)?);
                }
                let ety = vs.first().map_or(Ty::Int, Value::ty);
                Ok(Value::vector(Rc::new(ety), vs))
            }
            Ty::Str | Ty::StrView => {
                let mut bytes = Vec::with_capacity(elems.len());
                for e in elems {
                    let v = self.eval(e)?;
                    match v.as_i128() {
                        Some(x) => bytes.push(x as u8),
                        None => match v.bytes() {
                            Some(b) if elems.len() == 1 => bytes.extend_from_slice(b),
                            _ => {
                                return Err(
                                    self.err(RtKind::BadArg, "a string list holds characters")
                                );
                            }
                        },
                    }
                }
                Ok(if matches!(ty, Ty::Str) {
                    Value::string(bytes)
                } else {
                    Value::View(Rc::new(bytes))
                })
            }
            Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) | Ty::Stack(t) | Ty::Queue(t) => {
                self.reserve(elems.len() as u64)?;
                let mut vs = Vec::with_capacity(elems.len());
                for e in elems {
                    vs.push(self.eval_conv(e, t)?);
                }
                let mut c = empty_container(ty).unwrap_or(Value::Void);
                match &mut c {
                    Value::Vector(x) => x.v = vs,
                    Value::Deque(x) => x.v = vs.into(),
                    Value::List(x) => {
                        for v in vs {
                            x.insert_before(NIL, v);
                        }
                    }
                    _ => {}
                }
                Ok(c)
            }
            Ty::Array(t, n) => {
                if elems.len() > *n {
                    return Err(self.err(RtKind::BadArg, "too many initialisers for this array"));
                }
                self.reserve(*n as u64)?;
                let mut vs = Vec::with_capacity(*n);
                for e in elems {
                    vs.push(self.eval_conv(e, t)?);
                }
                while vs.len() < *n {
                    vs.push(self.zero_value(t)?);
                }
                Ok(Value::Vector(Box::new(VecV {
                    ety: Rc::new((**t).clone()),
                    v: vs,
                    ver: 0,
                    kind: VKind::Array,
                })))
            }
            Ty::Pair(..) | Ty::Tuple(_) => self.construct_ty(ty, elems, true),
            Ty::Set(k) | Ty::UnorderedSet(k) => {
                self.reserve(elems.len() as u64)?;
                let mut c = empty_container(ty).unwrap_or(Value::Void);
                for e in elems {
                    let v = self.eval_conv(e, k)?;
                    self.insert_elem(&mut c, v);
                }
                Ok(c)
            }
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
                self.reserve(elems.len() as u64)?;
                let pt = Ty::Pair(k.clone(), v.clone());
                let mut c = empty_container(ty).unwrap_or(Value::Void);
                for e in elems {
                    let p = self.eval_conv(e, &pt)?;
                    self.insert_elem(&mut c, p);
                }
                Ok(c)
            }
            Ty::Optional(_) => match elems {
                [] => self.zero_value(ty),
                [e] => self.eval_conv(e, ty),
                _ => Err(self.err(RtKind::BadArg, "an optional holds one value")),
            },
            Ty::Struct(name) => {
                let sid = self.struct_id(name)?;
                let mut vs = Vec::with_capacity(elems.len());
                for e in elems {
                    vs.push(self.eval(e)?);
                }
                if vs.is_empty() {
                    return self.struct_default(sid, true);
                }
                self.construct_struct(sid, vs)
            }
            Ty::PriorityQueue(..) => self.construct_ty(ty, elems, true),
            _ => {
                if elems.is_empty() {
                    self.zero_value(ty)
                } else {
                    Err(self.err(RtKind::BadArg, format!("cannot brace-initialise {ty}")))
                }
            }
        }
    }

    /// Insert into a set or map value built from a list (ignoring duplicates).
    fn insert_elem(&mut self, c: &mut Value, v: Value) {
        match c {
            Value::Set(o) => {
                o.m.entry(Key(v)).or_insert(Value::Void);
            }
            Value::Map(o) => {
                if let Value::Pair(p) = &v {
                    o.m.entry(Key(p[0].clone())).or_insert(v);
                }
            }
            Value::USet(u) => {
                let k = Key(v.clone());
                if !u.idx.contains_key(&k) {
                    u.push(k, v);
                }
            }
            Value::UMap(u) => {
                if let Value::Pair(p) = &v {
                    let k = Key(p[0].clone());
                    if !u.idx.contains_key(&k) {
                        u.push(k, v);
                    }
                }
            }
            _ => {}
        }
    }

    /// Evaluate an expression with a target type (brace lists and constructors take their type from it).
    pub(super) fn eval_conv(&mut self, e: &'p Expr, ty: &Ty) -> R<Value> {
        match &e.kind {
            ExprKind::InitList(elems) => {
                self.charge(1)?;
                self.list_value(elems, ty)
            }
            _ => {
                let v = self.eval(e)?;
                let v = self.check_init_val(v)?;
                self.convert(v, ty)
            }
        }
    }

    pub(super) fn check_init_val(&self, v: Value) -> R<Value> {
        self.check_init(v)
    }

    /// Build a value of `ty` from constructor arguments.
    pub(super) fn construct_ty(&mut self, ty: &Ty, args: &'p [Expr], brace: bool) -> R<Value> {
        if Sk::of_ty(ty).is_some() || matches!(ty, Ty::Ptr(_)) {
            return match args {
                [] => self.zero_value(ty),
                [e] => self.eval_conv(e, ty),
                _ => Err(self.err(RtKind::BadArg, "a number is built from one value")),
            };
        }
        if brace
            && matches!(
                ty,
                Ty::Vector(_)
                    | Ty::Deque(_)
                    | Ty::List(_)
                    | Ty::Set(_)
                    | Ty::UnorderedSet(_)
                    | Ty::Map(..)
                    | Ty::UnorderedMap(..)
                    | Ty::Array(..)
                    | Ty::Str
            )
        {
            // a single braced list of the same type, `T{other}`, copies
            if let [e] = args
                && !matches!(e.kind, ExprKind::InitList(_))
            {
                let v = self.eval(e)?;
                if matches_ty(&v, ty) {
                    return Ok(v);
                }
                return self.list_from_values(ty, vec![v]);
            }
            return self.list_value(args, ty);
        }
        match ty {
            Ty::Str => self.construct_string(args),
            Ty::StrView => match args {
                [] => Ok(Value::View(Rc::new(Vec::new()))),
                [e] => {
                    let v = self.eval(e)?;
                    match v.bytes() {
                        Some(b) => Ok(Value::View(Rc::new(b.to_vec()))),
                        None => {
                            Err(self.err(RtKind::BadArg, "a string_view is built from a string"))
                        }
                    }
                }
                _ => Err(self.err(RtKind::BadArg, "unsupported string_view constructor")),
            },
            Ty::Pair(a, b) => match args {
                [] => self.zero_value(ty),
                [x, y] => {
                    let x = self.eval_conv(x, a)?;
                    let y = self.eval_conv(y, b)?;
                    Ok(Value::pair(x, y))
                }
                [x] => {
                    let v = self.eval(x)?;
                    self.convert(v, ty)
                }
                _ => Err(self.err(RtKind::BadArg, "a pair is built from two values")),
            },
            Ty::Tuple(ts) => {
                if args.is_empty() {
                    return self.zero_value(ty);
                }
                if args.len() != ts.len() {
                    return Err(self.err(RtKind::BadArg, "wrong number of tuple elements"));
                }
                let mut out = Vec::with_capacity(ts.len());
                for (e, t) in args.iter().zip(ts) {
                    out.push(self.eval_conv(e, t)?);
                }
                Ok(Value::Tuple(Box::new(out)))
            }
            Ty::Optional(t) => match args {
                [] => self.zero_value(ty),
                [e] => {
                    let v = self.eval(e)?;
                    let _ = t;
                    self.convert(v, ty)
                }
                _ => Err(self.err(RtKind::BadArg, "an optional is built from one value")),
            },
            Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) => self.construct_seq(ty, t, args),
            Ty::Set(_) | Ty::UnorderedSet(_) | Ty::Map(..) | Ty::UnorderedMap(..) => match args {
                [] => self.zero_value(ty),
                [e] => {
                    let v = self.eval(e)?;
                    if matches_ty(&v, ty) {
                        self.charge(leaves(&v))?;
                        Ok(v)
                    } else {
                        Err(self.err(RtKind::BadArg, "unsupported container constructor"))
                    }
                }
                [a, b] => {
                    let ia = self.eval(a)?;
                    let ib = self.eval(b)?;
                    let vals = self.range_values(&ia, &ib)?;
                    self.list_from_values(ty, vals)
                }
                _ => Err(self.err(RtKind::BadArg, "unsupported container constructor")),
            },
            Ty::Stack(_) | Ty::Queue(_) => match args {
                [] => self.zero_value(ty),
                [e] => {
                    let v = self.eval(e)?;
                    if matches_ty(&v, ty) {
                        self.charge(leaves(&v))?;
                        Ok(v)
                    } else {
                        Err(self.err(RtKind::BadArg, "unsupported adaptor constructor"))
                    }
                }
                _ => Err(self.err(RtKind::BadArg, "unsupported adaptor constructor")),
            },
            Ty::PriorityQueue(t, _) => self.construct_pq(ty, t, args),
            Ty::Functor(f, t) => Ok(Value::Functor(*f, Rc::new((**t).clone()))),
            Ty::Function(..) => match args {
                [] => self.zero_value(ty),
                [e] => self.eval(e),
                _ => Err(self.err(RtKind::BadArg, "a std::function is built from one callable")),
            },
            Ty::Array(..) => self.list_value(args, ty),
            Ty::Struct(name) => {
                let sid = self.struct_id(name)?;
                if args.is_empty() {
                    return self.struct_default(sid, true);
                }
                let mut vs = Vec::with_capacity(args.len());
                for e in args {
                    vs.push(self.eval(e)?);
                }
                self.construct_struct(sid, vs)
            }
            Ty::Lambda(_) => match args {
                [e] => self.eval(e),
                _ => Err(self.err(RtKind::BadArg, "a lambda cannot be default constructed")),
            },
            _ => Err(self.err(RtKind::BadArg, format!("cannot construct a {ty} here"))),
        }
    }

    fn construct_string(&mut self, args: &'p [Expr]) -> R<Value> {
        match args {
            [] => Ok(Value::string(Vec::new())),
            [e] => {
                let v = self.eval(e)?;
                match v.bytes() {
                    Some(b) => {
                        self.charge(b.len() as u64)?;
                        Ok(Value::string(b.to_vec()))
                    }
                    None => {
                        match v.as_i128() {
                            Some(_) => Err(self
                                .err(RtKind::BadArg, "a string is not built from a single number")),
                            None => Err(self.err(RtKind::BadArg, "unsupported string constructor")),
                        }
                    }
                }
            }
            [a, b] => {
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                if let (Some(n), Some(c)) = (va.as_i128(), vb.as_i128()) {
                    let n = self.count_arg(n)?;
                    self.reserve(n / 8)?;
                    self.charge(n)?;
                    return Ok(Value::string(vec![c as u8; n as usize]));
                }
                if let (Some(s), Some(pos)) = (va.bytes(), vb.as_i128()) {
                    let pos = usize::try_from(pos).unwrap_or(usize::MAX);
                    if pos > s.len() {
                        return Err(self.err(
                            RtKind::Index,
                            "the start position is past the end of the string",
                        ));
                    }
                    self.charge((s.len() - pos) as u64)?;
                    return Ok(Value::string(s[pos..].to_vec()));
                }
                // (first, last) iterators
                let vals = self.range_values(&va, &vb)?;
                let mut out = Vec::with_capacity(vals.len());
                for v in vals {
                    out.push(v.as_i128().unwrap_or(0) as u8);
                }
                Ok(Value::string(out))
            }
            [a, b, c] => {
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                let vc = self.eval(c)?;
                match (va.bytes(), vb.as_i128(), vc.as_i128()) {
                    (Some(s), Some(pos), Some(len)) => {
                        let pos = usize::try_from(pos).unwrap_or(usize::MAX);
                        if pos > s.len() {
                            return Err(self.err(
                                RtKind::Index,
                                "the start position is past the end of the string",
                            ));
                        }
                        let len = usize::try_from(len)
                            .unwrap_or(usize::MAX)
                            .min(s.len() - pos);
                        self.charge(len as u64)?;
                        Ok(Value::string(s[pos..pos + len].to_vec()))
                    }
                    _ => Err(self.err(RtKind::BadArg, "unsupported string constructor")),
                }
            }
            _ => Err(self.err(RtKind::BadArg, "unsupported string constructor")),
        }
    }

    /// A count argument: negative becomes a huge size (wraps like size_t) and is refused.
    pub(super) fn count_arg(&self, n: i128) -> R<u64> {
        if n < 0 || n > i128::from(i64::MAX) {
            return Err(self.err(
                RtKind::Memory,
                "that size is negative or enormous: the allocation fails (length_error)",
            ));
        }
        Ok(n as u64)
    }

    fn construct_seq(&mut self, ty: &Ty, t: &Ty, args: &'p [Expr]) -> R<Value> {
        match args {
            [] => self.zero_value(ty),
            [e] => {
                let v = self.eval(e)?;
                if let Some(n) = v.as_i128() {
                    let n = self.count_arg(n)?;
                    self.reserve(n)?;
                    self.charge(n)?;
                    let mut vs = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        vs.push(self.zero_value(t)?);
                    }
                    return self.seq_from_vec(ty, vs);
                }
                if matches_ty(&v, ty) {
                    self.charge(leaves(&v))?;
                    return Ok(v);
                }
                Err(self.err(RtKind::BadArg, "unsupported container constructor"))
            }
            [a, b] => {
                let va = self.eval(a)?;
                if let Some(n) = va.as_i128() {
                    let x = self.eval_conv(b, t)?;
                    let n = self.count_arg(n)?;
                    let per = leaves(&x).max(1);
                    self.reserve(n.saturating_mul(per))?;
                    self.charge(n.saturating_mul(per))?;
                    let mut vs = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        vs.push(x.clone());
                    }
                    return self.seq_from_vec(ty, vs);
                }
                let vb = self.eval(b)?;
                let vals = self.range_values(&va, &vb)?;
                let mut out = Vec::with_capacity(vals.len());
                for v in vals {
                    out.push(self.convert(v, t)?);
                }
                self.seq_from_vec(ty, out)
            }
            _ => Err(self.err(RtKind::BadArg, "unsupported container constructor")),
        }
    }

    pub(super) fn seq_from_vec(&mut self, ty: &Ty, vs: Vec<Value>) -> R<Value> {
        let mut c = empty_container(ty).unwrap_or(Value::Void);
        match &mut c {
            Value::Vector(x) => x.v = vs,
            Value::Deque(x) => x.v = vs.into(),
            Value::List(x) => {
                for v in vs {
                    x.insert_before(NIL, v);
                }
            }
            _ => return Err(self.internal("not a sequence type")),
        }
        Ok(c)
    }

    /// Build any container type from a list of element values (pairs for maps).
    pub(super) fn list_from_values(&mut self, ty: &Ty, vals: Vec<Value>) -> R<Value> {
        match ty {
            Ty::Vector(t) | Ty::Deque(t) | Ty::List(t) | Ty::Stack(t) | Ty::Queue(t) => {
                let mut out = Vec::with_capacity(vals.len());
                for v in vals {
                    out.push(self.convert(v, t)?);
                }
                self.seq_from_vec(ty, out)
            }
            Ty::Set(k) | Ty::UnorderedSet(k) => {
                let mut c = empty_container(ty).unwrap_or(Value::Void);
                for v in vals {
                    let v = self.convert(v, k)?;
                    self.insert_elem(&mut c, v);
                }
                Ok(c)
            }
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
                let pt = Ty::Pair(k.clone(), v.clone());
                let mut c = empty_container(ty).unwrap_or(Value::Void);
                for x in vals {
                    let x = self.convert(x, &pt)?;
                    self.insert_elem(&mut c, x);
                }
                Ok(c)
            }
            Ty::Str => {
                let mut out = Vec::new();
                for v in vals {
                    out.push(v.as_i128().unwrap_or(0) as u8);
                }
                Ok(Value::string(out))
            }
            _ => Err(self.err(RtKind::BadArg, "cannot build this container from values")),
        }
    }

    fn construct_pq(&mut self, ty: &Ty, t: &Ty, args: &'p [Expr]) -> R<Value> {
        let mut pq = self.zero_value(ty)?;
        match args {
            [] => {}
            [e] => {
                let v = self.eval(e)?;
                match v {
                    Value::Closure(_) | Value::FnRef(_) => {
                        if let Value::Pq(p) = &mut pq {
                            p.cmp = PqCmp::Fn(v);
                        }
                    }
                    Value::Functor(f, _) => {
                        if let Value::Pq(p) = &mut pq {
                            p.cmp = if f == Functor::Less {
                                PqCmp::Less
                            } else {
                                PqCmp::Greater
                            };
                        }
                    }
                    Value::Pq(_) => return Ok(v),
                    _ => {
                        return Err(
                            self.err(RtKind::BadArg, "unsupported priority_queue constructor")
                        );
                    }
                }
            }
            [a, b] => {
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                if matches!(va, Value::Iter(_)) {
                    let vals = self.range_values(&va, &vb)?;
                    let pl = self.temp(pq);
                    for v in vals {
                        let v = self.convert(v, t)?;
                        self.pq_push(&pl, v)?;
                    }
                    return self.load(&pl).map_err(|f| self.fail(f));
                }
                if let Value::Pq(p) = &mut pq {
                    p.cmp = PqCmp::Fn(va);
                }
                let _ = vb;
            }
            _ => return Err(self.err(RtKind::BadArg, "unsupported priority_queue constructor")),
        }
        Ok(pq)
    }

    /// Wrap a value as an optional of its own type.
    pub(super) fn some(&self, v: Value) -> Value {
        Value::Opt(Box::new(OptV {
            ety: Rc::new(v.ty()),
            v: Some(v),
        }))
    }

    pub(super) fn rv_val(&mut self, rv: Rv) -> R<Value> {
        self.rv_value(rv)
    }
}

fn conv_score(v: &Value, t: &Ty) -> i32 {
    match_score(v, t)
}
