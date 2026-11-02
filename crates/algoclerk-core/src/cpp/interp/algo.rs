// SPDX-License-Identifier: Apache-2.0
//! Free library functions: `<algorithm>`, `<numeric>`, `<string>`, `<cctype>`, `<utility>`, `<limits>`.

use super::expr::lg;
use super::iter::SeqRange;
use super::ops;
use super::rval::*;
use super::{Loc, Machine, R, RtKind, Rv};
use crate::cpp::ast::*;
use std::collections::VecDeque;
use std::rc::Rc;

/// ceil(log2(n)) for n >= 1, 0 otherwise.
fn cl2(n: u64) -> u64 {
    lg(n.saturating_sub(1))
}

enum RC<'a> {
    R(&'a Value),
    C(Value),
}

impl RC<'_> {
    fn get(&self) -> &Value {
        match self {
            RC::R(v) => v,
            RC::C(v) => v,
        }
    }
}

enum SlK<'a> {
    V(&'a [Value]),
    D(&'a VecDeque<Value>),
    B(&'a [u8]),
}

struct Sl<'a> {
    k: SlK<'a>,
    lo: usize,
    n: usize,
    rev: bool,
}

impl Sl<'_> {
    fn at(&self, k: usize) -> RC<'_> {
        let i = if self.rev {
            self.lo + self.n - 1 - k
        } else {
            self.lo + k
        };
        match &self.k {
            SlK::V(s) => RC::R(&s[i]),
            SlK::D(d) => RC::R(&d[i]),
            SlK::B(b) => RC::C(Value::Char(b[i] as i8)),
        }
    }
}

const CT_UPPER: i32 = 256;
const CT_LOWER: i32 = 512;
const CT_ALPHA: i32 = 1024;
const CT_DIGIT: i32 = 2048;
const CT_XDIGIT: i32 = 4096;
const CT_SPACE: i32 = 8192;
const CT_PUNCT: i32 = 4;
const CT_ALNUM: i32 = 8;

fn ctype_mask(c: u8) -> i32 {
    let mut m = 0;
    if c.is_ascii_uppercase() {
        m |= CT_UPPER | CT_ALPHA | CT_ALNUM;
    }
    if c.is_ascii_lowercase() {
        m |= CT_LOWER | CT_ALPHA | CT_ALNUM;
    }
    if c.is_ascii_digit() {
        m |= CT_DIGIT | CT_ALNUM;
    }
    if c.is_ascii_hexdigit() {
        m |= CT_XDIGIT;
    }
    if matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        m |= CT_SPACE;
    }
    if c.is_ascii_punctuation() {
        m |= CT_PUNCT;
    }
    m
}

impl<'p> Machine<'p> {
    fn val_arg(&mut self, a: &'p Expr) -> R<Value> {
        let v = self.eval(a)?;
        self.check_init_val(v)
    }

    fn comparator_arg(&mut self, args: &'p [Expr], i: usize) -> R<Option<Value>> {
        match args.get(i) {
            Some(a) => Ok(Some(self.eval(a)?)),
            None => Ok(None),
        }
    }

    /// `comp(a, b)` or `a < b`.
    fn less_vals(&mut self, cmp: Option<&Value>, a: Rv, b: Rv) -> R<bool> {
        match cmp {
            None => {
                let x = self.rv_value(a)?;
                let y = self.rv_value(b)?;
                Ok(cmp_val(&x, &y) == std::cmp::Ordering::Less)
            }
            Some(f) => {
                let r = self.call_value(f, vec![a, b])?;
                let r = self.rv_val(r)?;
                self.truthy(&r)
            }
        }
    }

