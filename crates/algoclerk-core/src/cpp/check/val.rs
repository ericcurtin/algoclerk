// SPDX-License-Identifier: Apache-2.0
//! Typed values of the checker: a type plus constness, lvalue-ness and a known constant value, and the
//! arithmetic rules (integer promotion, usual arithmetic conversions, literal types, narrowing).

use crate::cpp::ast::*;

/// The result of checking an expression.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Val {
    /// Value type (never a reference).
    pub ty: Ty,
    /// The object is `const` (only meaningful for lvalues).
    pub konst: bool,
    /// For iterators and pointers: the pointee is const. For a pair: its `first` is const (map element).
    pub ic: bool,
    /// An lvalue (names an object).
    pub lval: bool,
    /// Known compile-time integer value (literals, constants, casts of constants).
    pub cval: Option<i128>,
    /// For a braced list `{a, b}`: the typed elements.
    pub list: Option<Vec<Val>>,
}

impl Val {
    /// A prvalue of this type.
    pub fn pr(ty: Ty) -> Val {
        Val {
            ty,
            konst: false,
            ic: false,
            lval: false,
            cval: None,
            list: None,
        }
    }
    /// A braced list.
    pub fn braced(elems: Vec<Val>) -> Val {
        Val {
            list: Some(elems),
            ..Val::pr(Ty::InitList)
        }
    }
    /// An lvalue of this type.
    pub fn lv(ty: Ty, konst: bool) -> Val {
        Val {
            ty,
            konst,
            ic: false,
            lval: true,
            cval: None,
            list: None,
        }
    }
    /// The "already reported" value.
    pub fn unknown() -> Val {
        Val::pr(Ty::Unknown)
    }
    /// A prvalue with a known constant value.
    pub fn constant(ty: Ty, v: i128) -> Val {
        Val {
            cval: Some(v),
            ..Val::pr(ty)
        }
    }
    /// Does the type mention `Unknown` (an earlier error)?
    pub fn is_unknown(&self) -> bool {
        has_unknown(&self.ty)
    }
    /// Same value with the pointee flag set.
    pub fn with_ic(mut self, ic: bool) -> Val {
        self.ic = ic;
        self
    }
}

/// Does the type contain `Unknown` or `Auto` anywhere?
pub(super) fn has_unknown(t: &Ty) -> bool {
    let mut found = false;
    t.walk(&mut |x| found |= matches!(x, Ty::Unknown | Ty::Auto));
    found
}

/// A string literal's type: `const char*`.
pub(super) fn cstr() -> Ty {
    Ty::Ptr(Box::new(Ty::Char))
}

/// Is this `const char*` (what a string literal is)?
pub(super) fn is_cstr(t: &Ty) -> bool {
    matches!(t, Ty::Ptr(p) if **p == Ty::Char)
}

/// Integral promotion.
pub(super) fn promote(t: &Ty) -> Ty {
    match t {
        Ty::Bool | Ty::Char => Ty::Int,
        t => t.clone(),
    }
}

/// Conversion rank of an integral type (`size_t` is `unsigned long`, between `int` and `long long`).
fn rank(t: &Ty) -> u8 {
    match t {
        Ty::Bool => 1,
        Ty::Char => 2,
        Ty::Int | Ty::UInt => 3,
        Ty::Size => 4,
        _ => 5,
    }
}

/// The usual arithmetic conversions on two integral types.
pub(super) fn uac(a: &Ty, b: &Ty) -> Ty {
    let (a, b) = (promote(a), promote(b));
    if a == b {
        return a;
    }
    let (ua, ub) = (a.is_unsigned(), b.is_unsigned());
    if ua == ub {
        return if rank(&a) >= rank(&b) { a } else { b };
    }
    let (u, s) = if ua { (a, b) } else { (b, a) };
    if rank(&u) >= rank(&s) {
        u
    } else if s.bits() > u.bits() {
        s
    } else {
        Ty::ULongLong
    }
}

/// Smallest and largest value of an integral type.
pub(super) fn int_range(t: &Ty) -> (i128, i128) {
    let bits = t.bits();
    match t {
        Ty::Bool => (0, 1),
        _ if t.is_unsigned() => (0, (1i128 << bits) - 1),
        _ => (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1),
    }
}

