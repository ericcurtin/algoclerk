// SPDX-License-Identifier: Apache-2.0
//! Expressions: literals, names, casts, construction, member access. Operators are in `ops`, calls and
//! lambdas in `call`.

use super::Resolved;
use super::ctx::Checker;
use super::libcall::deref_val;
use super::val::*;
use crate::cpp::ast::*;
use crate::cpp::stdlib::{MEMBERS, members_of};

/// Type and value of the `<climits>` macros.
fn macro_const(name: &str) -> Option<(Ty, i128)> {
    Some(match name {
        "INT_MAX" => (Ty::Int, i128::from(i32::MAX)),
        "INT_MIN" => (Ty::Int, i128::from(i32::MIN)),
        "UINT_MAX" => (Ty::UInt, i128::from(u32::MAX)),
        "LLONG_MAX" => (Ty::LongLong, i128::from(i64::MAX)),
        "LLONG_MIN" => (Ty::LongLong, i128::from(i64::MIN)),
        "ULLONG_MAX" => (Ty::ULongLong, i128::from(u64::MAX)),
        "CHAR_MAX" => (Ty::Int, 127),
        "CHAR_MIN" => (Ty::Int, -128),
        _ => return None,
    })
}

impl<'a> Checker<'a> {
    /// Check an expression and record its type.
    pub fn expr(&mut self, e: &'a Expr) -> Val {
        self.depth += 1;
        let v = if self.depth > 150 {
            Val::unknown()
        } else {
            self.expr_inner(e)
        };
        self.depth -= 1;
        self.set(e.id, &v);
        v
    }

    fn expr_inner(&mut self, e: &'a Expr) -> Val {
        match &e.kind {
            ExprKind::Int {
                value,
                suffix,
                base,
            } => match literal_ty(*value, *suffix, *base) {
                Some(t) => Val::constant(t, i128::from(*value)),
                None => {
                    self.err(
                        e.id,
                        "type.literal",
                        "this number is too big for any integer type in the subset",
                    );
                    Val::unknown()
                }
            },
            ExprKind::Bool(b) => Val::constant(Ty::Bool, i128::from(*b)),
            ExprKind::Char(c) => Val::constant(Ty::Char, i128::from(*c)),
            ExprKind::Str(_) => Val::pr(cstr()).with_ic(true),
            ExprKind::NullPtr => Val::pr(Ty::NullPtr),
            ExprKind::Var(n) => self.var_expr(e, n),
            ExprKind::Path(segs) => self.path_expr(e, segs),
            ExprKind::Unary(op, a) => self.unary(e, *op, a),
            ExprKind::Binary(op, a, b) => self.binary(e, *op, a, b),
            ExprKind::Assign { op, lhs, rhs } => self.assign(e, *op, lhs, rhs),
            ExprKind::Ternary(c, a, b) => self.ternary(e, c, a, b),
            ExprKind::Index(a, i) => self.index(e, a, i),
            ExprKind::Call { callee, args } => self.call_expr(e, callee, args),
            ExprKind::Member { obj, name, arrow } => self.member_expr(e, obj, name, *arrow),
            ExprKind::Cast { ty, e: inner, .. } => self.cast_expr(e, ty, inner),
            ExprKind::Lambda(l) => self.lambda_expr(e, l),
            ExprKind::InitList(items) => {
                let vals: Vec<Val> = items.iter().map(|x| self.expr(x)).collect();
                Val::braced(vals)
            }
            ExprKind::Construct { ty, args, brace } => {
                let target = self.fix_ty(&ty.ty, e.id);
                let vals: Vec<Val> = args.iter().map(|x| self.expr(x)).collect();
                if self.construct(e.id, &target, &vals, *brace) {
                    self.note_ctor(e.id, &target, &vals);
                    Val::pr(target)
                } else {
                    Val::unknown()
                }
            }
            ExprKind::New { ty, args, brace } => {
                let target = self.fix_ty(&ty.ty, e.id);
                let vals: Vec<Val> = args.iter().map(|x| self.expr(x)).collect();
                if self.construct(e.id, &target, &vals, *brace) {
                    self.note_ctor(e.id, &target, &vals);
                    Val::pr(Ty::Ptr(Box::new(target)))
                } else {
                    Val::unknown()
                }
            }
            ExprKind::Delete(p) => {
                let v = self.expr(p);
                if !v.is_unknown() && !matches!(v.ty, Ty::Ptr(_)) {
                    self.err(
                        e.id,
                        "type.delete",
                        format!("`delete` needs a pointer, but this is a {}", show(&v)),
                    );
                }
                Val::pr(Ty::Void)
            }
        }
    }

