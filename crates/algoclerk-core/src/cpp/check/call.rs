// SPDX-License-Identifier: Apache-2.0
//! Calls: user functions, library functions and methods, lambdas and `std::function` values.

use super::Resolved;
use super::ctx::{Checker, FnSym};
use super::ops::describe;
use super::val::*;
use crate::cpp::ast::*;

/// The value a call to a function with this return type produces.
fn ret_val(ret: &super::ctx::ParamTy) -> Val {
    if ret.is_ref {
        Val::lv(ret.ty.clone(), ret.konst)
    } else {
        Val::pr(ret.ty.clone())
    }
}

impl<'a> Checker<'a> {
    fn arg_vals(&mut self, args: &'a [Expr]) -> Vec<Val> {
        args.iter().map(|a| self.expr(a)).collect()
    }

    /// `callee(args)`.
    pub fn call_expr(&mut self, e: &'a Expr, callee: &'a Expr, args: &'a [Expr]) -> Val {
        match &callee.kind {
            ExprKind::Var(name) => self.call_named(e, callee, name, args),
            ExprKind::Path(segs) => self.call_path(e, callee, segs, args),
            ExprKind::Member { obj, name, arrow } => {
                self.call_method(e, callee, obj, name, *arrow, args)
            }
            _ => {
                let f = self.expr(callee);
                let argv = self.arg_vals(args);
                self.invoke(e.id, &f, &argv)
            }
        }
    }

    fn call_method(
        &mut self,
        e: &'a Expr,
        callee: &'a Expr,
        obj: &'a Expr,
        name: &str,
        arrow: bool,
        args: &'a [Expr],
    ) -> Val {
        let base = self.member_base(callee, obj, arrow);
        let argv = self.arg_vals(args);
        if base.is_unknown() || argv.iter().any(Val::is_unknown) {
            return Val::unknown();
        }
        if let Ty::Struct(s) = &base.ty {
            let field = self
                .structs
                .get(s)
                .and_then(|d| d.fields.iter().find(|f| f.0 == name))
                .cloned();
            return match field {
                Some((_, t, _)) => {
                    self.info
                        .refs
                        .insert(callee.id, Resolved::Field(name.to_string()));
                    self.invoke(e.id, &Val::lv(t, base.konst), &argv)
                }
                None => {
                    self.err(callee.id, "name.member", format!("`{s}` has no function called `{name}`: structs in the subset hold data only"));
                    Val::unknown()
                }
            };
        }
        let who = describe(obj);
        let v = self.lib_method(e.id, &base, name, &argv, &who);
        if let Some(r) = self.info.refs.get(&e.id).cloned() {
            self.info.refs.insert(callee.id, r);
        }
        v
    }

    fn call_named(&mut self, e: &'a Expr, callee: &'a Expr, name: &str, args: &'a [Expr]) -> Val {
        if self.scopes.iter().skip(1).flatten().any(|(n, _)| n == name)
            || self.scopes[0].iter().any(|(n, _)| n == name)
        {
            let f = self.expr(callee);
            let argv = self.arg_vals(args);
            if matches!(f.ty, Ty::Lambda(_) | Ty::Function(..) | Ty::Functor(..)) || f.is_unknown()
            {
                self.info.refs.insert(e.id, Resolved::Callable);
            }
            return self.invoke(e.id, &f, &argv);
        }
        if self.funcs.contains_key(name) {
            let argv = self.arg_vals(args);
            return self.call_user(e, callee, name, args, &argv);
        }
        let argv = self.arg_vals(args);
        if argv.iter().any(Val::is_unknown) {
            return Val::unknown();
        }
        if self.using_std
            && let Some(v) = self.lib_free(e.id, &format!("std::{name}"), &argv, &[])
        {
            return v;
        }
        if self.fn_names.contains(name) {
            self.err(e.id, "name.function", format!("`{name}` is declared further down in the file; C++ reads top to bottom, so move it above this call or add a forward declaration `...;`"));
        } else {
            self.err(e.id, "name.function", format!("there is no function called `{name}`; check the spelling, or declare it above this line"));
        }
        Val::unknown()
    }

    fn call_user(
        &mut self,
        e: &'a Expr,
        callee: &'a Expr,
        name: &str,
        args: &'a [Expr],
        argv: &[Val],
    ) -> Val {
        let cands: Vec<FnSym> = self.funcs.get(name).cloned().unwrap_or_default();
        if argv.iter().any(Val::is_unknown) {
            return cands.first().map_or_else(Val::unknown, |f| ret_val(&f.ret));
        }
        let arity: Vec<&FnSym> = cands
            .iter()
            .filter(|f| f.params.len() == argv.len())
            .collect();
        let mut best: Option<(&FnSym, usize)> = None;
        for f in &arity {
            if f.params.iter().zip(argv).all(|(p, v)| self.arg_fits(v, p)) {
                let exact = f
                    .params
                    .iter()
                    .zip(argv)
                    .filter(|(p, v)| p.ty == v.ty)
                    .count();
                if best.is_none_or(|(_, b)| exact > b) {
                    best = Some((f, exact));
                }
            }
        }
        if let Some((f, _)) = best {
            self.info.refs.insert(e.id, Resolved::Func(f.id));
            self.info.refs.insert(callee.id, Resolved::Func(f.id));
            return ret_val(&f.ret);
        }
        let Some(f) = arity.first().copied().or(cands.first()) else {
            return Val::unknown();
        };
        if f.params.len() != argv.len() {
            self.err(
                e.id,
                "type.arity",
                format!(
                    "`{name}` takes {} argument(s), but you gave it {}",
                    f.params.len(),
                    argv.len()
                ),
            );
        } else {
            for (i, (p, v)) in f.params.iter().zip(argv).enumerate() {
                let what = format!("argument {} of `{name}`", i + 1);
                if !self.bind_to(args[i].id, v, p, &what) {
                    break;
                }
            }
        }
        ret_val(&f.ret)
    }

