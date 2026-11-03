// SPDX-License-Identifier: Apache-2.0
//! Integer arithmetic with the C++ promotion and conversion rules and undefined behaviour detection.

use super::RtKind;
use super::rval::{Sk, Value};
use crate::cpp::ast::{BinOp, UnOp};

/// A failure to be turned into an `RtError` at the current node.
pub type Fail = (RtKind, String);

/// Integral promotion.
pub fn promote(s: Sk) -> Sk {
    match s {
        Sk::Bool | Sk::Char | Sk::UChar => Sk::Int,
        o => o,
    }
}

fn rank(s: Sk) -> u8 {
    match s {
        Sk::Bool => 0,
        Sk::Char | Sk::UChar => 0,
        Sk::Int | Sk::UInt => 1,
        Sk::Size => 2,
        Sk::LL | Sk::Ull => 3,
    }
}

fn unsigned_of(s: Sk) -> Sk {
    match s {
        Sk::Int => Sk::UInt,
        Sk::LL => Sk::Ull,
        o => o,
    }
}

/// The usual arithmetic conversions on two promoted kinds (LP64 data model: `size_t` is `unsigned long`).
pub fn common(a: Sk, b: Sk) -> Sk {
    let (a, b) = (promote(a), promote(b));
    if a == b {
        return a;
    }
    if a.signed() == b.signed() {
        return if rank(a) >= rank(b) { a } else { b };
    }
    let (s, u) = if a.signed() { (a, b) } else { (b, a) };
    if rank(u) >= rank(s) {
        u
    } else if s.bits() > u.bits() {
        s
    } else {
        unsigned_of(s)
    }
}

/// Name of a scalar kind as C++ spells it.
pub fn name(s: Sk) -> &'static str {
    match s {
        Sk::Bool => "bool",
        Sk::Char => "char",
        Sk::UChar => "unsigned char",
        Sk::Int => "int",
        Sk::UInt => "unsigned",
        Sk::LL => "long long",
        Sk::Ull => "unsigned long long",
        Sk::Size => "size_t",
    }
}

fn overflow(op: &str, s: Sk) -> Fail {
    (
        RtKind::Overflow,
        format!(
            "signed overflow: `{op}` on {} does not fit, which is undefined behaviour in C++",
            name(s)
        ),
    )
}

/// Convert an integral value to the kind `to` (wrapping like gcc and clang).
pub fn convert(v: &Value, to: Sk) -> Option<Value> {
    v.as_i128().map(|x| to.make(x))
}

/// `a op b` for two integral scalars (not `&&`, `||`, `,`).
pub fn binary(op: BinOp, a: &Value, b: &Value) -> Result<Value, Fail> {
    let (Some(x), Some(y)) = (a.as_i128(), b.as_i128()) else {
        return Err((RtKind::Internal, "arithmetic on a non-number".into()));
    };
    let (Some(sa), Some(sb)) = (a.sk(), b.sk()) else {
        return Err((RtKind::Internal, "arithmetic on a non-number".into()));
    };
    if matches!(op, BinOp::Shl | BinOp::Shr) {
        return shift(op, promote(sa), x, y);
    }
    let c = common(sa, sb);
    // convert both operands to the common type
    let x = wrap_to(c, x);
    let y = wrap_to(c, y);
    let checked = |r: i128, sym: &str| -> Result<Value, Fail> {
        if c.signed() && !c.fits(r) {
            Err(overflow(sym, c))
        } else {
            Ok(c.make(r))
        }
    };
    match op {
        BinOp::Add => checked(x + y, "+"),
        BinOp::Sub => checked(x - y, "-"),
        BinOp::Mul => checked(x * y, "*"),
        BinOp::Div | BinOp::Rem => {
            if y == 0 {
                return Err((
                    RtKind::DivZero,
                    if op == BinOp::Div {
                        "division by zero".into()
                    } else {
                        "remainder by zero".into()
                    },
                ));
            }
            if c.signed() && x == c.min() && y == -1 {
                return Err(overflow(if op == BinOp::Div { "/" } else { "%" }, c));
            }
            Ok(c.make(if op == BinOp::Div { x / y } else { x % y }))
        }
        BinOp::BitAnd => Ok(c.make(x & y)),
        BinOp::BitOr => Ok(c.make(x | y)),
        BinOp::BitXor => Ok(c.make(x ^ y)),
        BinOp::Eq => Ok(Value::Bool(x == y)),
        BinOp::Ne => Ok(Value::Bool(x != y)),
        BinOp::Lt => Ok(Value::Bool(x < y)),
        BinOp::Le => Ok(Value::Bool(x <= y)),
        BinOp::Gt => Ok(Value::Bool(x > y)),
        BinOp::Ge => Ok(Value::Bool(x >= y)),
        BinOp::And | BinOp::Or | BinOp::Comma | BinOp::Shl | BinOp::Shr => {
            Err((RtKind::Internal, "operator handled elsewhere".into()))
        }
    }
}

/// Value of `x` after conversion to kind `c` (as an i128).
pub fn wrap_to(c: Sk, x: i128) -> i128 {
    if c.fits(x) {
        return x;
    }
    c.make(x).as_i128().unwrap_or(0)
}

fn shift(op: BinOp, l: Sk, x: i128, count: i128) -> Result<Value, Fail> {
    let width = i128::from(l.bits());
    let sym = if op == BinOp::Shl { "<<" } else { ">>" };
    if count < 0 {
        return Err((
            RtKind::Shift,
            format!("shift by a negative count ({count}) with `{sym}` is undefined behaviour"),
        ));
    }
    if count >= width {
        return Err((
            RtKind::Shift,
            format!(
                "shift by {count} with `{sym}` is undefined behaviour: {} has only {width} bits",
                name(l)
            ),
        ));
    }
    let x = wrap_to(l, x);
    if op == BinOp::Shr {
        return Ok(l.make(x >> count));
    }
    if l.signed() {
        if x < 0 {
            return Err((
                RtKind::Shift,
                "left shift of a negative value is undefined behaviour".into(),
            ));
        }
        let r = x << count;
        if r > (1i128 << width) - 1 {
            return Err((
                RtKind::Shift,
                format!("`<<` shifts bits out of {}: undefined behaviour", name(l)),
            ));
        }
        return Ok(l.make(r));
    }
    Ok(l.make(x << count))
}

/// Unary arithmetic on an integral scalar (`- + ~ !`).
pub fn unary(op: UnOp, a: &Value) -> Result<Value, Fail> {
    let (Some(x), Some(s)) = (a.as_i128(), a.sk()) else {
        return Err((RtKind::Internal, "arithmetic on a non-number".into()));
    };
    let p = promote(s);
    match op {
        UnOp::Not => Ok(Value::Bool(x == 0)),
        UnOp::Plus => Ok(p.make(x)),
        UnOp::Neg => {
            let r = -x;
            if p.signed() && !p.fits(r) {
                Err(overflow("unary -", p))
            } else {
                Ok(p.make(r))
            }
        }
        UnOp::BitNot => Ok(p.make(!x)),
        _ => Err((RtKind::Internal, "not an arithmetic operator".into())),
    }
}