    fn slice_of(&self, r: &SeqRange) -> R<Sl<'_>> {
        let v = self.get(&r.cont).map_err(|f| self.fail(f))?;
        let k = match v {
            Value::Vector(c) => SlK::V(&c.v),
            Value::Deque(c) => SlK::D(&c.v),
            Value::Str(s) => SlK::B(&s.s),
            Value::View(b) => SlK::B(b),
            _ => return Err(self.internal("not a sequence")),
        };
        Ok(Sl {
            k,
            lo: r.lo,
            n: r.hi - r.lo,
            rev: r.rev,
        })
    }

    fn iter_at(&self, r: &SeqRange, k: usize) -> Value {
        Value::Iter(Box::new(IterV {
            cont: r.cont.clone(),
            pos: Pos::Idx(r.p0 + k),
            rev: r.rev,
            ver: r.ver,
        }))
    }

    fn elem_place(&self, r: &SeqRange, k: usize) -> Place {
        let n = r.hi - r.lo;
        let i = if r.rev { r.lo + n - 1 - k } else { r.lo + k };
        r.cont.with(Step::Idx(i, r.ver))
    }

    /// Move the elements of a range out (iteration order).
    fn take_range(&mut self, r: &SeqRange) -> R<Vec<Value>> {
        let e = self.internal("not a sequence");
        let mut out: Vec<Value> = match self.get_mut(&r.cont)? {
            Value::Vector(c) => c.v[r.lo..r.hi]
                .iter_mut()
                .map(|x| std::mem::replace(x, Value::Void))
                .collect(),
            Value::Deque(c) => {
                c.v.range_mut(r.lo..r.hi)
                    .map(|x| std::mem::replace(x, Value::Void))
                    .collect()
            }
            Value::Str(s) => s.s[r.lo..r.hi]
                .iter()
                .map(|&b| Value::Char(b as i8))
                .collect(),
            _ => return Err(e),
        };
        if r.rev {
            out.reverse();
        }
        Ok(out)
    }

    /// Put elements back (iteration order); the count must match.
    fn put_range(&mut self, r: &SeqRange, mut vals: Vec<Value>) -> R<()> {
        if r.rev {
            vals.reverse();
        }
        let e = self.internal("not a sequence");
        if vals.len() != r.hi - r.lo {
            return Err(e);
        }
        match self.get_mut(&r.cont)? {
            Value::Vector(c) => {
                for (slot, v) in c.v[r.lo..r.hi].iter_mut().zip(vals) {
                    *slot = v;
                }
            }
            Value::Deque(c) => {
                for (slot, v) in c.v.range_mut(r.lo..r.hi).zip(vals) {
                    *slot = v;
                }
            }
            Value::Str(s) => {
                for (slot, v) in s.s[r.lo..r.hi].iter_mut().zip(vals) {
                    *slot = v.as_i128().unwrap_or(0) as u8;
                }
            }
            _ => return Err(e),
        }
        Ok(())
    }

    /// Stable sort of owned values (used by `std::sort`, `list::sort`).
    pub(super) fn sort_owned(&mut self, vals: Vec<Value>, cmp: Option<&Value>) -> R<Vec<Value>> {
        let n = vals.len();
        self.charge((n as u64).saturating_mul(cl2(n as u64)))?;
        if n < 2 {
            return Ok(vals);
        }
        let mut vals = vals;
        let Some(f) = cmp else {
            // default operator<: sort directly
            let mut idx: Vec<usize> = (0..n).collect();
            idx.sort_by(|&a, &b| cmp_val(&vals[a], &vals[b]));
            let mut slots: Vec<Option<Value>> = vals.drain(..).map(Some).collect();
            return Ok(idx.into_iter().filter_map(|i| slots[i].take()).collect());
        };
        let ety: TyRc = Rc::new(vals[0].ty());
        let tp = self.temp(Value::vector(ety, vals));
        let mark = self.temps.len();
        let mut idx: Vec<usize> = (0..n).collect();
        {
            let mut less = |m: &mut Self, a: usize, b: usize| -> R<bool> {
                m.less_vals(
                    Some(f),
                    Rv::Place(tp.with(Step::Idx(a, 0))),
                    Rv::Place(tp.with(Step::Idx(b, 0))),
                )
            };
            merge_sort(self, &mut idx, &mut less)?;
        }
        // verify the comparator is a strict weak ordering where cheap to see (not charged)
        let saved = self.cost;
        for w in 0..n {
            let a = idx[w];
            let r = self.less_vals(
                Some(f),
                Rv::Place(tp.with(Step::Idx(a, 0))),
                Rv::Place(tp.with(Step::Idx(a, 0))),
            )?;
            if r {
                self.cost = saved;
                return Err(self.err(
                    RtKind::BadCompare,
                    "the comparator says an element is less than itself: use `<`, not `<=` (it must be a strict weak ordering)",
                ));
            }
            if w + 1 < n {
                let b = idx[w + 1];
                let r = self.less_vals(
                    Some(f),
                    Rv::Place(tp.with(Step::Idx(b, 0))),
                    Rv::Place(tp.with(Step::Idx(a, 0))),
                )?;
                if r {
                    self.cost = saved;
                    return Err(self.err(
                        RtKind::BadCompare,
                        "the comparator is not a strict weak ordering: the sorted result is inconsistent",
                    ));
                }
            }
        }
        self.cost = saved;
        let taken = match self.get_mut(&tp)? {
            Value::Vector(c) => std::mem::take(&mut c.v),
            _ => Vec::new(),
        };
        self.temps.truncate(mark.saturating_sub(1));
        let mut slots: Vec<Option<Value>> = taken.into_iter().map(Some).collect();
        Ok(idx.into_iter().filter_map(|i| slots[i].take()).collect())
    }

    fn range2(&mut self, args: &'p [Expr]) -> R<(Value, Value)> {
        if args.len() < 2 {
            return Err(self.err(RtKind::BadArg, "this algorithm needs a pair of iterators"));
        }
        let a = self.eval(&args[0])?;
        let b = self.eval(&args[1])?;
        Ok((a, b))
    }

    /// Element count and an index->place mapping for algorithms with callbacks; falls back to values.
    fn call_pred(&mut self, f: &Value, arg: Rv) -> R<bool> {
        let r = self.call_value(f, vec![arg])?;
        let r = self.rv_val(r)?;
        self.truthy(&r)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn lib_call(
        &mut self,
        _e: &'p Expr,
        name: &str,
        targs: Option<&'p Vec<TArg>>,
        args: &'p [Expr],
    ) -> R<Rv> {
        let ret = |v: Value| -> R<Rv> { Ok(Rv::Val(v)) };
        match name {
            "max" | "min" => {
                if args.is_empty() || args.len() > 3 {
                    return Err(self.err(RtKind::BadArg, "min and max take two values or a list"));
                }
                let want_max = name == "max";
                let explicit = targs.and_then(|t| match t.first() {
                    Some(TArg::Ty(t)) => Some(t.clone()),
                    _ => None,
                });
                if args.len() == 1
                    || (args.len() == 2 && matches!(args[0].kind, ExprKind::InitList(_)))
                {
                    let l = self.val_arg(&args[0])?;
                    let cmp = self.comparator_arg(args, 1)?;
                    let items: Vec<Value> = match l {
                        Value::Vector(c) => c.v,
                        _ => return Err(self.err(RtKind::BadArg, "expected a list of values")),
                    };
                    self.charge(items.len() as u64)?;
                    let mut it = items.into_iter();
                    let Some(mut best) = it.next() else {
                        return Err(self.err(RtKind::BadArg, "min/max of an empty list"));
                    };
                    for x in it {
                        let better = if want_max {
                            self.less_vals(cmp.as_ref(), Rv::Val(best.clone()), Rv::Val(x.clone()))?
                        } else {
                            self.less_vals(cmp.as_ref(), Rv::Val(x.clone()), Rv::Val(best.clone()))?
                        };
                        if better {
                            best = x;
                        }
                    }
                    return ret(best);
                }
                let (mut a, mut b) = (self.val_arg(&args[0])?, self.val_arg(&args[1])?);
                if let Some(t) = &explicit {
                    a = self.convert(a, t)?;
                    b = self.convert(b, t)?;
                }
                let cmp = self.comparator_arg(args, 2)?;
                let pick_b = if want_max {
                    self.less_vals(cmp.as_ref(), Rv::Val(a.clone()), Rv::Val(b.clone()))?
                } else {
                    self.less_vals(cmp.as_ref(), Rv::Val(b.clone()), Rv::Val(a.clone()))?
                };
                ret(if pick_b { b } else { a })
            }
            "clamp" => {
                let (v, lo, hi) = (
                    self.val_arg(&args[0])?,
                    self.val_arg(&args[1])?,
                    self.val_arg(&args[2])?,
                );
                if cmp_val(&hi, &lo) == std::cmp::Ordering::Less {
                    return Err(self.err(RtKind::BadArg, "clamp needs lo <= hi"));
                }
                ret(if cmp_val(&v, &lo) == std::cmp::Ordering::Less {
                    lo
                } else if cmp_val(&hi, &v) == std::cmp::Ordering::Less {
                    hi
                } else {
                    v
                })
            }
            "abs" | "labs" | "llabs" => {
                self.nargs(name, args, 1, 1)?;
                let v = self.val_arg(&args[0])?;
                let Some(sk) = v.sk() else {
                    return Err(self.err(RtKind::BadArg, "abs of a non-number"));
                };
                let p = ops::promote(sk);
                let x = v.as_i128().unwrap_or(0);
                if !p.signed() {
                    return Err(self.err(RtKind::BadArg, "abs of an unsigned number is ambiguous"));
                }
                if x == p.min() {
                    return Err(self.err(
                        RtKind::Overflow,
                        "abs of the most negative number does not fit: undefined behaviour",
                    ));
                }
                ret(p.make(x.abs()))
            }
            "gcd" | "lcm" => {
                self.nargs(name, args, 2, 2)?;
                let (a, b) = (self.val_arg(&args[0])?, self.val_arg(&args[1])?);
                let (Some(x), Some(y), Some(ka), Some(kb)) =
                    (a.as_i128(), b.as_i128(), a.sk(), b.sk())
                else {
                    return Err(self.err(RtKind::BadArg, "gcd of non-numbers"));
                };
                let (mut p, mut q) = (x.abs(), y.abs());
                while q != 0 {
                    (p, q) = (q, p % q);
                }
                let c = ops::common(ka, kb);
                if name == "gcd" {
                    return ret(c.make(p));
                }
                let r = if p == 0 { 0 } else { (x.abs() / p) * y.abs() };
                if !c.fits(r) {
                    return Err(self.err(RtKind::Overflow, "lcm does not fit"));
                }
                ret(c.make(r))
            }
            "swap" | "iter_swap" => {
                self.nargs(name, args, 2, 2)?;
                let a = self.eval_place(&args[0])?;
                let b = self.eval_place(&args[1])?;
                if name == "iter_swap" {
                    let (x, y) = (
                        self.load(&a).map_err(|f| self.fail(f))?,
                        self.load(&b).map_err(|f| self.fail(f))?,
                    );
                    let (Value::Iter(x), Value::Iter(y)) = (x, y) else {
                        return Err(self.err(RtKind::BadArg, "iter_swap needs iterators"));
                    };
                    let (pa, pb) = (self.iter_deref(&x)?, self.iter_deref(&y)?);
                    self.swap_places(&pa, &pb)?;
                } else {
                    self.swap_places(&a, &b)?;
                }
                ret(Value::Void)
            }
            "to_string" => {
                self.nargs(name, args, 1, 1)?;
                let v = self.val_arg(&args[0])?;
                let Some(x) = v.as_i128() else {
                    return Err(self.err(RtKind::BadArg, "to_string needs a number"));
                };
                let s = x.to_string().into_bytes();
                self.charge(s.len() as u64)?;
                ret(Value::string(s))
            }
            "stoi" | "stol" | "stoll" | "stoul" | "stoull" => {
                if args.is_empty() || args.len() > 3 {
                    return Err(self.err(RtKind::BadArg, "stoi takes a string"));
                }
                let s = self.arg_bytes(&args[0])?;
                let base = if args.len() == 3 {
                    self.arg_int(&args[2])?
                } else {
                    10
                };
                self.charge(s.len() as u64)?;
                self.parse_int(name, &s, base)
            }
            "isdigit" | "isalpha" | "isalnum" | "isupper" | "islower" | "isspace" | "ispunct"
            | "isxdigit" => {
                self.nargs(name, args, 1, 1)?;
                let c = self.arg_int(&args[0])?;
                let m = if (0..=127).contains(&c) {
                    ctype_mask(c as u8)
                } else {
                    0
                };
                let bit = match name {
                    "isdigit" => CT_DIGIT,
                    "isalpha" => CT_ALPHA,
                    "isalnum" => CT_ALNUM,
                    "isupper" => CT_UPPER,
                    "islower" => CT_LOWER,
                    "isspace" => CT_SPACE,
                    "ispunct" => CT_PUNCT,
                    _ => CT_XDIGIT,
                };
                ret(Value::Int(m & bit))
            }
            "tolower" | "toupper" => {
                self.nargs(name, args, 1, 1)?;
                let c = self.arg_int(&args[0])?;
                let r = if (0..=127).contains(&c) {
                    let b = c as u8;
                    i128::from(if name == "tolower" {
                        b.to_ascii_lowercase()
                    } else {
                        b.to_ascii_uppercase()
                    })
                } else {
                    c
                };
                ret(Value::Int(r as i32))
            }
            "numeric_limits::max" | "numeric_limits::min" | "numeric_limits::lowest" => {
                let t = match targs.and_then(|t| t.first()) {
                    Some(TArg::Ty(t)) => t.clone(),
                    _ => return Err(self.err(RtKind::BadArg, "numeric_limits needs a type")),
                };
                let Some(sk) = Sk::of_ty(&t) else {
                    return Err(self.err(
                        RtKind::BadArg,
                        format!("numeric_limits<{t}> is not available"),
                    ));
                };
                ret(sk.make(if name == "numeric_limits::max" {
                    sk.max()
                } else {
                    sk.min()
                }))
            }
            "make_pair" => {
                self.nargs(name, args, 2, 2)?;
                let (a, b) = (self.val_arg(&args[0])?, self.val_arg(&args[1])?);
                ret(Value::pair(a, b))
            }
            "make_tuple" => {
                let mut out = Vec::new();
                for a in args {
                    out.push(self.val_arg(a)?);
                }
                ret(Value::Tuple(Box::new(out)))
            }
            "make_optional" => {
                self.nargs(name, args, 1, 1)?;
                let v = self.val_arg(&args[0])?;
                ret(self.some(v))
            }
            "get" => {
                self.nargs(name, args, 1, 1)?;
                let n = match targs.and_then(|t| t.first()) {
                    Some(TArg::Int(n)) if *n >= 0 => *n as u16,
                    _ => return Err(self.err(RtKind::BadArg, "get needs an index")),
                };
                let p = self.eval_place(&args[0])?;
                let len = match self.get(&p).map_err(|f| self.fail(f))? {
                    Value::Pair(_) => 2,
                    Value::Tuple(t) => t.len(),
                    _ => return Err(self.err(RtKind::BadArg, "get works on pairs and tuples")),
                };
                if n as usize >= len {
                    return Err(self.err(RtKind::BadArg, "tuple index out of range"));
                }
                Ok(Rv::Place(p.with(Step::Field(n))))
            }
            "size" => {
                self.nargs(name, args, 1, 1)?;
                let p = self.eval_place(&args[0])?;
                let n = match self.get(&p).map_err(|f| self.fail(f))? {
                    Value::Vector(c) => c.v.len(),
                    Value::Str(s) => s.s.len(),
                    Value::Deque(c) => c.v.len(),
                    _ => return Err(self.err(RtKind::BadArg, "size of a non-container")),
                };
                ret(Value::Size(n as u64))
            }
            "distance" => {
                self.nargs(name, args, 2, 2)?;
                let (a, b) = self.range2(args)?;
                let (Value::Iter(x), Value::Iter(y)) = (&a, &b) else {
                    return Err(self.err(RtKind::BadArg, "distance needs iterators"));
                };
                if let (Pos::Idx(i), Pos::Idx(j)) = (&x.pos, &y.pos) {
                    return ret(Value::LL(*j as i64 - *i as i64));
                }
                let mut n = 0i64;
                let mut cur = (**x).clone();
                while cur.pos != y.pos {
                    cur = self.iter_advance(&cur, 1)?;
                    n += 1;
                    self.charge(1)?;
                }
                ret(Value::LL(n))
            }
            "advance" => {
                self.nargs(name, args, 2, 2)?;
                let p = self.eval_place(&args[0])?;
                let n = self.arg_int(&args[1])? as i64;
                let it = match self.load(&p).map_err(|f| self.fail(f))? {
                    Value::Iter(i) => i,
                    _ => return Err(self.err(RtKind::BadArg, "advance needs an iterator")),
                };
                if !matches!(it.pos, Pos::Idx(_)) {
                    self.charge(n.unsigned_abs())?;
                }
                let r = self.iter_advance(&it, n)?;
                self.store(&p, Value::Iter(Box::new(r)))
                    .map_err(|f| self.fail(f))?;
                ret(Value::Void)
            }
            "next" | "prev" => {
                self.nargs(name, args, 1, 2)?;
                let v = self.val_arg(&args[0])?;
                let n = if args.len() > 1 {
                    self.arg_int(&args[1])? as i64
                } else {
                    1
                };
                let n = if name == "prev" { -n } else { n };
                let Value::Iter(it) = v else {
                    return Err(self.err(RtKind::BadArg, "next needs an iterator"));
                };
                if !matches!(it.pos, Pos::Idx(_)) {
                    self.charge(n.unsigned_abs())?;
                }
                ret(Value::Iter(Box::new(self.iter_advance(&it, n)?)))
            }
            "sort" | "stable_sort" => {
                self.nargs(name, args, 2, 3)?;
                let (a, b) = self.range2(args)?;
                let cmp = self.comparator_arg(args, 2)?;
                let r = self.seq_range(&a, &b)?;
                let vals = self.take_range(&r)?;
                let sorted = self.sort_owned(vals, cmp.as_ref())?;
                self.put_range(&r, sorted)?;
                ret(Value::Void)
            }
            "reverse" => {
                self.nargs(name, args, 2, 2)?;
                let (a, b) = self.range2(args)?;
                let r = self.seq_range(&a, &b)?;
                self.charge((r.hi - r.lo) as u64)?;
                let mut vals = self.take_range(&r)?;
                vals.reverse();
                self.put_range(&r, vals)?;
                ret(Value::Void)
            }
            "fill" => {
                self.nargs(name, args, 3, 3)?;
                let (a, b) = self.range2(args)?;
                let r = self.seq_range(&a, &b)?;
                let ety = self.range_ety(&r)?;
                let x = match &ety {
                    Some(t) => self.eval_conv(&args[2], t)?,
                    None => self.eval_conv(&args[2], &Ty::Char)?,
                };
                self.charge(((r.hi - r.lo) as u64).saturating_mul(leaves(&x).max(1)))?;
                let vals = vec![x; r.hi - r.lo];
                self.put_range(&r, vals)?;
                ret(Value::Void)
            }
            "iota" => {
                self.nargs(name, args, 3, 3)?;
                let (a, b) = self.range2(args)?;
                let r = self.seq_range(&a, &b)?;
                let init = self.val_arg(&args[2])?;
                let ety = self.range_ety(&r)?;
                self.charge((r.hi - r.lo) as u64)?;
                let mut cur = init;
                let mut vals = Vec::with_capacity(r.hi - r.lo);
                for _ in r.lo..r.hi {
                    vals.push(match &ety {
                        Some(t) => self.convert(cur.clone(), t)?,
                        None => cur.clone(),
                    });
                    let next =
                        ops::binary(BinOp::Add, &cur, &Value::Int(1)).map_err(|f| self.fail(f))?;
                    cur = match cur.sk() {
                        Some(sk) => ops::convert(&next, sk).unwrap_or(next),
                        None => next,
                    };
                }
                self.put_range(&r, vals)?;
                ret(Value::Void)
            }
            "accumulate" => {
                self.nargs(name, args, 3, 4)?;
                let (a, b) = self.range2(args)?;
                let init = self.val_arg(&args[2])?;
                let op = self.comparator_arg(args, 3)?;
                let ity = init.ty();
                let vals = self.range_values(&a, &b)?;
                let mut acc = init;
                for x in vals {
                    acc = match &op {
                        Some(f) => {
                            let r = self.call_value(f, vec![Rv::Val(acc), Rv::Val(x)])?;
                            self.rv_val(r)?
                        }
                        None => self.binary_values(BinOp::Add, acc, x)?,
                    };
                    acc = self.convert(acc, &ity)?;
                }
                ret(acc)
            }
            "find" | "count" => {
                self.nargs(name, args, 3, 3)?;
                let (a, b) = self.range2(args)?;
                let x = self.val_arg(&args[2])?;
                if let Value::Iter(xi) = &a
                    && let Value::Iter(yi) = &b
                    && let Some(r) = self.try_seq_range(xi, yi)?
                {
                    let (found, scanned, cnt) = {
                        let sl = self.slice_of(&r)?;
                        let mut found = None;
                        let mut cnt = 0i64;
                        let mut scanned = 0u64;
                        for k in 0..sl.n {
                            scanned += 1;
                            let e = sl.at(k);
                            if eq_scalar_or_val(e.get(), &x) {
                                cnt += 1;
                                if found.is_none() {
                                    found = Some(k);
                                    if name == "find" {
                                        break;
                                    }
                                }
                            }
                        }
                        (found, scanned, cnt)
                    };
                    self.charge(scanned)?;
                    if name == "count" {
                        return ret(Value::LL(cnt));
                    }
                    return ret(match found {
                        Some(k) => self.iter_at(&r, k),
                        None => self.iter_at(&r, r.hi - r.lo),
                    });
                }
                // other containers: walk
                let Value::Iter(first) = &a else {
                    return Err(self.err(RtKind::BadArg, "expected iterators"));
                };
                let Value::Iter(last) = &b else {
                    return Err(self.err(RtKind::BadArg, "expected iterators"));
                };
                let mut cur = (**first).clone();
                let mut cnt = 0i64;
                while cur.pos != last.pos {
                    let p = self.iter_deref(&cur)?;
                    self.charge(1)?;
                    let e = self.load(&p).map_err(|f| self.fail(f))?;
                    if eq_scalar_or_val(&e, &x) {
                        if name == "find" {
                            return ret(Value::Iter(Box::new(cur)));
                        }
                        cnt += 1;
                    }
                    cur = self.iter_advance(&cur, 1)?;
                }
                if name == "count" {
                    ret(Value::LL(cnt))
                } else {
                    ret(Value::Iter(Box::new(cur)))
                }
            }
            "find_if" | "find_if_not" | "count_if" | "any_of" | "all_of" | "none_of" => {
                self.nargs(name, args, 3, 3)?;
                let (a, b) = self.range2(args)?;
                let f = self.val_arg(&args[2])?;
                let (Value::Iter(xi), Value::Iter(yi)) = (&a, &b) else {
                    return Err(self.err(RtKind::BadArg, "expected iterators"));
                };
                if let Some(r) = self.try_seq_range(xi, yi)? {
                    let n = r.hi - r.lo;
                    let mut cnt = 0i64;
                    for k in 0..n {
                        self.charge(1)?;
                        let p = self.elem_place(&r, k);
                        let t = self.call_pred(&f, Rv::Place(p))?;
                        match name {
                            "find_if" if t => return ret(self.iter_at(&r, k)),
                            "find_if_not" if !t => return ret(self.iter_at(&r, k)),
                            "any_of" if t => return ret(Value::Bool(true)),
                            "all_of" if !t => return ret(Value::Bool(false)),
                            "none_of" if t => return ret(Value::Bool(false)),
                            "count_if" if t => cnt += 1,
                            _ => {}
                        }
                    }
                    return ret(match name {
                        "find_if" | "find_if_not" => self.iter_at(&r, n),
                        "count_if" => Value::LL(cnt),
                        "any_of" => Value::Bool(false),
                        _ => Value::Bool(true),
                    });
                }
                let mut cur = (**xi).clone();
                let mut cnt = 0i64;
                while cur.pos != yi.pos {
                    self.charge(1)?;
                    let p = self.iter_deref(&cur)?;
                    let t = self.call_pred(&f, Rv::Place(p))?;
                    match name {
                        "find_if" if t => return ret(Value::Iter(Box::new(cur))),
                        "find_if_not" if !t => return ret(Value::Iter(Box::new(cur))),
                        "any_of" if t => return ret(Value::Bool(true)),
                        "all_of" if !t => return ret(Value::Bool(false)),
                        "none_of" if t => return ret(Value::Bool(false)),
                        "count_if" if t => cnt += 1,
                        _ => {}
                    }
                    cur = self.iter_advance(&cur, 1)?;
                }
                ret(match name {
                    "find_if" | "find_if_not" => Value::Iter(Box::new(cur)),
                    "count_if" => Value::LL(cnt),
                    "any_of" => Value::Bool(false),
                    _ => Value::Bool(true),
                })
            }
            "min_element" | "max_element" => {
                self.nargs(name, args, 2, 3)?;
                let (a, b) = self.range2(args)?;
                let cmp = self.comparator_arg(args, 2)?;
                let r = self.seq_range(&a, &b)?;
                let n = r.hi - r.lo;
                self.charge(n as u64)?;
                if n == 0 {
                    return ret(self.iter_at(&r, 0));
                }
                let mut best = 0usize;
                for k in 1..n {
                    let (pk, pb) = (self.elem_place(&r, k), self.elem_place(&r, best));
                    let better = if name == "min_element" {
                        self.less_vals(cmp.as_ref(), Rv::Place(pk), Rv::Place(pb))?
                    } else {
                        self.less_vals(cmp.as_ref(), Rv::Place(pb), Rv::Place(pk))?
                    };
                    if better {
                        best = k;
                    }
                }
                ret(self.iter_at(&r, best))
            }
            "binary_search" | "lower_bound" | "upper_bound" => {
                self.nargs(name, args, 3, 4)?;
                let (a, b) = self.range2(args)?;
                let x = self.val_arg(&args[2])?;
                let cmp = self.comparator_arg(args, 3)?;
                let r = self.seq_range(&a, &b)?;
                let n = r.hi - r.lo;
                self.charge(cl2(n as u64))?;
                // first index where !(elem < x)  (lower)  /  where x < elem (upper)
                let (mut lo, mut hi) = (0usize, n);
                let upper = name == "upper_bound";
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    let pm = self.elem_place(&r, mid);
                    let go_right = if upper {
                        !self.less_vals(cmp.as_ref(), Rv::Val(x.clone()), Rv::Place(pm))?
                    } else {
                        self.less_vals(cmp.as_ref(), Rv::Place(pm), Rv::Val(x.clone()))?
                    };
                    if go_right {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                if name == "binary_search" {
                    if lo >= n {
                        return ret(Value::Bool(false));
                    }
                    let pl = self.elem_place(&r, lo);
                    let eq_left = self.less_vals(cmp.as_ref(), Rv::Val(x), Rv::Place(pl))?;
                    return ret(Value::Bool(!eq_left));
                }
                ret(self.iter_at(&r, lo))
            }
            "unique" => {
                self.nargs(name, args, 2, 3)?;
                let (a, b) = self.range2(args)?;
                let pred = self.comparator_arg(args, 2)?;
                let r = self.seq_range(&a, &b)?;
                let n = r.hi - r.lo;
                self.charge(n as u64)?;
                let mut vals = self.take_range(&r)?;
                let mut dest = 0usize;
                if n > 0 {
                    // adjacent_find first, then compact (as libstdc++ does)
                    let mut first = 0usize;
                    let mut found = false;
                    while first + 1 < n {
                        let same = self.same_adj(pred.as_ref(), &vals[first], &vals[first + 1])?;
                        if same {
                            found = true;
                            break;
                        }
                        first += 1;
                    }
                    if !found {
                        dest = n;
                    } else {
                        dest = first;
                        let mut i = first + 2;
                        while i < n {
                            let same = self.same_adj(pred.as_ref(), &vals[dest], &vals[i])?;
                            if !same {
                                dest += 1;
                                vals[dest] = vals[i].clone();
                            }
                            i += 1;
                        }
                        dest += 1;
                    }
                }
                self.put_range(&r, vals)?;
                ret(self.iter_at(&r, dest))
            }
            "remove" | "remove_if" => {
                self.nargs(name, args, 3, 3)?;
                let (a, b) = self.range2(args)?;
                let arg = self.val_arg(&args[2])?;
                let r = self.seq_range(&a, &b)?;
                let n = r.hi - r.lo;
                self.charge(n as u64)?;
                let mut keep = Vec::with_capacity(n);
                for k in 0..n {
                    let p = self.elem_place(&r, k);
                    let kill = if name == "remove" {
                        let e = self.load(&p).map_err(|f| self.fail(f))?;
                        eq_scalar_or_val(&e, &arg)
                    } else {
                        self.call_pred(&arg, Rv::Place(p))?
                    };
                    keep.push(!kill);
                }
                let mut vals = self.take_range(&r)?;
                let mut dest = 0usize;
                for k in 0..n {
                    if keep[k] {
                        if dest != k {
                            vals[dest] = vals[k].clone();
                        }
                        dest += 1;
                    }
                }
                self.put_range(&r, vals)?;
                ret(self.iter_at(&r, dest))
            }
            "rotate" => {
                self.nargs(name, args, 3, 3)?;
                let (a, b) = self.range2(args)?;
                let m = self.val_arg(&args[1])?;
                let c = self.val_arg(&args[2])?;
                let r = self.seq_range(&a, &c)?;
                let (Value::Iter(mi), Value::Iter(ai)) = (&m, &a) else {
                    return Err(self.err(RtKind::BadArg, "rotate needs iterators"));
                };
                let _ = b;
                let (Pos::Idx(mp), Pos::Idx(ap)) = (&mi.pos, &ai.pos) else {
                    return Err(self.err(RtKind::BadArg, "rotate needs random access iterators"));
                };
                let n = r.hi - r.lo;
                if *mp < *ap || *mp - *ap > n {
                    return Err(
                        self.err(RtKind::BadArg, "the middle iterator is outside the range")
                    );
                }
                let k = *mp - *ap;
                self.charge(n as u64)?;
                let mut vals = self.take_range(&r)?;
                vals.rotate_left(k);
                self.put_range(&r, vals)?;
                ret(self.iter_at(&r, n - k))
            }
            "next_permutation" | "prev_permutation" => {
                self.nargs(name, args, 2, 3)?;
                let (a, b) = self.range2(args)?;
                let r = self.seq_range(&a, &b)?;
                let n = r.hi - r.lo;
                self.charge(n as u64)?;
                let mut vals = self.take_range(&r)?;
                let next = name == "next_permutation";
                let ord = |x: &Value, y: &Value| {
                    if next { cmp_val(x, y) } else { cmp_val(y, x) }
                };
                let mut done = false;
                if n >= 2 {
                    let mut i = n - 1;
                    while i > 0 && ord(&vals[i - 1], &vals[i]) != std::cmp::Ordering::Less {
                        i -= 1;
                    }
                    if i > 0 {
                        let mut j = n - 1;
                        while ord(&vals[i - 1], &vals[j]) != std::cmp::Ordering::Less {
                            j -= 1;
                        }
                        vals.swap(i - 1, j);
                        vals[i..].reverse();
                        done = true;
                    } else {
                        vals.reverse();
                    }
                }
                self.put_range(&r, vals)?;
                ret(Value::Bool(done))
            }
            "equal" => {
                self.nargs(name, args, 3, 4)?;
                let (a, b) = self.range2(args)?;
                let first = self.range_values(&a, &b)?;
                let c = self.val_arg(&args[2])?;
                let second: Vec<Value> = match (&c, args.get(3)) {
                    (Value::Iter(ci), Some(d)) => {
                        let d = self.val_arg(d)?;
                        self.range_values(&Value::Iter(ci.clone()), &d)?
                    }
                    (Value::Iter(ci), None) => {
                        // the second range is as long as the first
                        let mut out = Vec::new();
                        let mut cur = (**ci).clone();
                        for _ in 0..first.len() {
                            let p = self.iter_deref(&cur)?;
                            out.push(self.load(&p).map_err(|f| self.fail(f))?);
                            cur = self.iter_advance(&cur, 1)?;
                        }
                        out
                    }
                    _ => return Err(self.err(RtKind::BadArg, "equal needs iterators")),
                };
                ret(Value::Bool(
                    first.len() == second.len()
                        && first.iter().zip(&second).all(|(x, y)| eq_val(x, y)),
                ))
            }
            "is_sorted" => {
                self.nargs(name, args, 2, 3)?;
                let (a, b) = self.range2(args)?;
                let vals = self.range_values(&a, &b)?;
                ret(Value::Bool(
                    vals.windows(2)
                        .all(|w| cmp_val(&w[1], &w[0]) != std::cmp::Ordering::Less),
                ))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("this shop does not stock `std::{name}`"),
            )),
        }
    }

    fn same_adj(&mut self, pred: Option<&Value>, a: &Value, b: &Value) -> R<bool> {
        match pred {
            None => Ok(eq_val(a, b)),
            Some(f) => {
                let r = self.call_value(f, vec![Rv::Val(a.clone()), Rv::Val(b.clone())])?;
                let r = self.rv_val(r)?;
                self.truthy(&r)
            }
        }
    }

    /// The element type of a range's container (`None` for strings: characters).
    fn range_ety(&self, r: &SeqRange) -> R<Option<Ty>> {
        match self.get(&r.cont).map_err(|f| self.fail(f))? {
            Value::Vector(c) => Ok(Some((*c.ety).clone())),
            Value::Deque(c) => Ok(Some((*c.ety).clone())),
            Value::Str(_) => Ok(Some(Ty::Char)),
            _ => Err(self.err(RtKind::BadArg, "this range cannot be written to")),
        }
    }

    fn parse_int(&mut self, name: &str, s: &[u8], base: i128) -> R<Rv> {
        let bad = |m: &Self, what: &str| m.err(RtKind::BadArg, format!("{name}: {what}"));
        let mut i = 0;
        while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
            i += 1;
        }
        let mut neg = false;
        if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
            neg = s[i] == b'-';
            i += 1;
        }
        let mut base = base;
        if (base == 16 || base == 0) && s.len() > i + 1 && s[i] == b'0' && (s[i + 1] | 32) == b'x' {
            i += 2;
            base = 16;
        } else if base == 0 {
            base = if s.get(i) == Some(&b'0') { 8 } else { 10 };
        }
        if !(2..=36).contains(&base) {
            return Err(bad(self, "bad base"));
        }
        let start = i;
        let mut val: i128 = 0;
        while i < s.len() {
            let d = match s[i] {
                c @ b'0'..=b'9' => i128::from(c - b'0'),
                c @ b'a'..=b'z' => i128::from(c - b'a' + 10),
                c @ b'A'..=b'Z' => i128::from(c - b'A' + 10),
                _ => break,
            };
            if d >= base {
                break;
            }
            val = val.saturating_mul(base).saturating_add(d);
            if val > i128::from(u64::MAX) * 4 {
                val = i128::from(u64::MAX) * 4;
            }
            i += 1;
        }
        if i == start {
            return Err(bad(self, "no digits (this would throw invalid_argument)"));
        }
        let val = if neg { -val } else { val };
        let sk = match name {
            "stoi" => Sk::Int,
            "stol" | "stoll" => Sk::LL,
            "stoul" | "stoull" => Sk::Ull,
            _ => Sk::Int,
        };
        let ok = if matches!(sk, Sk::Ull) {
            val >= -i128::from(u64::MAX) && val <= i128::from(u64::MAX)
        } else {
            sk.fits(val)
        };
        if !ok {
            return Err(bad(
                self,
                "the number is out of range (this would throw out_of_range)",
            ));
        }
        Ok(Rv::Val(sk.make(val)))
    }
}