    fn call_path(
        &mut self,
        e: &'a Expr,
        callee: &'a Expr,
        segs: &[PathSeg],
        args: &'a [Expr],
    ) -> Val {
        let skip = usize::from(segs.first().is_some_and(|s| s.name == "std"));
        let rest = &segs[skip.min(segs.len())..];
        let argv = self.arg_vals(args);
        let names: Vec<&str> = rest.iter().map(|s| s.name.as_str()).collect();
        if argv.iter().any(Val::is_unknown) {
            return Val::unknown();
        }
        match names.as_slice() {
            ["get"] => return self.call_get(e, rest[0].targs.as_deref(), &argv),
            ["numeric_limits", f] => {
                return self.call_limits(e, rest[0].targs.as_deref(), f, argv.len());
            }
            _ => {}
        }
        let key = format!("std::{}", names.join("::"));
        let explicit: Vec<Ty> = rest
            .iter()
            .filter_map(|s| s.targs.as_ref())
            .flatten()
            .filter_map(|t| match t {
                TArg::Ty(t) => Some(t.clone()),
                TArg::Int(_) => None,
            })
            .collect();
        match self.lib_free(e.id, &key, &argv, &explicit) {
            Some(v) => {
                if let Some(r) = self.info.refs.get(&e.id).cloned() {
                    self.info.refs.insert(callee.id, r);
                }
                v
            }
            None => {
                self.err(
                    e.id,
                    "unsupported.name",
                    format!("the supply cupboard does not stock `{key}`"),
                );
                Val::unknown()
            }
        }
    }

    fn call_get(&mut self, e: &Expr, targs: Option<&[TArg]>, argv: &[Val]) -> Val {
        let (Some([TArg::Int(i)]), [a]) = (targs, argv) else {
            self.err(
                e.id,
                "type.arity",
                "`std::get<N>(x)` takes a position like `<0>` and one pair or tuple",
            );
            return Val::unknown();
        };
        let i = usize::try_from(*i).unwrap_or(usize::MAX);
        let header = if matches!(a.ty, Ty::Tuple(_)) {
            "<tuple>"
        } else {
            "<utility>"
        };
        self.need_header(e.id, header, "std::get");
        let elem = match &a.ty {
            Ty::Pair(x, y) => [&**x, &**y].get(i).map(|t| (*t).clone()),
            Ty::Tuple(ts) => ts.get(i).cloned(),
            Ty::Array(t, n) if i < *n => Some((**t).clone()),
            _ => {
                self.err(
                    e.id,
                    "type.mismatch",
                    format!("`std::get` wants a pair or a tuple, not a {}", show(a)),
                );
                return Val::unknown();
            }
        };
        match elem {
            Some(t) => Val::lv(t, a.konst),
            None => {
                self.err(
                    e.id,
                    "type.get",
                    format!("position {i} does not exist in a {}", a.ty),
                );
                Val::unknown()
            }
        }
    }

    fn call_limits(&mut self, e: &Expr, targs: Option<&[TArg]>, f: &str, nargs: usize) -> Val {
        self.need_header(e.id, "<limits>", "std::numeric_limits");
        let Some([TArg::Ty(t)]) = targs else {
            self.err(
                e.id,
                "type.arity",
                "`std::numeric_limits` needs a type like `<int>`",
            );
            return Val::unknown();
        };
        if nargs != 0 || !matches!(f, "max" | "min" | "lowest") || !t.is_integral() {
            self.err(e.id, "unsupported.name", format!("the supply cupboard stocks only `std::numeric_limits<integer>::max()`, `min()` and `lowest()`, not `{f}`"));
            return Val::unknown();
        }
        let (lo, hi) = int_range(t);
        Val::constant(t.clone(), if f == "max" { hi } else { lo })
    }

    /// Call a lambda, `std::function` or functor value with typed arguments.
    pub fn invoke(&mut self, node: NodeId, f: &Val, args: &[Val]) -> Val {
        if f.is_unknown() || args.iter().any(Val::is_unknown) {
            return match &f.ty {
                Ty::Function(_, r) => Val::pr((**r).clone()),
                _ => Val::unknown(),
            };
        }
        match &f.ty {
            Ty::Lambda(id) => self.call_lambda(node, *id, args),
            Ty::Function(ps, r) => {
                if ps.len() != args.len() {
                    self.err(
                        node,
                        "type.arity",
                        format!(
                            "this function takes {} argument(s), but you gave it {}",
                            ps.len(),
                            args.len()
                        ),
                    );
                } else if let Some((i, (p, a))) = ps
                    .iter()
                    .zip(args)
                    .enumerate()
                    .find(|(_, (p, a))| !self.convertible(a, p))
                {
                    self.err(
                        node,
                        "type.mismatch",
                        format!(
                            "argument {}: a {} cannot be turned into a {p}",
                            i + 1,
                            show(a)
                        ),
                    );
                }
                Val::pr((**r).clone())
            }
            Ty::Functor(_, t) => {
                if args.len() != 2 || args.iter().any(|a| !self.convertible(a, t)) {
                    self.err(
                        node,
                        "type.mismatch",
                        format!("this comparison takes two {t} values"),
                    );
                }
                Val::pr(Ty::Bool)
            }
            t => {
                self.err(
                    node,
                    "type.call",
                    format!("a {t} is not something you can call like a function"),
                );
                Val::unknown()
            }
        }
    }
}