/// The C++ type of an integer literal.
pub(super) fn literal_ty(value: u64, suffix: IntSuffix, base: IntBase) -> Option<Ty> {
    let v = i128::from(value);
    let decimal = base == IntBase::Dec;
    let cands: &[Ty] = match (suffix, decimal) {
        (IntSuffix::None, true) => &[Ty::Int, Ty::LongLong],
        (IntSuffix::None, false) => &[Ty::Int, Ty::UInt, Ty::LongLong, Ty::ULongLong],
        (IntSuffix::U, _) => &[Ty::UInt, Ty::ULongLong],
        (IntSuffix::LL, true) => &[Ty::LongLong],
        (IntSuffix::LL, false) => &[Ty::LongLong, Ty::ULongLong],
        (IntSuffix::ULL, _) => &[Ty::ULongLong],
    };
    cands.iter().find(|t| v <= int_range(t).1).cloned()
}

/// Wrap a constant into the range of an integral type (what a conversion does at run time).
pub(super) fn wrap(t: &Ty, v: i128) -> i128 {
    if *t == Ty::Bool {
        return i128::from(v != 0);
    }
    let bits = t.bits();
    let m = 1i128 << bits;
    let mut r = v.rem_euclid(m);
    if !t.is_unsigned() && r >= m / 2 {
        r -= m;
    }
    r
}

/// Would `{}`-initialising an integral `to` from `from` be a narrowing conversion?
pub(super) fn narrowing(from: &Val, to: &Ty) -> bool {
    if !from.ty.is_integral() || !to.is_integral() || from.ty == *to {
        return false;
    }
    let (lo, hi) = int_range(to);
    if let Some(v) = from.cval {
        return v < lo || v > hi;
    }
    let (flo, fhi) = int_range(&from.ty);
    !(flo >= lo && fhi <= hi)
}

/// Fold a binary operation on two known integral constants (`None` when it is not a constant or not
/// well defined).
pub(super) fn fold(op: BinOp, a: i128, b: i128, ty: &Ty) -> Option<i128> {
    let r = match op {
        BinOp::Add => a.checked_add(b)?,
        BinOp::Sub => a.checked_sub(b)?,
        BinOp::Mul => a.checked_mul(b)?,
        BinOp::Div if b != 0 => a.checked_div(b)?,
        BinOp::Rem if b != 0 => a.checked_rem(b)?,
        BinOp::BitAnd => a & b,
        BinOp::BitOr => a | b,
        BinOp::BitXor => a ^ b,
        BinOp::Shl if (0..64).contains(&b) => a.checked_shl(u32::try_from(b).ok()?)?,
        BinOp::Shr if (0..64).contains(&b) => a >> b,
        BinOp::Eq => i128::from(a == b),
        BinOp::Ne => i128::from(a != b),
        BinOp::Lt => i128::from(a < b),
        BinOp::Le => i128::from(a <= b),
        BinOp::Gt => i128::from(a > b),
        BinOp::Ge => i128::from(a >= b),
        BinOp::And => i128::from(a != 0 && b != 0),
        BinOp::Or => i128::from(a != 0 || b != 0),
        _ => return None,
    };
    if ty.is_integral() && !matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le) {
        let (lo, hi) = int_range(ty);
        if r < lo || r > hi {
            return if ty.is_unsigned() {
                Some(wrap(ty, r))
            } else {
                None
            };
        }
    }
    Some(r)
}

/// Describe a value's type for messages.
pub(super) fn show(v: &Val) -> String {
    show_ty(&v.ty, v.ic)
}

/// Describe a type for messages (`const char*` for string literals).
pub(super) fn show_ty(t: &Ty, ic: bool) -> String {
    match t {
        Ty::Ptr(p) if **p == Ty::Char && ic => "const char*".to_string(),
        Ty::Lambda(_) => "a lambda".to_string(),
        Ty::InitList => "a braced list".to_string(),
        t => t.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_conversions() {
        assert_eq!(uac(&Ty::Char, &Ty::Char), Ty::Int);
        assert_eq!(uac(&Ty::Int, &Ty::UInt), Ty::UInt);
        assert_eq!(uac(&Ty::UInt, &Ty::LongLong), Ty::LongLong);
        assert_eq!(uac(&Ty::Size, &Ty::Int), Ty::Size);
        assert_eq!(uac(&Ty::Size, &Ty::LongLong), Ty::ULongLong);
        assert_eq!(uac(&Ty::LongLong, &Ty::ULongLong), Ty::ULongLong);
    }

    #[test]
    fn literals() {
        assert_eq!(literal_ty(5, IntSuffix::None, IntBase::Dec), Some(Ty::Int));
        assert_eq!(
            literal_ty(3_000_000_000, IntSuffix::None, IntBase::Dec),
            Some(Ty::LongLong)
        );
        assert_eq!(
            literal_ty(0x8000_0000, IntSuffix::None, IntBase::Hex),
            Some(Ty::UInt)
        );
        assert_eq!(literal_ty(1, IntSuffix::U, IntBase::Dec), Some(Ty::UInt));
    }
}
