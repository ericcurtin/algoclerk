// SPDX-License-Identifier: Apache-2.0
//! C++ types as Rust types, defaults, `Copy`-ness and conversions between integral types.

use super::Rs;
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

impl Rs<'_> {
    /// The Rust spelling of a C++ type (the type of a value, never a reference).
    pub fn rty(&mut self, t: &Ty) -> R<String> {
        Ok(match t {
            Ty::Void => "()".into(),
            Ty::Bool => "bool".into(),
            Ty::Char => "u8".into(),
            Ty::Int => "i32".into(),
            Ty::UInt => "u32".into(),
            Ty::LongLong => "i64".into(),
            Ty::ULongLong => "u64".into(),
            Ty::Size => "usize".into(),
            Ty::Str => "String".into(),
            Ty::StrView => "&str".into(),
            Ty::Vector(e) | Ty::Stack(e) => format!("Vec<{}>", self.rty(e)?),
            Ty::Array(e, n) => format!("[{}; {n}]", self.rty(e)?),
            Ty::Pair(a, b) => format!("({}, {})", self.rty(a)?, self.rty(b)?),
            Ty::Tuple(ts) => {
                let parts: R<Vec<String>> = ts.iter().map(|t| self.rty(t)).collect();
                format!("({})", parts?.join(", "))
            }
            Ty::Optional(e) => format!("Option<{}>", self.rty(e)?),
            Ty::UnorderedMap(k, v) => {
                self.uses.insert("HashMap");
                format!("HashMap<{}, {}>", self.rty(k)?, self.rty(v)?)
            }
            Ty::Map(k, v) => {
                self.uses.insert("BTreeMap");
                format!("BTreeMap<{}, {}>", self.rty(k)?, self.rty(v)?)
            }
            Ty::UnorderedSet(e) => {
                self.uses.insert("HashSet");
                format!("HashSet<{}>", self.rty(e)?)
            }
            Ty::Set(e) => {
                self.uses.insert("BTreeSet");
                format!("BTreeSet<{}>", self.rty(e)?)
            }
            Ty::Queue(e) | Ty::Deque(e) | Ty::List(e) => {
                self.uses.insert("VecDeque");
                format!("VecDeque<{}>", self.rty(e)?)
            }
            Ty::PriorityQueue(e, cmp) => {
                self.uses.insert("BinaryHeap");
                match &**cmp {
                    Ty::Functor(Functor::Less, _) => format!("BinaryHeap<{}>", self.rty(e)?),
                    Ty::Functor(Functor::Greater, _) => {
                        self.uses.insert("Reverse");
                        format!("BinaryHeap<Reverse<{}>>", self.rty(e)?)
                    }
                    _ => return no("Rust", "a priority_queue ordered by a lambda"),
                }
            }
            Ty::Ptr(inner) => match &**inner {
                Ty::Struct(n) => {
                    self.use_struct(n)?;
                    n.clone()
                }
                _ => return no("Rust", "a pointer to a non-struct type"),
            },
            Ty::Struct(n) => {
                self.use_struct(n)?;
                n.clone()
            }
            Ty::NullPtr => "()".into(),
            t => return no("Rust", format!("the type {t}")),
        })
    }

    /// Is the Rust type `Copy`?
    pub fn is_copy(&self, t: &Ty) -> bool {
        match t {
            Ty::Void
            | Ty::Bool
            | Ty::Char
            | Ty::Int
            | Ty::UInt
            | Ty::LongLong
            | Ty::ULongLong
            | Ty::Size
            | Ty::StrView
            | Ty::Ptr(_)
            | Ty::NullPtr
            | Ty::Struct(_) => true,
            Ty::Pair(a, b) => self.is_copy(a) && self.is_copy(b),
            Ty::Tuple(ts) => ts.iter().all(|t| self.is_copy(t)),
            Ty::Optional(e) | Ty::Array(e, _) => self.is_copy(e),
            _ => false,
        }
    }

    /// The value a default-constructed C++ object of this type has (also what an uninitialised scalar can
    /// hold: reading one is reported by the interpreter, so any value is faithful).
    pub fn default_val(&mut self, t: &Ty) -> R<String> {
        Ok(match t {
            Ty::Bool => "false".into(),
            Ty::Char | Ty::Int | Ty::UInt | Ty::LongLong | Ty::ULongLong | Ty::Size => "0".into(),
            Ty::Str => "String::new()".into(),
            Ty::StrView => "\"\"".into(),
            Ty::Vector(_) | Ty::Stack(_) => "Vec::new()".into(),
            Ty::Array(e, n) => format!("[{}; {n}]", self.default_val(e)?),
            Ty::Pair(a, b) => format!("({}, {})", self.default_val(a)?, self.default_val(b)?),
            Ty::Tuple(ts) => {
                let parts: R<Vec<String>> = ts.iter().map(|t| self.default_val(t)).collect();
                format!("({})", parts?.join(", "))
            }
            Ty::Optional(_) => "None".into(),
            Ty::UnorderedMap(..) => "HashMap::new()".into(),
            Ty::Map(..) => "BTreeMap::new()".into(),
            Ty::UnorderedSet(_) => "HashSet::new()".into(),
            Ty::Set(_) => "BTreeSet::new()".into(),
            Ty::Queue(_) | Ty::Deque(_) | Ty::List(_) => "VecDeque::new()".into(),
            Ty::PriorityQueue(..) => "BinaryHeap::new()".into(),
            Ty::Ptr(_) => {
                let n = self.rty(t)?;
                format!("{n}::NULL")
            }
            t => return no("Rust", format!("a default value of type {t}")),
        })
    }

    /// An integer literal of type `t` (negative values wrap for unsigned types like C++ does).
    pub fn int_lit(&self, v: i128, t: &Ty) -> Ex {
        let signed = |bits: u32, v: i128| {
            let m = 1i128 << bits;
            (v + (m >> 1)).rem_euclid(m) - (m >> 1)
        };
        let (suffix, v) = match t {
            Ty::Int => ("", signed(32, v)),
            Ty::Char => ("u8", v.rem_euclid(256)),
            Ty::Bool => ("", v),
            Ty::UInt => ("u32", v.rem_euclid(1 << 32)),
            Ty::LongLong => ("i64", signed(64, v)),
            Ty::ULongLong => ("u64", v.rem_euclid(1 << 64)),
            Ty::Size => ("usize", v.rem_euclid(1 << 64)),
            _ => ("", v),
        };
        let mut x = Ex::val(format!("{v}{suffix}"));
        if v < 0 {
            x.p = 17;
        }
        x.lit = Some(v);
        x
    }

    /// Convert an integral or boolean expression of C++ type `from` to type `to`.
    pub fn conv_int(&mut self, x: Ex, from: &Ty, to: &Ty) -> R<Ex> {
        if from == to {
            return Ok(self.own(x, from));
        }
        let x = self.own(x, from);
        if let Some(v) = x.lit {
            if to == &Ty::Bool {
                return Ok(Ex::val(if v != 0 { "true" } else { "false" }));
            }
            if to.is_integral() {
                return Ok(self.int_lit(v, to));
            }
        }
        if to == &Ty::Bool {
            return Ok(Ex::new(format!("{} != 0", x.at(8)), 7, Kind::Val));
        }
        let t = self.rty(to)?;
        let mut r = Ex::new(format!("{} as {t}", x.at(17)), 16, Kind::Val);
        r.cast = true;
        Ok(r)
    }

    /// Use a translated expression as an owned value of its type: dereference references and clone places
    /// of types that are not `Copy`.
    pub fn own(&self, x: Ex, ty: &Ty) -> Ex {
        let copy = self.is_copy(ty);
        if *ty == Ty::Str && (x.slice || x.kind == Kind::Ref) {
            return Ex::new(format!("{}.to_string()", x.at(19)), 19, Kind::Val);
        }
        match x.kind {
            Kind::Val => x,
            Kind::Place if copy => Ex {
                kind: Kind::Val,
                ..x
            },
            Kind::Place => Ex::new(format!("{}.clone()", x.at(19)), 19, Kind::Val),
            Kind::Ref if copy => Ex::new(format!("*{}", x.at(17)), 17, Kind::Val),
            Kind::Ref => Ex::new(format!("{}.clone()", x.at(19)), 19, Kind::Val),
        }
    }

    /// Convert `x` (of C++ type `from`) to the type `to` the context needs: integral casts, `Some(..)` for
    /// optionals, owned values.
    pub fn convert(&mut self, x: Ex, from: &Ty, to: &Ty) -> R<Ex> {
        if matches!(from, Ty::Struct(_)) && x.kind != Kind::Val {
            return no(
                "Rust",
                "copying a struct value (nodes are shared through pointers)",
            );
        }
        if from == to {
            return Ok(self.own(x, from));
        }
        match (from, to) {
            (f, t) if (f.is_integral() && t.is_integral()) => self.conv_int(x, f, t),
            (Ty::NullPtr, _) | (Ty::Unknown, _) => Ok(x),
            (f, Ty::Optional(inner)) if !matches!(f, Ty::Optional(_)) => {
                let y = self.convert(x, f, inner)?;
                Ok(Ex::val(format!("Some({})", y.s)))
            }
            (Ty::Str, Ty::StrView) => Ok(Ex::new(format!("{}.as_str()", x.at(19)), 19, Kind::Val)),
            (Ty::Pair(a1, b1), Ty::Pair(a2, b2)) if a1 == a2 && b1 == b2 => Ok(self.own(x, from)),
            _ => Ok(self.own(x, from)),
        }
    }
}
