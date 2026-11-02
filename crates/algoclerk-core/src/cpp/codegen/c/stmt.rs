// SPDX-License-Identifier: Apache-2.0
//! C translation of statements. The control flow keywords are the same as in C++; declarations, references
//! (pointers), structured bindings and lambdas (lifted to functions) need work.

use super::{Cx, Rk};
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

pub(super) fn pad(n: usize) -> String {
    "    ".repeat(n)
}

impl Cx<'_> {
    pub(super) fn stmts(&mut self, list: &[Stmt], ind: usize, out: &mut String) -> R<()> {
        for s in list {
            self.stmt(s, ind, out)?;
        }
        Ok(())
    }

    /// A statement or block as the body of a control statement (always braced).
    pub(super) fn body(&mut self, s: &Stmt, ind: usize) -> R<String> {
        let mut out = String::new();
        self.push_scope();
        match &s.kind {
            StmtKind::Block(b) => self.stmts(&b.stmts, ind, &mut out)?,
            _ => self.stmt(s, ind, &mut out)?,
        }
        self.pop_scope();
        Ok(out)
    }

    pub(super) fn stmt(&mut self, s: &Stmt, ind: usize, out: &mut String) -> R<()> {
        let p = pad(ind);
        for c in &s.comments {
            out.push_str(&format!("{p}//{c}\n"));
        }
        let mut line = String::new();
        match &s.kind {
            StmtKind::Decl { spec, vars } => {
                for v in vars {
                    let d = self.decl(spec, v)?;
                    if !d.is_empty() {
                        line.push_str(&format!("{p}{d}\n"));
                    }
                }
            }
            StmtKind::Structured { spec, names, init } => {
                let b: Vec<(String, Ty)> = self.info.bindings(s.id).to_vec();
                for l in self.structured(spec, names, &b, init)? {
                    line.push_str(&format!("{p}{l}\n"));
                }
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
                    line.push_str(&format!("{p}{{\n"));
                    self.push_scope();
                    self.stmt(i, ind + 1, &mut line)?;
                    self.if_chain(cond, then, els.as_deref(), ind + 1, &mut line)?;
                    self.pop_scope();
                    line.push_str(&format!("{p}}}\n"));
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
            } => self.for_loop(
                init.as_deref(),
                cond.as_ref(),
                incr.as_ref(),
                body,
                ind,
                &mut line,
            )?,
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
                let c = self.cond(cond)?.s;
                let b = self.body(body, ind + 1)?;
                line.push_str(&format!("{p}while ({c}) {{\n{b}{p}}}\n"));
            }
            StmtKind::DoWhile { body, cond } => {
                let b = self.body(body, ind + 1)?;
                let c = self.cond(cond)?.s;
                line.push_str(&format!("{p}do {{\n{b}{p}}} while ({c});\n"));
            }
            StmtKind::Return(None) => line.push_str(&format!("{p}return;\n")),
            StmtKind::Return(Some(e)) => {
                let ret = self.ret.clone();
                let moved = match &e.kind {
                    ExprKind::Var(n) => self
                        .lookup(n)
                        .filter(|v| v.rk == Rk::Val && v.ty == ret && !self.is_global(n))
                        .map(|_| self.name(n)),
                    _ => None,
                };
                let text = match moved {
                    Some(t) => t,
                    None => self.conv_val(e, &ret)?.s,
                };
                line.push_str(&format!("{p}return {text};\n"));
            }
            StmtKind::Break => line.push_str(&format!("{p}break;\n")),
            StmtKind::Continue => line.push_str(&format!("{p}continue;\n")),
            StmtKind::Block(b) => {
                line.push_str(&format!("{p}{{\n"));
                self.push_scope();
                self.stmts(&b.stmts, ind + 1, &mut line)?;
                self.pop_scope();
                line.push_str(&format!("{p}}}\n"));
            }
            StmtKind::Empty => {}
            StmtKind::Case(_) => return no("C", "a case label outside a switch"),
        }
        if let Some(t) = &s.trailing
            && let Some(first_end) = line.find('\n')
        {
            line.insert_str(first_end, &format!(" //{t}"));
        }
        out.push_str(&super::tidy(&line));
        Ok(())
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
        let c = self.cond(cond)?.s;
        let t = self.body(then, ind + 1)?;
        out.push_str(&format!("{p}if ({c}) {{\n{t}{p}}}"));
        let mut rest = els;
        while let Some(e) = rest {
            match &e.kind {
                StmtKind::If {
                    init: None,
                    cond,
                    then,
                    els,
                } => {
                    let c = self.cond(cond)?.s;
                    let t = self.body(then, ind + 1)?;
                    out.push_str(&format!(" else if ({c}) {{\n{t}{p}}}"));
                    rest = els.as_deref();
                }
                _ => {
                    let b = self.body(e, ind + 1)?;
                    out.push_str(&format!(" else {{\n{b}{p}}}"));
                    rest = None;
                }
            }
        }
        out.push('\n');
        Ok(())
    }

    /// One declarator as a C declaration (with `;`); a lifted lambda produces no text.
    pub(super) fn decl(&mut self, spec: &TypeSpec, v: &Declarator) -> R<String> {
        let ty = self.info.decl_ty(v.id).clone();
        let name = self.name(&v.name);
        if let Ty::Iter { container, .. } = &ty {
            return self.found_decl(v, container);
        }
        if let (Ty::Lambda(id), Init::Copy(e)) = (&ty, &v.init)
            && let ExprKind::Lambda(l) = &e.kind
        {
            let f = self.lift_lambda(*id, l, &name)?;
            self.declare(&v.name, ty, Rk::Lambda(f));
            return Ok(String::new());
        }
        if spec.is_ref && !(spec.konst && self.is_plain(&ty)) {
            return self.ref_decl(v, &ty);
        }
        let sp = |t: &str| if t.ends_with('*') { "" } else { " " };
        if let Some(n) = &v.array {
            let Ty::Array(el, _) = &ty else {
                return no("C", "this array declaration");
            };
            let ct = self.ctype(el)?;
            let len = self.conv_val(n, &Ty::Size)?;
            let konst = if spec.konst && self.is_plain(el) {
                "const "
            } else {
                ""
            };
            let elems: Vec<&Expr> = match &v.init {
                Init::Brace { elems, .. } | Init::Direct(elems) => elems.iter().collect(),
                _ => Vec::new(),
            };
            let mut parts = Vec::new();
            for e in elems {
                parts.push(self.conv_val(e, el)?.s);
            }
            if parts.is_empty() {
                parts.push("0".into());
            }
            self.declare(&v.name, ty.clone(), Rk::Native);
            return Ok(format!(
                "{konst}{ct}{}{name}[{}] = {{{}}};",
                sp(&ct),
                len.s,
                parts.join(", ")
            ));
        }
        let ct = self.ctype(&ty)?;
        let konst = if spec.konst && self.is_plain(&ty) {
            "const "
        } else {
            ""
        };
        let init = match &v.init {
            Init::None if self.is_plain(&ty) => None,
            Init::None => Some(self.zero_of(&ty)?),
            Init::Copy(e) => Some(self.conv_val(e, &ty)?.s),
            Init::Direct(a) => Some(self.construct(&ty, a, false)?.s),
            Init::Brace { elems, .. } => Some(self.construct(&ty, elems, true)?.s),
        };
        self.declare(&v.name, ty.clone(), Rk::Val);
        Ok(match init {
            Some(i) => format!("{konst}{ct}{}{name} = {i};", sp(&ct)),
            None => format!("{konst}{ct}{}{name};", sp(&ct)),
        })
    }

    fn ref_decl(&mut self, v: &Declarator, ty: &Ty) -> R<String> {
        let Init::Copy(e) = &v.init else {
            return no("C", "a reference without an initialiser");
        };
        let ct = self.ctype(ty)?;
        let x = self.ex(e, true)?;
        let p = self.addr(x, ty)?;
        self.declare(&v.name, ty.clone(), Rk::Ref);
        let sp = if ct.ends_with('*') { "" } else { " " };
        Ok(format!("{ct}{sp}*{} = {p};", self.name(&v.name)))
    }

    fn structured(
        &mut self,
        spec: &TypeSpec,
        names: &[String],
        b: &[(String, Ty)],
        init: &Expr,
    ) -> R<Vec<String>> {
        let from = self.ty_of(init);
        let tmp = self.fresh("sb");
        let n = self.name_of(&from)?;
        let mut lines = Vec::new();
        let by_ptr = spec.is_ref && !spec.konst;
        let x = self.ex(init, by_ptr)?;
        let fields: Vec<String> = match &from {
            Ty::Pair(..) => vec!["first".into(), "second".into()],
            Ty::Tuple(ts) => (0..ts.len()).map(|i| format!("f{i}")).collect(),
            t => return no("C", format!("a structured binding of {t}")),
        };
        let base = if by_ptr || (spec.is_ref && x.kind != Kind::Val) {
            let p = self.addr(x, &from)?;
            lines.push(format!("{n} *{tmp} = {p};"));
            format!("{tmp}->")
        } else {
            let v = self.own(x, &from)?;
            lines.push(format!("{n} {tmp} = {};", v.s));
            format!("{tmp}.")
        };
        for ((nm, (_, t)), f) in names.iter().zip(b).zip(&fields) {
            let ct = self.ctype(t)?;
            if by_ptr {
                lines.push(format!("{ct} *{} = &{base}{f};", self.name(nm)));
                self.declare(nm, t.clone(), Rk::Ref);
            } else {
                let sp = if ct.ends_with('*') { "" } else { " " };
                lines.push(format!("{ct}{sp}{} = {base}{f};", self.name(nm)));
                self.declare(nm, t.clone(), Rk::Val);
            }
        }
        Ok(lines)
    }

    fn found_decl(&mut self, v: &Declarator, container: &Ty) -> R<String> {
        let Init::Copy(e) = &v.init else {
            return no("C", "an iterator variable");
        };
        let ExprKind::Call { callee, args } = &e.kind else {
            return no("C", "an iterator variable");
        };
        let ExprKind::Member { obj, name, .. } = &callee.kind else {
            return no("C", "an iterator variable");
        };
        let k = match container {
            Ty::Map(k, _) | Ty::UnorderedMap(k, _) | Ty::Set(k) | Ty::UnorderedSet(k)
                if name == "find" && args.len() == 1 =>
            {
                (**k).clone()
            }
            _ => return no("C", "an iterator other than the result of find"),
        };
        let n = self.name_of(container)?;
        let o = self.ex(obj, true)?;
        let cont = self.peek(o.clone());
        let cont_text = cont.at(16);
        let p = self.addr(o, container)?;
        let key = self.conv_val(&args[0], &k)?;
        self.declare(&v.name, container.clone(), Rk::Found { cont: cont_text });
        Ok(format!(
            "size_t {} = {n}_find({p}, {});",
            self.name(&v.name),
            key.s
        ))
    }
}
