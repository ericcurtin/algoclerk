// SPDX-License-Identifier: Apache-2.0
//! Statements and variable declarations.

use super::Resolved;
use super::ctx::{Checker, ParamTy, RetKind, VarSym};
use super::val::*;
use crate::cpp::ast::*;

impl<'a> Checker<'a> {
    /// A block in its own scope.
    pub fn block(&mut self, b: &'a Block) {
        self.push_scope();
        self.block_in_scope(b);
        self.pop_scope();
    }

    /// The statements of a block in the current scope (function bodies share the parameter scope).
    pub fn block_in_scope(&mut self, b: &'a Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn scoped(&mut self, s: &'a Stmt) {
        self.push_scope();
        self.stmt(s);
        self.pop_scope();
    }

    /// A condition: must be usable as yes/no.
    fn cond(&mut self, c: &'a Expr, what: &str) {
        let v = self.expr(c);
        if !v.is_unknown() && !self.truthy(&v.ty) {
            self.err(
                c.id,
                "type.condition",
                format!(
                    "the condition of {what} must be a yes/no value, but this is a {}",
                    show(&v)
                ),
            );
        }
    }

    /// Check one statement.
    pub fn stmt(&mut self, s: &'a Stmt) {
        self.snapshot(s.id);
        match &s.kind {
            StmtKind::Decl { spec, vars } => {
                for d in vars {
                    self.decl_var(spec, d);
                }
            }
            StmtKind::Structured { spec, names, init } => {
                let v = self.expr(init);
                self.bind_names(s.id, spec, names, &v, init.id);
            }
            StmtKind::Expr(e) => {
                self.expr(e);
            }
            StmtKind::If {
                init,
                cond,
                then,
                els,
            } => {
                self.push_scope();
                if let Some(i) = init {
                    self.stmt(i);
                }
                self.cond(cond, "`if`");
                self.scoped(then);
                if let Some(e) = els {
                    self.scoped(e);
                }
                self.pop_scope();
            }
            StmtKind::Switch { cond, body } => {
                let v = self.expr(cond);
                if !v.is_unknown() && !v.ty.is_integral() {
                    self.err(
                        cond.id,
                        "type.switch",
                        format!(
                            "`switch` works on whole numbers and characters, but this is a {}",
                            show(&v)
                        ),
                    );
                }
                self.cases.push(Vec::new());
                self.block(body);
                self.cases.pop();
            }
            StmtKind::Case(Some(e)) => {
                let v = self.expr(e);
                match v.cval {
                    Some(c) => {
                        if self.cases.last().is_some_and(|l| l.contains(&c)) {
                            self.err(
                                e.id,
                                "type.case",
                                format!("two `case {c}` labels in one switch"),
                            );
                        }
                        if let Some(l) = self.cases.last_mut() {
                            l.push(c);
                        }
                    }
                    None if !v.is_unknown() => self.err(
                        e.id,
                        "type.case",
                        "a `case` label must be a constant whole number",
                    ),
                    None => {}
                }
            }
            StmtKind::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.push_scope();
                if let Some(i) = init {
                    self.stmt(i);
                }
                if let Some(c) = cond {
                    self.cond(c, "`for`");
                }
                if let Some(i) = incr {
                    self.expr(i);
                }
                self.scoped(body);
                self.pop_scope();
            }
            StmtKind::RangeFor {
                spec,
                names,
                structured,
                range,
                body,
            } => {
                self.push_scope();
                self.range_for(s.id, spec, names, *structured, range);
                self.scoped(body);
                self.pop_scope();
            }
            StmtKind::While { cond, body } | StmtKind::DoWhile { body, cond } => {
                self.cond(cond, "the loop");
                self.scoped(body);
            }
            StmtKind::Return(e) => self.return_stmt(s.id, e.as_ref()),
            StmtKind::Block(b) => self.block(b),
            StmtKind::Case(None) | StmtKind::Break | StmtKind::Continue | StmtKind::Empty => {}
        }
    }

    fn return_stmt(&mut self, id: NodeId, e: Option<&'a Expr>) {
        let kind = self.fctx.last().map(|f| f.ret.clone());
        let v = e.map(|x| self.expr(x));
        match (kind, v) {
            (Some(RetKind::Fixed(r)), None) => {
                if r.ty != Ty::Void && !has_unknown(&r.ty) {
                    self.err(
                        id,
                        "type.return",
                        format!(
                            "this function must return a {}, so `return;` needs a value",
                            r.ty
                        ),
                    );
                }
            }
            (Some(RetKind::Fixed(r)), Some(v)) => {
                let node = e.map_or(id, |x| x.id);
                if r.ty == Ty::Void {
                    if v.ty != Ty::Void && !v.is_unknown() {
                        self.err(
                            node,
                            "type.return",
                            "this function returns nothing (`void`), so it cannot return a value",
                        );
                    }
                } else {
                    self.bind_to(node, &v, &r, "returning");
                }
            }
            (Some(RetKind::Deduce), v) => {
                let t = match v {
                    None => Ty::Void,
                    Some(v) if v.ty == Ty::InitList => {
                        self.err(
                            id,
                            "type.return",
                            "a lambda cannot return a braced list `{...}` without `-> type`",
                        );
                        Ty::Unknown
                    }
                    Some(v) => v.ty,
                };
                if let Some(f) = self.fctx.last_mut() {
                    f.rets.push(t);
                }
            }
            (None, _) => {}
        }
    }

    /// Deduce the variable type for `auto` (`auto`, `auto&`, `const auto&`): type, const, pointee flag.
    pub fn deduce(
        &mut self,
        node: NodeId,
        spec: &TypeSpec,
        v: &Val,
        name: &str,
    ) -> Option<(Ty, bool, bool)> {
        if v.is_unknown() {
            return Some((Ty::Unknown, spec.konst, false));
        }
        if matches!(v.ty, Ty::Void | Ty::InitList) {
            let why = if v.ty == Ty::Void {
                "this call returns nothing (`void`)"
            } else {
                "a braced list has no type of its own; write the type: `std::vector<int> x{...}`"
            };
            self.err(
                node,
                "type.auto",
                format!("`auto {name}` cannot be deduced: {why}"),
            );
            return None;
        }
        if spec.is_ref {
            if !spec.konst && !v.lval {
                self.err(node, "type.temporary", format!("`auto& {name}` cannot refer to a temporary value; use `const auto&` or `auto`"));
                return None;
            }
            return Some((v.ty.clone(), spec.konst || v.konst, v.ic));
        }
        let keep = matches!(v.ty, Ty::Iter { .. } | Ty::Ptr(_));
        Some((v.ty.clone(), spec.konst, keep && v.ic))
    }

    /// One declarator of `T a = 1, b;`.
    pub fn decl_var(&mut self, spec: &TypeSpec, d: &'a Declarator) {
        let base = self.spec_param(spec, d.id);
        let mut ty = base.ty.clone();
        if let Some(dim) = &d.array {
            let dv = self.expr(dim);
            match dv.cval {
                Some(n) if n > 0 && n < 100_000_000 => ty = Ty::Array(Box::new(ty), n as usize),
                _ if dv.is_unknown() => ty = Ty::Unknown,
                _ => {
                    self.err(
                        dim.id,
                        "type.array",
                        "the size of an array must be a positive constant number",
                    );
                    ty = Ty::Unknown;
                }
            }
        }
        if ty == Ty::Void {
            self.err(
                d.id,
                "type.void",
                format!("`{}` cannot be a variable of type `void`", d.name),
            );
            ty = Ty::Unknown;
        }
        let auto = ty == Ty::Auto;
        let mut sym = VarSym {
            ty: ty.clone(),
            konst: base.konst,
            ic: false,
            id: Some(Resolved::Var(d.id)),
            cval: None,
        };
        let target = ParamTy {
            ty: ty.clone(),
            konst: base.konst,
            is_ref: base.is_ref,
        };
        let what = format!("initialising `{}`", d.name);
        if !auto {
            self.info.put(d.id, ty.clone(), true, base.konst);
            self.declare(d.id, &d.name, sym.clone());
        }
        match &d.init {
            Init::None => {
                if auto {
                    self.err(
                        d.id,
                        "type.auto",
                        format!("`auto {}` needs a value to copy its type from", d.name),
                    );
                } else if base.is_ref {
                    self.err(d.id, "type.reference", format!("`{}` is a reference, so it must be bound to something when it is declared", d.name));
                } else if base.konst && (ty.is_integral() || matches!(ty, Ty::Ptr(_))) {
                    self.err(
                        d.id,
                        "const.uninit",
                        format!(
                            "`{}` is const, so it must be given a value when it is declared",
                            d.name
                        ),
                    );
                } else {
                    self.construct(d.id, &ty, &[], false);
                }
            }
            Init::Copy(e) => {
                let v = self.expr(e);
                if auto {
                    if let Some((t, k, ic)) = self.deduce(d.id, spec, &v, &d.name) {
                        sym.ty = t;
                        sym.konst = k;
                        sym.ic = ic;
                    }
                } else {
                    self.bind_to(e.id, &v, &target, &what);
                    if base.konst && ty.is_integral() {
                        sym.cval = v.cval.map(|c| wrap(&ty, c));
                    }
                }
            }
            Init::Brace { elems, .. } | Init::Direct(elems) => {
                let brace = matches!(d.init, Init::Brace { .. });
                let vals: Vec<Val> = elems.iter().map(|x| self.expr(x)).collect();
                if auto {
                    match vals.as_slice() {
                        [v] => {
                            if let Some((t, k, ic)) = self.deduce(d.id, spec, v, &d.name) {
                                sym.ty = t;
                                sym.konst = k;
                                sym.ic = ic;
                            }
                        }
                        _ => self.err(
                            d.id,
                            "type.auto",
                            format!(
                                "`auto {}` needs exactly one value to copy its type from",
                                d.name
                            ),
                        ),
                    }
                } else if base.is_ref {
                    if let [v] = vals.as_slice() {
                        self.bind_to(elems[0].id, v, &target, &what);
                    }
                } else if self.construct(d.id, &ty, &vals, brace) {
                    if let (true, [v]) = (base.konst && ty.is_integral(), vals.as_slice()) {
                        sym.cval = v.cval.map(|c| wrap(&ty, c));
                    }
                    if let Ty::Struct(_) = ty {
                        self.note_ctor(d.id, &ty, &vals);
                    }
                }
            }
        }
        if auto && sym.ty == Ty::Auto {
            sym.ty = Ty::Unknown;
        }
        self.info.put(d.id, sym.ty.clone(), true, sym.konst);
        if auto {
            self.declare(d.id, &d.name, sym);
        } else {
            self.update_last(&d.name, sym);
        }
    }

    /// Replace the most recent declaration of `name` in the innermost scope (constant value known now).
    fn update_last(&mut self, name: &str, sym: VarSym) {
        if let Some(slot) = self
            .scopes
            .last_mut()
            .and_then(|s| s.iter_mut().rev().find(|(n, _)| n == name))
            && let (Some(Resolved::Var(a)), Some(Resolved::Var(b))) = (&slot.1.id, &sym.id)
            && a == b
        {
            slot.1 = sym;
        }
    }
}
