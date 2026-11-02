// SPDX-License-Identifier: Apache-2.0
//! C translation of iterator ranges and the library's free functions. A pair `(c.begin() + a, c.end() - b)`
//! becomes a pointer and an element count; `find(...) != c.end()` compares an index with `NOT_FOUND`.

use super::{Cx, Rk};
use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;
use crate::cpp::print::print_expr;

/// A `[first, last)` pair of iterators into one array-like container.
pub struct Rg {
    pub ptr: String,
    pub count: String,
    pub ty: Ty,
}

const CTYPE: &[(&str, &str)] = &[
    ("std::isdigit", "isdigit"),
    ("std::isalpha", "isalpha"),
    ("std::isalnum", "isalnum"),
    ("std::isupper", "isupper"),
    ("std::islower", "islower"),
    ("std::ispunct", "ispunct"),
    ("std::isspace", "isspace"),
    ("std::tolower", "tolower"),
    ("std::toupper", "toupper"),
];

impl Cx<'_> {
    pub(super) fn range(&mut self, a: &Expr, b: &Expr) -> R<Option<Rg>> {
        let (Some(pa), Some(pb)) = (iter_pos(a), iter_pos(b)) else {
            return Ok(None);
        };
        let first_end = matches!(pa, Pos::End(..));
        let end_side = matches!(pb, Pos::End(..));
        let (
            Pos::Begin(oa, ka, ra) | Pos::End(oa, ka, ra),
            Pos::End(ob, kb, rb) | Pos::Begin(ob, kb, rb),
        ) = (pa, pb);
        if print_expr(oa) != print_expr(ob) || ra || rb {
            return Ok(None);
        }
        let ty = self.ty_of(oa);
        if !matches!(
            ty,
            Ty::Vector(_) | Ty::Array(..) | Ty::Stack(_) | Ty::Str | Ty::StrView | Ty::Set(_)
        ) {
            return Ok(None);
        }
        let base = self.ex(oa, true)?;
        let base = self.peek(base);
        let data = format!(
            "{}.{}",
            base.at(16),
            if matches!(ty, Ty::Set(_)) {
                "keys"
            } else {
                "data"
            }
        );
        let len = format!("{}.len", base.at(16));
        let off = |this: &mut Self, k: Option<&Expr>| -> R<Option<String>> {
            Ok(match k {
                None => None,
                Some(k) => {
                    let t = this.ty_of(k);
                    let x = this.ex(k, false)?;
                    Some(this.own(x, &t)?.at(13))
                }
            })
        };
        let lo = match (first_end, off(self, ka)?) {
            (false, k) => k,
            (true, Some(k)) => Some(format!("({len} - {k})")),
            (true, None) => Some(len.clone()),
        };
        let hi = match (end_side, off(self, kb)?) {
            (true, None) => len.clone(),
            (true, Some(k)) => format!("{len} - {k}"),
            (false, Some(k)) => k,
            (false, None) => "0".into(),
        };
        let ptr = lo
            .as_ref()
            .map_or(data.clone(), |l| format!("{data} + {l}"));
        let count = lo.map_or(hi.clone(), |l| format!("{hi} - {l}"));
        Ok(Some(Rg { ptr, count, ty }))
    }

    pub(super) fn elem_of(&self, t: &Ty) -> Ty {
        match t {
            Ty::Vector(e)
            | Ty::Array(e, _)
            | Ty::Stack(e)
            | Ty::Deque(e)
            | Ty::List(e)
            | Ty::Set(e)
            | Ty::UnorderedSet(e) => (**e).clone(),
            Ty::Str | Ty::StrView => Ty::Char,
            _ => Ty::Unknown,
        }
    }

    /// The prelude name of the vector type holding the range's elements.
    fn vec_of(&mut self, r: &Rg) -> R<String> {
        let el = self.elem_of(&r.ty);
        self.name_of(&Ty::vector(el))
    }

    /// `it != c.end()`, `c.find(k) == c.end()`, `std::find(a, b, x) != b`.
    pub(super) fn iter_compare(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Option<R<Ex>> {
        if !matches!(op, BinOp::Eq | BinOp::Ne) {
            return None;
        }
        let is_end = |e: &Expr| matches!(iter_pos(e), Some(Pos::End(_, None, false)));
        let other = if is_end(b) {
            a
        } else if is_end(a) {
            b
        } else {
            return None;
        };
        let idx = match self.found_index(other) {
            Ok(Some(t)) => t,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };
        let o = if op == BinOp::Eq { "==" } else { "!=" };
        Some(Ok(Ex::new(format!("{idx} {o} NOT_FOUND"), 9, Kind::Val)))
    }

    /// An index-valued text for a find-like call or a `find` iterator variable.
    fn found_index(&mut self, e: &Expr) -> R<Option<String>> {
        if let ExprKind::Var(v) = &e.kind {
            return Ok(
                matches!(self.lookup(v).map(|x| x.rk.clone()), Some(Rk::Found { .. }))
                    .then(|| self.name(v)),
            );
        }
        let ExprKind::Call { callee, args } = &e.kind else {
            return Ok(None);
        };
        match &callee.kind {
            ExprKind::Member { obj, name, .. } if name == "find" && args.len() == 1 => {
                let ot = self.ty_of(obj);
                let k = match &ot {
                    Ty::Map(k, _) | Ty::UnorderedMap(k, _) | Ty::Set(k) | Ty::UnorderedSet(k) => {
                        (**k).clone()
                    }
                    _ => return Ok(None),
                };
                let n = self.name_of(&ot)?;
                let o = self.ex(obj, false)?;
                let p = self.addr(o, &ot)?;
                let key = self.conv_val(&args[0], &k)?;
                Ok(Some(format!("{n}_find({p}, {})", key.s)))
            }
            ExprKind::Path(_) | ExprKind::Var(_) if args.len() == 3 => {
                let Some(Resolved::Lib(mem)) = self.info.resolved(callee.id) else {
                    return Ok(None);
                };
                if mem.name != "std::find" {
                    return Ok(None);
                }
                let Some(r) = self.range(&args[0], &args[1])? else {
                    return Ok(None);
                };
                let n = self.vec_of(&r)?;
                let x = self.conv_val(&args[2], &self.elem_of(&r.ty))?;
                Ok(Some(format!("{n}_find({}, {}, {})", r.ptr, r.count, x.s)))
            }
            _ => Ok(None),
        }
    }

    /// `*it` for the iterator-valued calls we understand.
    pub(super) fn deref_iter(&mut self, a: &Expr, m: bool) -> R<Ex> {
        let _ = m;
        let ExprKind::Call { callee, args } = &a.kind else {
            return no("C", "dereferencing this iterator");
        };
        let Some(Resolved::Lib(mem)) = self.info.resolved(callee.id) else {
            return no("C", "dereferencing this iterator");
        };
        match (mem.name, args.as_slice()) {
            ("std::min_element" | "std::max_element", [x, y]) => {
                let Some(r) = self.range(x, y)? else {
                    return no("C", "this range");
                };
                let f = if mem.name == "std::min_element" {
                    "min_at"
                } else {
                    "max_at"
                };
                let n = self.vec_of(&r)?;
                Ok(Ex::new(
                    format!("*{n}_{f}({}, {})", r.ptr, r.count),
                    15,
                    Kind::Place,
                ))
            }
            _ => no("C", "dereferencing this iterator"),
        }
    }

    fn limit(&mut self, t: &Ty, max: bool) -> R<String> {
        Ok(match (t, max) {
            (Ty::Int, true) => "INT_MAX",
            (Ty::Int, false) => "INT_MIN",
            (Ty::UInt, true) => "UINT_MAX",
            (Ty::UInt, false) => "0u",
            (Ty::LongLong, true) => "LLONG_MAX",
            (Ty::LongLong, false) => "LLONG_MIN",
            (Ty::ULongLong, true) => "ULLONG_MAX",
            (Ty::ULongLong, false) => "0ull",
            (Ty::Size, true) => "SIZE_MAX",
            (Ty::Size, false) => "0",
            (Ty::Char, true) => "CHAR_MAX",
            (Ty::Char, false) => "CHAR_MIN",
            (Ty::Bool, true) => "true",
            (Ty::Bool, false) => "false",
            _ => return no("C", format!("numeric_limits of {t}")),
        }
        .to_string())
    }

    /// The `char *` of a string-typed expression.
    pub(super) fn cdata(&mut self, e: &Expr) -> R<String> {
        let x = self.ex(e, false)?;
        if x.slice {
            return Ok(x.s);
        }
        let x = self.peek(x);
        if x.kind == Kind::Val {
            return Ok(format!("(Str[]){{ {} }}[0].data", x.s));
        }
        Ok(format!("{}.data", x.at(16)))
    }

    pub(super) fn free_call(
        &mut self,
        e: &Expr,
        callee: &Expr,
        name: &str,
        args: &[Expr],
        m: bool,
    ) -> R<Ex> {
        let rt = self.ty_of(e);
        let val = |s: String| Ex::new(s, 16, Kind::Val);
        if let Some((_, f)) = CTYPE.iter().find(|(n, _)| *n == name) {
            let [a] = args else {
                return no("C", "this call");
            };
            let at = self.ty_of(a);
            let x = self.ex(a, false)?;
            let x = self.own(x, &at)?;
            return Ok(val(format!("{f}((unsigned char){})", x.at(14))));
        }
        match (name, args) {
            (
                "std::max" | "std::min",
                [
                    Expr {
                        kind: ExprKind::InitList(items),
                        ..
                    },
                ],
            ) => {
                let f = if name == "std::max" { "max" } else { "min" };
                let n = self.name_of(&rt)?;
                let mut it = items.iter();
                let first = it.next().ok_or("empty list")?;
                let mut acc = self.conv_val(first, &rt)?.s;
                for x in it {
                    acc = format!("{n}_{f}({acc}, {})", self.conv_val(x, &rt)?.s);
                }
                Ok(val(acc))
            }
            ("std::max" | "std::min", [a, b]) => {
                let f = if name == "std::max" { "max" } else { "min" };
                let n = self.name_of(&rt)?;
                Ok(val(format!(
                    "{n}_{f}({}, {})",
                    self.conv_val(a, &rt)?.s,
                    self.conv_val(b, &rt)?.s
                )))
            }
            ("std::swap", [a, b]) => {
                let t = self.ty_of(a);
                let ct = self.ctype(&t)?;
                let (x, y) = (self.place_text(a)?, self.place_text(b)?);
                Ok(val(format!("SWAP({ct}, {x}, {y})")))
            }
            ("std::abs", [a]) => {
                let f = if rt == Ty::LongLong { "llabs" } else { "abs" };
                Ok(val(format!("{f}({})", self.conv_val(a, &rt)?.s)))
            }
            ("std::to_string", [a]) => {
                let at = self.ty_of(a);
                let x = self.ex(a, false)?;
                let x = self.own(x, &at)?;
                let (f, c) = if at.is_unsigned() {
                    ("Str_from_ull", "ull")
                } else {
                    ("Str_from_ll", "ll")
                };
                Ok(val(format!("{f}(({c}){})", x.at(14))))
            }
            ("std::stoi" | "std::stoll", [a]) => {
                let d = self.cdata(a)?;
                let cast = if name == "std::stoi" { "(int)" } else { "" };
                Ok(Ex::new(
                    format!("{cast}strtoll({d}, NULL, 10)"),
                    if cast.is_empty() { 16 } else { 15 },
                    Kind::Val,
                ))
            }
            ("std::make_pair" | "std::make_tuple", _) => {
                let n = self.name_of(&rt)?;
                let tys: Vec<Ty> = match &rt {
                    Ty::Pair(a, b) => vec![(**a).clone(), (**b).clone()],
                    Ty::Tuple(ts) => ts.clone(),
                    _ => return no("C", "this tuple"),
                };
                let parts: R<Vec<String>> = args
                    .iter()
                    .zip(&tys)
                    .map(|(a, t)| self.conv_val(a, t).map(|x| x.s))
                    .collect();
                Ok(val(format!("({n}){{{}}}", parts?.join(", "))))
            }
            ("std::get", [a]) => {
                let ExprKind::Path(segs) = &callee.kind else {
                    return no("C", "std::get");
                };
                let Some(TArg::Int(i)) = segs
                    .last()
                    .and_then(|s| s.targs.as_ref())
                    .and_then(|t| t.first())
                else {
                    return no("C", "std::get");
                };
                let x = self.ex(a, m)?;
                let f = match self.ty_of(a) {
                    Ty::Pair(..) => (if *i == 0 { "first" } else { "second" }).to_string(),
                    _ => format!("f{i}"),
                };
                Ok(Ex::new(self.dot(&x, &f), 16, Kind::Place))
            }
            (
                "std::numeric_limits<T>::max"
                | "std::numeric_limits<T>::min"
                | "std::numeric_limits<T>::lowest",
                [],
            ) => {
                let ExprKind::Path(segs) = &callee.kind else {
                    return no("C", "numeric_limits");
                };
                let Some(TArg::Ty(t)) = segs
                    .iter()
                    .find_map(|s| s.targs.as_ref())
                    .and_then(|t| t.first())
                else {
                    return no("C", "numeric_limits");
                };
                Ok(Ex::val(self.limit(t, name.ends_with("max"))?))
            }
            (
                "std::sort" | "std::stable_sort" | "std::reverse" | "std::fill" | "std::iota"
                | "std::count" | "std::binary_search",
                _,
            ) => self.range_fn(name, args),
            _ => no("C", format!("`{name}`")),
        }
    }

    fn range_fn(&mut self, name: &str, args: &[Expr]) -> R<Ex> {
        let [a, b, ..] = args else {
            return no("C", format!("`{name}`"));
        };
        let Some(r) = self.range(a, b)? else {
            return no("C", format!("`{name}` on this kind of range"));
        };
        let n = self.vec_of(&r)?;
        let el = self.elem_of(&r.ty);
        let val = |s: String| Ok(Ex::new(s, 16, Kind::Val));
        match (name, args.len()) {
            ("std::sort" | "std::stable_sort", 2) => {
                val(format!("{n}_sort({}, {})", r.ptr, r.count))
            }
            ("std::sort" | "std::stable_sort", 3) => {
                let f = self.comparator(&args[2], &n)?;
                val(format!("{n}_sort_by({}, {}, {f})", r.ptr, r.count))
            }
            ("std::reverse", 2) => val(format!("{n}_reverse({}, {})", r.ptr, r.count)),
            ("std::count", 3) => val(format!(
                "{n}_count({}, {}, {})",
                r.ptr,
                r.count,
                self.conv_val(&args[2], &el)?.s
            )),
            ("std::binary_search", 3) => val(format!(
                "{n}_has({}, {}, {})",
                r.ptr,
                r.count,
                self.conv_val(&args[2], &el)?.s
            )),
            ("std::fill", 3) => {
                let x = self.conv_val(&args[2], &el)?;
                let i = self.fresh("i");
                val(format!(
                    "for (size_t {i} = 0; {i} < {}; ++{i}) {{ ({})[{i}] = {}; }}",
                    r.count, r.ptr, x.s
                ))
            }
            ("std::iota", 3) => {
                let x = self.conv_val(&args[2], &el)?;
                let i = self.fresh("i");
                let ct = self.ctype(&el)?;
                val(format!(
                    "for (size_t {i} = 0; {i} < {}; ++{i}) {{ ({})[{i}] = ({ct})({} + {i}); }}",
                    r.count,
                    r.ptr,
                    x.at(12)
                ))
            }
            _ => no("C", format!("`{name}` with these arguments")),
        }
    }

    /// The comparison function passed to `sort_by`.
    fn comparator(&mut self, cmp: &Expr, vec: &str) -> R<String> {
        match &cmp.kind {
            ExprKind::Lambda(l) => self.lift_lambda(cmp.id, l, "lambda"),
            ExprKind::Var(v) => match self.lookup(v).map(|x| x.rk.clone()) {
                Some(Rk::Lambda(f)) => Ok(f),
                _ => no("C", "this comparator"),
            },
            ExprKind::Construct { ty, .. } => match &ty.ty {
                Ty::Functor(Functor::Greater, _) => Ok(format!("{vec}_desc")),
                Ty::Functor(Functor::Less, _) => Ok(format!("{vec}_asc")),
                _ => no("C", "this comparator"),
            },
            _ => no("C", "this comparator"),
        }
    }
}
