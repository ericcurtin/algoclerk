// SPDX-License-Identifier: Apache-2.0
//! Rust translation of `for`, range-`for` and `switch`.

use super::{Rk, Rs};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

fn pad(n: usize) -> String {
    "    ".repeat(n)
}

struct Counting<'e> {
    var: String,
    ty: Ty,
    start: &'e Expr,
    limit: &'e Expr,
    op: BinOp,
    up: bool,
    step: u64,
}

impl Rs<'_> {
    /// Recognise `for (T i = a; i < b; ++i)` and its variants.
    fn counting<'e>(
        &self,
        init: Option<&'e Stmt>,
        cond: Option<&'e Expr>,
        incr: Option<&'e Expr>,
        body: &Stmt,
    ) -> Option<Counting<'e>> {
        let StmtKind::Decl { spec, vars } = &init?.kind else {
            return None;
        };
        let [v] = vars.as_slice() else { return None };
        let Init::Copy(start) = &v.init else {
            return None;
        };
        let ty = self.info.decl_ty(v.id).clone();
        if spec.is_ref || v.array.is_some() || !ty.is_integral() || ty == Ty::Bool {
            return None;
        }
        let ExprKind::Binary(op, l, limit) = &cond?.kind else {
            return None;
        };
        if !matches!(&l.kind, ExprKind::Var(n) if n == &v.name) {
            return None;
        }
        let (up, step) = match &incr?.kind {
            ExprKind::Unary(UnOp::PreInc | UnOp::PostInc, a) if is_var(a, &v.name) => (true, 1),
            ExprKind::Unary(UnOp::PreDec | UnOp::PostDec, a) if is_var(a, &v.name) => (false, 1),
            ExprKind::Assign {
                op: Some(o @ (BinOp::Add | BinOp::Sub)),
                lhs,
                rhs,
            } if is_var(lhs, &v.name) => {
                let ExprKind::Int { value, .. } = rhs.kind else {
                    return None;
                };
                if value == 0 {
                    return None;
                }
                (*o == BinOp::Add, value)
            }
            _ => return None,
        };
        let ok = matches!(
            (up, op),
            (true, BinOp::Lt | BinOp::Le) | (false, BinOp::Gt | BinOp::Ge)
        );
        if !ok {
            return None;
        }
        let m = mutated_in_stmt(self.prog, self.info, body);
        let (names, calls) = names_and_calls(limit);
        let (sn, sc) = names_and_calls(start);
        if m.contains(&v.name) || calls || sc || names.contains(&v.name) || sn.contains(&v.name) {
            return None;
        }
        if names.iter().any(|n| m.contains(n)) {
            return None;
        }
        let lt = self.info.ty(limit.id);
        let non_neg_lit = matches!(start.kind, ExprKind::Int { .. });
        let lit_limit = matches!(limit.kind, ExprKind::Int { .. });
        let compat =
            *lt == ty || lit_limit || (non_neg_lit && lt.is_integral() && !ty.is_unsigned());
        if !compat || !lt.is_integral() {
            return None;
        }
        Some(Counting {
            var: v.name.clone(),
            ty,
            start,
            limit,
            op: *op,
            up,
            step,
        })
    }

    pub(super) fn for_loop(
        &mut self,
        init: Option<&Stmt>,
        cond: Option<&Expr>,
        incr: Option<&Expr>,
        body: &Stmt,
        ind: usize,
        out: &mut String,
    ) -> R<()> {
        let p = pad(ind);
        if let Some(c) = self.counting(init, cond, incr, body) {
            let ty = c.ty.clone();
            let lo = self.conv_val(c.start, &ty)?;
            let lim_ty = self.info.ty(c.limit.id).clone();
            let lx = self.ex(c.limit, false)?;
            let hi = self.conv_int(lx, &lim_ty, &ty)?;
            let range = if c.up {
                let r = if c.op == BinOp::Lt {
                    format!("{}..{}", lo.at(5), hi.at(5))
                } else {
                    format!("{}..={}", lo.at(5), hi.at(5))
                };
                if c.step > 1 {
                    format!("({r}).step_by({})", c.step)
                } else {
                    r
                }
            } else {
                let lim = if c.op == BinOp::Gt {
                    match hi.lit {
                        Some(v) => self.int_lit(v + 1, &ty),
                        None => Ex::new(format!("{} + 1", hi.at(12)), 12, Kind::Val),
                    }
                } else {
                    hi
                };
                let step = if c.step > 1 {
                    format!(".step_by({})", c.step)
                } else {
                    String::new()
                };
                format!("({}..={}).rev(){step}", lim.at(5), lo.at(5))
            };
            self.push_scope();
            self.declare(&c.var, ty, Rk::Val);
            self.cont.push(None);
            let b = self.block_body(body, ind + 1)?;
            self.cont.pop();
            self.pop_scope();
            out.push_str(&format!(
                "{p}for {} in {range} {{\n{b}{p}}}\n",
                self.name(&c.var)
            ));
            return Ok(());
        }
        let wrap = init.is_some();
        let ip = if wrap { ind + 1 } else { ind };
        let q = pad(ip);
        self.push_scope();
        if wrap {
            out.push_str(&format!("{p}{{\n"));
        }
        if let Some(i) = init {
            self.stmt(i, ip, out, false)?;
        }
        let inc = match incr {
            Some(e) => Some(self.expr_stmt(e)?),
            None => None,
        };
        self.cont.push(inc.clone());
        let b = self.block_body(body, ip + 1)?;
        self.cont.pop();
        let tail = inc.map_or(String::new(), |t| format!("{}{t}\n", pad(ip + 1)));
        match cond {
            None => out.push_str(&format!("{q}loop {{\n{b}{tail}{q}}}\n")),
            Some(c) => {
                let c = self.as_bool(c)?.s;
                out.push_str(&format!("{q}while {c} {{\n{b}{tail}{q}}}\n"));
            }
        }
        if wrap {
            out.push_str(&format!("{p}}}\n"));
        }
        self.pop_scope();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn range_for(
        &mut self,
        id: NodeId,
        spec: &TypeSpec,
        names: &[String],
        structured: bool,
        range: &Expr,
        body: &Stmt,
        ind: usize,
        out: &mut String,
    ) -> R<()> {
        let p = pad(ind);
        let cty = self.info.ty(range.id).clone();
        let binds: Vec<(String, Ty)> = self.info.bindings(id).to_vec();
        let by_ref = spec.is_ref;
        let mutable = by_ref && !spec.konst;
        let x = self.ex(range, mutable)?;
        let xs = x.at(19);
        let mut declared: Vec<(String, Ty, Rk)> = Vec::new();
        let rk_ref = if mutable { Rk::RefMut } else { Rk::Ref };
        let pat_mut = |this: &Self, n: &str| if this.mutated.contains(n) { "mut " } else { "" };
        let (pat, iter): (String, String) = match &cty {
            Ty::Str | Ty::StrView => {
                if mutable || structured {
                    return no("Rust", "this loop over a string");
                }
                let n = &names[0];
                declared.push((n.clone(), Ty::Char, Rk::Val));
                (
                    format!("{}{}", pat_mut(self, n), self.name(n)),
                    format!("{xs}.bytes()"),
                )
            }
            Ty::Map(..) | Ty::UnorderedMap(..) => {
                let it = if mutable {
                    format!("{xs}.iter_mut()")
                } else {
                    format!("{xs}.iter()")
                };
                if structured {
                    let tys: Vec<Ty> = binds.iter().map(|b| b.1.clone()).collect();
                    for (n, t) in names.iter().zip(&tys) {
                        declared.push((
                            n.clone(),
                            t.clone(),
                            if by_ref { rk_ref } else { Rk::Val },
                        ));
                    }
                    let ps: Vec<String> = names.iter().map(|n| self.name(n)).collect();
                    if by_ref {
                        (format!("({})", ps.join(", ")), it)
                    } else {
                        let ps2: Vec<String> = names
                            .iter()
                            .zip(&ps)
                            .map(|(n, q)| format!("{}{q}", pat_mut(self, n)))
                            .collect();
                        (
                            format!("({})", ps2.join(", ")),
                            format!("{xs}.iter().map(|(k, v)| (k.clone(), v.clone()))"),
                        )
                    }
                } else {
                    let n = &names[0];
                    let t = binds.first().map(|b| b.1.clone()).unwrap_or(Ty::Unknown);
                    if by_ref {
                        declared.push((n.clone(), t, Rk::Entry));
                        (self.name(n), it)
                    } else {
                        declared.push((n.clone(), t, Rk::Val));
                        (
                            format!("{}{}", pat_mut(self, n), self.name(n)),
                            format!("{xs}.iter().map(|(k, v)| (k.clone(), v.clone()))"),
                        )
                    }
                }
            }
            Ty::Vector(e)
            | Ty::Array(e, _)
            | Ty::Deque(e)
            | Ty::List(e)
            | Ty::Set(e)
            | Ty::UnorderedSet(e) => {
                let copy = self.is_copy(e);
                let it = if mutable {
                    format!("{xs}.iter_mut()")
                } else {
                    format!("{xs}.iter()")
                };
                if structured {
                    let tys: Vec<Ty> = binds.iter().map(|b| b.1.clone()).collect();
                    for (n, t) in names.iter().zip(&tys) {
                        declared.push((
                            n.clone(),
                            t.clone(),
                            if by_ref { rk_ref } else { Rk::Val },
                        ));
                    }
                    if by_ref {
                        let ps: Vec<String> = names.iter().map(|n| self.name(n)).collect();
                        (format!("({})", ps.join(", ")), it)
                    } else {
                        let ps: Vec<String> = names
                            .iter()
                            .map(|n| format!("{}{}", pat_mut(self, n), self.name(n)))
                            .collect();
                        let adapter = if copy { "copied" } else { "cloned" };
                        (
                            format!("({})", ps.join(", ")),
                            format!("{xs}.iter().{adapter}()"),
                        )
                    }
                } else {
                    let n = &names[0];
                    let nm = self.name(n);
                    if by_ref {
                        declared.push((n.clone(), (**e).clone(), rk_ref));
                        (nm, it)
                    } else {
                        declared.push((n.clone(), (**e).clone(), Rk::Val));
                        let m = pat_mut(self, n);
                        if copy && m.is_empty() {
                            (format!("&{nm}"), format!("{xs}.iter()"))
                        } else {
                            let adapter = if copy { "copied" } else { "cloned" };
                            (format!("{m}{nm}"), format!("{xs}.iter().{adapter}()"))
                        }
                    }
                }
            }
            t => return no("Rust", format!("a range-for over {t}")),
        };
        self.push_scope();
        for (n, t, rk) in declared {
            self.declare(&n, t, rk);
        }
        self.cont.push(None);
        let b = self.block_body(body, ind + 1)?;
        self.cont.pop();
        self.pop_scope();
        out.push_str(&format!("{p}for {pat} in {iter} {{\n{b}{p}}}\n"));
        Ok(())
    }

    pub(super) fn switch(
        &mut self,
        cond: &Expr,
        body: &Block,
        ind: usize,
        out: &mut String,
    ) -> R<()> {
        let p = pad(ind);
        let ct = self.info.ty(cond.id).clone();
        let c = self.ex(cond, false)?;
        let c = self.own(c, &ct);
        let mut groups: Vec<(Vec<Option<&Expr>>, Vec<&Stmt>)> = Vec::new();
        let mut fresh = true;
        for s in &body.stmts {
            match &s.kind {
                StmtKind::Case(l) => {
                    if fresh {
                        groups.push((Vec::new(), Vec::new()));
                    }
                    if let Some(g) = groups.last_mut() {
                        g.0.push(l.as_ref());
                    }
                    fresh = false;
                }
                _ => {
                    let Some(g) = groups.last_mut() else {
                        return no("Rust", "statements before the first case");
                    };
                    g.1.push(s);
                    fresh = true;
                    if matches!(s.kind, StmtKind::Break) {
                        fresh = true;
                    }
                }
            }
        }
        let mut arms = String::new();
        let n = groups.len();
        for (gi, (labels, stmts)) in groups.iter().enumerate() {
            let mut stmts = stmts.clone();
            let ends_break = matches!(stmts.last().map(|s| &s.kind), Some(StmtKind::Break));
            if ends_break {
                stmts.pop();
            }
            let last_diverges = matches!(
                stmts.last().map(|s| &s.kind),
                Some(StmtKind::Return(_) | StmtKind::Continue)
            );
            if !ends_break && !last_diverges && gi + 1 != n && !stmts.is_empty() {
                return no("Rust", "a switch case that falls through into the next");
            }
            if gi + 1 != n && stmts.is_empty() {
                return no("Rust", "a switch case that falls through into the next");
            }
            if stmts.iter().any(|s| has_break(s)) {
                return no("Rust", "a `break` inside a switch case");
            }
            if labels.iter().any(Option::is_none) && gi + 1 != n {
                return no("Rust", "a switch whose default label is not last");
            }
            let mut pats = Vec::new();
            for l in labels {
                match l {
                    None => pats.push("_".to_string()),
                    Some(e) => {
                        let v = self.conv_val(e, &ct)?;
                        pats.push(v.s);
                    }
                }
            }
            let pat = if pats.iter().any(|x| x == "_") {
                "_".to_string()
            } else {
                pats.join(" | ")
            };
            let mut body_s = String::new();
            self.push_scope();
            for s in stmts {
                self.stmt(s, ind + 2, &mut body_s, false)?;
            }
            self.pop_scope();
            arms.push_str(&format!(
                "{}{pat} => {{\n{body_s}{}}}\n",
                pad(ind + 1),
                pad(ind + 1)
            ));
        }
        if !arms.contains("_ => {") {
            arms.push_str(&format!("{}_ => {{}}\n", pad(ind + 1)));
        }
        out.push_str(&format!("{p}match {} {{\n{arms}{p}}}\n", c.s));
        Ok(())
    }
}

fn is_var(e: &Expr, n: &str) -> bool {
    matches!(&e.kind, ExprKind::Var(m) if m == n)
}
