// SPDX-License-Identifier: Apache-2.0
//! Generator state and numeric expressions. Every numeric expression carries a conservative interval of its
//! true value; operations that could leave the range of a signed type reduce their operands with `%` first,
//! so the generated programs are free of signed overflow by construction.

use super::rng::Src;

/// Arithmetic types in order of conversion rank.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum N {
    Int,
    U,
    Ll,
    Ull,
}

pub const ALL_N: [N; 4] = [N::Int, N::U, N::Ll, N::Ull];

impl N {
    pub fn signed(self) -> bool {
        matches!(self, N::Int | N::Ll)
    }
    pub fn bits(self) -> u32 {
        if matches!(self, N::Int | N::U) {
            32
        } else {
            64
        }
    }
    pub fn min(self) -> i128 {
        if self.signed() {
            -(1i128 << (self.bits() - 1))
        } else {
            0
        }
    }
    pub fn max(self) -> i128 {
        if self.signed() {
            (1i128 << (self.bits() - 1)) - 1
        } else {
            (1i128 << self.bits()) - 1
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            N::Int => "int",
            N::U => "unsigned",
            N::Ll => "long long",
            N::Ull => "unsigned long long",
        }
    }
}

/// A numeric expression: text, exact C++ type, interval of its value.
#[derive(Clone, Debug)]
pub struct E {
    pub s: String,
    pub t: N,
    pub lo: i128,
    pub hi: i128,
}

impl E {
    pub fn new(s: String, t: N, lo: i128, hi: i128) -> E {
        E { s, t, lo, hi }
    }
    pub fn full(s: String, t: N) -> E {
        E {
            s,
            t,
            lo: t.min(),
            hi: t.max(),
        }
    }
    /// Interval of the value after conversion to `to`.
    pub fn conv_iv(&self, to: N) -> (i128, i128) {
        if self.lo >= to.min() && self.hi <= to.max() {
            (self.lo, self.hi)
        } else {
            (to.min(), to.max())
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum K {
    Int,
    U,
    Ll,
    Ull,
    Char,
    Bool,
    Str,
    Vec,
    Grid,
    Map,
    UMap,
    Set,
    USet,
    Stack,
    Queue,
    Deque,
    Pq,
}

impl K {
    pub fn num(self) -> Option<N> {
        match self {
            K::Int => Some(N::Int),
            K::U => Some(N::U),
            K::Ll => Some(N::Ll),
            K::Ull => Some(N::Ull),
            _ => None,
        }
    }
    pub fn of(n: N) -> K {
        match n {
            N::Int => K::Int,
            N::U => K::U,
            N::Ll => K::Ll,
            N::Ull => K::Ull,
        }
    }
    pub fn scalar(self) -> bool {
        matches!(self, K::Int | K::U | K::Ll | K::Ull | K::Char | K::Bool)
    }
}

/// Invariant bound of `int` variables and container elements.
pub const KI: i128 = 30000;
/// Invariant bound of `long long` variables.
pub const KL: i128 = 1_000_000_000;

#[derive(Clone, Debug)]
pub struct Var {
    pub name: String,
    pub k: K,
    pub lo: i128,
    pub hi: i128,
    /// Upper bound of the length (containers and strings).
    pub len: i64,
    /// Rows and columns of a grid.
    pub rows: i64,
    pub cols: i64,
    /// May be assigned.
    pub rw: bool,
    /// May not be changed or have elements added or removed (it is being iterated).
    pub frozen: bool,
    /// May be used inside a lambda (structured bindings may not).
    pub capturable: bool,
}

impl Var {
    pub fn new(name: String, k: K) -> Var {
        let (lo, hi) = match k {
            K::Int => (-KI, KI),
            K::Ll => (-KL, KL),
            K::Char => (32, 126),
            K::U => (0, N::U.max()),
            K::Ull => (0, N::Ull.max()),
            K::Bool => (0, 1),
            _ => (-KI, KI),
        };
        Var {
            name,
            k,
            lo,
            hi,
            len: 0,
            rows: 0,
            cols: 0,
            rw: true,
            frozen: false,
            capturable: true,
        }
    }
}

/// A helper function the generated code may call.
#[derive(Clone, Debug)]
pub struct Helper {
    pub name: String,
    /// Parameter kinds (`Int`, `Ll`, `Str`, `Vec`); `Vec` is passed by reference.
    pub params: Vec<K>,
    /// Return kind (`Int` or `Ll`) or `None` for void.
    pub ret: Option<K>,
    /// Recursion bound mask for the first argument (the depth) when the helper is recursive.
    pub rec_mask: Option<i64>,
}

/// Generator state.
pub struct Gen {
    pub src: Src,
    pub scopes: Vec<Vec<Var>>,
    pub counter: u32,
    pub out: String,
    pub indent: usize,
    /// Product of the bounds of the enclosing loops.
    pub mult: i64,
    pub loops: u32,
    pub in_loop: bool,
    pub budget: i32,
    pub helpers: Vec<Helper>,
    /// Return statements allowed inside nested statements (helpers with a scalar result).
    pub early_return: Option<K>,
    /// Lambdas in scope: (name, number of int parameters).
    pub lambdas: Vec<(String, usize)>,
    /// The void vector helpers are defined.
    pub void_helpers: bool,
    pub lambda_marks: Vec<usize>,
}

impl Gen {
    pub fn new(src: Src) -> Gen {
        Gen {
            src,
            scopes: vec![Vec::new()],
            counter: 0,
            out: String::new(),
            indent: 1,
            mult: 1,
            loops: 0,
            in_loop: false,
            budget: 40,
            helpers: Vec::new(),
            early_return: None,
            lambdas: Vec::new(),
            void_helpers: false,
            lambda_marks: Vec::new(),
        }
    }

    pub fn fresh(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}{}", self.counter)
    }

    pub fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
        self.lambda_marks.push(self.lambdas.len());
    }
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
        if let Some(m) = self.lambda_marks.pop() {
            self.lambdas.truncate(m);
        }
    }
    pub fn add(&mut self, v: Var) {
        self.scopes.last_mut().expect("scope").push(v);
    }

    pub fn find_vars(&self, ks: &[K]) -> Vec<Var> {
        self.scopes
            .iter()
            .flatten()
            .filter(|v| ks.contains(&v.k))
            .cloned()
            .collect()
    }

    pub fn pick_var(&mut self, ks: &[K]) -> Option<Var> {
        let vs = self.find_vars(ks);
        if vs.is_empty() {
            None
        } else {
            Some(vs[self.src.below(vs.len())].clone())
        }
    }

    pub fn var_mut(&mut self, name: &str) -> Option<&mut Var> {
        self.scopes.iter_mut().flatten().find(|v| v.name == name)
    }

    pub fn lit_int(&mut self, lo: i64, hi: i64) -> E {
        let v = self.src.range(lo, hi);
        let s = if v < 0 {
            format!("(-{})", -v)
        } else {
            v.to_string()
        };
        E::new(s, N::Int, v as i128, v as i128)
    }
}
