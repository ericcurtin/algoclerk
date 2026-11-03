// SPDX-License-Identifier: Apache-2.0
//! Rust translation of expressions: names, operators, conversions, member access and indexing.

use super::{Rk, Rs};
use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;

/// Does the expression text end in an `as <primitive>` cast (a following `<` would start generics)?
fn ends_with_cast(s: &str) -> bool {
    s.rsplit_once(" as ").is_some_and(|(_, ty)| {
        !ty.is_empty() && ty.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    })
}

/// An integer literal without its type suffix.
fn bare_lit(mut x: Ex) -> Ex {
    if let Some(v) = x.lit {
        let t = v.to_string();
        if x.s.len() > t.len() && x.s.starts_with(&t) && x.s[t.len()..].starts_with(['u', 'i']) {
            x.s = t;
        }
    }
    x
}

fn is_strty(t: &Ty) -> bool {
    matches!(t, Ty::Str | Ty::StrView)
}

pub(super) fn str_lit(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            '\0' => o.push_str("\\0"),
            c if (' '..='~').contains(&c) => o.push(c),
            c => o.push_str(&format!("\\x{:02x}", c as u32 & 0x7f)),
        }
    }
    o.push('"');
    o
}

fn char_lit(c: u8) -> String {
    match c {
        b'\'' => "b'\\''".into(),
        b'\\' => "b'\\\\'".into(),
        b'\n' => "b'\\n'".into(),
        b'\t' => "b'\\t'".into(),
        b'\r' => "b'\\r'".into(),
        0 => "b'\\0'".into(),
        0x20..=0x7e => format!("b'{}'", c as char),
        c => format!("{}u8", c),
    }
}

