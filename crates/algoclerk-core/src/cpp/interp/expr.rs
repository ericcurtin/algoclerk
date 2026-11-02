// SPDX-License-Identifier: Apache-2.0
//! Expression evaluation.

use super::resolve::{Src, VarRef};
use super::rval::*;
use super::{Loc, Machine, R, RtKind, Rv, ops};
use crate::cpp::ast::*;
use std::rc::Rc;

/// The C++ type of an integer literal.
pub(super) fn int_lit(value: u64, suffix: IntSuffix, base: IntBase) -> Value {
    let dec = base == IntBase::Dec;
    let v = i128::from(value);
    let order: &[Sk] = match (suffix, dec) {
        (IntSuffix::None, true) => &[Sk::Int, Sk::LL, Sk::Ull],
        (IntSuffix::None, false) => &[Sk::Int, Sk::UInt, Sk::LL, Sk::Ull],
        (IntSuffix::U, _) => &[Sk::UInt, Sk::Ull],
        (IntSuffix::LL, true) => &[Sk::LL, Sk::Ull],
        (IntSuffix::LL, false) => &[Sk::LL, Sk::Ull],
        (IntSuffix::ULL, _) => &[Sk::Ull],
    };
    for sk in order {
        if sk.fits(v) {
            return sk.make(v);
        }
    }
    Value::Ull(value)
}

fn lib_const(name: &str) -> Option<Value> {
    Some(match name {
        "INT_MAX" => Value::Int(i32::MAX),
        "INT_MIN" => Value::Int(i32::MIN),
        "UINT_MAX" => Value::UInt(u32::MAX),
        "LLONG_MAX" => Value::LL(i64::MAX),
        "LLONG_MIN" => Value::LL(i64::MIN),
        "ULLONG_MAX" => Value::Ull(u64::MAX),
        "SIZE_MAX" => Value::Size(u64::MAX),
        "CHAR_MAX" => Value::Int(127),
        "CHAR_MIN" => Value::Int(-128),
        "CHAR_BIT" => Value::Int(8),
        "nullopt" => nullopt(),
        "npos" => Value::Size(u64::MAX),
        _ => return None,
    })
}

pub(super) fn nullopt() -> Value {
    Value::Opt(Box::new(OptV {
        ety: Rc::new(Ty::Unknown),
        v: None,
    }))
}

impl<'p> Machine<'p> {
    pub(super) fn truthy(&self, v: &Value) -> R<bool> {
        match v {
            Value::Bool(b) => Ok(*b),
            Value::Uninit(t) => Err(self.err(
                RtKind::Uninit,
                format!("this {t} value was never set, so testing it is undefined behaviour"),
            )),
            Value::PNull => Ok(false),
            Value::PHeap(..) | Value::PPlace(_) => Ok(true),
            Value::Opt(o) => Ok(o.v.is_some()),
            v => match v.as_i128() {
                Some(x) => Ok(x != 0),
                None => Err(self.err(RtKind::BadArg, "this value cannot be used as a condition")),
            },
        }
    }

    pub(super) fn eval_cond(&mut self, e: &'p Expr) -> R<bool> {
        let v = self.eval(e)?;
        self.truthy(&v)
    }

    /// Evaluate to a value (copying, and charging the copy, when the expression designates an object).
    pub(super) fn eval(&mut self, e: &'p Expr) -> R<Value> {
        match &e.kind {
            ExprKind::Int {
                value,
                suffix,
                base,
            } => Ok(int_lit(*value, *suffix, *base)),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Char(c) => Ok(Value::Char(*c as i8)),
            ExprKind::Var(_) => {
                if let VarRef::Local(s) = self.rs.var(e.id)
                    && let Some(sl) = self.stack.get(self.base + s as usize)
                {
                    match &sl.v {
                        Value::Int(i) => return Ok(Value::Int(*i)),
                        Value::Bool(b) => return Ok(Value::Bool(*b)),
                        Value::LL(i) => return Ok(Value::LL(*i)),
                        Value::Size(i) => return Ok(Value::Size(*i)),
                        Value::Char(i) => return Ok(Value::Char(*i)),
                        _ => {}
                    }
                }
                let rv = self.eval_rv(e)?;
                self.rv_value(rv)
            }
            _ => {
                let rv = self.eval_rv(e)?;
                self.rv_value(rv)
            }
        }
    }

    pub(super) fn eval_place(&mut self, e: &'p Expr) -> R<Place> {
        let rv = self.eval_rv(e)?;
        Ok(self.rv_place(rv))
    }

