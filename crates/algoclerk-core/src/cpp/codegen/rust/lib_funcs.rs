// SPDX-License-Identifier: Apache-2.0
//! Rust translation of the library's free functions and of the comparisons that mean "found / not found".

use super::Rs;

use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;
use crate::cpp::print::print_expr;

const CTYPE: &[(&str, &str)] = &[
    ("std::isdigit", "is_ascii_digit"),
    ("std::isalpha", "is_ascii_alphabetic"),
    ("std::isalnum", "is_ascii_alphanumeric"),
    ("std::isupper", "is_ascii_uppercase"),
    ("std::islower", "is_ascii_lowercase"),
    ("std::ispunct", "is_ascii_punctuation"),
    ("std::isspace", "SPACE"),
];

impl Rs<'_> {
    fn lib_name(&self, e: &Expr) -> Option<&'static str> {
        let ExprKind::Call { callee, .. } = &e.kind else {
            return None;
        };
        match self.info.resolved(callee.id) {
            Some(Resolved::Lib(m)) => Some(m.name),
            _ => None,
        }
    }

    fn ctype_arg(&mut self, a: &Expr) -> R<Ex> {
        let t = self.ty_of(a);
        let x = self.ex(a, false)?;
        self.conv_int(x, &t, &Ty::Char)
    }

    /// `std::isdigit(c)` and friends as a `bool` (they return `int` in C++).
    pub(super) fn ctype_bool(&mut self, e: &Expr) -> Option<R<Ex>> {
        let name = self.lib_name(e)?;
        let (_, f) = CTYPE.iter().find(|(n, _)| *n == name)?;
        let ExprKind::Call { args, .. } = &e.kind else {
            return None;
        };
        let [a] = args.as_slice() else { return None };
        Some(self.ctype_arg(a).map(|x| {
            if *f == "SPACE" {
                Ex::val(format!("matches!({}, b' ' | 9..=13)", x.s))
            } else {
                Ex::new(format!("{}.{f}()", x.at(19)), 19, Kind::Val)
            }
        }))
    }

    /// `m.count(k)` / `s.count(k)` as `contains`.
    pub(super) fn count_bool(&mut self, e: &Expr) -> Option<R<Ex>> {
        let ExprKind::Call { callee, args } = &e.kind else {
            return None;
        };
        let ExprKind::Member { obj, name, .. } = &callee.kind else {
            return None;
        };
        if name != "count" || args.len() != 1 {
            return None;
        }
        let ot = self.ty_of(obj);
        let (is_map, k) = match &ot {
            Ty::Map(k, _) | Ty::UnorderedMap(k, _) => (true, k.as_ref().clone()),
            Ty::Set(k) | Ty::UnorderedSet(k) => (false, k.as_ref().clone()),
            _ => return None,
        };
        let r = (|| {
            let o = self.ex(obj, false)?;
            let key = self.conv_val(&args[0], &k)?;
            let f = if is_map { "contains_key" } else { "contains" };
            Ok(Ex::new(
                format!("{}.{f}(&{})", o.at(19), key.at(17)),
                19,
                Kind::Val,
            ))
        })();
        Some(r)
    }

    /// `count(..) != 0`, `s.find(c) != npos` and the like.
    pub(super) fn special_compare(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Option<R<Ex>> {
        let zero = |e: &Expr| matches!(e.kind, ExprKind::Int { value: 0, .. });
        if zero(b) && matches!(op, BinOp::Ne | BinOp::Gt | BinOp::Eq) {
            let x = self.count_bool(a)?;
            return Some(x.map(|x| {
                if op == BinOp::Eq {
                    Ex::new(format!("!{}", x.s), 17, Kind::Val)
                } else {
                    x
                }
            }));
        }
        let npos = |this: &Self, e: &Expr| {
            matches!(this.info.resolved(e.id), Some(Resolved::Lib(m)) if m.name == "npos")
                || matches!(&e.kind, ExprKind::Path(segs) if segs.last().is_some_and(|s| s.name == "npos"))
        };
        if npos(self, b) && matches!(op, BinOp::Eq | BinOp::Ne) {
            let ExprKind::Call { callee, args } = &a.kind else {
                return None;
            };
            let ExprKind::Member { obj, name, .. } = &callee.kind else {
                return None;
            };
            if !matches!(name.as_str(), "find" | "rfind")
                || !matches!(self.ty_of(obj), Ty::Str | Ty::StrView)
            {
                return None;
            }
            let r = self.str_find(obj, args, name == "rfind").map(|t| {
                let f = if op == BinOp::Ne {
                    "is_some"
                } else {
                    "is_none"
                };
                Ex::new(format!("{t}.{f}()"), 19, Kind::Val)
            });
            return Some(r);
        }
        None
    }

    /// The comparator of a sort as a `sort_by` closure body, given the comparator argument.
    fn sort_by(&mut self, slice: &str, cmp: &Expr) -> R<String> {
        let lam: Option<&Lambda> = match &cmp.kind {
            ExprKind::Lambda(l) => Some(&**l),
            ExprKind::Var(n) => match self.lookup(n).map(|v| v.ty.clone()) {
                Some(Ty::Lambda(id)) => self.lams.get(&id).copied(),
                _ => None,
            },
            ExprKind::Construct { ty, .. } => {
                return Ok(match &ty.ty {
                    Ty::Functor(Functor::Greater, _) => format!("{slice}.sort_by(|a, b| b.cmp(a))"),
                    Ty::Functor(Functor::Less, _) => format!("{slice}.sort()"),
                    _ => return no("Rust", "this comparator"),
                });
            }
            _ => None,
        };
        let Some(l) = lam else {
            return no("Rust", "this comparator");
        };
        self.allow_generic = true;
        let f = self.ex(cmp, false);
        self.allow_generic = false;
        let f = f?.s;
        let f = if matches!(cmp.kind, ExprKind::Lambda(_)) {
            format!("({f})")
        } else {
            f
        };
        let mut fwd = Vec::new();
        let mut bwd = Vec::new();
        for (i, p) in l.params.iter().enumerate() {
            let mut t = p.spec.ty.clone();
            if t == Ty::Auto {
                t = self.info.decl_ty(p.id).clone();
            }
            let conv = |x: &str| {
                if p.spec.is_ref {
                    x.to_string()
                } else if self.is_copy(&t) {
                    format!("*{x}")
                } else {
                    format!("{x}.clone()")
                }
            };
            let (first, second) = if i == 0 { ("a", "b") } else { ("b", "a") };
            fwd.push(conv(first));
            bwd.push(conv(second));
        }
        self.uses.insert("Ordering");
        Ok(format!(
            "{slice}.sort_by(|a, b| {{ let lt = {f}; if lt({}) {{ Ordering::Less }} else if lt({}) {{ Ordering::Greater }} else {{ Ordering::Equal }} }})",
            fwd.join(", "),
            bwd.join(", ")
        ))
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
        let val = |s: String| Ex::new(s, 19, Kind::Val);
        if let Some((_, f)) = CTYPE.iter().find(|(n, _)| *n == name) {
            let _ = f;
            let b = self.as_bool(e)?;
            return Ok(Ex::new(format!("i32::from({})", b.s), 19, Kind::Val));
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
                let v: R<Vec<String>> = items
                    .iter()
                    .map(|a| self.conv_val(a, &rt).map(|x| x.s))
                    .collect();
                Ok(val(format!(
                    "[{}].into_iter().{f}().unwrap()",
                    v?.join(", ")
                )))
            }
            ("std::max" | "std::min", [a, b]) => {
                let f = if name == "std::max" { "max" } else { "min" };
                let x = self.conv_val(a, &rt)?;
                let y = self.conv_val(b, &rt)?;
                Ok(val(format!("std::cmp::{f}({}, {})", x.s, y.s)))
            }
            ("std::swap", [a, b]) => self.swap(a, b),
            ("std::abs", [a]) => {
                let x = self.conv_val(a, &rt)?;
                Ok(val(format!("{}::abs({})", self.rty(&rt)?, x.s)))
            }
            ("std::to_string", [a]) => {
                let at = self.ty_of(a);
                let x = self.ex(a, false)?;
                let x = self.conv_int(x, &at, &promote(&at))?;
                Ok(val(format!("{}.to_string()", x.at(19))))
            }
            ("std::stoi" | "std::stoll", [a]) => {
                let h = if name == "std::stoi" { "stoi" } else { "stoll" };
                self.helpers.insert(h);
                let x = self.str_ref(a)?;
                Ok(val(format!("{h}({})", x.s)))
            }
            ("std::tolower" | "std::toupper", [a]) => {
                let f = if name == "std::tolower" {
                    "to_ascii_lowercase"
                } else {
                    "to_ascii_uppercase"
                };
                let x = self.ctype_arg(a)?;
                Ok(val(format!("i32::from({}.{f}())", x.at(19))))
            }
            ("std::make_pair" | "std::make_tuple", _) => {
                let tys: Vec<Ty> = match &rt {
                    Ty::Pair(a, b) => vec![(**a).clone(), (**b).clone()],
                    Ty::Tuple(ts) => ts.clone(),
                    _ => return no("Rust", "this tuple"),
                };
                let parts: R<Vec<String>> = args
                    .iter()
                    .zip(&tys)
                    .map(|(a, t)| self.conv_val(a, t).map(|x| x.s))
                    .collect();
                Ok(Ex::val(format!("({})", parts?.join(", "))))
            }
            ("std::get", [a]) => {
                let ExprKind::Path(segs) = &callee.kind else {
                    return no("Rust", "std::get");
                };
                let Some(TArg::Int(i)) = segs
                    .last()
                    .and_then(|s| s.targs.as_ref())
                    .and_then(|t| t.first())
                else {
                    return no("Rust", "std::get");
                };
                let x = self.ex(a, m)?;
                Ok(Ex::new(format!("{}.{i}", x.at(19)), 19, Kind::Place))
            }
            (
                "std::numeric_limits<T>::max"
                | "std::numeric_limits<T>::min"
                | "std::numeric_limits<T>::lowest",
                [],
            ) => {
                let ExprKind::Path(segs) = &callee.kind else {
                    return no("Rust", "numeric_limits");
                };
                let Some(TArg::Ty(t)) = segs
                    .iter()
                    .find_map(|s| s.targs.as_ref())
                    .and_then(|t| t.first())
                else {
                    return no("Rust", "numeric_limits");
                };
                let c = if name.ends_with("max") { "MAX" } else { "MIN" };
                Ok(Ex::val(format!("{}::{c}", self.rty(t)?)))
            }
            (
                "std::sort" | "std::stable_sort" | "std::reverse" | "std::fill" | "std::iota"
                | "std::count" | "std::accumulate" | "std::binary_search",
                _,
            ) => self.range_fn(name, e, args, &rt),
            _ => no("Rust", format!("`{name}`")),
        }
    }

    fn swap(&mut self, a: &Expr, b: &Expr) -> R<Ex> {
        if let (ExprKind::Index(ba, ia), ExprKind::Index(bb, ib)) = (&a.kind, &b.kind)
            && print_expr(ba) == print_expr(bb)
            && matches!(self.ty_of(ba), Ty::Vector(_) | Ty::Array(..) | Ty::Deque(_))
        {
            let base = self.ex(ba, true)?;
            let (ta, tb) = (self.ty_of(ia), self.ty_of(ib));
            let (x, y) = (self.index_text(ia, &ta)?, self.index_text(ib, &tb)?);
            return Ok(Ex::new(
                format!("{}.swap({x}, {y})", base.at(19)),
                19,
                Kind::Val,
            ));
        }
        if root(a) == root(b)
            && !(matches!(a.kind, ExprKind::Var(_)) && matches!(b.kind, ExprKind::Var(_)))
        {
            return no(
                "Rust",
                "swapping two parts of one container (the borrow checker would refuse)",
            );
        }
        let side = |this: &mut Self, x: &Expr| -> R<String> {
            let v = this.ex(x, true)?;
            match v.kind {
                Kind::Place => Ok(format!("&mut {}", v.at(17))),
                Kind::Ref => Ok(v.s),
                Kind::Val => no("Rust", "swapping a temporary"),
            }
        };
        let (x, y) = (side(self, a)?, side(self, b)?);
        Ok(Ex::new(format!("std::mem::swap({x}, {y})"), 19, Kind::Val))
    }

    fn range_fn(&mut self, name: &str, e: &Expr, args: &[Expr], rt: &Ty) -> R<Ex> {
        let val = |s: String| Ex::new(s, 19, Kind::Val);
        if let [a, b, ..] = args
            && matches!(name, "std::sort" | "std::reverse")
            && let Some(Pos::Begin(o, None, false)) = iter_pos(a)
            && matches!(self.ty_of(o), Ty::Str)
        {
            let p = self.place_text(o)?;
            let _ = b;
            let body = if name == "std::sort" {
                format!(
                    "{{ let mut b = std::mem::take(&mut {p}).into_bytes(); b.sort(); String::from_utf8(b).unwrap() }}"
                )
            } else {
                format!("{p}.chars().rev().collect::<String>()")
            };
            return Ok(Ex::new(format!("{p} = {body}"), 1, Kind::Val));
        }
        let (a, b) = match args {
            [a, b, ..] => (a, b),
            _ => return no("Rust", format!("`{name}`")),
        };
        let Some(r) = self.range(a, b)? else {
            return no("Rust", format!("`{name}` on this kind of range"));
        };
        if r.rev || matches!(r.ty, Ty::Str | Ty::StrView) && name != "std::count" {
            return no("Rust", format!("`{name}` on this range"));
        }
        let mutating = matches!(
            name,
            "std::sort" | "std::stable_sort" | "std::reverse" | "std::fill" | "std::iota"
        );
        let (r, pre) = if mutating {
            self.hoist_len(r)
        } else {
            (r, None)
        };
        let s = self.range_slice(&r);
        let el = self.elem_of(&r.ty);
        let out: R<Ex> = match (name, args.len()) {
            ("std::sort" | "std::stable_sort", 2) => Ok(val(format!("{s}.sort()"))),
            ("std::sort" | "std::stable_sort", 3) => Ok(val(self.sort_by(&s, &args[2])?)),
            ("std::reverse", 2) => Ok(val(format!("{s}.reverse()"))),
            ("std::fill", 3) => Ok(val(format!(
                "{s}.fill({})",
                self.conv_val(&args[2], &el)?.s
            ))),
            ("std::iota", 3) => {
                let x = self.conv_val(&args[2], &el)?;
                let t = self.rty(&el)?;
                Ok(Ex::val(format!(
                    "for (i, x) in {s}.iter_mut().enumerate() {{ *x = {} + i as {t}; }}",
                    x.at(12)
                )))
            }
            ("std::count", 3) => {
                let x = self.conv_val(&args[2], &el)?;
                let t = format!("{s}.iter().filter(|e| **e == {}).count()", x.at(8));
                self.conv_int(val(t), &Ty::Size, rt)
            }
            ("std::binary_search", 3) => Ok(val(format!(
                "{s}.binary_search(&{}).is_ok()",
                self.conv_val(&args[2], &el)?.at(17)
            ))),
            ("std::accumulate", 3) => {
                let it = self.ty_of(&args[2]);
                let init = self.conv_val(&args[2], &it)?;
                let acc = self.arith(
                    BinOp::Add,
                    Ex::val("acc"),
                    &it,
                    Ex::new("*x", 17, Kind::Val),
                    &el,
                )?;
                let c = common_ty(&it, &el);
                let acc = self.conv_int(acc, &c, &it)?;
                Ok(val(format!(
                    "{s}.iter().fold({}, |acc, x| {})",
                    init.s, acc.s
                )))
            }
            _ => {
                let _ = e;
                no("Rust", format!("`{name}` with these arguments"))
            }
        };
        match (out, pre) {
            (Ok(x), Some(p)) => Ok(Ex::val(format!("{{ {p} {} }}", x.s))),
            (out, _) => out,
        }
    }

    /// A mutable slice `v[a..v.len() - 1]` cannot name `v` twice: read the length into a variable first.
    fn hoist_len(&mut self, r: super::lib_free::Rg) -> (super::lib_free::Rg, Option<String>) {
        let pat = format!("{}.len()", r.obj);
        if !r.lo.iter().chain(r.hi.iter()).any(|t| t.contains(&pat)) {
            return (r, None);
        }
        let n = self.fresh("n");
        let fix = |t: &Option<String>| t.as_ref().map(|s| s.replace(&pat, &n));
        let pre = format!("let {n} = {pat};");
        let r2 = super::lib_free::Rg {
            lo: fix(&r.lo),
            hi: fix(&r.hi),
            ..r
        };
        (r2, Some(pre))
    }
}