    /// Remember which user constructor a construction picked.
    pub fn note_ctor(&mut self, id: NodeId, ty: &Ty, args: &[Val]) {
        let Ty::Struct(s) = ty else { return };
        let Some(d) = self.structs.get(s) else { return };
        let pick = d.ctors.iter().position(|c| {
            c.len() == args.len() && c.iter().zip(args).all(|(p, v)| self.arg_fits(v, p))
        });
        if let Some(i) = pick {
            self.info.refs.insert(id, Resolved::Ctor(i));
        }
    }

    fn var_expr(&mut self, e: &Expr, name: &str) -> Val {
        if let Some(s) = self.lookup(name, e.id) {
            if let Some(r) = s.id.clone() {
                self.info.refs.insert(e.id, r);
            }
            let mut v = Val::lv(s.ty, s.konst).with_ic(s.ic);
            if s.konst {
                v.cval = s.cval;
            }
            return v;
        }
        if let Some(fs) = self.funcs.get(name)
            && let [f] = fs.as_slice()
        {
            self.info.refs.insert(e.id, Resolved::Func(f.id));
            let args = f.params.iter().map(|p| p.ty.clone()).collect();
            return Val::pr(Ty::Function(args, Box::new(f.ret.ty.clone())));
        }
        if let Some((t, v)) = macro_const(name) {
            if let Some(m) = MEMBERS.iter().find(|m| m.recv == "free" && m.name == name) {
                self.need_header(e.id, m.header, name);
                self.info.refs.insert(e.id, Resolved::Lib(m));
            }
            return Val::constant(t, v);
        }
        if self.using_std && name == "nullopt" {
            return self.path_value(e, &["nullopt"]);
        }
        if self.fn_names.contains(name) {
            self.err(e.id, "name.function", format!("the function `{name}` is declared further down; put it above this line or add a forward declaration"));
        } else {
            self.err(e.id, "name.undeclared", format!("`{name}` has not been declared at this point; check the spelling or declare it above this line"));
        }
        Val::unknown()
    }

    fn path_expr(&mut self, e: &Expr, segs: &[PathSeg]) -> Val {
        let names: Vec<&str> = segs
            .iter()
            .map(|s| s.name.as_str())
            .skip(usize::from(segs.first().is_some_and(|s| s.name == "std")))
            .collect();
        self.path_value(e, &names)
    }

    fn path_value(&mut self, e: &Expr, names: &[&str]) -> Val {
        match names {
            ["string", "npos"] => {
                self.need_header(e.id, "<string>", "std::string::npos");
                Val::constant(Ty::Size, i128::from(u64::MAX))
            }
            ["string_view", "npos"] => {
                self.need_header(e.id, "<string_view>", "std::string_view::npos");
                Val::constant(Ty::Size, i128::from(u64::MAX))
            }
            ["nullopt"] => {
                self.need_header(e.id, "<optional>", "std::nullopt");
                Val::pr(Ty::Struct("nullopt_t".into()))
            }
            ["numeric_limits", ..] => {
                self.err(
                    e.id,
                    "type.call",
                    "`std::numeric_limits<T>::max` is a function: add `()` to call it",
                );
                Val::unknown()
            }
            _ => {
                let full = format!("std::{}", names.join("::"));
                let why = if MEMBERS.iter().any(|m| m.name == full) {
                    "that one is a function, call it with `(...)`"
                } else {
                    "it is not in the subset"
                };
                self.err(
                    e.id,
                    "unsupported.name",
                    format!("the supply cupboard does not stock `{full}` here: {why}"),
                );
                Val::unknown()
            }
        }
    }