    pub(super) fn eval_rv(&mut self, e: &'p Expr) -> R<Rv> {
        match &e.kind {
            ExprKind::Int {
                value,
                suffix,
                base,
            } => Ok(Rv::Val(int_lit(*value, *suffix, *base))),
            ExprKind::Bool(b) => Ok(Rv::Val(Value::Bool(*b))),
            ExprKind::Char(c) => Ok(Rv::Val(Value::Char(*c as i8))),
            ExprKind::Str(s) => {
                self.charge(1)?;
                Ok(Rv::Val(Value::string(s.as_bytes().to_vec())))
            }
            ExprKind::NullPtr => Ok(Rv::Val(Value::PNull)),
            ExprKind::Var(name) => self.eval_var(e, name),
            ExprKind::Path(segs) => self.eval_path(segs),
            ExprKind::Unary(op, a) => self.eval_unary(*op, a),
            ExprKind::Binary(op, a, b) => self.eval_binary(*op, a, b),
            ExprKind::Assign { op, lhs, rhs } => self.eval_assign(*op, lhs, rhs),
            ExprKind::Ternary(c, a, b) => self.eval_ternary(c, a, b),
            ExprKind::Index(a, i) => self.eval_index(a, i),
            ExprKind::Call { callee, args } => self.eval_call(e, callee, args),
            ExprKind::Member { obj, name, arrow } => self.eval_member(obj, name, *arrow),
            ExprKind::Cast { ty, e: a, .. } => {
                self.charge(1)?;
                let v = self.eval(a)?;
                let v = self.check_init_val(v)?;
                if ty.ty == Ty::Void {
                    return Ok(Rv::Val(Value::Void));
                }
                Ok(Rv::Val(self.convert(v, &ty.ty)?))
            }
            ExprKind::Lambda(_) => self.eval_lambda(e),
            ExprKind::InitList(elems) => {
                self.charge(1)?;
                Ok(Rv::Val(self.list_value(elems, &Ty::Auto)?))
            }
            ExprKind::Construct { ty, args, brace } => {
                self.charge(1)?;
                Ok(Rv::Val(self.construct_ty(&ty.ty, args, *brace)?))
            }
            ExprKind::New { ty, args, brace } => {
                self.charge(1)?;
                let v = self.construct_ty(&ty.ty, args, *brace)?;
                Ok(Rv::Val(self.alloc_heap(v)))
            }
            ExprKind::Delete(a) => {
                self.charge(1)?;
                let v = self.eval(a)?;
                match v {
                    Value::PNull => {}
                    Value::PHeap(i, g) => self.free_heap(i, g).map_err(|f| self.fail(f))?,
                    Value::Uninit(_) => {
                        return Err(
                            self.err(RtKind::Uninit, "deleting a pointer that was never set")
                        );
                    }
                    _ => {
                        return Err(self.err(
                            RtKind::Dangling,
                            "only objects made with `new` can be deleted",
                        ));
                    }
                }
                Ok(Rv::Val(Value::Void))
            }
        }
    }

    #[inline(never)]
    fn eval_var(&mut self, e: &'p Expr, name: &str) -> R<Rv> {
        match self.rs.var(e.id) {
            VarRef::Func(g) => Ok(Rv::Val(Value::FnRef(g))),
            VarRef::This => Ok(Rv::Val(Value::PPlace(Box::new(
                self.slot_place(self.base)?,
            )))),
            VarRef::Unset => match lib_const(name) {
                Some(v) => Ok(Rv::Val(v)),
                None => Err(self.err(
                    RtKind::BadArg,
                    format!("the name `{name}` means nothing here"),
                )),
            },
            vr => Ok(Rv::Place(self.var_place(vr)?)),
        }
    }

    fn eval_path(&mut self, segs: &[PathSeg]) -> R<Rv> {
        let names: Vec<&str> = segs.iter().map(|s| s.name.as_str()).collect();
        let names: &[&str] = if names.first() == Some(&"std") {
            &names[1..]
        } else {
            &names
        };
        match names {
            ["string", "npos"] | ["npos"] => Ok(Rv::Val(Value::Size(u64::MAX))),
            ["nullopt"] => Ok(Rv::Val(nullopt())),
            [n] => match lib_const(n) {
                Some(v) => Ok(Rv::Val(v)),
                None => Err(self.err(RtKind::BadArg, format!("the name `{n}` means nothing here"))),
            },
            _ => Err(self.err(
                RtKind::BadArg,
                format!("the name `{}` cannot be used as a value", names.join("::")),
            )),
        }
    }

    #[inline(never)]
    fn eval_unary(&mut self, op: UnOp, a: &'p Expr) -> R<Rv> {
        self.charge(1)?;
        match op {
            UnOp::Deref => {
                let rv = self.eval_rv(a)?;
                Ok(Rv::Place(self.deref_rv(rv)?))
            }
            UnOp::AddrOf => {
                let p = self.eval_place(a)?;
                Ok(Rv::Val(self.addr_of(p)))
            }
            UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => {
                let p = self.eval_place(a)?;
                let old = self.read(&p)?;
                let inc = matches!(op, UnOp::PreInc | UnOp::PostInc);
                let new = match &old {
                    Value::Iter(it) => {
                        let n = self.iter_advance(it, if inc { 1 } else { -1 })?;
                        Value::Iter(Box::new(n))
                    }
                    v if v.sk().is_some() => {
                        let one = Value::Int(1);
                        let r = ops::binary(if inc { BinOp::Add } else { BinOp::Sub }, v, &one)
                            .map_err(|f| self.fail(f))?;
                        match v.sk() {
                            Some(sk) => ops::convert(&r, sk).unwrap_or(r),
                            None => r,
                        }
                    }
                    _ => return Err(self.err(RtKind::BadArg, "cannot increment this value")),
                };
                self.store(&p, new).map_err(|f| self.fail(f))?;
                if matches!(op, UnOp::PreInc | UnOp::PreDec) {
                    Ok(Rv::Place(p))
                } else {
                    Ok(Rv::Val(old))
                }
            }
            UnOp::Not => {
                let v = self.eval(a)?;
                Ok(Rv::Val(Value::Bool(!self.truthy(&v)?)))
            }
            UnOp::Neg | UnOp::Plus | UnOp::BitNot => {
                let v = self.eval(a)?;
                let v = self.check_init_val(v)?;
                Ok(Rv::Val(ops::unary(op, &v).map_err(|f| self.fail(f))?))
            }
        }
    }

