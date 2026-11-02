// SPDX-License-Identifier: Apache-2.0
//! Unification-based local type inference.

use super::ast::Ty;

/// Substitution table for inference variables.
#[derive(Debug, Default)]
pub struct Infer {
    subst: Vec<Option<Ty>>,
}

impl Infer {
    /// A fresh unknown type.
    pub fn fresh(&mut self) -> Ty {
        self.subst.push(None);
        Ty::Var(self.subst.len() as u32 - 1)
    }

    /// Resolve the outermost layer only.
    pub fn shallow(&self, t: &Ty) -> Ty {
        let mut cur = t.clone();
        while let Ty::Var(v) = cur {
            match self.subst.get(v as usize).and_then(|x| x.clone()) {
                Some(n) => cur = n,
                None => return Ty::Var(v),
            }
        }
        cur
    }

    /// Resolve all layers.
    pub fn resolve(&self, t: &Ty) -> Ty {
        match self.shallow(t) {
            Ty::Vec(a) => Ty::Vec(Box::new(self.resolve(&a))),
            Ty::Set(a) => Ty::Set(Box::new(self.resolve(&a))),
            Ty::Option(a) => Ty::Option(Box::new(self.resolve(&a))),
            Ty::Stack(a) => Ty::Stack(Box::new(self.resolve(&a))),
            Ty::Queue(a) => Ty::Queue(Box::new(self.resolve(&a))),
            Ty::Deque(a) => Ty::Deque(Box::new(self.resolve(&a))),
            Ty::Heap(a) => Ty::Heap(Box::new(self.resolve(&a))),
            Ty::Map(k, v) => Ty::Map(Box::new(self.resolve(&k)), Box::new(self.resolve(&v))),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.resolve(t)).collect()),
            other => other,
        }
    }

    /// Does the type still contain unknowns?
    pub fn has_var(&self, t: &Ty) -> bool {
        match self.shallow(t) {
            Ty::Var(_) => true,
            Ty::Vec(a) | Ty::Set(a) | Ty::Option(a) | Ty::Stack(a) | Ty::Queue(a) | Ty::Deque(a) | Ty::Heap(a) => {
                self.has_var(&a)
            }
            Ty::Map(k, v) => self.has_var(&k) || self.has_var(&v),
            Ty::Tuple(ts) => ts.iter().any(|t| self.has_var(t)),
            _ => false,
        }
    }

    fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.shallow(t) {
            Ty::Var(w) => v == w,
            Ty::Vec(a) | Ty::Set(a) | Ty::Option(a) | Ty::Stack(a) | Ty::Queue(a) | Ty::Deque(a) | Ty::Heap(a) => {
                self.occurs(v, &a)
            }
            Ty::Map(k, x) => self.occurs(v, &k) || self.occurs(v, &x),
            Ty::Tuple(ts) => ts.iter().any(|t| self.occurs(v, t)),
            _ => false,
        }
    }

    /// Make two types equal; `false` when impossible.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let a = self.shallow(a);
        let b = self.shallow(b);
        match (a, b) {
            (Ty::Var(x), Ty::Var(y)) if x == y => true,
            (Ty::Var(x), t) | (t, Ty::Var(x)) => {
                if self.occurs(x, &t) {
                    return false;
                }
                self.subst[x as usize] = Some(t);
                true
            }
            (Ty::Vec(x), Ty::Vec(y))
            | (Ty::Set(x), Ty::Set(y))
            | (Ty::Option(x), Ty::Option(y))
            | (Ty::Stack(x), Ty::Stack(y))
            | (Ty::Queue(x), Ty::Queue(y))
            | (Ty::Deque(x), Ty::Deque(y))
            | (Ty::Heap(x), Ty::Heap(y)) => self.unify(&x, &y),
            (Ty::Map(k1, v1), Ty::Map(k2, v2)) => self.unify(&k1, &k2) && self.unify(&v1, &v2),
            (Ty::Tuple(xs), Ty::Tuple(ys)) => {
                xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(x, y)| self.unify(x, y))
            }
            (x, y) => x == y,
        }
    }
}

/// Can values of this (resolved) type be set members and map keys?
pub fn hashable(t: &Ty) -> bool {
    match t {
        Ty::Int | Ty::Bool | Ty::Char | Ty::Str => true,
        Ty::Tuple(ts) => ts.iter().all(hashable),
        Ty::Vec(t) => hashable(t) && matches!(**t, Ty::Int | Ty::Char | Ty::Bool),
        Ty::Var(_) => true,
        _ => false,
    }
}

/// Can values of this (resolved) type be compared with `<` and sorted?
pub fn orderable(t: &Ty) -> bool {
    match t {
        Ty::Int | Ty::Bool | Ty::Char | Ty::Str => true,
        Ty::Tuple(ts) => ts.iter().all(orderable),
        Ty::Vec(t) => orderable(t),
        Ty::Var(_) => true,
        _ => false,
    }
}