    fn cast_expr(&mut self, e: &'a Expr, ty: &TypeSpec, inner: &'a Expr) -> Val {
        let target = if ty.ty == Ty::Void {
            Ty::Void
        } else {
            self.fix_ty(&ty.ty, e.id)
        };
        let v = self.expr(inner);
        if v.is_unknown() || has_unknown(&target) || target == Ty::Void {
            return Val::pr(target);
        }
        let scalar = |t: &Ty| t.is_integral();
        let ok = (scalar(&v.ty) && scalar(&target))
            || (v.ty == target)
            || (!scalar(&target)
                && !matches!(target, Ty::Ptr(_))
                && self.construct_quiet(&target, &v))
            || (matches!(target, Ty::Ptr(_))
                && matches!(v.ty, Ty::Ptr(_) | Ty::NullPtr)
                && v.ty == target);
        if !ok {
            self.err(
                e.id,
                "type.cast",
                format!(
                    "a {} cannot be converted to a {target} with a cast",
                    show(&v)
                ),
            );
            return Val::pr(target);
        }
        let mut out = Val::pr(target.clone());
        if let (Some(c), true) = (v.cval, target.is_integral()) {
            out.cval = Some(wrap(&target, c));
        }
        out
    }

    fn construct_quiet(&self, target: &Ty, v: &Val) -> bool {
        self.convertible(v, target)
            || matches!((target, &v.ty), (Ty::Str, Ty::Ptr(_) | Ty::StrView))
    }

    /// The object a member access applies to (after `.` or `->`).
    pub fn member_base(&mut self, e: &Expr, obj: &'a Expr, arrow: bool) -> Val {
        let ov = self.expr(obj);
        if ov.is_unknown() {
            return Val::unknown();
        }
        let is_ptr = matches!(ov.ty, Ty::Ptr(_));
        if arrow {
            return match &ov.ty {
                Ty::Ptr(_) | Ty::Iter { .. } => deref_val(&ov).unwrap_or_else(Val::unknown),
                Ty::Optional(t) => Val::lv((**t).clone(), ov.konst),
                _ => {
                    self.err(e.id, "type.arrow", format!("`->` needs a pointer or an iterator, but this is a {}; use `.` instead", show(&ov)));
                    Val::unknown()
                }
            };
        }
        if is_ptr {
            self.err(
                e.id,
                "type.arrow",
                "this is a pointer; use `->` to reach its members",
            );
            return Val::unknown();
        }
        ov
    }

    fn member_expr(&mut self, e: &Expr, obj: &'a Expr, name: &str, arrow: bool) -> Val {
        let b = self.member_base(e, obj, arrow);
        if b.is_unknown() {
            return Val::unknown();
        }
        match &b.ty {
            Ty::Struct(s) => {
                let found = self
                    .structs
                    .get(s)
                    .and_then(|d| d.fields.iter().find(|f| f.0 == name))
                    .cloned();
                match found {
                    Some((_, t, k)) => {
                        self.info
                            .refs
                            .insert(e.id, Resolved::Field(name.to_string()));
                        Val::lv(t, b.konst || k)
                    }
                    None => {
                        self.err(
                            e.id,
                            "name.member",
                            format!("`{s}` has no member called `{name}`"),
                        );
                        Val::unknown()
                    }
                }
            }
            Ty::Pair(x, y) if name == "first" || name == "second" => {
                let first = name == "first";
                let t = if first { (**x).clone() } else { (**y).clone() };
                self.info
                    .refs
                    .insert(e.id, Resolved::Field(name.to_string()));
                Val::lv(t, b.konst || (first && b.ic))
            }
            t => {
                let is_method = members_of(t).iter().any(|m| m.name == name);
                let msg = if is_method {
                    format!("`{name}` is a function of {t}: call it with `(...)`")
                } else {
                    format!("{t} has no member called `{name}`")
                };
                self.err(e.id, "name.member", msg);
                Val::unknown()
            }
        }
    }
}