impl Rs<'_> {
    pub(super) fn ty_of(&self, e: &Expr) -> Ty {
        self.info.ty(e.id).clone()
    }

    /// The expression as an owned value of type `to` (the type the context needs).
    pub(super) fn conv_val(&mut self, e: &Expr, to: &Ty) -> R<Ex> {
        match &e.kind {
            ExprKind::InitList(items) => return self.init_list(items, to),
            ExprKind::NullPtr if matches!(to, Ty::Ptr(_)) => {
                return Ok(Ex::val(self.default_val(to)?));
            }
            ExprKind::Str(s) if *to == Ty::Str => {
                return Ok(Ex::new(
                    format!("{}.to_string()", str_lit(s)),
                    19,
                    Kind::Val,
                ));
            }
            _ => {}
        }
        if self.is_nullopt(e) && matches!(to, Ty::Optional(_)) {
            return Ok(Ex::val("None"));
        }
        let from = self.ty_of(e);
        let x = self.ex(e, false)?;
        self.convert(x, &from, to)
    }

    pub(super) fn is_nullopt(&self, e: &Expr) -> bool {
        matches!(self.info.resolved(e.id), Some(Resolved::Lib(m)) if m.name == "std::nullopt")
    }

    /// A string-typed expression as `&str`.
    pub(super) fn str_ref(&mut self, e: &Expr) -> R<Ex> {
        let x = self.ex(e, false)?;
        if x.slice {
            return Ok(x);
        }
        Ok(Ex::new(format!("{}.as_str()", x.at(19)), 19, Kind::Val))
    }

    /// The expression as a Rust `bool` (integers compare with zero, pointers with null).
    pub(super) fn as_bool(&mut self, e: &Expr) -> R<Ex> {
        if let Some(r) = self.ctype_bool(e) {
            return r;
        }
        if let Some(r) = self.count_bool(e) {
            return r;
        }
        let t = self.ty_of(e);
        match &t {
            Ty::Ptr(_) => {
                let x = self.ex(e, false)?;
                let n = self.default_val(&t)?;
                Ok(Ex::new(format!("{} != {n}", x.at(8)), 7, Kind::Val))
            }
            Ty::Optional(_) => {
                let x = self.ex(e, false)?;
                Ok(Ex::new(format!("{}.is_some()", x.at(19)), 19, Kind::Val))
            }
            Ty::Bool => {
                let x = self.ex(e, false)?;
                Ok(self.own(x, &t))
            }
            t if t.is_integral() => {
                let x = self.ex(e, false)?;
                self.conv_int(x, t, &Ty::Bool)
            }
            t => no("Rust", format!("a condition of type {t}")),
        }
    }

    pub(super) fn lib_const(&self, name: &str) -> Option<&'static str> {
        Some(match name {
            "INT_MAX" => "i32::MAX",
            "INT_MIN" => "i32::MIN",
            "UINT_MAX" => "u32::MAX",
            "LLONG_MAX" => "i64::MAX",
            "LLONG_MIN" => "i64::MIN",
            "ULLONG_MAX" => "u64::MAX",
            "CHAR_MAX" => "127",
            "CHAR_MIN" => "-128",
            "npos" | "std::string::npos" => "usize::MAX",
            "std::nullopt" | "nullopt" => "None",
            _ => return None,
        })
    }

    pub(super) fn ex(&mut self, e: &Expr, m: bool) -> R<Ex> {
        match &e.kind {
            ExprKind::Int { value, .. } => {
                let t = self.ty_of(e);
                Ok(self.int_lit(*value as i128, &t))
            }
            ExprKind::Bool(b) => Ok(Ex::val(if *b { "true" } else { "false" })),
            ExprKind::Char(c) => Ok(Ex::val(char_lit(*c))),
            ExprKind::Str(s) => {
                let mut x = Ex::val(str_lit(s));
                x.slice = true;
                Ok(x)
            }
            ExprKind::NullPtr => no("Rust", "nullptr outside a pointer context"),
            ExprKind::Var(n) => self.var_ex(e, n),
            ExprKind::Path(segs) => {
                let name = match self.info.resolved(e.id) {
                    Some(Resolved::Lib(m)) => m.name.to_string(),
                    _ => segs.last().map(|s| s.name.clone()).unwrap_or_default(),
                };
                match self.lib_const(&name) {
                    Some(c) => Ok(Ex::val(c)),
                    None => no("Rust", format!("the name {name}")),
                }
            }
            ExprKind::Unary(op, a) => self.unary(e, *op, a, m),
            ExprKind::Binary(op, a, b) => self.binary(e, *op, a, b),
            ExprKind::Assign { op, lhs, rhs } => {
                let st = self.assign_stmt(lhs, *op, rhs)?;
                let v = self.ex(lhs, false)?;
                let t = self.ty_of(lhs);
                let v = self.own(v, &t);
                Ok(Ex::new(format!("{{ {st}; {} }}", v.s), 20, Kind::Val))
            }
            ExprKind::Ternary(c, a, b) => {
                let t = self.ty_of(e);
                let c = self.as_bool(c)?;
                let x = self.conv_val(a, &t)?;
                let y = self.conv_val(b, &t)?;
                Ok(Ex::new(
                    format!("if {} {{ {} }} else {{ {} }}", c.s, x.s, y.s),
                    2,
                    Kind::Val,
                ))
            }
            ExprKind::Index(a, i) => self.index_ex(a, i, m),
            ExprKind::Call { callee, args } => self.call(e, callee, args, m),
            ExprKind::Member { obj, name, arrow } => self.member(e, obj, name, *arrow, m),
            ExprKind::Cast { ty, e: a, .. } => {
                let from = self.ty_of(a);
                let x = self.ex(a, false)?;
                if from.is_integral() && ty.ty.is_integral() {
                    self.conv_int(x, &from, &ty.ty)
                } else if from == ty.ty {
                    Ok(self.own(x, &from))
                } else {
                    no("Rust", format!("a cast from {from} to {}", ty.ty))
                }
            }
            ExprKind::Lambda(l) => self.lambda_ex(e, l),
            ExprKind::InitList(_) => no("Rust", "a braced list without a known type"),
            ExprKind::Construct { ty, args, brace } => self.construct(&ty.ty, args, *brace),
            ExprKind::New { ty, args, brace } => match &ty.ty {
                Ty::Struct(_) => self.construct(&ty.ty, args, *brace),
                t => no("Rust", format!("new {t}")),
            },
            ExprKind::Delete(_) => no("Rust", "delete inside an expression"),
        }
    }

    fn var_ex(&mut self, e: &Expr, n: &str) -> R<Ex> {
        if let Some(v) = self.lookup(n).cloned() {
            let name = self.name(n);
            return Ok(match v.rk {
                Rk::Val | Rk::Entry => Ex::new(name, 20, Kind::Place),
                Rk::Ref | Rk::RefMut => Ex::new(name, 20, Kind::Ref),
                Rk::StrRef => {
                    let mut x = Ex::new(name, 20, Kind::Ref);
                    x.slice = true;
                    x
                }
                Rk::Found(_) => Ex::new(name, 20, Kind::Val),
            });
        }
        let lib = match self.info.resolved(e.id) {
            Some(Resolved::Lib(m)) => Some(m.name),
            _ => None,
        };
        match self.lib_const(lib.unwrap_or(n)) {
            Some(c) => Ok(Ex::val(c)),
            None => no("Rust", format!("the name `{n}` used as a value")),
        }
    }

    fn unary(&mut self, e: &Expr, op: UnOp, a: &Expr, m: bool) -> R<Ex> {
        let t = self.ty_of(e);
        let at = self.ty_of(a);
        match op {
            UnOp::Not => {
                let x = self.as_bool(a)?;
                Ok(Ex::new(format!("!{}", x.at(17)), 17, Kind::Val))
            }
            UnOp::Neg | UnOp::BitNot | UnOp::Plus => {
                let x = self.ex(a, false)?;
                let x = self.conv_int(x, &at, &t)?;
                if let (UnOp::Neg, Some(v)) = (op, x.lit) {
                    return Ok(self.int_lit(-v, &t));
                }
                Ok(match op {
                    UnOp::Plus => x,
                    UnOp::Neg if t.is_unsigned() => {
                        Ex::new(format!("{}.wrapping_neg()", x.at(19)), 19, Kind::Val)
                    }
                    UnOp::Neg => Ex::new(format!("-{}", x.at(17)), 17, Kind::Val),
                    _ => Ex::new(format!("!{}", x.at(17)), 17, Kind::Val),
                })
            }
            UnOp::Deref => self.deref_ex(a, &at, m),
            UnOp::AddrOf => {
                if matches!(&at, Ty::Struct(_)) {
                    self.ex(a, false).map(|x| self.own(x, &at))
                } else {
                    no("Rust", "taking an address")
                }
            }
            UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => {
                let delta = if matches!(op, UnOp::PreInc | UnOp::PostInc) {
                    BinOp::Add
                } else {
                    BinOp::Sub
                };
                let st = self.step_stmt(a, delta)?;
                let v = self.ex(a, false)?;
                let v = self.own(v, &at);
                if matches!(op, UnOp::PreInc | UnOp::PreDec) {
                    Ok(Ex::new(format!("{{ {st}; {} }}", v.s), 20, Kind::Val))
                } else {
                    let tmp = self.fresh("old");
                    Ok(Ex::new(
                        format!("{{ let {tmp} = {}; {st}; {tmp} }}", v.s),
                        20,
                        Kind::Val,
                    ))
                }
            }
        }
    }

    pub(super) fn binary(&mut self, e: &Expr, op: BinOp, a: &Expr, b: &Expr) -> R<Ex> {
        let (at, bt, rt) = (self.ty_of(a), self.ty_of(b), self.ty_of(e));
        match op {
            BinOp::And | BinOp::Or => {
                let p = if op == BinOp::And { 6 } else { 5 };
                let x = self.as_bool(a)?;
                let y = self.as_bool(b)?;
                Ok(Ex::new(
                    format!("{} {} {}", x.at(p), op.text(), y.at(p + 1)),
                    p,
                    Kind::Val,
                ))
            }
            BinOp::Comma => no("Rust", "the comma operator inside an expression"),
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.compare(op, a, b, &at, &bt)
            }
            BinOp::Add if is_strty(&rt) => self.concat(e),
            _ => {
                let x = self.ex(a, false)?;
                let y = self.ex(b, false)?;
                self.arith(op, x, &at, y, &bt)
            }
        }
    }

    /// Arithmetic on two translated operands with C++ conversions; unsigned `+ - *` wrap.
    pub(super) fn arith(&mut self, op: BinOp, x: Ex, at: &Ty, y: Ex, bt: &Ty) -> R<Ex> {
        if !at.is_integral() || !bt.is_integral() {
            return no("Rust", format!("arithmetic on {at} and {bt}"));
        }
        if matches!(op, BinOp::Shl | BinOp::Shr) {
            let p = promote(at);
            let x = self.conv_int(x, at, &p)?;
            let y = self.own(y, bt);
            // `a as i32 << 2` would read `<<` as the start of generic arguments
            let xs = if op == BinOp::Shl && (x.cast || ends_with_cast(&x.s)) {
                format!("({})", x.s)
            } else {
                x.at(11)
            };
            return Ok(Ex::new(
                format!("{xs} {} {}", op.text(), y.at(12)),
                11,
                Kind::Val,
            ));
        }
        let c = common_ty(at, bt);
        let x = self.conv_int(x, at, &c)?;
        let y = self.conv_int(y, bt, &c)?;
        let method = match op {
            BinOp::Add => Some("wrapping_add"),
            BinOp::Sub => Some("wrapping_sub"),
            BinOp::Mul => Some("wrapping_mul"),
            _ => None,
        };
        if let (Some(mth), true) = (method, c.is_unsigned()) {
            return Ok(Ex::new(
                format!("{}.{mth}({})", x.at(19), y.s),
                19,
                Kind::Val,
            ));
        }
        let p = match op {
            BinOp::Mul | BinOp::Div | BinOp::Rem => 13,
            BinOp::Add | BinOp::Sub => 12,
            BinOp::BitAnd => 10,
            BinOp::BitXor => 9,
            _ => 8,
        };
        Ok(Ex::new(
            format!("{} {} {}", x.at(p), op.text(), y.at(p + 1)),
            p,
            Kind::Val,
        ))
    }

    fn compare(&mut self, op: BinOp, a: &Expr, b: &Expr, at: &Ty, bt: &Ty) -> R<Ex> {
        if let Some(r) = self.iter_compare(op, a, b) {
            return r;
        }
        if let Some(r) = self.special_compare(op, a, b) {
            return r;
        }
        let lit = |e: &Expr| matches!(e.kind, ExprKind::Str(_));
        let (x, y) = if (is_strty(at) && is_strty(bt))
            || (is_strty(at) && lit(b))
            || (lit(a) && is_strty(bt))
        {
            (self.str_ref(a)?, self.str_ref(b)?)
        } else if matches!(at, Ty::Ptr(_) | Ty::NullPtr) || matches!(bt, Ty::Ptr(_) | Ty::NullPtr) {
            let pt = if matches!(at, Ty::Ptr(_)) { at } else { bt };
            let x = self.conv_val(a, pt)?;
            let y = self.conv_val(b, pt)?;
            (x, y)
        } else if at.is_integral() && bt.is_integral() {
            let c = if at == bt {
                at.clone()
            } else {
                common_ty(at, bt)
            };
            let x = self.ex(a, false)?;
            let y = self.ex(b, false)?;
            (self.conv_int(x, at, &c)?, self.conv_int(y, bt, &c)?)
        } else {
            let r = |x: Ex| {
                if x.kind == Kind::Ref {
                    Ex::new(format!("*{}", x.at(17)), 17, Kind::Place)
                } else {
                    x
                }
            };
            let x = self.ex(a, false)?;
            let y = self.ex(b, false)?;
            (r(x), r(y))
        };
        // the other operand fixes the type of a literal: `n == 0`, not `n == 0usize`
        let (x, y) = match (x.lit, y.lit) {
            (None, Some(_)) => (x, bare_lit(y)),
            (Some(_), None) => (bare_lit(x), y),
            _ => (x, y),
        };
        let xs = if (x.cast || ends_with_cast(&x.s)) && matches!(op, BinOp::Lt | BinOp::Shl) {
            format!("({})", x.s)
        } else {
            x.at(8)
        };
        Ok(Ex::new(
            format!("{xs} {} {}", op.text(), y.at(8)),
            7,
            Kind::Val,
        ))
    }

    /// `a + b + c` on strings as one `format!`.
    fn concat(&mut self, e: &Expr) -> R<Ex> {
        let mut parts: Vec<&Expr> = Vec::new();
        fn flat<'e>(rs: &Rs<'_>, e: &'e Expr, out: &mut Vec<&'e Expr>) {
            if let ExprKind::Binary(BinOp::Add, l, r) = &e.kind
                && is_strty(rs.info.ty(e.id))
            {
                flat(rs, l, out);
                out.push(r);
                return;
            }
            out.push(e);
        }
        flat(self, e, &mut parts);
        let mut fmt = String::new();
        let mut args = Vec::new();
        for p in parts {
            match &p.kind {
                ExprKind::Str(s) => fmt.push_str(
                    &str_lit(s)[1..str_lit(s).len() - 1]
                        .replace('{', "{{")
                        .replace('}', "}}"),
                ),
                _ => {
                    fmt.push_str("{}");
                    let t = self.ty_of(p);
                    let x = self.ex(p, false)?;
                    if t == Ty::Char {
                        args.push(format!("{} as char", x.at(16)));
                    } else {
                        args.push(x.s);
                    }
                }
            }
        }
        Ok(Ex::val(format!("format!(\"{fmt}\", {})", args.join(", "))))
    }

    fn index_ex(&mut self, a: &Expr, i: &Expr, m: bool) -> R<Ex> {
        let at = self.ty_of(a);
        match &at {
            Ty::Vector(_) | Ty::Array(..) | Ty::Deque(_) | Ty::List(_) => {
                let base = self.ex(a, m)?;
                let it = self.ty_of(i);
                let idx = self.index_text(i, &it)?;
                Ok(Ex::new(format!("{}[{idx}]", base.at(19)), 19, Kind::Place))
            }
            Ty::Str | Ty::StrView => {
                if m {
                    return no("Rust", "assigning to a character of a string");
                }
                let base = self.ex(a, false)?;
                let it = self.ty_of(i);
                let idx = self.index_text(i, &it)?;
                Ok(Ex::new(
                    format!("{}.as_bytes()[{idx}]", base.at(19)),
                    19,
                    Kind::Val,
                ))
            }
            Ty::Map(k, _) | Ty::UnorderedMap(k, _) => {
                let base = self.ex(a, true)?;
                let key = self.conv_val(i, k)?;
                Ok(Ex::new(
                    format!("{}.entry({}).or_default()", base.at(19), key.s),
                    19,
                    Kind::Ref,
                ))
            }
            t => no("Rust", format!("indexing {t}")),
        }
    }

    /// An index expression as a `usize`.
    pub(super) fn index_text(&mut self, i: &Expr, it: &Ty) -> R<String> {
        let x = self.ex(i, false)?;
        if let Some(v) = x.lit {
            return Ok(v.to_string());
        }
        Ok(self.conv_int(x, it, &Ty::Size)?.s)
    }

    fn member(&mut self, e: &Expr, obj: &Expr, name: &str, arrow: bool, m: bool) -> R<Ex> {
        let _ = (e, arrow);
        if name == "second"
            && let Some(r) = self.insert_second(obj)
        {
            return r;
        }
        let ot = self.ty_of(obj);
        let idx = |n: &str| match n {
            "first" => Some("0"),
            "second" => Some("1"),
            _ => None,
        };
        match &ot {
            Ty::Pair(..) => {
                let Some(i) = idx(name) else {
                    return no("Rust", format!("the member {name}"));
                };
                let o = self.ex(obj, m)?;
                let k = if matches!(&obj.kind, ExprKind::Var(n) if self.lookup(n).is_some_and(|v| v.rk == Rk::Entry))
                {
                    Kind::Ref
                } else {
                    Kind::Place
                };
                Ok(Ex::new(format!("{}.{i}", o.at(19)), 19, k))
            }
            Ty::Struct(_) => {
                let o = self.ex(obj, false)?;
                Ok(Ex::new(format!("{}.{}()", o.at(19), name), 19, Kind::Val))
            }
            Ty::Ptr(inner) if matches!(&**inner, Ty::Struct(_)) => {
                let o = self.ex(obj, false)?;
                Ok(Ex::new(format!("{}.{}()", o.at(19), name), 19, Kind::Val))
            }
            Ty::Iter { .. }
            | Ty::Map(..)
            | Ty::Set(_)
            | Ty::UnorderedMap(..)
            | Ty::UnorderedSet(_) => self.found_member(obj, name, m),
            Ty::Optional(_) => {
                let o = self.ex(obj, m)?;
                let Some(i) = idx(name) else {
                    return no("Rust", format!("the member {name}"));
                };
                let call = if m { "as_mut" } else { "as_ref" };
                Ok(Ex::new(
                    format!("{}.{call}().unwrap().{i}", o.at(19)),
                    19,
                    Kind::Place,
                ))
            }
            t => no("Rust", format!("the member {name} of {t}")),
        }
    }

    /// `it->first` / `it->second` on the `Option` an iterator from `find` became.
    fn found_member(&mut self, obj: &Expr, name: &str, m: bool) -> R<Ex> {
        let ExprKind::Var(v) = &obj.kind else {
            return no("Rust", "an iterator expression other than a variable");
        };
        let Some(Rk::Found(mutable)) = self.lookup(v).map(|x| x.rk) else {
            return no("Rust", "an iterator other than the result of find");
        };
        let it = self.name(v);
        match (mutable, name) {
            (false, "first") => Ok(Ex::new(format!("{it}.unwrap().0"), 19, Kind::Ref)),
            (false, "second") => Ok(Ex::new(format!("{it}.unwrap().1"), 19, Kind::Ref)),
            (true, "second") if m => Ok(Ex::new(
                format!("{it}.as_deref_mut().unwrap()"),
                19,
                Kind::Ref,
            )),
            (true, "second") => Ok(Ex::new(format!("{it}.as_deref().unwrap()"), 19, Kind::Ref)),
            _ => no("Rust", "this use of an iterator"),
        }
    }

    fn deref_ex(&mut self, a: &Expr, at: &Ty, m: bool) -> R<Ex> {
        match at {
            Ty::Optional(_) => {
                let o = self.ex(a, m)?;
                let call = if m { "as_mut" } else { "as_ref" };
                Ok(Ex::new(
                    format!("{}.{call}().unwrap()", o.at(19)),
                    19,
                    Kind::Ref,
                ))
            }
            Ty::Iter { .. } => self.deref_iter(a, m),
            t => no("Rust", format!("dereferencing {t}")),
        }
    }
}
