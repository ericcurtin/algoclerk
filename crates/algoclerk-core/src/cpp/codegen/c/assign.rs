// SPDX-License-Identifier: Apache-2.0
//! C translation of assignments, construction, braced lists and lambdas (lifted to functions).

use super::{Cx, Rk};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

impl Cx<'_> {
    /// An expression used as a statement (with the trailing `;`).
    pub(super) fn expr_stmt(&mut self, e: &Expr) -> R<String> {
        match &e.kind {
            ExprKind::Binary(BinOp::Comma, a, b) => {
                Ok(format!("{} {}", self.expr_stmt(a)?, self.expr_stmt(b)?))
            }
            ExprKind::Delete(p) => {
                let x = self.ex(p, false)?;
                Ok(format!("free({});", x.s))
            }
            _ => {
                let t = self.ex(e, true)?.s;
                Ok(if t.is_empty() || t.starts_with("for (") {
                    t
                } else {
                    format!("{t};")
                })
            }
        }
    }

    /// The lvalue text of an assignable expression (`v.data[i]`, `(*r)`, `x`).
    pub(super) fn place_text(&mut self, lhs: &Expr) -> R<String> {
        let x = self.ex(lhs, true)?;
        match x.kind {
            Kind::Place => Ok(x.s),
            Kind::Ref => Ok(format!("(*{})", x.s)),
            Kind::Val => no("C", "assigning to a temporary"),
        }
    }

    /// `lhs = rhs` / `lhs op= rhs` (no trailing `;`).
    pub(super) fn assign_stmt(&mut self, lhs: &Expr, op: Option<BinOp>, rhs: &Expr) -> R<String> {
        let lt = self.ty_of(lhs);
        let rt = self.ty_of(rhs);
        match op {
            None => {
                let v = self.conv_val(rhs, &lt)?;
                let p = self.place_text(lhs)?;
                Ok(format!("{p} = {}", v.s))
            }
            Some(BinOp::Add) if matches!(lt, Ty::Str) => {
                let p = self.place_text(lhs)?;
                if rt == Ty::Char {
                    let c = self.ex(rhs, false)?;
                    Ok(format!("Str_push(&{p}, {})", c.s))
                } else {
                    let y = self.str_val(rhs)?;
                    Ok(format!("Str_cat(&{p}, {})", y.s))
                }
            }
            Some(bo) => {
                let y = self.ex(rhs, false)?;
                let y = self.own(y, &rt)?;
                let p = self.place_text(lhs)?;
                Ok(format!("{p} {}= {}", bo.text(), y.at(3)))
            }
        }
    }

    /// `T(args)`, `T{args}`, `new T(args)` and declarations `T x(args)`.
    pub(super) fn construct(&mut self, ty: &Ty, args: &[Expr], brace: bool) -> R<Ex> {
        let n = self.name_of(ty)?;
        match ty {
            t if t.is_integral() => match args {
                [] => Ok(Ex::val("0")),
                [a] => self.conv_val(a, t),
                _ => no("C", "this initialisation"),
            },
            Ty::Vector(el) | Ty::Stack(el) | Ty::Array(el, _) if brace || args.is_empty() => {
                let en = self.name_of(el)?;
                if args.is_empty() {
                    return Ok(Ex::val(match ty {
                        Ty::Array(_, k) => format!("{n}_filled({k}, {})", self.zero_of(el)?),
                        _ => format!("{n}_zero()"),
                    }));
                }
                let items: R<Vec<String>> = args
                    .iter()
                    .map(|a| self.conv_val(a, el).map(|x| x.s))
                    .collect();
                Ok(Ex::val(format!(
                    "{n}_from(({en}[]){{{}}}, {})",
                    items?.join(", "),
                    args.len()
                )))
            }
            Ty::Vector(el) | Ty::Stack(el) | Ty::Array(el, _) => match args {
                [k] if self.ty_of(k).is_integral() => {
                    let z = self.zero_of(el)?;
                    Ok(Ex::val(format!(
                        "{n}_filled({}, {z})",
                        self.conv_val(k, &Ty::Size)?.s
                    )))
                }
                [k, x]
                    if self.ty_of(k).is_integral() && !matches!(self.ty_of(x), Ty::Iter { .. }) =>
                {
                    let x = self.conv_val(x, el)?;
                    Ok(Ex::val(format!(
                        "{n}_filled({}, {})",
                        self.conv_val(k, &Ty::Size)?.s,
                        x.s
                    )))
                }
                [a, b] => match self.range(a, b)? {
                    Some(r) if self.is_seq(&r.ty) => {
                        Ok(Ex::val(format!("{n}_from({}, {})", r.ptr, r.count)))
                    }
                    _ => no("C", "this vector constructor"),
                },
                [a] => self.conv_val(a, ty),
                _ => no("C", "this vector constructor"),
            },
            Ty::Str | Ty::StrView => match args {
                [] => Ok(Ex::val("Str_from(\"\")")),
                [a] => self.conv_val(a, &Ty::Str),
                [k, c] if self.ty_of(c) == Ty::Char => {
                    let c = self.ex(c, false)?;
                    Ok(Ex::val(format!(
                        "Str_repeat({}, {})",
                        self.conv_val(k, &Ty::Size)?.s,
                        c.s
                    )))
                }
                [a, b] => match self.range(a, b)? {
                    Some(r) => Ok(Ex::val(format!("Str_n({}, {})", r.ptr, r.count))),
                    None => no("C", "this string constructor"),
                },
                _ => no("C", "this string constructor"),
            },
            Ty::Pair(a, b) => match args {
                [] => Ok(Ex::val(format!("{n}_zero()"))),
                [x, y] => Ok(Ex::val(format!(
                    "({n}){{{}, {}}}",
                    self.conv_val(x, a)?.s,
                    self.conv_val(y, b)?.s
                ))),
                [x] => self.conv_val(x, ty),
                _ => no("C", "this pair constructor"),
            },
            Ty::Tuple(ts) => {
                if args.is_empty() {
                    return Ok(Ex::val(format!("{n}_zero()")));
                }
                if args.len() != ts.len() {
                    return no("C", "this tuple constructor");
                }
                let parts: R<Vec<String>> = args
                    .iter()
                    .zip(ts)
                    .map(|(a, t)| self.conv_val(a, t).map(|x| x.s))
                    .collect();
                Ok(Ex::val(format!("({n}){{{}}}", parts?.join(", "))))
            }
            Ty::Optional(t) => match args {
                [] => Ok(Ex::val(format!("({n}){{false}}"))),
                [x] => Ok(Ex::val(format!(
                    "({n}){{true, {}}}",
                    self.conv_val(x, t)?.s
                ))),
                _ => no("C", "this optional constructor"),
            },
            Ty::Struct(s) => {
                self.use_struct(s)?;
                let Some(tys) = self.struct_params(s) else {
                    return no("C", format!("the type {s}"));
                };
                if tys.len() != args.len() {
                    return no("C", format!("this constructor call of {s}"));
                }
                let parts: R<Vec<String>> = args
                    .iter()
                    .zip(&tys)
                    .map(|(a, t)| self.conv_val(a, t).map(|x| x.s))
                    .collect();
                Ok(Ex::val(format!("{s}_make({})", parts?.join(", "))))
            }
            Ty::Map(..)
            | Ty::UnorderedMap(..)
            | Ty::Set(_)
            | Ty::UnorderedSet(_)
            | Ty::Deque(_)
            | Ty::Queue(_)
            | Ty::List(_)
            | Ty::PriorityQueue(..)
                if args.is_empty() =>
            {
                Ok(Ex::val(format!("{n}_zero()")))
            }
            Ty::Set(el) | Ty::UnorderedSet(el) => match args {
                [a, b] if !brace => match self.range(a, b)? {
                    Some(r) if self.is_seq(&r.ty) || matches!(r.ty, Ty::Str) => {
                        let _ = el;
                        Ok(Ex::val(format!("{n}_from_keys({}, {})", r.ptr, r.count)))
                    }
                    _ => no("C", "this set constructor"),
                },
                _ => no("C", "this set constructor"),
            },
            t => no("C", format!("constructing {t}")),
        }
    }

    pub(super) fn is_seq(&self, t: &Ty) -> bool {
        matches!(t, Ty::Vector(_) | Ty::Array(..) | Ty::Stack(_) | Ty::Set(_))
    }

    /// A braced list `{a, b}` converted to the type the context needs.
    pub(super) fn init_list(&mut self, items: &[Expr], to: &Ty) -> R<Ex> {
        match to {
            Ty::Vector(_)
            | Ty::Stack(_)
            | Ty::Array(..)
            | Ty::Pair(..)
            | Ty::Tuple(_)
            | Ty::Struct(_)
            | Ty::Optional(_)
            | Ty::Str => self.construct(to, items, true),
            Ty::Map(..)
            | Ty::UnorderedMap(..)
            | Ty::Set(_)
            | Ty::UnorderedSet(_)
            | Ty::Deque(_)
            | Ty::Queue(_)
            | Ty::List(_)
                if items.is_empty() =>
            {
                self.construct(to, items, true)
            }
            t if t.is_integral() && items.len() == 1 => self.conv_val(&items[0], t),
            t if t.is_integral() && items.is_empty() => Ok(Ex::val("0")),
            t => no("C", format!("a braced list as {t}")),
        }
    }

    /// Lift a non-capturing lambda to a static function; returns its name. All parameters are pointers
    /// (`const T *a`), which is what `sort_by` calls.
    pub(super) fn lift_lambda(&mut self, id: NodeId, l: &Lambda, hint: &str) -> R<String> {
        let bound: std::collections::HashSet<String> =
            l.params.iter().map(|p| p.name.clone()).collect();
        let mut free = std::collections::HashSet::new();
        let mut locals = std::collections::HashSet::new();
        walk_stmts(&l.body, &mut |s| {
            if let StmtKind::Decl { vars, .. } = &s.kind {
                locals.extend(vars.iter().map(|v| v.name.clone()));
            }
            if let StmtKind::Structured { names, .. } | StmtKind::RangeFor { names, .. } = &s.kind {
                locals.extend(names.iter().cloned());
            }
            stmt_exprs(s, &mut |e| {
                walk_expr(e, &mut |x| {
                    if let ExprKind::Var(n) = &x.kind {
                        free.insert(n.clone());
                    }
                });
            });
        });
        if free.iter().any(|n| {
            !bound.contains(n)
                && !locals.contains(n)
                && self
                    .lookup(n)
                    .is_some_and(|v| !matches!(v.rk, Rk::Lambda(_)) && !self.is_global(n))
        }) {
            return no("C", "a lambda that captures variables (C has no closures)");
        }
        let name = format!("{hint}_{}", id);
        let lret = l
            .ret
            .as_ref()
            .map_or_else(|| self.lambda_ret(&l.body), |r| r.ty.clone());
        let saved_ret = std::mem::replace(&mut self.ret, lret);
        self.push_scope();
        let mut params = Vec::new();
        let mut prologue = String::new();
        for p in &l.params {
            let mut ty = p.spec.ty.clone();
            if ty == Ty::Auto {
                ty = self.info.decl_ty(p.id).clone();
            }
            let ct = self.ctype(&ty)?;
            let sp = if ct.ends_with('*') { "" } else { " " };
            let pn = self.name(&p.name);
            if p.spec.is_ref {
                let k = if p.spec.konst { "const " } else { "" };
                params.push(format!("{k}{ct}{sp}*{pn}"));
                self.declare(&p.name, ty, Rk::Ref);
            } else {
                params.push(format!("const {ct}{sp}*{pn}_"));
                prologue.push_str(&format!("    {ct}{sp}{pn} = *{pn}_;\n"));
                self.declare(&p.name, ty, Rk::Val);
            }
        }
        let mut body = String::new();
        let r = self.stmts(&l.body.stmts, 1, &mut body);
        self.pop_scope();
        let ret = std::mem::replace(&mut self.ret, saved_ret);
        r?;
        let rt = if ret == Ty::Bool {
            "int".to_string()
        } else {
            self.ctype(&ret)?
        };
        self.lifted.push(format!(
            "static {rt} {name}({}) {{\n{prologue}{body}}}\n",
            params.join(", ")
        ));
        Ok(name)
    }

    pub(super) fn is_global(&self, n: &str) -> bool {
        self.scopes.first().is_some_and(|s| s.contains_key(n))
    }

    fn lambda_ret(&self, b: &Block) -> Ty {
        let mut r = Ty::Void;
        let mut found = false;
        walk_stmts(b, &mut |s| {
            if let (StmtKind::Return(Some(x)), false) = (&s.kind, found) {
                r = self.info.ty(x.id).clone();
                found = true;
            }
        });
        r
    }
}