    /// The object a pointer, iterator or optional designates (`*x`, `x->m`).
    pub(super) fn deref_rv(&mut self, rv: Rv) -> R<Place> {
        enum K {
            Opt,
            Iter(Box<IterV>),
            Other,
        }
        let k = match self.rv_ref_norm(&rv)? {
            Value::Opt(_) => K::Opt,
            Value::Iter(it) => K::Iter(it.clone()),
            _ => K::Other,
        };
        match k {
            K::Opt => {
                let p = self.rv_place(rv);
                Ok(p.with(Step::Opt))
            }
            K::Iter(it) => self.iter_deref(&it),
            K::Other => {
                let v = self.rv_value(rv)?;
                self.deref_ptr(&v)
            }
        }
    }

    pub(super) fn addr_of(&self, p: Place) -> Value {
        match (&p.root, p.path.is_empty()) {
            (Root::Heap(i, g), true) => Value::PHeap(*i, *g),
            _ => Value::PPlace(Box::new(p)),
        }
    }

    fn peek_scalar(&self, rv: &Rv) -> Option<Value> {
        match rv {
            Rv::Val(v) => v.sk().map(|_| v.clone()),
            Rv::Place(p) => match self.loc(p) {
                Ok(Loc::V(v)) => v.sk().map(|_| v.clone()),
                Ok(Loc::B(b)) => Some(Value::Char(b as i8)),
                Err(_) => None,
            },
        }
    }

    #[inline(never)]
    fn eval_binary(&mut self, op: BinOp, a: &'p Expr, b: &'p Expr) -> R<Rv> {
        match op {
            BinOp::And => {
                self.charge(1)?;
                let l = self.eval_cond(a)?;
                if !l {
                    return Ok(Rv::Val(Value::Bool(false)));
                }
                let r = self.eval_cond(b)?;
                return Ok(Rv::Val(Value::Bool(r)));
            }
            BinOp::Or => {
                self.charge(1)?;
                let l = self.eval_cond(a)?;
                if l {
                    return Ok(Rv::Val(Value::Bool(true)));
                }
                let r = self.eval_cond(b)?;
                return Ok(Rv::Val(Value::Bool(r)));
            }
            BinOp::Comma => {
                self.charge(1)?;
                let _ = self.eval_rv(a)?;
                return self.eval_rv(b);
            }
            _ => {}
        }
        self.charge(1)?;
        let ra = self.eval_rv(a)?;
        let rb = self.eval_rv(b)?;
        if let (Some(x), Some(y)) = (self.peek_scalar(&ra), self.peek_scalar(&rb)) {
            return Ok(Rv::Val(ops::binary(op, &x, &y).map_err(|f| self.fail(f))?));
        }
        self.binary_rv(op, ra, rb)
    }

    fn binary_rv(&mut self, op: BinOp, ra: Rv, rb: Rv) -> R<Rv> {
        let (val, cost) = {
            let x = self.rv_ref_norm(&ra)?;
            let y = self.rv_ref_norm(&rb)?;
            if matches!(x, Value::Uninit(_)) || matches!(y, Value::Uninit(_)) {
                return Err(self.err(
                    RtKind::Uninit,
                    "this value was never set, so using it is undefined behaviour",
                ));
            }
            if matches!(x, Value::Iter(_)) || matches!(y, Value::Iter(_)) {
                (self.iter_binary(op, x, y)?, 1)
            } else {
                pure_binary(op, x, y).map_err(|f| self.fail(f))?
            }
        };
        self.charge(cost)?;
        Ok(Rv::Val(val))
    }

    fn rv_ref_norm<'a>(&'a self, rv: &'a Rv) -> R<&'a Value> {
        match rv {
            Rv::Val(v) => Ok(v),
            Rv::Place(p) => match self.loc(p).map_err(|f| self.fail(f))? {
                Loc::V(v) => Ok(v),
                Loc::B(_) => Err(self.err(
                    RtKind::BadArg,
                    "unsupported operation on a string character",
                )),
            },
        }
    }

    /// Apply an arithmetic operator to two values (used by compound assignment and algorithms).
    pub(super) fn binary_values(&mut self, op: BinOp, x: Value, y: Value) -> R<Value> {
        if x.sk().is_some() && y.sk().is_some() {
            return ops::binary(op, &x, &y).map_err(|f| self.fail(f));
        }
        match self.binary_rv(op, Rv::Val(x), Rv::Val(y))? {
            Rv::Val(v) => Ok(v),
            Rv::Place(p) => self.load(&p).map_err(|f| self.fail(f)),
        }
    }

