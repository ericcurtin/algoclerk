// SPDX-License-Identifier: Apache-2.0
//! Rust translation of statements.

use super::{Rk, Rs};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

fn pad(n: usize) -> String {
    "    ".repeat(n)
}

/// Does control never reach the end of this statement list?
fn diverges(stmts: &[Stmt]) -> bool {
    match stmts.last().map(|s| &s.kind) {
        Some(StmtKind::Return(_)) => true,
        Some(StmtKind::Block(b)) => diverges(&b.stmts),
        Some(StmtKind::If {
            then, els: Some(e), ..
        }) => diverges(std::slice::from_ref(then)) && diverges(std::slice::from_ref(e)),
        Some(StmtKind::While { cond, body }) => {
            matches!(cond.kind, ExprKind::Bool(true)) && !has_break(body)
        }
        _ => false,
    }
}

impl Rs<'_> {
    /// The body of a function: statements with the final `return` as the tail expression.
    pub(super) fn fn_body(&mut self, b: &Block, ret: &Ty) -> R<String> {
        let mut out = String::new();
        self.stmts(&b.stmts, 1, &mut out, true)?;
        if *ret != Ty::Void && !diverges(&b.stmts) {
            out.push_str(&format!(
                "{}unreachable!(\"fell off the end of the function\")\n",
                pad(1)
            ));
        }
        Ok(out)
    }

    /// A braced block at indentation `ind` (the braces are the caller's).
    pub(super) fn block_body(&mut self, s: &Stmt, ind: usize) -> R<String> {
        let mut out = String::new();
        self.push_scope();
        match &s.kind {
            StmtKind::Block(b) => self.stmts(&b.stmts, ind, &mut out, false)?,
            _ => self.stmt(s, ind, &mut out, false)?,
        }
        self.pop_scope();
        Ok(out)
    }

    pub(super) fn stmts(
        &mut self,
        list: &[Stmt],
        ind: usize,
        out: &mut String,
        tail: bool,
    ) -> R<()> {
        for (i, s) in list.iter().enumerate() {
            self.stmt(s, ind, out, tail && i + 1 == list.len())?;
        }
        Ok(())
    }

    fn cond(&mut self, e: &Expr) -> R<String> {
        Ok(self.as_bool(e)?.s)
    }

    pub(super) fn stmt(&mut self, s: &Stmt, ind: usize, out: &mut String, tail: bool) -> R<()> {
        let p = pad(ind);
        for c in &s.comments {
            out.push_str(&format!("{p}//{c}\n"));
        }
        let mut line = String::new();
        let trailing = s.trailing.as_ref().map(|t| format!(" //{t}"));
        match &s.kind {
            StmtKind::Decl { spec, vars } => {
                for v in vars {
                    line.push_str(&format!("{p}{}\n", self.decl(spec, v)?));
                }
            }
            StmtKind::Structured { spec, names, init } => {
                let b: Vec<(String, Ty)> = self.info.bindings(s.id).to_vec();
                line.push_str(&format!("{p}{}\n", self.structured(spec, names, &b, init)?));
            }
            StmtKind::Expr(e) => {
                let t = self.expr_stmt(e)?;
                if !t.is_empty() {
                    line.push_str(&format!("{p}{t}\n"));
                }
            }
            StmtKind::If {
                init,
                cond,
                then,
                els,
            } => {
                if let Some(i) = init {
                    // without a name clash the `let` is simply hoisted in front of the `if`
                    let wrap = !self.hoistable(i);
                    let ip = if wrap { ind + 1 } else { ind };
                    if wrap {
                        line.push_str(&format!("{p}{{\n"));
                    }
                    self.push_scope();
                    self.stmt(i, ip, &mut line, false)?;
                    self.if_chain(cond, then, els.as_deref(), ip, &mut line)?;
                    self.pop_scope();
                    if wrap {
                        line.push_str(&format!("{p}}}\n"));
                    }
                } else {
                    self.if_chain(cond, then, els.as_deref(), ind, &mut line)?;
                }
            }
            StmtKind::Switch { cond, body } => self.switch(cond, body, ind, &mut line)?,
            StmtKind::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.for_loop(
                    init.as_deref(),
                    cond.as_ref(),
                    incr.as_ref(),
                    body,
                    ind,
                    &mut line,
                )?;
            }
            StmtKind::RangeFor {
                spec,
                names,
                structured,
                range,
                body,
            } => {
                self.range_for(s.id, spec, names, *structured, range, body, ind, &mut line)?;
            }
            StmtKind::While { cond, body } => {
                self.cont.push(None);
                let b = self.block_body(body, ind + 1)?;
                self.cont.pop();
                if matches!(cond.kind, ExprKind::Bool(true)) {
                    line.push_str(&format!("{p}loop {{\n{b}{p}}}\n"));
                } else {
                    let c = self.cond(cond)?;
                    line.push_str(&format!("{p}while {c} {{\n{b}{p}}}\n"));
                }
            }
            StmtKind::DoWhile { body, cond } => {
                if has_continue(body) {
                    return no("Rust", "a `continue` inside do-while");
                }
                self.cont.push(None);
                let b = self.block_body(body, ind + 1)?;
                self.cont.pop();
                let c = self.cond(cond)?;
                line.push_str(&format!(
                    "{p}loop {{\n{b}{}if !({c}) {{\n{}break;\n{}}}\n{p}}}\n",
                    pad(ind + 1),
                    pad(ind + 2),
                    pad(ind + 1)
                ));
            }
            StmtKind::Return(None) => line.push_str(&format!("{p}return;\n")),
            StmtKind::Return(Some(e)) => {
                let ret = self.ret.clone();
                let x = self.ret_val(e, &ret)?;
                if tail && self.in_lambda == 0 {
                    line.push_str(&format!("{p}{x}\n"));
                } else {
                    line.push_str(&format!("{p}return {x};\n"));
                }
            }
            StmtKind::Break => line.push_str(&format!("{p}break;\n")),
            StmtKind::Continue => match self.cont.last() {
                Some(Some(inc)) => line.push_str(&format!("{p}{inc}\n{p}continue;\n")),
                _ => line.push_str(&format!("{p}continue;\n")),
            },
            StmtKind::Block(b) => {
                line.push_str(&format!("{p}{{\n"));
                self.push_scope();
                self.stmts(&b.stmts, ind + 1, &mut line, false)?;
                self.pop_scope();
                line.push_str(&format!("{p}}}\n"));
            }
            StmtKind::Empty => {}
            StmtKind::Case(_) => return no("Rust", "a case label outside a switch"),
        }
        if let Some(t) = trailing
            && let Some(first_end) = line.find('\n')
        {
            line.insert_str(first_end, &t);
        }
        out.push_str(&line);
        Ok(())
    }

    /// May the names declared by an init-statement be introduced in the enclosing Rust block? Yes when none
    /// of them is already visible there: the C++ scope ends with the statement, so no later code can mean
    /// anything else by the name, and a later declaration of the same name just shadows the hoisted one.
    pub(super) fn hoistable(&self, init: &Stmt) -> bool {
        declared_names(init)
            .iter()
            .all(|n| self.lookup(n).is_none())
    }

    fn if_chain(
        &mut self,
        cond: &Expr,
        then: &Stmt,
        els: Option<&Stmt>,
        ind: usize,
        out: &mut String,
    ) -> R<()> {
        let p = pad(ind);
        let c = self.cond(cond)?;
        let t = self.block_body(then, ind + 1)?;
        out.push_str(&format!("{p}if {c} {{\n{t}{p}}}"));
        let mut rest = els;
        while let Some(e) = rest {
            match &e.kind {
                StmtKind::If {
                    init: None,
                    cond,
                    then,
                    els,
                } => {
                    let c = self.cond(cond)?;
                    let t = self.block_body(then, ind + 1)?;
                    out.push_str(&format!(" else if {c} {{\n{t}{p}}}"));
                    rest = els.as_deref();
                }
                _ => {
                    let b = self.block_body(e, ind + 1)?;
                    out.push_str(&format!(" else {{\n{b}{p}}}"));
                    rest = None;
                }
            }
        }
        out.push('\n');
        Ok(())
    }

    /// `return e;` value: a local variable is moved, everything else converted to the return type.
    fn ret_val(&mut self, e: &Expr, ret: &Ty) -> R<String> {
        if let (ExprKind::Var(n), 0) = (&e.kind, self.in_lambda)
            && let Some(v) = self.lookup(n)
            && v.rk == Rk::Val
            && &v.ty == ret
            && !matches!(
                self.info.resolved(e.id),
                Some(crate::cpp::check::Resolved::Func(_))
            )
        {
            return Ok(self.name(n));
        }
        Ok(self.conv_val(e, ret)?.s)
    }

    fn decl(&mut self, spec: &TypeSpec, v: &Declarator) -> R<String> {
        let ty = self.info.decl_ty(v.id).clone();
        let name = self.name(&v.name);
        if let Ty::Iter { container, .. } = &ty {
            return self.found_decl(v, container);
        }
        if spec.is_ref {
            return self.ref_decl(spec, v, &ty);
        }
        let mut mutable = !spec.konst && self.mutated.contains(&v.name);
        if let (Ty::Lambda(_), Init::Copy(e)) = (&ty, &v.init)
            && let ExprKind::Lambda(l) = &e.kind
        {
            let own: std::collections::HashSet<&str> =
                l.params.iter().map(|p| p.name.as_str()).collect();
            mutable = mutated_names(self.prog, self.info, &l.body)
                .iter()
                .any(|n| !own.contains(n.as_str()));
        }
        let m = if mutable { "mut " } else { "" };
        let init = match (&v.array, &v.init) {
            (Some(n), init) => self.array_init(&ty, n, init)?,
            (None, Init::None) => self.default_val(&ty)?,
            // the annotation fixes the type of a literal: `let n: usize = 0`
            (None, Init::Copy(e)) if !matches!(ty, Ty::Lambda(_)) => {
                super::expr::bare_lit(self.conv_val(e, &ty)?).s
            }
            (None, Init::Copy(e)) => self.conv_val(e, &ty)?.s,
            (None, Init::Direct(a)) => self.construct(&ty, a, false)?.s,
            (None, Init::Brace { elems, .. }) => self.construct(&ty, elems, true)?.s,
        };
        let rk = if ty == Ty::StrView {
            Rk::StrRef
        } else {
            Rk::Val
        };
        self.declare(&v.name, ty.clone(), rk);
        if let Ty::Lambda(_) = ty {
            return Ok(format!("let {m}{name} = {init};"));
        }
        let t = self.rty(&ty)?;
        Ok(format!("let {m}{name}: {t} = {init};"))
    }

    fn array_init(&mut self, ty: &Ty, n: &Expr, init: &Init) -> R<String> {
        let Ty::Array(elem, _) = ty else {
            return no("Rust", "this array declaration");
        };
        let len = self.conv_val(n, &Ty::Size)?;
        let elems: &[Expr] = match init {
            Init::None => &[],
            Init::Brace { elems, .. } | Init::Direct(elems) => elems,
            Init::Copy(_) => return no("Rust", "this array initialiser"),
        };
        if elems.len() > 1
            || (elems.len() == 1 && !matches!(elems[0].kind, ExprKind::Int { value: 0, .. }))
        {
            let ExprKind::Int { value: n, .. } = n.kind else {
                return no("Rust", "an array of unknown length with values");
            };
            let mut parts = Vec::new();
            for e in elems {
                parts.push(self.conv_val(e, elem)?.s);
            }
            let zero = self.default_val(elem)?;
            while parts.len() < n as usize {
                parts.push(zero.clone());
            }
            return Ok(format!("[{}]", parts.join(", ")));
        }
        let d = self.default_val(elem)?;
        Ok(format!("[{d}; {}]", len.s))
    }

    fn ref_decl(&mut self, spec: &TypeSpec, v: &Declarator, ty: &Ty) -> R<String> {
        let Init::Copy(e) = &v.init else {
            return no("Rust", "a reference without an initialiser");
        };
        let name = self.name(&v.name);
        let mutable = !spec.konst;
        let x = self.ex(e, mutable)?;
        let (text, rk) = match (x.kind, mutable) {
            (Kind::Val, false) => (x.s, Rk::Val),
            (Kind::Val, true) => return no("Rust", "a reference to a temporary"),
            (Kind::Ref, _) => (x.s, if mutable { Rk::RefMut } else { Rk::Ref }),
            (Kind::Place, true) => (format!("&mut {}", x.at(17)), Rk::RefMut),
            (Kind::Place, false) => (format!("&{}", x.at(17)), Rk::Ref),
        };
        self.declare(&v.name, ty.clone(), rk);
        Ok(format!("let {name} = {text};"))
    }

    fn structured(
        &mut self,
        spec: &TypeSpec,
        names: &[String],
        b: &[(String, Ty)],
        init: &Expr,
    ) -> R<String> {
        let mutable = spec.is_ref && !spec.konst;
        let x = self.ex(init, mutable)?;
        let from = self.info.ty(init.id).clone();
        let (rhs, rk) = if spec.is_ref && x.kind != Kind::Val {
            let amp = if mutable { "&mut " } else { "&" };
            let rk = if mutable { Rk::RefMut } else { Rk::Ref };
            let t = if x.kind == Kind::Ref {
                x.s.clone()
            } else {
                format!("{amp}{}", x.at(17))
            };
            (t, rk)
        } else {
            (self.own(x, &from).s, Rk::Val)
        };
        let mut pats = Vec::new();
        for (n, (_, t)) in names.iter().zip(b) {
            let m = if rk == Rk::Val && self.mutated.contains(n) {
                "mut "
            } else {
                ""
            };
            pats.push(format!("{m}{}", self.name(n)));
            self.declare(n, t.clone(), rk);
        }
        Ok(format!("let ({}) = {rhs};", pats.join(", ")))
    }

    fn found_decl(&mut self, v: &Declarator, container: &Ty) -> R<String> {
        let Init::Copy(e) = &v.init else {
            return no("Rust", "an iterator variable");
        };
        let ExprKind::Call { callee, args } = &e.kind else {
            return no("Rust", "an iterator variable");
        };
        let ExprKind::Member { obj, name, .. } = &callee.kind else {
            return no("Rust", "an iterator variable");
        };
        if name != "find" || args.len() != 1 {
            return no("Rust", "an iterator other than the result of find");
        }
        let (kt, is_map) = match container {
            Ty::Map(k, _) | Ty::UnorderedMap(k, _) => (k.as_ref().clone(), true),
            Ty::Set(k) | Ty::UnorderedSet(k) => (k.as_ref().clone(), false),
            _ => return no("Rust", "find on this container"),
        };
        let key = self.conv_val(&args[0], &kt)?;
        let mutable = self.mutated.contains(&v.name);
        let o = self.ex(obj, mutable)?;
        let text = match (is_map, mutable) {
            (true, false) => format!("{}.get_key_value(&{})", o.at(19), key.at(17)),
            (true, true) => format!("{}.get_mut(&{})", o.at(19), key.at(17)),
            (false, _) => format!("{}.get(&{})", o.at(19), key.at(17)),
        };
        self.declare(&v.name, container.clone(), Rk::Found(mutable));
        let m = if mutable { "mut " } else { "" };
        Ok(format!("let {m}{} = {text};", self.name(&v.name)))
    }
}
