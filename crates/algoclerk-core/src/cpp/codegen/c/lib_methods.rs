// SPDX-License-Identifier: Apache-2.0
//! C translation of member functions of sequences, adaptors, priority queues, optionals, strings, maps and
//! sets. Receivers that must be named twice (`v.back()` is `v.data[v.len - 1]`) have to be side-effect free.

use super::Cx;
use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;
use crate::cpp::print::print_expr;

type Member<'e> = (&'e Expr, &'e str, &'e [Expr]);

impl Cx<'_> {
    pub(super) fn method(
        &mut self,
        e: &Expr,
        obj: &Expr,
        name: &str,
        args: &[Expr],
        m: bool,
    ) -> R<Ex> {
        let _ = e;
        let ot = self.ty_of(obj);
        let call: Member = (obj, name, args);
        match &ot {
            Ty::Vector(el) | Ty::Array(el, _) | Ty::Stack(el) => self.seq_method(call, &ot, el, m),
            Ty::Deque(el) | Ty::Queue(el) | Ty::List(el) => self.deq_method(call, &ot, el),
            Ty::PriorityQueue(el, _) => self.pq_method(call, &ot, el),
            Ty::Optional(el) => self.opt_method(call, el, m),
            Ty::Str | Ty::StrView => self.str_method(call, m),
            Ty::Map(..) | Ty::UnorderedMap(..) | Ty::Set(_) | Ty::UnorderedSet(_) => {
                self.assoc_method(call, &ot)
            }
            t => no("C", format!("the member `{name}` of {t}")),
        }
    }

    /// The receiver as a pointer to the container.
    fn recv(&mut self, obj: &Expr, ot: &Ty) -> R<String> {
        let x = self.ex(obj, true)?;
        self.addr(x, ot)
    }

    /// The receiver as a place usable twice (variable or field chain).
    fn twice(&mut self, obj: &Expr) -> R<Ex> {
        if !is_pure(obj) {
            return no("C", "this member call on a computed container");
        }
        let x = self.ex(obj, true)?;
        Ok(self.peek(x))
    }

    fn pos_idx(&mut self, obj: &Expr, p: &Expr) -> R<String> {
        match iter_pos(p) {
            Some(Pos::Begin(o, k, false)) if print_expr(o) == print_expr(obj) => match k {
                None => Ok("0".into()),
                Some(k) => {
                    let t = self.ty_of(k);
                    let x = self.ex(k, false)?;
                    Ok(self.own(x, &t)?.s)
                }
            },
            Some(Pos::End(o, None, false)) if print_expr(o) == print_expr(obj) => {
                let b = self.twice(obj)?;
                Ok(format!("{}.len", b.at(16)))
            }
            _ => no("C", "this iterator position"),
        }
    }

    fn seq_method(&mut self, (obj, name, args): Member, ot: &Ty, el: &Ty, m: bool) -> R<Ex> {
        let _ = m;
        let n = self.name_of(ot)?;
        let val = |s: String| Ok(Ex::new(s, 16, Kind::Val));
        match (name, args) {
            ("size", []) => {
                let b = self.twice(obj)?;
                val(format!("{}.len", b.at(16)))
            }
            ("empty", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.len == 0", b.at(16)), 9, Kind::Val))
            }
            ("push_back" | "push", [x]) => {
                let (p, v) = (self.recv(obj, ot)?, self.conv_val(x, el)?);
                val(format!("{n}_push({p}, {})", v.s))
            }
            ("emplace_back" | "emplace", _) => {
                let (p, v) = (self.recv(obj, ot)?, self.construct(el, args, false)?);
                val(format!("{n}_push({p}, {})", v.s))
            }
            ("pop_back" | "pop", []) => val(format!("{n}_pop({})", self.recv(obj, ot)?)),
            ("back" | "top", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(
                    format!("{0}.data[{0}.len - 1]", b.at(16)),
                    16,
                    Kind::Place,
                ))
            }
            ("front", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.data[0]", b.at(16)), 16, Kind::Place))
            }
            ("clear", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.len = 0", b.at(16)), 2, Kind::Val))
            }
            ("at", [i]) => {
                let b = self.twice(obj)?;
                let it = self.ty_of(i);
                let x = self.ex(i, false)?;
                let x = self.own(x, &it)?;
                Ok(Ex::new(
                    format!("{}.data[{}]", b.at(16), x.s),
                    16,
                    Kind::Place,
                ))
            }
            ("reserve" | "shrink_to_fit", _) => Ok(Ex::val("")),
            ("resize", [k]) => {
                let (p, z) = (self.recv(obj, ot)?, self.zero_of(el)?);
                val(format!(
                    "{n}_resize({p}, {}, {z})",
                    self.conv_val(k, &Ty::Size)?.s
                ))
            }
            ("resize", [k, x]) => {
                let p = self.recv(obj, ot)?;
                val(format!(
                    "{n}_resize({p}, {}, {})",
                    self.conv_val(k, &Ty::Size)?.s,
                    self.conv_val(x, el)?.s
                ))
            }
            ("assign", [k, x]) => {
                let place = self.place_text(obj)?;
                let v = format!(
                    "{n}_filled({}, {})",
                    self.conv_val(k, &Ty::Size)?.s,
                    self.conv_val(x, el)?.s
                );
                Ok(Ex::new(format!("{place} = {v}"), 2, Kind::Val))
            }
            ("swap", [other]) => {
                let ct = self.ctype(ot)?;
                let (a, b) = (self.place_text(obj)?, self.place_text(other)?);
                val(format!("SWAP({ct}, {a}, {b})"))
            }
            ("insert", [p, x]) if !matches!(self.ty_of(x), Ty::Iter { .. }) => {
                let i = self.pos_idx(obj, p)?;
                let (pt, v) = (self.recv(obj, ot)?, self.conv_val(x, el)?);
                val(format!("{n}_insert({pt}, {i}, {})", v.s))
            }
            ("insert", [p, a, b]) => {
                let Some(r) = self.range(a, b)? else {
                    return no("C", "this insert");
                };
                let i = self.pos_idx(obj, p)?;
                val(format!(
                    "{n}_insert_n({}, {i}, {}, {})",
                    self.recv(obj, ot)?,
                    r.ptr,
                    r.count
                ))
            }
            ("erase", [p]) => {
                let i = self.pos_idx(obj, p)?;
                val(format!("{n}_erase({}, {i}, 1)", self.recv(obj, ot)?))
            }
            ("erase", [a, b]) => {
                if let ExprKind::Call { callee, args: ua } = &a.kind
                    && let Some(Resolved::Lib(mem)) = self.info.resolved(callee.id)
                    && mem.name == "std::unique"
                    && ua.len() == 2
                {
                    return val(format!("{n}_unique({})", self.recv(obj, ot)?));
                }
                if let ExprKind::Call { callee, args: ra } = &a.kind
                    && let Some(Resolved::Lib(mem)) = self.info.resolved(callee.id)
                    && mem.name == "std::remove"
                    && ra.len() == 3
                    && matches!(iter_pos(b), Some(Pos::End(_, None, false)))
                {
                    // `v.erase(std::remove(v.begin(), v.end(), x), v.end())` over the whole vector
                    let (lo, hi) = (self.pos_idx(obj, &ra[0])?, self.pos_idx(obj, &ra[1])?);
                    if lo == "0" && hi.ends_with(".len") {
                        let x = self.conv_val(&ra[2], el)?;
                        return val(format!("{n}_remove({}, {})", self.recv(obj, ot)?, x.s));
                    }
                    return no("C", "this erase-remove range");
                }
                let (lo, hi) = (self.pos_idx(obj, a)?, self.pos_idx(obj, b)?);
                val(format!(
                    "{n}_erase({}, {lo}, {hi} - {lo})",
                    self.recv(obj, ot)?
                ))
            }
            _ => no("C", format!("the member `{name}` of a vector")),
        }
    }

    fn deq_method(&mut self, (obj, name, args): Member, ot: &Ty, el: &Ty) -> R<Ex> {
        let n = self.name_of(ot)?;
        let stack_like = matches!(ot, Ty::Queue(_));
        let val = |s: String| Ok(Ex::new(s, 16, Kind::Val));
        match (name, args) {
            ("size", []) => {
                let b = self.twice(obj)?;
                val(format!("{}.len", b.at(16)))
            }
            ("empty", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.len == 0", b.at(16)), 9, Kind::Val))
            }
            ("clear", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(
                    format!("{0}.len = 0, {0}.head = 0", b.at(16)),
                    1,
                    Kind::Val,
                ))
            }
            ("push" | "push_back", [x]) => {
                let (p, v) = (self.recv(obj, ot)?, self.conv_val(x, el)?);
                val(format!("{n}_push_back({p}, {})", v.s))
            }
            ("push_front", [x]) => {
                let (p, v) = (self.recv(obj, ot)?, self.conv_val(x, el)?);
                val(format!("{n}_push_front({p}, {})", v.s))
            }
            ("emplace" | "emplace_back", _) => {
                let (p, v) = (self.recv(obj, ot)?, self.construct(el, args, false)?);
                val(format!("{n}_push_back({p}, {})", v.s))
            }
            ("emplace_front", _) => {
                let (p, v) = (self.recv(obj, ot)?, self.construct(el, args, false)?);
                val(format!("{n}_push_front({p}, {})", v.s))
            }
            ("pop" | "pop_front", []) => val(format!("{n}_pop_front({})", self.recv(obj, ot)?)),
            ("pop_back", []) if !stack_like => {
                val(format!("{n}_pop_back({})", self.recv(obj, ot)?))
            }
            ("front", []) => Ok(Ex::new(
                format!("{n}_at({}, 0)", self.recv(obj, ot)?),
                16,
                Kind::Ref,
            )),
            ("back", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(
                    format!("{n}_at({}, {}.len - 1)", self.recv(obj, ot)?, b.at(16)),
                    16,
                    Kind::Ref,
                ))
            }
            ("at", [i]) => {
                let it = self.ty_of(i);
                let x = self.ex(i, false)?;
                let x = self.own(x, &it)?;
                Ok(Ex::new(
                    format!("{n}_at({}, {})", self.recv(obj, ot)?, x.s),
                    16,
                    Kind::Ref,
                ))
            }
            _ => no("C", format!("the member `{name}` of this container")),
        }
    }

    fn pq_method(&mut self, (obj, name, args): Member, ot: &Ty, el: &Ty) -> R<Ex> {
        let n = self.name_of(ot)?;
        let val = |s: String| Ok(Ex::new(s, 16, Kind::Val));
        match (name, args) {
            ("size", []) => {
                let b = self.twice(obj)?;
                val(format!("{}.len", b.at(16)))
            }
            ("empty", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.len == 0", b.at(16)), 9, Kind::Val))
            }
            ("push", [x]) => {
                let (p, v) = (self.recv(obj, ot)?, self.conv_val(x, el)?);
                val(format!("{n}_push({p}, {})", v.s))
            }
            ("emplace", _) => {
                let (p, v) = (self.recv(obj, ot)?, self.construct(el, args, false)?);
                val(format!("{n}_push({p}, {})", v.s))
            }
            ("pop", []) => val(format!("{n}_pop({})", self.recv(obj, ot)?)),
            ("top", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.data[0]", b.at(16)), 16, Kind::Place))
            }
            _ => no("C", format!("the member `{name}` of a priority_queue")),
        }
    }

    fn opt_method(&mut self, (obj, name, args): Member, el: &Ty, m: bool) -> R<Ex> {
        let o = self.ex(obj, m)?;
        match (name, args) {
            ("has_value", []) => Ok(Ex::new(self.dot(&o, "has"), 16, Kind::Val)),
            ("value", []) => Ok(Ex::new(self.dot(&o, "v"), 16, Kind::Place)),
            ("value_or", [x]) => {
                if !is_pure(obj) {
                    return no("C", "value_or on a computed optional");
                }
                let v = self.conv_val(x, el)?;
                Ok(Ex::new(
                    format!(
                        "{} ? {} : {}",
                        self.dot(&o, "has"),
                        self.dot(&o, "v"),
                        v.at(3)
                    ),
                    3,
                    Kind::Val,
                ))
            }
            ("reset", []) => Ok(Ex::new(
                format!("{} = false", self.dot(&o, "has")),
                2,
                Kind::Val,
            )),
            _ => no("C", format!("the member `{name}` of an optional")),
        }
    }

    fn str_method(&mut self, (obj, name, args): Member, m: bool) -> R<Ex> {
        let st = Ty::Str;
        let val = |s: String| Ok(Ex::new(s, 16, Kind::Val));
        match (name, args) {
            ("size" | "length", []) => {
                let b = self.twice(obj)?;
                val(format!("{}.len", b.at(16)))
            }
            ("empty", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.len == 0", b.at(16)), 9, Kind::Val))
            }
            ("push_back", [c]) => {
                let (p, c) = (self.recv(obj, &st)?, self.ex(c, false)?);
                val(format!("Str_push({p}, {})", c.s))
            }
            ("pop_back", []) => {
                let b = self.twice(obj)?;
                let b = b.at(16);
                Ok(Ex::new(format!("{b}.data[--{b}.len] = 0"), 2, Kind::Val))
            }
            ("back", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(
                    format!("{0}.data[{0}.len - 1]", b.at(16)),
                    16,
                    Kind::Place,
                ))
            }
            ("front", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.data[0]", b.at(16)), 16, Kind::Place))
            }
            ("at", [i]) => {
                let b = self.twice(obj)?;
                let it = self.ty_of(i);
                let x = self.ex(i, false)?;
                let x = self.own(x, &it)?;
                Ok(Ex::new(
                    format!("{}.data[{}]", b.at(16), x.s),
                    16,
                    Kind::Place,
                ))
            }
            ("clear", []) => {
                let b = self.twice(obj)?;
                let b = b.at(16);
                Ok(Ex::new(
                    format!("{b}.len = 0, {b}.data[0] = 0"),
                    1,
                    Kind::Val,
                ))
            }
            ("append", [x]) => {
                let (p, y) = (self.recv(obj, &st)?, self.str_val(x)?);
                val(format!("Str_cat({p}, {})", y.s))
            }
            ("substr", [p]) | ("substr", [p, _]) => {
                let s = self.str_val(obj)?;
                let pos = self.conv_val(p, &Ty::Size)?;
                let len = match args.get(1) {
                    Some(l) => self.conv_val(l, &Ty::Size)?.s,
                    None => "NOT_FOUND".into(),
                };
                val(format!("Str_substr({}, {}, {len})", s.s, pos.s))
            }
            ("find" | "rfind", [a, ..]) => {
                let s = self.str_val(obj)?;
                let pos = match args.get(1) {
                    Some(p) => self.conv_val(p, &Ty::Size)?.s,
                    None => "0".into(),
                };
                if self.ty_of(a) == Ty::Char {
                    let c = self.ex(a, false)?;
                    let f = if name == "find" {
                        format!("Str_find_c({}, {}, {pos})", s.s, c.s)
                    } else {
                        format!("Str_rfind_c({}, {})", s.s, c.s)
                    };
                    val(f)
                } else if name == "find" {
                    let t = self.str_val(a)?;
                    val(format!("Str_find({}, {}, {pos})", s.s, t.s))
                } else {
                    no("C", "rfind with a string argument")
                }
            }
            ("compare", [x]) => {
                let (a, b) = (self.str_val(obj)?, self.str_val(x)?);
                val(format!("Str_cmp({}, {})", a.s, b.s))
            }
            ("insert", [p, x]) if self.ty_of(p).is_integral() => {
                let (s, pos, y) = (
                    self.recv(obj, &st)?,
                    self.conv_val(p, &Ty::Size)?,
                    self.str_val(x)?,
                );
                val(format!("Str_insert({s}, {}, {})", pos.s, y.s))
            }
            ("erase", [p]) => {
                let (s, pos) = (self.recv(obj, &st)?, self.conv_val(p, &Ty::Size)?);
                val(format!("Str_erase({s}, {}, NOT_FOUND)", pos.s))
            }
            ("erase", [p, l]) if self.ty_of(p).is_integral() => {
                let (s, pos, len) = (
                    self.recv(obj, &st)?,
                    self.conv_val(p, &Ty::Size)?,
                    self.conv_val(l, &Ty::Size)?,
                );
                val(format!("Str_erase({s}, {}, {})", pos.s, len.s))
            }
            ("resize", [k]) | ("resize", [k, _]) => {
                let s = self.recv(obj, &st)?;
                let c = match args.get(1) {
                    Some(c) => self.ex(c, false)?.s,
                    None => "0".into(),
                };
                val(format!(
                    "Str_resize({s}, {}, {c})",
                    self.conv_val(k, &Ty::Size)?.s
                ))
            }
            _ => {
                let _ = m;
                no("C", format!("the member `{name}` of a string"))
            }
        }
    }

    fn assoc_method(&mut self, (obj, name, args): Member, ot: &Ty) -> R<Ex> {
        let (k, v) = match ot {
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => ((**k).clone(), Some((**v).clone())),
            Ty::Set(k) | Ty::UnorderedSet(k) => ((**k).clone(), None),
            _ => return no("C", "this container"),
        };
        let n = self.name_of(ot)?;
        let ordered = matches!(ot, Ty::Map(..) | Ty::Set(_));
        let val = |s: String| Ok(Ex::new(s, 16, Kind::Val));
        match (name, args) {
            ("size", []) => {
                let b = self.twice(obj)?;
                val(format!("{}.len", b.at(16)))
            }
            ("empty", []) => {
                let b = self.twice(obj)?;
                Ok(Ex::new(format!("{}.len == 0", b.at(16)), 9, Kind::Val))
            }
            ("clear", []) => {
                let place = self.place_text(obj)?;
                Ok(Ex::new(format!("{place} = {n}_zero()"), 2, Kind::Val))
            }
            ("count", [x]) => {
                let (p, key) = (self.recv(obj, ot)?, self.conv_val(x, &k)?);
                val(format!("{n}_contains({p}, {})", key.s))
            }
            ("erase", [x]) if !matches!(self.ty_of(x), Ty::Iter { .. }) => {
                let (p, key) = (self.recv(obj, ot)?, self.conv_val(x, &k)?);
                val(format!("{n}_erase({p}, {})", key.s))
            }
            ("at", [x]) => {
                let (p, key) = (self.recv(obj, ot)?, self.conv_val(x, &k)?);
                Ok(Ex::new(format!("{n}_get({p}, {})", key.s), 16, Kind::Ref))
            }
            ("insert" | "emplace", _) if v.is_none() && args.len() == 1 => {
                let (p, key) = (self.recv(obj, ot)?, self.conv_val(&args[0], &k)?);
                val(format!("{n}_insert({p}, {}, 0)", key.s))
            }
            ("insert" | "emplace", _) if v.is_some() => {
                let (ke, ve) = Self::pair_args(name, args)?;
                let p = self.recv(obj, ot)?;
                let vt = v.unwrap_or(Ty::Unknown);
                let (key, value) = (self.conv_val(ke, &k)?, self.conv_val(ve, &vt)?);
                val(format!("{n}_insert({p}, {}, {})", key.s, value.s))
            }
            _ => {
                let _ = ordered;
                no("C", format!("the member `{name}` of a map or set"))
            }
        }
    }

    fn pair_args<'e>(name: &str, args: &'e [Expr]) -> R<(&'e Expr, &'e Expr)> {
        let two = |items: &'e [Expr]| match items {
            [a, b] => Ok((a, b)),
            _ => no("C", "this insert"),
        };
        match (name, args) {
            ("emplace", [_, _]) => two(args),
            (_, [a]) => match &a.kind {
                ExprKind::InitList(items) => two(items),
                ExprKind::Construct { args: items, .. } => two(items),
                ExprKind::Call { args: items, .. } => two(items),
                _ => no("C", "this insert"),
            },
            _ => no("C", "this insert"),
        }
    }

    /// `s.insert(x).second` (1 when the element was new).
    pub(super) fn insert_second(&mut self, obj: &Expr) -> Option<R<Ex>> {
        let ExprKind::Call { callee, .. } = &obj.kind else {
            return None;
        };
        let ExprKind::Member { obj: c, name, .. } = &callee.kind else {
            return None;
        };
        if !matches!(name.as_str(), "insert" | "emplace") {
            return None;
        }
        let ct = self.ty_of(c);
        if !matches!(
            ct,
            Ty::Map(..) | Ty::UnorderedMap(..) | Ty::Set(_) | Ty::UnorderedSet(_)
        ) {
            return None;
        }
        Some(self.ex(obj, true))
    }
}