fn eq_scalar_or_val(a: &Value, b: &Value) -> bool {
    eq_val(a, b)
}

/// Stable merge sort of indices with a fallible `less(a, b)`.
fn merge_sort<'p>(
    m: &mut Machine<'p>,
    idx: &mut [usize],
    less: &mut dyn FnMut(&mut Machine<'p>, usize, usize) -> R<bool>,
) -> R<()> {
    let n = idx.len();
    let mut src: Vec<usize> = idx.to_vec();
    let mut dst: Vec<usize> = vec![0; n];
    let mut width = 1;
    while width < n {
        let mut start = 0;
        while start < n {
            let mid = (start + width).min(n);
            let end = (start + 2 * width).min(n);
            let (mut i, mut j, mut k) = (start, mid, start);
            while i < mid && j < end {
                if less(m, src[j], src[i])? {
                    dst[k] = src[j];
                    j += 1;
                } else {
                    dst[k] = src[i];
                    i += 1;
                }
                k += 1;
            }
            while i < mid {
                dst[k] = src[i];
                i += 1;
                k += 1;
            }
            while j < end {
                dst[k] = src[j];
                j += 1;
                k += 1;
            }
            start += 2 * width;
        }
        std::mem::swap(&mut src, &mut dst);
        width *= 2;
    }
    idx.copy_from_slice(&src);
    Ok(())
}

#[allow(dead_code)]
fn unused(_: &Loc<'_>) {}
