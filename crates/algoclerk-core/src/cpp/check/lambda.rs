// SPDX-License-Identifier: Apache-2.0
//! Lambdas: capture rules, return type deduction, generic (`auto` parameter) instantiation per call.

use super::Resolved;
use super::ctx::{Checker, FnCtx, LamCap, LamInfo, ParamTy, RetKind, VarSym};
use super::val::*;
use crate::cpp::ast::*;

impl<'a> Checker<'a> {
    /// A lambda expression: its type is the unnamed closure type.
    pub fn lambda_expr(&mut self, e: &'a Expr, l: &'a Lambda) -> Val {
        let ptys: Vec<ParamTy> = l
            .params
            .iter()
            .map(|p| self.spec_param(&p.spec, p.id))
            .collect();
        let generic = ptys.iter().any(|p| p.ty == Ty::Auto);
        for (n, _) in &l.captures.items {
            if self.scopes.iter().flatten().all(|(m, _)| m != n) {
                self.err(
                    e.id,
                    "name.undeclared",
                    format!("the lambda captures `{n}`, which has not been declared at this point"),
                );
            }
        }
        let declared = l.ret.as_ref().map(|r| self.spec_param(r, e.id).ty);
        let env = generic.then(|| (self.scopes.clone(), self.fctx.clone()));
        self.lams.insert(
            e.id,
            LamInfo {
                ast: l,
                params: ptys.clone(),
                ret: declared,
                generic,
                env,
            },
        );
        if generic {
            let probe: Vec<ParamTy> = ptys
                .iter()
                .map(|p| {
                    if p.ty == Ty::Auto {
                        ParamTy {
                            ty: Ty::Unknown,
                            ..p.clone()
                        }
                    } else {
                        p.clone()
                    }
                })
                .collect();
            self.run_lambda_body(e.id, l, &probe);
        } else {
            let ret = self.run_lambda_body(e.id, l, &ptys);
            if let Some(i) = self.lams.get_mut(&e.id) {
                i.ret = Some(ret);
            }
        }
        Val::pr(Ty::Lambda(e.id))
    }

    /// Check the body with these parameter types; the lambda's return type.
    fn run_lambda_body(&mut self, id: NodeId, l: &'a Lambda, ptys: &[ParamTy]) -> Ty {
        let declared = l.ret.as_ref().map(|r| self.spec_param(r, id));
        self.fctx.push(FnCtx {
            base: self.scopes.len(),
            ret: declared.clone().map_or(RetKind::Deduce, RetKind::Fixed),
            lambda: Some(LamCap {
                default: l.captures.default,
                items: l.captures.items.clone(),
                mutable: l.mutable,
            }),
            rets: Vec::new(),
        });
        self.push_scope();
        for (p, pt) in l.params.iter().zip(ptys) {
            self.info.put(p.id, pt.ty.clone(), true, pt.konst);
            let sym = VarSym {
                ty: pt.ty.clone(),
                konst: pt.konst,
                ic: false,
                id: Some(Resolved::Var(p.id)),
                cval: None,
            };
            self.declare(p.id, &p.name, sym);
        }
        self.block_in_scope(&l.body);
        self.pop_scope();
        let ctx = self.fctx.pop();
        if let Some(d) = declared {
            return d.ty;
        }
        let all: Vec<Ty> = ctx.map(|c| c.rets).unwrap_or_default();
        let rets: Vec<Ty> = all.iter().filter(|t| !has_unknown(t)).cloned().collect();
        match rets.split_first() {
            None if !all.is_empty() => Ty::Unknown,
            None => Ty::Void,
            Some((first, rest)) => {
                if let Some(other) = rest.iter().find(|t| *t != first) {
                    self.err(id, "type.return", format!("the returns of this lambda give different types ({first} and {other}); add `-> type` after the parameters to say which one you mean"));
                }
                first.clone()
            }
        }
    }

    /// Call the lambda `id` with typed arguments.
    pub fn call_lambda(&mut self, node: NodeId, id: NodeId, args: &[Val]) -> Val {
        let Some(info) = self.lams.get(&id) else {
            return Val::unknown();
        };
        let (ast, params, ret, generic) = (
            info.ast,
            info.params.clone(),
            info.ret.clone(),
            info.generic,
        );
        if params.len() != args.len() {
            self.err(
                node,
                "type.arity",
                format!(
                    "this lambda takes {} argument(s), but you gave it {}",
                    params.len(),
                    args.len()
                ),
            );
            return Val::pr(ret.unwrap_or(Ty::Unknown));
        }
        let mut ptys = Vec::new();
        for (i, (p, a)) in params.iter().zip(args).enumerate() {
            if p.ty == Ty::Auto {
                if a.ty == Ty::InitList {
                    self.err(
                        node,
                        "type.deduce",
                        "a generic lambda cannot take a braced list `{...}`",
                    );
                    return Val::unknown();
                }
                ptys.push(ParamTy {
                    ty: a.ty.clone(),
                    konst: p.konst || (p.is_ref && a.konst),
                    is_ref: p.is_ref,
                });
            } else {
                let what = format!("argument {} of the lambda", i + 1);
                self.bind_to(node, a, p, &what);
                ptys.push(p.clone());
            }
        }
        if !generic {
            return Val::pr(ret.unwrap_or(Ty::Unknown));
        }
        let env = self.lams.get(&id).and_then(|i| i.env.clone());
        let saved = env.map(|(sc, fc)| {
            (
                std::mem::replace(&mut self.scopes, sc),
                std::mem::replace(&mut self.fctx, fc),
            )
        });
        let r = self.run_lambda_body(id, ast, &ptys);
        if let Some((sc, fc)) = saved {
            self.scopes = sc;
            self.fctx = fc;
        }
        Val::pr(r)
    }
}