    fn assign_target_ty(&self, p: &Place) -> R<Option<Ty>> {
        match self.loc(p).map_err(|f| self.fail(f))? {
            Loc::B(_) => Ok(Some(Ty::Char)),
            Loc::V(v) => Ok(match v {
                Value::Void | Value::Ref(_) => None,
                Value::Uninit(t) => Some((**t).clone()),
                _ => Some(v.ty()),
            }),
        }
    }

    /// Convert `v` to the type of the object at `p` (for assignment).
    fn convert_for_assign(&mut self, p: &Place, v: Value) -> R<Value> {
        if let Some(sk) = v.sk()
            && let Ok(Loc::V(old)) = self.loc(p)
        {
            match old.sk() {
                Some(t) if t == sk => return Ok(v),
                Some(t) => return Ok(ops::convert(&v, t).unwrap_or(v)),
                None => {}
            }
        }
        match self.assign_target_ty(p)? {
            Some(ty) => self.convert(v, &ty),
            None => Ok(v),
        }
    }

    #[inline(never)]
    fn eval_assign(&mut self, op: Option<BinOp>, lhs: &'p Expr, rhs: &'p Expr) -> R<Rv> {
        self.charge(1)?;
        match op {
            None => {
                if let ExprKind::InitList(elems) = &rhs.kind {
                    let p = self.eval_place(lhs)?;
                    let ty = self
                        .assign_target_ty(&p)?
                        .ok_or_else(|| self.internal("assignment target has no type"))?;
                    let v = self.list_value(elems, &ty)?;
                    self.store(&p, v).map_err(|f| self.fail(f))?;
                    return Ok(Rv::Place(p));
                }
                let rv = self.eval_rv(rhs)?;
                let v = self.rv_value(rv)?;
                let v = self.check_init_val(v)?;
                let p = self.eval_place(lhs)?;
                let v = self.convert_for_assign(&p, v)?;
                self.store(&p, v).map_err(|f| self.fail(f))?;
                Ok(Rv::Place(p))
            }
            Some(bop) => {
                let rv = self.eval_rv(rhs)?;
                let p = self.eval_place(lhs)?;
                // in place string append
                if bop == BinOp::Add
                    && let Ok(Loc::V(Value::Str(_))) = self.loc(&p)
                {
                    let add: Vec<u8> = match &rv {
                        Rv::Val(v) => str_part(v),
                        Rv::Place(pl) => match self.loc(pl) {
                            Ok(Loc::V(v)) => str_part(v),
                            Ok(Loc::B(b)) => Some(vec![b]),
                            Err(f) => return Err(self.fail(f)),
                        },
                    }
                    .ok_or_else(|| self.err(RtKind::BadArg, "cannot append this to a string"))?;
                    self.charge(add.len() as u64)?;
                    if let Value::Str(s) = self.get_mut(&p)? {
                        s.s.extend_from_slice(&add);
                        s.ver = s.ver.wrapping_add(1);
                    }
                    self.reserve((add.len() / 8) as u64)?;
                    return Ok(Rv::Place(p));
                }
                let rhs_v = self.rv_value(rv)?;
                let old = self.read(&p)?;
                let new = match (&old, &rhs_v) {
                    (Value::Iter(it), r) if r.sk().is_some() => {
                        let n = r.as_i128().unwrap_or(0) as i64;
                        let n = if bop == BinOp::Sub { -n } else { n };
                        Value::Iter(Box::new(self.iter_advance(it, n)?))
                    }
                    (o, r) if o.sk().is_some() && r.sk().is_some() => {
                        let res = ops::binary(bop, o, r).map_err(|f| self.fail(f))?;
                        match o.sk() {
                            Some(sk) => ops::convert(&res, sk).unwrap_or(res),
                            None => res,
                        }
                    }
                    _ => self.binary_values(bop, old, rhs_v)?,
                };
                self.store(&p, new).map_err(|f| self.fail(f))?;
                Ok(Rv::Place(p))
            }
        }
    }

