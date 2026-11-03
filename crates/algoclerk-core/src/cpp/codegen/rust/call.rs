// SPDX-License-Identifier: Apache-2.0
//! Rust translation of calls: user functions, lambdas, library members and library functions (the latter
//! two live in `lib_methods` and `lib_free`).

use super::Rs;
use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;

impl Rs<'_> {
    pub(super) fn call(&mut self, e: &Expr, callee: &Expr, args: &[Expr], m: bool) -> R<Ex> {
        match &callee.kind {
            ExprKind::Member { obj, name, .. } => self.method(e, obj, name, args, m),
            ExprKind::Path(_) | ExprKind::Var(_) => match self.info.resolved(callee.id) {
                Some(Resolved::Lib(mem)) => self.free_call(e, callee, mem.name, args, m),
                Some(Resolved::Func(id)) => self.user_call(callee, *id, args),
                Some(Resolved::Callable) => self.closure_call(callee, args),
                _ => match &callee.kind {
                    ExprKind::Var(n) if self.lookup(n).is_some() => self.closure_call(callee, args),
                    _ => no("Rust", "this call"),
                },
            },
            _ => no("Rust", "calling the result of an expression"),
        }
    }

    /// One argument for a parameter declared with `spec`.
    pub(super) fn arg_for(&mut self, spec: &TypeSpec, ty: &Ty, a: &Expr) -> R<String> {
        if spec.is_ref && spec.konst {
            if matches!(a.kind, ExprKind::InitList(_)) {
                let v = self.conv_val(a, ty)?;
                return Ok(format!("&{}", v.at(17)));
            }
            let x = self.ex(a, false)?;
            return Ok(match x.kind {
                Kind::Ref => x.s,
                _ if x.slice => x.s,
                _ => format!("&{}", x.at(17)),
            });
        }
        if spec.is_ref {
            let x = self.ex(a, true)?;
            return match x.kind {
                Kind::Ref => Ok(x.s),
                Kind::Place => Ok(format!("&mut {}", x.at(17))),
                Kind::Val => no("Rust", "passing a temporary by non-const reference"),
            };
        }
        Ok(self.conv_val(a, ty)?.s)
    }

    fn user_call(&mut self, callee: &Expr, id: NodeId, args: &[Expr]) -> R<Ex> {
        let Some(f) = self.prog.fns().find(|f| f.id == id) else {
            return no("Rust", "this call");
        };
        if f.params.len() != args.len() {
            return no("Rust", "a call with default arguments");
        }
        let mut parts = Vec::new();
        for (p, a) in f.params.iter().zip(args) {
            parts.push(self.arg_for(&p.spec, &p.spec.ty.clone(), a)?);
        }
        let name = match &callee.kind {
            ExprKind::Var(n) => self.name(n),
            _ => self.name(&f.name),
        };
        // `f(&mut v, v.len())`: the other arguments are read first, the borrow checker insists
        let mut_roots: Vec<&str> = parts
            .iter()
            .zip(args)
            .filter(|(p, _)| p.starts_with("&mut "))
            .filter_map(|(_, a)| root(a))
            .collect();
        let mut lets = String::new();
        if !mut_roots.is_empty() {
            for (i, (p, a)) in parts.iter_mut().zip(args).enumerate() {
                let (names, _) = names_and_calls(a);
                if !p.starts_with("&mut ") && names.iter().any(|n| mut_roots.contains(&n.as_str()))
                {
                    let v = format!("arg{i}");
                    lets.push_str(&format!("let {v} = {p}; "));
                    *p = v;
                }
            }
        }
        if !lets.is_empty() {
            return Ok(Ex::new(
                format!("{{ {lets}{name}({}) }}", parts.join(", ")),
                20,
                Kind::Val,
            ));
        }
        Ok(Ex::new(
            format!("{name}({})", parts.join(", ")),
            19,
            Kind::Val,
        ))
    }

    fn closure_call(&mut self, callee: &Expr, args: &[Expr]) -> R<Ex> {
        let ExprKind::Var(n) = &callee.kind else {
            return no("Rust", "calling a lambda expression directly");
        };
        let Some(v) = self.lookup(n).cloned() else {
            return no("Rust", "this call");
        };
        let Ty::Lambda(id) = v.ty else {
            return no("Rust", "calling a std::function");
        };
        let Some(l) = self.lams.get(&id).copied() else {
            return no("Rust", "this lambda call");
        };
        if l.params.len() != args.len() {
            return no("Rust", "this lambda call");
        }
        let mut parts = Vec::new();
        for (p, a) in l.params.iter().zip(args) {
            let mut spec = p.spec.clone();
            if spec.ty == Ty::Auto {
                spec.ty = self.info.decl_ty(p.id).clone();
            }
            let t = spec.ty.clone();
            parts.push(self.arg_for(&spec, &t, a)?);
        }
        Ok(Ex::new(
            format!("{}({})", self.name(n), parts.join(", ")),
            19,
            Kind::Val,
        ))
    }
}
