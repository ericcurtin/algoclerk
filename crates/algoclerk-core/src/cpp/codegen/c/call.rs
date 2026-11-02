// SPDX-License-Identifier: Apache-2.0
//! C translation of calls: user functions, lifted lambdas, library members and library functions.

use super::{Cx, Rk};
use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;

impl Cx<'_> {
    pub(super) fn call(&mut self, e: &Expr, callee: &Expr, args: &[Expr], m: bool) -> R<Ex> {
        match &callee.kind {
            ExprKind::Member { obj, name, .. } => self.method(e, obj, name, args, m),
            ExprKind::Path(_) | ExprKind::Var(_) => match self.info.resolved(callee.id) {
                Some(Resolved::Lib(mem)) => self.free_call(e, callee, mem.name, args, m),
                Some(Resolved::Func(id)) => self.user_call(callee, *id, args),
                Some(Resolved::Callable) => self.lambda_call(callee, args),
                _ => match &callee.kind {
                    ExprKind::Var(n) if self.lookup(n).is_some() => self.lambda_call(callee, args),
                    _ => no("C", "this call"),
                },
            },
            _ => no("C", "calling the result of an expression"),
        }
    }

    /// One argument for a parameter declared with `spec`.
    pub(super) fn arg_for(&mut self, spec: &TypeSpec, ty: &Ty, a: &Expr) -> R<String> {
        let by_ptr = spec.is_ref && !(spec.konst && self.is_plain(ty));
        if by_ptr {
            if matches!(a.kind, ExprKind::InitList(_)) {
                let v = self.conv_val(a, ty)?;
                return self.addr(v, ty);
            }
            let x = self.ex(a, !spec.konst)?;
            return self.addr(x, ty);
        }
        Ok(self.conv_val(a, ty)?.s)
    }

    fn user_call(&mut self, callee: &Expr, id: NodeId, args: &[Expr]) -> R<Ex> {
        let Some(f) = self.prog.fns().find(|f| f.id == id) else {
            return no("C", "this call");
        };
        if f.params.len() != args.len() {
            return no("C", "a call with default arguments");
        }
        let mut parts = Vec::new();
        for (p, a) in f.params.iter().zip(args) {
            parts.push(self.arg_for(&p.spec, &p.spec.ty.clone(), a)?);
        }
        let name = match &callee.kind {
            ExprKind::Var(n) => self.name(n),
            _ => self.name(&f.name),
        };
        Ok(Ex::new(
            format!("{name}({})", parts.join(", ")),
            16,
            Kind::Val,
        ))
    }

    /// A call through a lambda variable: the lifted function takes pointers.
    fn lambda_call(&mut self, callee: &Expr, args: &[Expr]) -> R<Ex> {
        let ExprKind::Var(n) = &callee.kind else {
            return no("C", "calling a lambda expression directly");
        };
        let Some(Rk::Lambda(f)) = self.lookup(n).map(|v| v.rk.clone()) else {
            return no("C", "calling a std::function or a capturing lambda");
        };
        let mut parts = Vec::new();
        for a in args {
            let t = self.ty_of(a);
            let x = self.ex(a, false)?;
            parts.push(match x.kind {
                Kind::Ref => x.s,
                Kind::Place => format!("&{}", x.at(15)),
                Kind::Val => format!("(({}[]){{ {} }})", self.ctype(&t)?, x.s),
            });
        }
        Ok(Ex::new(format!("{f}({})", parts.join(", ")), 16, Kind::Val))
    }
}