    fn static_ty(&self, e: &Expr) -> Option<Ty> {
        match &e.kind {
            ExprKind::Int {
                value,
                suffix,
                base,
            } => Some(int_lit(*value, *suffix, *base).ty()),
            ExprKind::Bool(_) => Some(Ty::Bool),
            ExprKind::Char(_) => Some(Ty::Char),
            ExprKind::Str(_) => Some(Ty::Str),
            ExprKind::Var(_) => match self.rs.var(e.id) {
                vr @ (VarRef::Local(_) | VarRef::Capture(_) | VarRef::Global(_)) => {
                    let p = self.var_place(vr).ok()?;
                    match self.loc(&p).ok()? {
                        Loc::V(Value::Uninit(t)) => Some((**t).clone()),
                        Loc::V(v) => Some(v.ty()),
                        Loc::B(_) => Some(Ty::Char),
                    }
                }
                _ => None,
            },
            ExprKind::Unary(op, a) => match op {
                UnOp::Not => Some(Ty::Bool),
                UnOp::Neg | UnOp::Plus | UnOp::BitNot => {
                    let t = Sk::of_ty(&self.static_ty(a)?)?;
                    Some(ops::promote(t).ty())
                }
                UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => self.static_ty(a),
                _ => None,
            },
            ExprKind::Binary(op, a, b) => match op {
                BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Le
                | BinOp::Gt
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or => Some(Ty::Bool),
                BinOp::Comma => self.static_ty(b),
                BinOp::Shl | BinOp::Shr => Some(ops::promote(Sk::of_ty(&self.static_ty(a)?)?).ty()),
                _ => {
                    let (x, y) = (self.static_ty(a)?, self.static_ty(b)?);
                    match (Sk::of_ty(&x), Sk::of_ty(&y)) {
                        (Some(p), Some(q)) => Some(ops::common(p, q).ty()),
                        _ => None,
                    }
                }
            },
            ExprKind::Cast { ty, .. } => Some(ty.ty.clone()),
            ExprKind::Ternary(_, a, b) => {
                let (x, y) = (self.static_ty(a)?, self.static_ty(b)?);
                if x == y {
                    return Some(x);
                }
                Some(ops::common(Sk::of_ty(&x)?, Sk::of_ty(&y)?).ty())
            }
            ExprKind::Index(a, _) => match self.static_ty(a)? {
                Ty::Vector(t) | Ty::Array(t, _) | Ty::Deque(t) => Some(*t),
                Ty::Str | Ty::StrView => Some(Ty::Char),
                Ty::Map(_, v) | Ty::UnorderedMap(_, v) => Some(*v),
                _ => None,
            },
            ExprKind::Member { obj, name, arrow } => {
                let ot = self.static_ty(obj)?;
                let ot = if *arrow {
                    match ot {
                        Ty::Ptr(t) => *t,
                        _ => return None,
                    }
                } else {
                    ot
                };
                match (&ot, name.as_str()) {
                    (Ty::Pair(a, _), "first") => Some((**a).clone()),
                    (Ty::Pair(_, b), "second") => Some((**b).clone()),
                    (Ty::Struct(n), f) => {
                        let sid = self.struct_ix.get(n.as_str())?;
                        let d = self.rs.structs[*sid as usize].decl;
                        d.fields
                            .iter()
                            .find(|x| x.name == f)
                            .map(|x| x.spec.ty.clone())
                    }
                    _ => None,
                }
            }
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Var(_) => match self.rs.var(callee.id) {
                    VarRef::Func(g) => {
                        let f = *self.rs.groups[g as usize].first()?;
                        let t = &self.rs.fns[f as usize].decl.ret.ty;
                        (!matches!(t, Ty::Auto)).then(|| t.clone())
                    }
                    _ => None,
                },
                ExprKind::Member { obj, name, .. } => match name.as_str() {
                    "size" | "length" | "count" => Some(Ty::Size),
                    "back" | "front" | "top" | "at" | "value" => match self.static_ty(obj)? {
                        Ty::Vector(t)
                        | Ty::Array(t, _)
                        | Ty::Deque(t)
                        | Ty::List(t)
                        | Ty::Stack(t)
                        | Ty::Queue(t)
                        | Ty::PriorityQueue(t, _)
                        | Ty::Optional(t) => Some(*t),
                        Ty::Str | Ty::StrView => Some(Ty::Char),
                        Ty::Map(_, v) | Ty::UnorderedMap(_, v) => Some(*v),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
            ExprKind::Assign { lhs, .. } => self.static_ty(lhs),
            _ => None,
        }
    }

    #[inline(never)]
    fn eval_ternary(&mut self, c: &'p Expr, a: &'p Expr, b: &'p Expr) -> R<Rv> {
        self.charge(1)?;
        let cond = self.eval_cond(c)?;
        let (taken, other) = if cond { (a, b) } else { (b, a) };
        let rv = self.eval_rv(taken)?;
        // common type of two different scalar types
        if let Some(v) = self.peek_scalar(&rv)
            && let Some(sk) = v.sk()
            && let Some(Some(osk)) = self.static_ty(other).map(|t| Sk::of_ty(&t))
            && osk != sk
        {
            let c = ops::common(sk, osk);
            return Ok(Rv::Val(ops::convert(&v, c).unwrap_or(v)));
        }
        // a string literal against a std::string, an lvalue against a prvalue: value result
        if let Rv::Place(_) = &rv
            && !matches!(
                other.kind,
                ExprKind::Var(_) | ExprKind::Index(..) | ExprKind::Member { .. }
            )
        {
            let v = self.rv_value(rv)?;
            return Ok(Rv::Val(v));
        }
        Ok(rv)
    }

    #[inline(never)]
    fn eval_index(&mut self, a: &'p Expr, i: &'p Expr) -> R<Rv> {
        self.charge(1)?;
        let base = self.eval_rv(a)?;
        let bp = self.rv_place(base);
        let iv = if let ExprKind::InitList(_) = &i.kind {
            let kty = match self.get(&bp).map_err(|f| self.fail(f))? {
                Value::Map(m) => Some((*m.kty).clone()),
                Value::UMap(m) => Some((*m.kty).clone()),
                _ => None,
            };
            match kty {
                Some(t) => self.eval_conv(i, &t)?,
                None => self.eval(i)?,
            }
        } else {
            self.eval(i)?
        };
        let iv = self.check_init_val(iv)?;
        Ok(Rv::Place(self.index_place(bp, iv)?))
    }

    pub(super) fn index_place(&mut self, bp: Place, iv: Value) -> R<Place> {
        enum K {
            Seq(usize, u32),
            Map,
            UMap,
            Bad,
        }
        let k = match self.get(&bp).map_err(|f| self.fail(f))? {
            Value::Vector(c) => K::Seq(c.v.len(), c.ver),
            Value::Deque(c) => K::Seq(c.v.len(), c.ver),
            Value::Str(c) => K::Seq(c.s.len(), c.ver),
            Value::View(b) => K::Seq(b.len(), 0),
            Value::Map(_) => K::Map,
            Value::UMap(_) => K::UMap,
            _ => K::Bad,
        };
        match k {
            K::Seq(len, ver) => {
                let Some(x) = iv.as_i128() else {
                    return Err(self.err(RtKind::BadArg, "an index must be a number"));
                };
                if x < 0 || x >= len as i128 {
                    let what = if len == 0 {
                        "the container is empty".to_string()
                    } else {
                        format!("valid indexes are 0 to {}", len - 1)
                    };
                    return Err(self.err(
                        RtKind::Index,
                        format!("index {x} is out of range: {what} (a container of size {len})"),
                    ));
                }
                Ok(bp.with(Step::Idx(x as usize, ver)))
            }
            K::Map | K::UMap => {
                let (kty, vty) = match self.get(&bp).map_err(|f| self.fail(f))? {
                    Value::Map(m) => (m.kty.clone(), m.vty.clone()),
                    Value::UMap(m) => (m.kty.clone(), m.vty.clone()),
                    _ => return Err(self.internal("not a map")),
                };
                let kv = self.convert(iv, &kty)?;
                let key = Key(kv);
                let n = match self.get(&bp).map_err(|f| self.fail(f))? {
                    Value::Map(m) => m.m.len(),
                    Value::UMap(m) => m.live,
                    _ => 0,
                };
                self.charge(if matches!(k, K::Map) { lg(n as u64) } else { 0 })?;
                let present = match self.get(&bp).map_err(|f| self.fail(f))? {
                    Value::Map(m) => m.m.contains_key(&key),
                    Value::UMap(m) => m.idx.contains_key(&key),
                    _ => false,
                };
                if !present {
                    let vt = vty.map_or(Ty::Int, |t| (*t).clone());
                    let nv = self.zero_value(&vt)?;
                    self.reserve(1)?;
                    let pair = Value::pair(key.0.clone(), nv);
                    match self.get_mut(&bp)? {
                        Value::Map(m) => {
                            m.m.insert(key.clone(), pair);
                        }
                        Value::UMap(m) => m.push(key.clone(), pair),
                        _ => {}
                    }
                }
                Ok(bp.with(Step::Key(key)).with(Step::Field(1)))
            }
            K::Bad => Err(self.err(RtKind::BadArg, "this value cannot be indexed")),
        }
    }

    #[inline(never)]
    fn eval_member(&mut self, obj: &'p Expr, name: &str, arrow: bool) -> R<Rv> {
        self.charge(1)?;
        let base = if arrow {
            let rv = self.eval_rv(obj)?;
            self.deref_rv(rv)?
        } else {
            self.eval_place(obj)?
        };
        let idx = match self.get(&base).map_err(|f| self.fail(f))? {
            Value::Pair(_) => match name {
                "first" => Some(0),
                "second" => Some(1),
                _ => None,
            },
            Value::Struct(s) => self.rs.structs[s.sid as usize]
                .decl
                .fields
                .iter()
                .position(|f| f.name == name),
            _ => None,
        };
        match idx {
            Some(i) => Ok(Rv::Place(base.with(Step::Field(i as u16)))),
            None => Err(self.err(RtKind::BadArg, format!("this value has no member `{name}`"))),
        }
    }

    #[inline(never)]
    fn eval_lambda(&mut self, e: &'p Expr) -> R<Rv> {
        self.charge(1)?;
        let Some(li) = self.rs.lam(e.id) else {
            return Err(self.internal("unresolved lambda"));
        };
        let ncaps = self.rs.lams[li as usize].caps.len();
        let mut caps = Vec::with_capacity(ncaps);
        for i in 0..ncaps {
            let (src, by_ref) = {
                let c = &self.rs.lams[li as usize].caps[i];
                (c.src, c.by_ref)
            };
            let abs = match src {
                Src::Local(s) => self.base + s as usize,
                Src::Capture(k) => self.base + self.cap_off + k as usize,
            };
            let place = self.slot_place(abs)?;
            if by_ref {
                caps.push(Value::Ref(Box::new(place)));
                continue;
            }
            let v = self.load(&place).map_err(|f| self.fail(f))?;
            let big = !matches!(
                &v,
                Value::Bool(_)
                    | Value::Char(_)
                    | Value::Int(_)
                    | Value::UInt(_)
                    | Value::LL(_)
                    | Value::Ull(_)
                    | Value::Size(_)
                    | Value::PNull
                    | Value::PHeap(..)
                    | Value::Closure(_)
                    | Value::FnRef(_)
                    | Value::Uninit(_)
                    | Value::Functor(..)
            );
            if big {
                self.charge(leaves(&v))?;
                let h = self.alloc_heap(v);
                let hp = self.addr_of_heap(&h);
                caps.push(Value::Ref(Box::new(hp)));
            } else {
                caps.push(v);
            }
        }
        Ok(Rv::Val(Value::Closure(Rc::new(Closure {
            lam: li,
            caps: std::cell::RefCell::new(caps),
        }))))
    }

    fn addr_of_heap(&self, h: &Value) -> Place {
        match h {
            Value::PHeap(i, g) => Place::root(Root::Heap(*i, *g)),
            _ => Place::root(Root::Global(u32::MAX)),
        }
    }

    // ----- calls -----

    #[inline(never)]
    fn eval_call(&mut self, e: &'p Expr, callee: &'p Expr, args: &'p [Expr]) -> R<Rv> {
        self.charge(1)?;
        match &callee.kind {
            ExprKind::Member { obj, name, arrow } => self.call_method(e, obj, name, *arrow, args),
            ExprKind::Var(name) => match self.rs.var(callee.id) {
                VarRef::Func(g) => self.call_group(g, args),
                VarRef::Unset => {
                    if let Some(&sid) = self.struct_ix.get(name.as_str()) {
                        let mut vs = Vec::with_capacity(args.len());
                        for a in args {
                            vs.push(self.eval(a)?);
                        }
                        return Ok(Rv::Val(self.construct_struct(sid, vs)?));
                    }
                    self.lib_call(e, name, None, args)
                }
                _ => self.call_callable_expr(callee, args),
            },
            ExprKind::Path(segs) => {
                let names: Vec<&str> = segs.iter().map(|s| s.name.as_str()).collect();
                let (names, targs): (&[&str], Option<&Vec<TArg>>) = (&names, None);
                let _ = targs;
                let joined: String = if names.first() == Some(&"std") {
                    names[1..].join("::")
                } else {
                    names.join("::")
                };
                let targs = segs.iter().find_map(|s| s.targs.as_ref());
                self.lib_call(e, &joined, targs, args)
            }
            _ => self.call_callable_expr(callee, args),
        }
    }

    fn call_callable_expr(&mut self, callee: &'p Expr, args: &'p [Expr]) -> R<Rv> {
        let rv = self.eval_rv(callee)?;
        let f = match &rv {
            Rv::Val(v) => v.clone(),
            Rv::Place(p) => match self.get_or_char(p)? {
                Some(v) => v.clone(),
                None => return Err(self.err(RtKind::BadArg, "this value cannot be called")),
            },
        };
        self.call_callable(&f, args)
    }

    fn call_callable(&mut self, f: &Value, args: &'p [Expr]) -> R<Rv> {
        match f {
            Value::Closure(c) => {
                let specs: Vec<&'p TypeSpec> = self.rs.lams[c.lam as usize]
                    .lam
                    .params
                    .iter()
                    .map(|p| &p.spec)
                    .collect();
                if specs.len() != args.len() {
                    return Err(self.err(RtKind::BadArg, "wrong number of arguments in this call"));
                }
                let mut bound = Vec::with_capacity(args.len());
                for (s, a) in specs.into_iter().zip(args) {
                    bound.push(self.bind_arg(s, a)?);
                }
                let c = c.clone();
                self.call_closure(&c, bound)
            }
            Value::FnRef(g) => self.call_group(*g, args),
            Value::Uninit(_) => Err(self.err(
                RtKind::Uninit,
                "this std::function holds nothing: calling it would throw bad_function_call",
            )),
            _ => {
                let mut rvs = Vec::with_capacity(args.len());
                for a in args {
                    rvs.push(self.eval_rv(a)?);
                }
                let f = f.clone();
                self.call_value(&f, rvs)
            }
        }
    }

    pub(super) fn bind_arg(&mut self, spec: &'p TypeSpec, a: &'p Expr) -> R<Value> {
        let auto = matches!(spec.ty, Ty::Auto | Ty::Unknown);
        if spec.is_ref {
            if let (ExprKind::InitList(_), false) = (&a.kind, auto) {
                let v = self.eval_conv(a, &spec.ty)?;
                let t = self.temp(v);
                return Ok(Value::Ref(Box::new(t)));
            }
            let rv = self.eval_rv(a)?;
            self.bind_rv(spec, rv)
        } else if auto {
            let v = self.eval(a)?;
            self.check_init_val(v)
        } else {
            self.eval_conv(a, &spec.ty)
        }
    }

    fn call_group(&mut self, g: u32, args: &'p [Expr]) -> R<Rv> {
        let cands: Vec<u32> = self.rs.groups[g as usize]
            .iter()
            .copied()
            .filter(|&f| self.rs.fns[f as usize].decl.params.len() == args.len())
            .collect();
        match cands.len() {
            0 => Err(self.err(
                RtKind::BadArg,
                "no function of that name takes that many arguments",
            )),
            1 => {
                let fi = cands[0];
                let specs: Vec<&'p TypeSpec> = self.rs.fns[fi as usize]
                    .decl
                    .params
                    .iter()
                    .map(|p| &p.spec)
                    .collect();
                let mut bound = Vec::with_capacity(args.len());
                for (s, a) in specs.into_iter().zip(args) {
                    bound.push(self.bind_arg(s, a)?);
                }
                self.call_fn(fi, bound)
            }
            _ => {
                let mut rvs = Vec::with_capacity(args.len());
                for a in args {
                    rvs.push(self.eval_rv(a)?);
                }
                let fi = self.pick_overload_rv(g, &rvs)?;
                let specs: Vec<&'p TypeSpec> = self.rs.fns[fi as usize]
                    .decl
                    .params
                    .iter()
                    .map(|p| &p.spec)
                    .collect();
                let mut bound = Vec::with_capacity(args.len());
                for (s, a) in specs.into_iter().zip(rvs) {
                    bound.push(self.bind_rv(s, a)?);
                }
                self.call_fn(fi, bound)
            }
        }
    }
}

pub(super) fn lg(n: u64) -> u64 {
    u64::from(64 - n.leading_zeros())
}

/// Bytes a string, string_view or character contributes when appended to a string.
fn str_part(v: &Value) -> Option<Vec<u8>> {
    match v {
        Value::Str(s) => Some(s.s.clone()),
        Value::View(b) => Some((**b).clone()),
        v => v.as_i128().map(|x| vec![x as u8]),
    }
}

/// Operators on non-integral operands: strings, containers, pointers, optionals.
fn pure_binary(op: BinOp, x: &Value, y: &Value) -> Result<(Value, u64), ops::Fail> {
    use std::cmp::Ordering::*;
    let bad = || -> ops::Fail {
        (
            RtKind::BadArg,
            format!(
                "the operator `{}` does not work on these operands",
                op.text()
            ),
        )
    };
    if op == BinOp::Add {
        let ax = x.bytes();
        let by = y.bytes();
        match (ax, by) {
            (Some(a), Some(b)) => {
                let mut s = Vec::with_capacity(a.len() + b.len());
                s.extend_from_slice(a);
                s.extend_from_slice(b);
                let c = (a.len() + b.len()) as u64;
                return Ok((Value::string(s), c.max(1)));
            }
            (Some(a), None) => {
                if let (Some(c), Value::Char(_)) = (y.as_i128(), y) {
                    let mut s = a.to_vec();
                    s.push(c as u8);
                    let n = s.len() as u64;
                    return Ok((Value::string(s), n));
                }
            }
            (None, Some(b)) => {
                if let (Some(c), Value::Char(_)) = (x.as_i128(), x) {
                    let mut s = vec![c as u8];
                    s.extend_from_slice(b);
                    let n = s.len() as u64;
                    return Ok((Value::string(s), n));
                }
            }
            _ => {}
        }
        return Err(bad());
    }
    let ord = |o: std::cmp::Ordering| -> Result<Value, ops::Fail> {
        Ok(Value::Bool(match op {
            BinOp::Eq => o == Equal,
            BinOp::Ne => o != Equal,
            BinOp::Lt => o == Less,
            BinOp::Le => o != Greater,
            BinOp::Gt => o == Greater,
            BinOp::Ge => o != Less,
            _ => return Err(bad()),
        }))
    };
    if !matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    ) {
        return Err(bad());
    }
    if x.is_ptr() || y.is_ptr() {
        let null_vs_zero = |a: &Value, b: &Value| a.is_ptr() && b.as_i128() == Some(0);
        let eq = if null_vs_zero(x, y) {
            matches!(x, Value::PNull)
        } else if null_vs_zero(y, x) {
            matches!(y, Value::PNull)
        } else {
            ptr_eq(x, y)
        };
        return match op {
            BinOp::Eq => Ok((Value::Bool(eq), 1)),
            BinOp::Ne => Ok((Value::Bool(!eq), 1)),
            _ => Ok((ord(cmp_val(x, y))?, 1)),
        };
    }
    if let (Value::Opt(a), Value::Opt(_)) = (x, y) {
        let _ = a;
    }
    // an optional against nullopt or a plain value
    let (x, y) = match (x, y) {
        (Value::Opt(_), Value::Opt(_)) => (x, y),
        _ => (x, y),
    };
    let cost = match (x, y) {
        (a, b) if a.bytes().is_some() && b.bytes().is_some() => {
            let (a, b) = (a.bytes().unwrap_or(&[]), b.bytes().unwrap_or(&[]));
            a.len().min(b.len()) as u64 + 1
        }
        (Value::Vector(a), Value::Vector(b)) => a.v.len().min(b.v.len()) as u64 + 1,
        (Value::Deque(a), Value::Deque(b)) => a.v.len().min(b.v.len()) as u64 + 1,
        _ => 1,
    };
    // string vs string comparisons use unsigned bytes via cmp_val
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        let e = eq_val(x, y);
        return Ok((Value::Bool(if op == BinOp::Eq { e } else { !e }), cost));
    }
    Ok((ord(cmp_val(x, y))?, cost))
}
