// SPDX-License-Identifier: Apache-2.0
//! C translation of expressions. C and C++ share the arithmetic rules, so operators are kept as written;
//! what changes is containers (function calls on the prelude types), strings, references (pointers) and the
//! explicit copies C++ makes implicitly.

use super::{Cx, Rk};
use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;

pub(super) fn c_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.bytes() {
        match c {
            b'"' => o.push_str("\\\""),
            b'\\' => o.push_str("\\\\"),
            b'\n' => o.push_str("\\n"),
            b'\t' => o.push_str("\\t"),
            b'\r' => o.push_str("\\r"),
            b'?' => o.push_str("\\?"),
            0x20..=0x7e => o.push(c as char),
            c => o.push_str(&format!("\\{c:03o}")),
        }
    }
    o.push('"');
    o
}

fn c_char(c: u8) -> String {
    match c {
        b'\'' => "'\\''".into(),
        b'\\' => "'\\\\'".into(),
        b'\n' => "'\\n'".into(),
        b'\t' => "'\\t'".into(),
        b'\r' => "'\\r'".into(),
        0 => "'\\0'".into(),
        b'?' => "'\\?'".into(),
        0x20..=0x7e => format!("'{}'", c as char),
        c => format!("'\\{c:03o}'"),
    }
}

impl Cx<'_> {
    pub(super) fn ty_of(&self, e: &Expr) -> Ty {
        self.info.ty(e.id).clone()
    }

    /// The value of an expression as owned: scalars are read, aggregates copied unless they are temporaries.
    pub(super) fn own(&mut self, x: Ex, ty: &Ty) -> R<Ex> {
        if matches!(ty, Ty::Struct(_)) && x.kind != Kind::Val {
            return no(
                "C",
                "copying a struct value (nodes are shared through pointers)",
            );
        }
        if self.is_plain(ty) {
            return Ok(match x.kind {
                Kind::Ref => Ex::new(format!("*{}", x.at(15)), 15, Kind::Val),
                _ => Ex {
                    kind: Kind::Val,
                    ..x
                },
            });
        }
        let n = self.name_of(ty)?;
        Ok(match x.kind {
            Kind::Val => x,
            Kind::Place => Ex::val(format!("{n}_copy({})", x.s)),
            Kind::Ref => Ex::val(format!("{n}_copy(*{})", x.at(15))),
        })
    }

    /// The value for reading only (no copy of aggregates).
    pub(super) fn peek(&self, x: Ex) -> Ex {
        match x.kind {
            Kind::Ref => Ex::new(format!("*{}", x.at(15)), 15, Kind::Place),
            _ => x,
        }
    }

    /// `x.field` or `x->field` depending on whether `x` is a pointer.
    pub(super) fn dot(&self, x: &Ex, field: &str) -> String {
        let op = if x.kind == Kind::Ref { "->" } else { "." };
        format!("{}{op}{field}", x.at(16))
    }

    /// A pointer to the aggregate `x` of type `ty`.
    pub(super) fn addr(&mut self, x: Ex, ty: &Ty) -> R<String> {
        if x.slice {
            return Ok(format!("(Str[]){{ Str_from({}) }}", x.s));
        }
        Ok(match x.kind {
            Kind::Ref => x.s,
            Kind::Place => format!("&{}", x.at(15)),
            Kind::Val => format!("({}[]){{ {} }}", self.name_of(ty)?, x.s),
        })
    }

    pub(super) fn conv_val(&mut self, e: &Expr, to: &Ty) -> R<Ex> {
        match &e.kind {
            ExprKind::InitList(items) => return self.init_list(items, to),
            ExprKind::NullPtr => return Ok(Ex::val("NULL")),
            ExprKind::Str(s) if matches!(to, Ty::Str | Ty::StrView) => {
                return Ok(Ex::new(format!("Str_from({})", c_str(s)), 16, Kind::Val));
            }
            _ => {}
        }
        if self.is_nullopt(e) {
            return Ok(Ex::val(format!("({}){{false}}", self.name_of(to)?)));
        }
        let from = self.ty_of(e);
        let x = self.ex(e, false)?;
        if let Ty::Optional(inner) = to
            && !matches!(from, Ty::Optional(_))
        {
            let v = self.own(x, &from)?;
            let _ = inner;
            return Ok(Ex::val(format!("({}){{true, {}}}", self.name_of(to)?, v.s)));
        }
        self.own(x, &from)
    }

    pub(super) fn is_nullopt(&self, e: &Expr) -> bool {
        matches!(self.info.resolved(e.id), Some(Resolved::Lib(m)) if m.name == "std::nullopt")
    }

    /// The condition text of `if` / `while` / `?:`.
    pub(super) fn cond(&mut self, e: &Expr) -> R<Ex> {
        let t = self.ty_of(e);
        let x = self.ex(e, false)?;
        match t {
            Ty::Optional(_) => Ok(Ex::new(self.dot(&x, "has"), 16, Kind::Val)),
            _ => self.own(x, &t),
        }
    }

    pub(super) fn ex(&mut self, e: &Expr, m: bool) -> R<Ex> {
        match &e.kind {
            ExprKind::Int { value, .. } => {
                let t = self.ty_of(e);
                let suffix = match t {
                    Ty::UInt => "u",
                    Ty::LongLong => "LL",
                    Ty::ULongLong | Ty::Size => "ULL",
                    _ => "",
                };
                let mut x = Ex::val(format!("{value}{suffix}"));
                x.lit = Some(*value as i128);
                Ok(x)
            }
            ExprKind::Bool(b) => Ok(Ex::val(if *b { "true" } else { "false" })),
            ExprKind::Char(c) => Ok(Ex::val(c_char(*c))),
            ExprKind::Str(s) => {
                let mut x = Ex::val(c_str(s));
                x.slice = true;
                Ok(x)
            }
            ExprKind::NullPtr => Ok(Ex::val("NULL")),
            ExprKind::Var(n) => self.var_ex(e, n),
            ExprKind::Path(segs) => {
                let name = match self.info.resolved(e.id) {
                    Some(Resolved::Lib(m)) => m.name.to_string(),
                    _ => segs.last().map(|s| s.name.clone()).unwrap_or_default(),
                };
                match name.as_str() {
                    "npos" => Ok(Ex::val("NOT_FOUND")),
                    "INT_MAX" | "INT_MIN" | "UINT_MAX" | "LLONG_MAX" | "LLONG_MIN"
                    | "ULLONG_MAX" | "CHAR_MAX" | "CHAR_MIN" => Ok(Ex::val(name)),
                    _ => no("C", format!("the name {name}")),
                }
            }
            ExprKind::Unary(op, a) => self.unary(e, *op, a, m),
            ExprKind::Binary(op, a, b) => self.binary(e, *op, a, b),
            ExprKind::Assign { op, lhs, rhs } => {
                let s = self.assign_stmt(lhs, *op, rhs)?;
                Ok(Ex::new(s, 2, Kind::Val))
            }
            ExprKind::Ternary(c, a, b) => {
                let t = self.ty_of(e);
                let c = self.cond(c)?;
                let x = self.conv_val(a, &t)?;
                let y = self.conv_val(b, &t)?;
                Ok(Ex::new(
                    format!("{} ? {} : {}", c.at(4), x.at(3), y.at(3)),
                    3,
                    Kind::Val,
                ))
            }
            ExprKind::Index(a, i) => self.index_ex(a, i, m),
            ExprKind::Call { callee, args } => self.call(e, callee, args, m),
            ExprKind::Member { obj, name, .. } => self.member(obj, name, m),
            ExprKind::Cast { ty, e: a, .. } => {
                let from = self.ty_of(a);
                let x = self.ex(a, false)?;
                if ty.ty.is_integral() && from.is_integral() {
                    let x = self.own(x, &from)?;
                    let t = self.ctype(&ty.ty)?;
                    Ok(Ex::new(format!("({t}){}", x.at(15)), 14, Kind::Val))
                } else if from == ty.ty {
                    self.own(x, &from)
                } else {
                    no("C", format!("a cast from {from} to {}", ty.ty))
                }
            }
            ExprKind::Lambda(_) => no("C", "a lambda used as a value (C has none)"),
            ExprKind::InitList(_) => no("C", "a braced list without a known type"),
            ExprKind::Construct { ty, args, brace } => self.construct(&ty.ty, args, *brace),
            ExprKind::New { ty, args, brace } => match &ty.ty {
                Ty::Struct(n) => {
                    self.use_struct(n)?;
                    let x = self.construct(&ty.ty, args, *brace)?;
                    Ok(Ex::val(x.s.replacen("_make(", "_new(", 1)))
                }
                t => no("C", format!("new {t}")),
            },
            ExprKind::Delete(_) => no("C", "delete inside an expression"),
        }
    }

    fn var_ex(&mut self, e: &Expr, n: &str) -> R<Ex> {
        if let Some(v) = self.lookup(n).cloned() {
            let name = self.name(n);
            return Ok(match v.rk {
                Rk::Val | Rk::Native => Ex::new(name, 20, Kind::Place),
                Rk::Ref => Ex::new(name, 20, Kind::Ref),
                Rk::Found { .. } => Ex::new(name, 20, Kind::Val),
                Rk::Lambda(f) => Ex::new(f, 20, Kind::Val),
            });
        }
        if let Some(Resolved::Lib(m)) = self.info.resolved(e.id) {
            return match m.name {
                "INT_MAX" | "INT_MIN" | "UINT_MAX" | "LLONG_MAX" | "LLONG_MIN" | "ULLONG_MAX"
                | "CHAR_MAX" | "CHAR_MIN" => Ok(Ex::val(m.name)),
                "npos" => Ok(Ex::val("NOT_FOUND")),
                other => no("C", format!("the name {other}")),
            };
        }
        no("C", format!("the name `{n}` used as a value"))
    }

    fn unary(&mut self, e: &Expr, op: UnOp, a: &Expr, m: bool) -> R<Ex> {
        let at = self.ty_of(a);
        let _ = e;
        match op {
            UnOp::Not => {
                let x = self.cond(a)?;
                if let Some(base) = x.s.strip_suffix(".len == 0") {
                    return Ok(Ex::new(format!("{base}.len != 0"), 9, Kind::Val));
                }
                Ok(Ex::new(format!("!{}", x.at(15)), 15, Kind::Val))
            }
            UnOp::Neg | UnOp::BitNot | UnOp::Plus => {
                let x = self.ex(a, false)?;
                let x = self.own(x, &at)?;
                let c = match op {
                    UnOp::Neg => "-",
                    UnOp::Plus => "+",
                    _ => "~",
                };
                let inner = if x.s.starts_with(c) {
                    format!("({})", x.s)
                } else {
                    x.at(15)
                };
                Ok(Ex::new(format!("{c}{inner}"), 15, Kind::Val))
            }
            UnOp::Deref => match &at {
                Ty::Optional(_) => {
                    let o = self.ex(a, m)?;
                    Ok(Ex::new(self.dot(&o, "v"), 16, Kind::Place))
                }
                Ty::Iter { .. } => self.deref_iter(a, m),
                t => no("C", format!("dereferencing {t}")),
            },
            UnOp::AddrOf => {
                if matches!(at, Ty::Struct(_)) {
                    let x = self.ex(a, false)?;
                    Ok(Ex::new(format!("&{}", x.at(15)), 15, Kind::Val))
                } else {
                    no("C", "taking an address")
                }
            }
            UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => {
                let p = self.place_text(a)?;
                let (txt, prec) = match op {
                    UnOp::PreInc => (format!("++{p}"), 15),
                    UnOp::PreDec => (format!("--{p}"), 15),
                    UnOp::PostInc => (format!("{p}++"), 16),
                    _ => (format!("{p}--"), 16),
                };
                Ok(Ex::new(txt, prec, Kind::Val))
            }
        }
    }

    pub(super) fn binary(&mut self, e: &Expr, op: BinOp, a: &Expr, b: &Expr) -> R<Ex> {
        let (at, bt, rt) = (self.ty_of(a), self.ty_of(b), self.ty_of(e));
        if op == BinOp::Comma {
            let (x, y) = (self.ex(a, true)?, self.ex(b, true)?);
            return Ok(Ex::val(format!("({}, {})", x.at(3), y.at(3))));
        }
        if matches!(
            op,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        ) {
            return self.compare(op, a, b, &at, &bt);
        }
        if op == BinOp::Add && matches!(rt, Ty::Str | Ty::StrView) {
            let x = self.str_val(a)?;
            let y = self.str_val(b)?;
            return Ok(Ex::new(format!("Str_add({}, {})", x.s, y.s), 16, Kind::Val));
        }
        let (x, y) = if matches!(op, BinOp::And | BinOp::Or) {
            (self.cond(a)?, self.cond(b)?)
        } else {
            let x = self.ex(a, false)?;
            let y = self.ex(b, false)?;
            (self.own(x, &at)?, self.own(y, &bt)?)
        };
        let p = op.prec();
        // keep the parentheses compilers like to see around mixed bitwise / logical operators
        let bitwise = matches!(
            op,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr
        );
        let (lp, rp) = if bitwise {
            (14, 14)
        } else if op == BinOp::Or {
            (6, 6)
        } else {
            (p, p + 1)
        };
        Ok(Ex::new(
            format!("{} {} {}", x.at(lp), op.text(), y.at(rp)),
            p,
            Kind::Val,
        ))
    }

    /// A string-typed operand as a `Str` value (a character becomes a one-character string).
    pub(super) fn str_val(&mut self, e: &Expr) -> R<Ex> {
        let t = self.ty_of(e);
        if t == Ty::Char {
            let x = self.ex(e, false)?;
            return Ok(Ex::val(format!("Str_n(&(char){{ {} }}, 1)", x.s)));
        }
        let x = self.ex(e, false)?;
        if x.slice {
            return Ok(Ex::val(format!("Str_from({})", x.s)));
        }
        Ok(self.peek(x))
    }

    fn compare(&mut self, op: BinOp, a: &Expr, b: &Expr, at: &Ty, bt: &Ty) -> R<Ex> {
        if let Some(r) = self.iter_compare(op, a, b) {
            return r;
        }
        let stry = |e: &Expr, t: &Ty| {
            matches!(t, Ty::Str | Ty::StrView) || matches!(e.kind, ExprKind::Str(_))
        };
        let strs = stry(a, at) && stry(b, bt);
        if strs {
            let (x, y) = (self.ex(a, false)?, self.ex(b, false)?);
            let (x, y) = (self.peek(x), self.peek(y));
            let lit = |v: &Ex| v.slice;
            let text = match (lit(&x), lit(&y)) {
                (false, true) => format!("Str_cmp_c({}, {}) {} 0", x.s, y.s, op.text()),
                (true, false) => format!("Str_cmp_c({}, {}) {} 0", y.s, x.s, flip(op).text()),
                (false, false) => format!("Str_cmp({}, {}) {} 0", x.s, y.s, op.text()),
                (true, true) => return no("C", "comparing two string literals"),
            };
            let text = if matches!(op, BinOp::Eq | BinOp::Ne) && !(lit(&x) && lit(&y)) {
                let (s, c) = if lit(&y) { (&x, &y) } else { (&y, &x) };
                if lit(&x) || lit(&y) {
                    let ne = if op == BinOp::Ne { "!" } else { "" };
                    format!("{ne}Str_eq_c({}, {})", s.s, c.s)
                } else {
                    let ne = if op == BinOp::Ne { "!" } else { "" };
                    format!("{ne}Str_eq({}, {})", x.s, y.s)
                }
            } else {
                text
            };
            let p = if text.starts_with('!') || text.starts_with("Str_eq") {
                15
            } else {
                10
            };
            return Ok(Ex::new(text, p, Kind::Val));
        }
        if self.is_plain(at) && self.is_plain(bt) {
            let (x, y) = (self.ex(a, false)?, self.ex(b, false)?);
            let (x, y) = (self.own(x, at)?, self.own(y, bt)?);
            let p = op.prec();
            return Ok(Ex::new(
                format!("{} {} {}", x.at(p), op.text(), y.at(p + 1)),
                p,
                Kind::Val,
            ));
        }
        let ty = if matches!(at, Ty::Unknown) { bt } else { at };
        let n = self.name_of(ty)?;
        let (x, y) = (self.ex(a, false)?, self.ex(b, false)?);
        let (x, y) = (self.peek(x).s, self.peek(y).s);
        let text = match op {
            BinOp::Eq => format!("{n}_eq({x}, {y})"),
            BinOp::Ne => format!("!{n}_eq({x}, {y})"),
            BinOp::Lt => format!("{n}_lt({x}, {y})"),
            BinOp::Gt => format!("{n}_lt({y}, {x})"),
            BinOp::Le => format!("!{n}_lt({y}, {x})"),
            _ => format!("!{n}_lt({x}, {y})"),
        };
        Ok(Ex::new(
            text,
            if matches!(op, BinOp::Eq | BinOp::Lt | BinOp::Gt) {
                16
            } else {
                15
            },
            Kind::Val,
        ))
    }

    fn index_ex(&mut self, a: &Expr, i: &Expr, m: bool) -> R<Ex> {
        let at = self.ty_of(a);
        let it = self.ty_of(i);
        match &at {
            Ty::Vector(_) | Ty::Array(..) | Ty::Stack(_) | Ty::Str | Ty::StrView => {
                let native = matches!(&a.kind, ExprKind::Var(n) if self.lookup(n).is_some_and(|v| v.rk == Rk::Native));
                let base = self.ex(a, m)?;
                let idx = self.ex(i, false)?;
                let idx = self.own(idx, &it)?;
                let b = if native {
                    base.at(16)
                } else {
                    self.dot(&base, "data")
                };
                Ok(Ex::new(format!("{b}[{}]", idx.s), 16, Kind::Place))
            }
            Ty::Deque(_) | Ty::Queue(_) | Ty::List(_) => {
                let base = self.ex(a, m)?;
                let n = self.name_of(&at)?;
                let p = self.addr(base, &at)?;
                let idx = self.ex(i, false)?;
                let idx = self.own(idx, &it)?;
                Ok(Ex::new(format!("{n}_at({p}, {})", idx.s), 16, Kind::Ref))
            }
            Ty::Map(k, _) | Ty::UnorderedMap(k, _) => {
                let base = self.ex(a, true)?;
                let n = self.name_of(&at)?;
                let p = self.addr(base, &at)?;
                let key = self.conv_val(i, k)?;
                Ok(Ex::new(format!("{n}_get({p}, {})", key.s), 16, Kind::Ref))
            }
            t => no("C", format!("indexing {t}")),
        }
    }

    fn member(&mut self, obj: &Expr, name: &str, m: bool) -> R<Ex> {
        if name == "second"
            && let Some(r) = self.insert_second(obj)
        {
            return r;
        }
        let ot = self.ty_of(obj);
        match &ot {
            Ty::Pair(..) => {
                let f = if name == "first" { "first" } else { "second" };
                let o = self.ex(obj, m)?;
                Ok(Ex::new(self.dot(&o, f), 16, Kind::Place))
            }
            Ty::Struct(_) | Ty::Ptr(_) => {
                let o = self.ex(obj, m)?;
                let arrow = matches!(ot, Ty::Ptr(_)) || o.kind == Kind::Ref;
                let op = if arrow { "->" } else { "." };
                Ok(Ex::new(format!("{}{op}{name}", o.at(16)), 16, Kind::Place))
            }
            Ty::Iter { .. }
            | Ty::Map(..)
            | Ty::Set(_)
            | Ty::UnorderedMap(..)
            | Ty::UnorderedSet(_) => self.found_member(obj, name),
            Ty::Optional(_) => {
                let o = self.ex(obj, m)?;
                let f = if name == "first" { "first" } else { "second" };
                Ok(Ex::new(
                    format!("{}.{f}", self.dot(&o, "v")),
                    16,
                    Kind::Place,
                ))
            }
            t => no("C", format!("the member {name} of {t}")),
        }
    }

    /// `it->first` / `it->second` on an index returned by `find`.
    fn found_member(&mut self, obj: &Expr, name: &str) -> R<Ex> {
        let ExprKind::Var(v) = &obj.kind else {
            return no("C", "an iterator expression other than a variable");
        };
        let Some(Rk::Found { cont }) = self.lookup(v).map(|x| x.rk.clone()) else {
            return no("C", "an iterator other than the result of find");
        };
        let it = self.name(v);
        match name {
            "first" => Ok(Ex::new(format!("{cont}.keys[{it}]"), 16, Kind::Place)),
            "second" => Ok(Ex::new(format!("{cont}.vals[{it}]"), 16, Kind::Place)),
            _ => no("C", "this use of an iterator"),
        }
    }
}

fn flip(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::Gt,
        BinOp::Gt => BinOp::Lt,
        BinOp::Le => BinOp::Ge,
        BinOp::Ge => BinOp::Le,
        o => o,
    }
}
