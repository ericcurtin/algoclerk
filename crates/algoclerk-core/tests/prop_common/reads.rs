// SPDX-License-Identifier: Apache-2.0
//! Expressions that read containers and strings, booleans, characters, strings, helper calls.

use super::expr::*;

/// Largest length any generated string or container is assumed to reach.
pub const LEN_CAP: i64 = 300;

impl Gen {
    /// Reduce `e` into `-KI..=KI` (it keeps its type).
    pub fn fit_int(&mut self, e: E) -> E {
        if e.lo >= -KI && e.hi <= KI {
            return e;
        }
        self.reduce(e, KI + 1)
    }

    /// Reduce `e` into the invariant range of `v`.
    pub fn fit_var(&mut self, e: E, v: &Var) -> E {
        if !v.k.num().is_some_and(|n| n.signed()) {
            return e;
        }
        if e.lo >= v.lo && e.hi <= v.hi {
            return e;
        }
        self.reduce(e, v.hi + 1)
    }

    /// `(e % n + n) % n`, a valid index into something of size `n`, as text.
    pub fn idx_mod(&mut self, n: i64, d: u32) -> String {
        let e = self.num(N::Int, d);
        let e = self.fit_int(e);
        format!("(({} % {n} + {n}) % {n})", e.s)
    }

    /// A valid index of a possibly empty container `name` (use only under an emptiness guard).
    fn size_idx(&mut self, name: &str, d: u32) -> String {
        let e = self.num(N::Int, d);
        let e = self.fit_int(e);
        format!("static_cast<size_t>({}) % {name}.size()", e.s)
    }

    /// An `int` read from a container, a string, or a helper.
    pub fn int_read(&mut self) -> E {
        let all = [K::Vec, K::Str, K::Grid, K::Map, K::UMap, K::Set, K::USet, K::Stack, K::Queue, K::Deque, K::Pq];
        let Some(v) = self.pick_var(&all) else {
            return self.lit(N::Int);
        };
        let n = v.name.clone();
        let ik = |s: String, lo: i128, hi: i128| E::new(s, N::Int, lo, hi);
        let d = 1;
        match v.k {
            K::Vec => match self.src.weighted(&[3, 2, 2, 2, 2, 2, 2, 1]) {
                0 => {
                    let i = self.size_idx(&n, d);
                    ik(format!("({n}.empty() ? 0 : {n}[{i}])"), -KI, KI)
                }
                1 => {
                    let i = self.size_idx(&n, d);
                    ik(format!("({n}.empty() ? 0 : {n}.at({i}))"), -KI, KI)
                }
                2 => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
                3 => {
                    let b = if self.src.chance(1, 2) { "front" } else { "back" };
                    ik(format!("({n}.empty() ? 0 : {n}.{b}())"), -KI, KI)
                }
                4 => ik(
                    format!("std::accumulate({n}.begin(), {n}.end(), 0)"),
                    -KI * v.len as i128,
                    KI * v.len as i128,
                ),
                5 => {
                    let k = self.num(N::Int, 1);
                    ik(format!("static_cast<int>(std::count({n}.begin(), {n}.end(), {}))", k.s), 0, v.len as i128)
                }
                6 => {
                    let k = self.num(N::Int, 1);
                    let f = format!("std::find({n}.begin(), {n}.end(), {})", k.s);
                    ik(format!("static_cast<int>(std::distance({n}.begin(), {f}))"), 0, v.len as i128)
                }
                _ => {
                    let f = if self.src.chance(1, 2) { "min_element" } else { "max_element" };
                    ik(format!("({n}.empty() ? 0 : *std::{f}({n}.begin(), {n}.end()))"), -KI, KI)
                }
            },
            K::Str => match self.src.weighted(&[3, 2, 2, 1]) {
                0 => {
                    let i = self.size_idx(&n, d);
                    ik(format!("({n}.empty() ? 0 : {n}[{i}])"), 0, 126)
                }
                1 => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
                2 => {
                    let c = self.char_expr(1);
                    ik(format!("static_cast<int>({n}.find({c}))"), -1, v.len as i128)
                }
                _ => {
                    let c = self.char_expr(1);
                    ik(format!("static_cast<int>(std::count({n}.begin(), {n}.end(), {c}))"), 0, v.len as i128)
                }
            },
            K::Grid => {
                let r = self.idx_mod(v.rows, 1);
                let c = self.idx_mod(v.cols, 1);
                ik(format!("{n}[{r}][{c}]"), -KI, KI)
            }
            K::Map | K::UMap => {
                if v.frozen || self.src.chance(1, 3) {
                    let k = self.num(N::Int, 1);
                    ik(format!("static_cast<int>({n}.count({}))", k.s), 0, 1)
                } else if self.src.chance(1, 4) {
                    ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128)
                } else {
                    let k = self.num(N::Int, 1);
                    let m = self.mult;
                    self.grow(&n, m);
                    ik(format!("{n}[{}]", k.s), -KI, KI)
                }
            }
            K::Set | K::USet => match self.src.below(4) {
                0 => {
                    let k = self.num(N::Int, 1);
                    ik(format!("static_cast<int>({n}.count({}))", k.s), 0, 1)
                }
                1 => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
                2 if v.k == K::Set => {
                    let e = if self.src.chance(1, 2) { format!("*{n}.begin()") } else { format!("*{n}.rbegin()") };
                    ik(format!("({n}.empty() ? 0 : {e})"), -KI, KI)
                }
                _ if v.k == K::Set => {
                    let k = self.num(N::Int, 1);
                    ik(
                        format!("({n}.lower_bound({0}) == {n}.end() ? -1 : *{n}.lower_bound({0}))", k.s),
                        -KI,
                        KI,
                    )
                }
                _ => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
            },
            K::Stack => match self.src.below(2) {
                0 => ik(format!("({n}.empty() ? 0 : {n}.top())"), -KI, KI),
                _ => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
            },
            K::Pq => match self.src.below(2) {
                0 => ik(format!("({n}.empty() ? 0 : {n}.top())"), -KI, KI),
                _ => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
            },
            K::Queue => match self.src.below(3) {
                0 => ik(format!("({n}.empty() ? 0 : {n}.front())"), -KI, KI),
                1 => ik(format!("({n}.empty() ? 0 : {n}.back())"), -KI, KI),
                _ => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
            },
            K::Deque => match self.src.below(4) {
                0 => ik(format!("({n}.empty() ? 0 : {n}.front())"), -KI, KI),
                1 => ik(format!("({n}.empty() ? 0 : {n}.back())"), -KI, KI),
                2 => {
                    let i = self.size_idx(&n, d);
                    ik(format!("({n}.empty() ? 0 : {n}[{i}])"), -KI, KI)
                }
                _ => ik(format!("static_cast<int>({n}.size())"), 0, v.len as i128),
            },
            _ => self.lit(N::Int),
        }
    }
}
