// SPDX-License-Identifier: Apache-2.0
//! Rust translation of assignments, increments, construction, braced lists and lambdas.

use super::{Rk, Rs};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

impl Rs<'_> {
    /// An expression used as a statement (with the trailing `;`).
    pub(super) fn expr_stmt(&mut self, e: &Expr) -> R<String> {
        match &e.kind {
            ExprKind::Assign { op, lhs, rhs } => {
                Ok(format!("{};", self.assign_stmt(lhs, *op, rhs)?))
            }
            ExprKind::Unary(UnOp::PreInc | UnOp::PostInc, a) => {
                Ok(format!("{};", self.step_stmt(a, BinOp::Add)?))
            }
            ExprKind::Unary(UnOp::PreDec | UnOp::PostDec, a) => {
                Ok(format!("{};", self.step_stmt(a, BinOp::Sub)?))
            }
            ExprKind::Binary(BinOp::Comma, a, b) => {
                Ok(format!("{} {}", self.expr_stmt(a)?, self.expr_stmt(b)?))
            }
            ExprKind::Delete(_) => Ok("// delete: the arena owns the node".to_string()),
            _ => {
                let t = self.ex(e, true)?.s;
                if t.is_empty() || (t.ends_with('}') && t.starts_with("for ")) {
                    Ok(t)
                } else {
                    Ok(format!("{t};"))
                }
            }
        }
    }

    /// The place text of an lvalue (`v[i]`, `*r`, `x`).
    pub(super) fn place_text(&mut self, lhs: &Expr) -> R<String> {
        let x = self.ex(lhs, true)?;
        match x.kind {
            Kind::Place => Ok(x.s),
            Kind::Ref => Ok(format!("*{}", x.at(17))),
            Kind::Val => no("Rust", "assigning to a temporary"),
        }
    }

    fn handle_field<'e>(&self, lhs: &'e Expr) -> Option<(&'e Expr, &'e str)> {
        if let ExprKind::Member { obj, name, .. } = &lhs.kind
            && matches!(self.ty_of(obj), Ty::Struct(_) | Ty::Ptr(_))
        {
            return Some((obj, name));
        }
        None
    }

    /// `lhs = rhs` / `lhs op= rhs` (no trailing `;`).
    pub(super) fn assign_stmt(&mut self, lhs: &Expr, op: Option<BinOp>, rhs: &Expr) -> R<String> {
        let lt = self.ty_of(lhs);
        let rt = self.ty_of(rhs);
        if let Some((obj, name)) = self.handle_field(lhs) {
            let o = self.ex(obj, false)?;
            let new = match op {
                None => self.conv_val(rhs, &lt)?,
                Some(bo) => {
                    if !is_pure(obj) {
                        return no("Rust", "a compound assignment through a computed pointer");
                    }
                    let cur = Ex::val(format!("{}.{name}()", o.at(19)));
                    let y = self.ex(rhs, false)?;
                    let r = self.arith(bo, cur, &lt, y, &rt)?;
                    let c = if matches!(bo, BinOp::Shl | BinOp::Shr) {
                        promote(&lt)
                    } else {
                        common_ty(&lt, &rt)
                    };
                    self.conv_int(r, &c, &lt)?
                }
            };
            return Ok(format!("{}.set_{name}({})", o.at(19), new.s));
        }
        match op {
            None => {
                let v = self.conv_val(rhs, &lt)?;
                let place = self.place_text(lhs)?;
                Ok(format!("{place} = {}", v.s))
            }
            Some(BinOp::Add) if lt == Ty::Str => {
                let place = self.place_text(lhs)?;
                match rt {
                    Ty::Char => {
                        let y = self.ex(rhs, false)?;
                        Ok(format!("{place}.push({} as char)", y.at(17)))
                    }
                    _ => {
                        let y = self.str_ref(rhs)?;
                        Ok(format!("{place}.push_str({})", y.s))
                    }
                }
            }
            Some(bo) => {
                let y = self.ex(rhs, false)?;
                self.compound(lhs, bo, y, &rt)
            }
        }
    }

    /// `x += 1` / `x -= 1` on any lvalue.
    pub(super) fn step_stmt(&mut self, target: &Expr, op: BinOp) -> R<String> {
        let y = self.int_lit(1, &Ty::Int);
        self.compound(target, op, y, &Ty::Int)
    }

    fn compound(&mut self, lhs: &Expr, bo: BinOp, y: Ex, rt: &Ty) -> R<String> {
        let lt = self.ty_of(lhs);
        if let Some((obj, name)) = self.handle_field(lhs) {
            if !is_pure(obj) {
                return no("Rust", "a compound assignment through a computed pointer");
            }
            let o = self.ex(obj, false)?;
            let cur = Ex::val(format!("{}.{name}()", o.at(19)));
            let r = self.arith(bo, cur, &lt, y, rt)?;
            let c = if matches!(bo, BinOp::Shl | BinOp::Shr) {
                promote(&lt)
            } else {
                common_ty(&lt, rt)
            };
            let r = self.conv_int(r, &c, &lt)?;
            return Ok(format!("{}.set_{name}({})", o.at(19), r.s));
        }
        if !lt.is_integral() || !rt.is_integral() {
            return no("Rust", format!("`{}=` on {lt}", bo.text()));
        }
        let place = self.place_text(lhs)?;
        let plain = |y: &Ex| y.lit.map_or_else(|| y.s.clone(), |v| v.to_string());
        let shift = matches!(bo, BinOp::Shl | BinOp::Shr);
        let wrap = matches!(bo, BinOp::Add | BinOp::Sub | BinOp::Mul) && lt.is_unsigned();
        let direct = (lt == *rt || y.lit.is_some() || shift)
            && !matches!(lt, Ty::Char | Ty::Bool)
            && (!wrap || (lt == Ty::Size && bo == BinOp::Add && y.lit.is_some()));
        if direct {
            let y = if shift { self.own(y, rt) } else { y };
            return Ok(format!("{place} {}= {}", bo.text(), plain(&y)));
        }
        if !is_pure(lhs) {
            return no("Rust", "this compound assignment");
        }
        if wrap && (lt == *rt || y.lit.is_some()) {
            let m = match bo {
                BinOp::Add => "wrapping_add",
                BinOp::Sub => "wrapping_sub",
                _ => "wrapping_mul",
            };
            return Ok(format!("{place} = {place}.{m}({})", plain(&y)));
        }
        let cur = self.ex(lhs, false)?;
        let cur = self.own(cur, &lt);
        let r = self.arith(bo, cur, &lt, y, rt)?;
        let c = if shift {
            promote(&lt)
        } else {
            common_ty(&lt, rt)
        };
        let r = self.conv_int(r, &c, &lt)?;
        Ok(format!("{place} = {}", r.s))
    }

    /// `T(args)`, `T{args}`, `new T(args)` and declarations `T x(args)`.
    pub(super) fn construct(&mut self, ty: &Ty, args: &[Expr], brace: bool) -> R<Ex> {
        let list = |this: &mut Self, el: &Ty| -> R<Vec<String>> {
            args.iter()
                .map(|a| this.conv_val(a, el).map(|x| x.s))
                .collect()
        };
        match ty {
            t if t.is_integral() => match args {
                [] => Ok(Ex::val(self.default_val(t)?)),
                [a] => self.conv_val(a, t),
                _ => no("Rust", "this initialisation"),
            },
            Ty::Vector(el) if brace || args.is_empty() => {
                let items = list(self, el)?;
                Ok(Ex::val(if items.is_empty() {
                    "Vec::new()".to_string()
                } else {
                    format!("vec![{}]", items.join(", "))
                }))
            }
            Ty::Vector(el) => match args {
                [n] if self.ty_of(n).is_integral() => {
                    let d = self.default_val(el)?;
                    let n = self.conv_val(n, &Ty::Size)?;
                    Ok(Ex::val(format!("vec![{d}; {}]", n.s)))
                }
                [n, x]
                    if self.ty_of(n).is_integral() && !matches!(self.ty_of(x), Ty::Iter { .. }) =>
                {
                    let x = self.conv_val(x, el)?;
                    let n = self.conv_val(n, &Ty::Size)?;
                    Ok(Ex::val(format!("vec![{}; {}]", x.s, n.s)))
                }
                [a, b] => match self.range(a, b)? {
                    Some(rg) => Ok(Ex::val(self.range_collect(&rg, ty)?)),
                    None => no("Rust", "this vector constructor"),
                },
                [a] => self.conv_val(a, ty),
                _ => no("Rust", "this vector constructor"),
            },
            Ty::Str => match args {
                [] => Ok(Ex::val("String::new()")),
                [a] => self.conv_val(a, ty),
                [n, c] if self.ty_of(c) == Ty::Char => {
                    let c = self.ex(c, false)?;
                    let n = self.conv_val(n, &Ty::Size)?;
                    Ok(Ex::val(format!(
                        "({} as char).to_string().repeat({})",
                        c.at(17),
                        n.s
                    )))
                }
                _ => no("Rust", "this string constructor"),
            },
            Ty::Pair(a, b) => match args {
                [] => Ok(Ex::val(self.default_val(ty)?)),
                [x, y] => Ok(Ex::val(format!(
                    "({}, {})",
                    self.conv_val(x, a)?.s,
                    self.conv_val(y, b)?.s
                ))),
                [x] => self.conv_val(x, ty),
                _ => no("Rust", "this pair constructor"),
            },
            Ty::Tuple(ts) => {
                if args.is_empty() {
                    return Ok(Ex::val(self.default_val(ty)?));
                }
                if args.len() != ts.len() {
                    return no("Rust", "this tuple constructor");
                }
                let parts: R<Vec<String>> = args
                    .iter()
                    .zip(ts)
                    .map(|(a, t)| self.conv_val(a, t).map(|x| x.s))
                    .collect();
                Ok(Ex::val(format!("({})", parts?.join(", "))))
            }
            Ty::Optional(t) => match args {
                [] => Ok(Ex::val("None")),
                [x] => Ok(Ex::val(format!("Some({})", self.conv_val(x, t)?.s))),
                _ => no("Rust", "this optional constructor"),
            },
            Ty::Struct(n) => {
                self.use_struct(n)?;
                let Some(s) = self.prog.find_struct(n) else {
                    return no("Rust", format!("the type {n}"));
                };
                let tys: Vec<Ty> = match s.ctors.first() {
                    Some(c) => c.params.iter().map(|p| p.spec.ty.clone()).collect(),
                    None => s.fields.iter().map(|f| f.spec.ty.clone()).collect(),
                };
                if tys.len() != args.len() {
                    return no("Rust", format!("this constructor call of {n}"));
                }
                let parts: R<Vec<String>> = args
                    .iter()
                    .zip(&tys)
                    .map(|(a, t)| self.conv_val(a, t).map(|x| x.s))
                    .collect();
                Ok(Ex::val(format!("{n}::new({})", parts?.join(", "))))
            }
            Ty::Map(..)
            | Ty::UnorderedMap(..)
            | Ty::Set(_)
            | Ty::UnorderedSet(_)
            | Ty::Deque(_)
            | Ty::Queue(_)
            | Ty::List(_)
            | Ty::Stack(_)
            | Ty::PriorityQueue(..) => match (args, brace) {
                ([], _) => Ok(Ex::val(self.default_val(ty)?)),
                ([a, b], false) => match self.range(a, b)? {
                    Some(rg) => Ok(Ex::val(self.range_collect(&rg, ty)?)),
                    None => no("Rust", "this container constructor"),
                },
                (_, true) => self.init_list(args, ty),
                _ => no("Rust", "this container constructor"),
            },
            t => no("Rust", format!("constructing {t}")),
        }
    }

    /// A braced list `{a, b}` converted to the type the context needs.
    pub(super) fn init_list(&mut self, items: &[Expr], to: &Ty) -> R<Ex> {
        let conv = |this: &mut Self, el: &Ty| -> R<Vec<String>> {
            items
                .iter()
                .map(|a| this.conv_val(a, el).map(|x| x.s))
                .collect()
        };
        match to {
            Ty::Vector(el) => {
                let v = conv(self, el)?;
                Ok(Ex::val(if v.is_empty() {
                    "Vec::new()".to_string()
                } else {
                    format!("vec![{}]", v.join(", "))
                }))
            }
            Ty::Array(el, _) => Ok(Ex::val(format!("[{}]", conv(self, el)?.join(", ")))),
            Ty::Deque(el) | Ty::Queue(el) | Ty::List(el) => {
                let v = conv(self, el)?;
                Ok(Ex::val(if v.is_empty() {
                    "VecDeque::new()".to_string()
                } else {
                    format!("VecDeque::from([{}])", v.join(", "))
                }))
            }
            Ty::Set(el) | Ty::UnorderedSet(el) => {
                let v = conv(self, el)?;
                let c = if matches!(to, Ty::Set(_)) {
                    "BTreeSet"
                } else {
                    "HashSet"
                };
                Ok(Ex::val(if v.is_empty() {
                    format!("{c}::new()")
                } else {
                    format!("{c}::from([{}])", v.join(", "))
                }))
            }
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => {
                let p = Ty::Pair(k.clone(), v.clone());
                let items = conv(self, &p)?;
                let c = if matches!(to, Ty::Map(..)) {
                    "BTreeMap"
                } else {
                    "HashMap"
                };
                Ok(Ex::val(if items.is_empty() {
                    format!("{c}::new()")
                } else {
                    format!("{c}::from([{}])", items.join(", "))
                }))
            }
            Ty::Pair(..) | Ty::Tuple(_) | Ty::Struct(_) | Ty::Optional(_) | Ty::Str => {
                self.construct(to, items, true)
            }
            t if t.is_integral() && items.len() == 1 => self.conv_val(&items[0], t),
            t if t.is_integral() && items.is_empty() => Ok(Ex::val(self.default_val(t)?)),
            t => no("Rust", format!("a braced list as {t}")),
        }
    }

    pub(super) fn lambda_ex(&mut self, e: &Expr, l: &Lambda) -> R<Ex> {
        let mut ps = Vec::new();
        self.push_scope();
        for p in &l.params {
            let mut spec = p.spec.clone();
            if spec.ty == Ty::Auto {
                spec.ty = self.info.decl_ty(p.id).clone();
            }
            let (t, rk) = self.param_ty(&spec)?;
            let m = if rk == Rk::Val && self.mutated.contains(&p.name) {
                "mut "
            } else {
                ""
            };
            ps.push(format!("{m}{}: {t}", self.name(&p.name)));
            let ty = if spec.ty == Ty::StrView {
                Ty::Str
            } else {
                spec.ty.clone()
            };
            self.declare(&p.name, ty, rk);
        }
        let ret = match &l.ret {
            Some(r) => r.ty.clone(),
            None => self.lambda_ret(&l.body),
        };
        let saved = std::mem::replace(&mut self.ret, ret.clone());
        self.in_lambda += 1;
        let conts = std::mem::take(&mut self.cont);
        let single = match l.body.stmts.as_slice() {
            [
                Stmt {
                    kind: StmtKind::Return(Some(x)),
                    ..
                },
            ] => Some(x),
            _ => None,
        };
        let text = if let Some(x) = single {
            self.conv_val(x, &ret).map(|v| v.s)
        } else {
            let mut body = String::new();
            let r = self.stmts(&l.body.stmts, 2, &mut body, false);
            let rt = if ret == Ty::Void {
                String::new()
            } else {
                format!(" -> {}", self.rty(&ret)?)
            };
            r.map(|()| format!("{rt} {{\n{body}    }}"))
        };
        self.cont = conts;
        self.in_lambda -= 1;
        self.ret = saved;
        self.pop_scope();
        let text = text?;
        let mv = if l.captures.default == CaptureDefault::Copy {
            "move "
        } else {
            ""
        };
        let sep = if single.is_some() { " " } else { "" };
        let _ = e;
        Ok(Ex::new(
            format!("{mv}|{}|{sep}{text}", ps.join(", ")),
            1,
            Kind::Val,
        ))
    }

    /// The type a lambda without `-> T` returns: the type of its first `return e;`.
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
