// SPDX-License-Identifier: Apache-2.0
//! Boolean, character, string and vector expressions, calls of helpers and lambdas.

use super::expr::*;
use super::reads::LEN_CAP;

impl Gen {
    pub fn boolean(&mut self, d: u32) -> String {
        if d == 0 {
            return match self.pick_var(&[K::Bool]) {
                Some(v) if self.src.chance(2, 3) => v.name,
                _ => {
                    if self.src.chance(1, 2) {
                        "true".into()
                    } else {
                        "false".into()
                    }
                }
            };
        }
        match self.src.weighted(&[8, 2, 3, 2, 3, 2, 2, 2]) {
            0 => {
                let ta = *self.src.pick(&ALL_N);
                let tb = if self.src.chance(1, 2) {
                    ta
                } else {
                    *self.src.pick(&ALL_N)
                };
                let a = self.num(ta, d - 1);
                let b = self.num(tb, d - 1);
                let op = *self.src.pick(&["<", "<=", ">", ">=", "==", "!="]);
                format!("({} {op} {})", a.s, b.s)
            }
            1 => format!("(!{})", self.boolean(d - 1)),
            2 => {
                let op = if self.src.chance(1, 2) { "&&" } else { "||" };
                format!("({} {op} {})", self.boolean(d - 1), self.boolean(d - 1))
            }
            3 => {
                // a guard that is only safe because && short-circuits
                let Some(v) = self.pick_var(&[K::Vec, K::Str, K::Stack, K::Queue, K::Deque, K::Pq])
                else {
                    return self.boolean(0);
                };
                let n = v.name;
                let last = match v.k {
                    K::Vec | K::Deque | K::Str => format!("{n}.back()"),
                    K::Queue => format!("{n}.front()"),
                    _ => format!("{n}.top()"),
                };
                let k = self.num(N::Int, d - 1);
                let op = *self.src.pick(&["<", ">", "==", "!="]);
                if self.src.chance(1, 2) {
                    format!("(!{n}.empty() && {last} {op} {})", k.s)
                } else {
                    format!("({n}.empty() || {last} {op} {})", k.s)
                }
            }
            4 => {
                let Some(v) = self.pick_var(&[K::Map, K::UMap, K::Set, K::USet, K::Vec, K::Str])
                else {
                    return self.boolean(0);
                };
                let n = v.name;
                let k = self.num(N::Int, d - 1);
                match v.k {
                    K::Vec => format!("{n}.empty()"),
                    K::Str => {
                        let c = self.char_expr(1);
                        format!("({n}.find({c}) != std::string::npos)")
                    }
                    K::Map | K::UMap if self.src.chance(1, 2) => {
                        format!("({n}.find({}) != {n}.end())", k.s)
                    }
                    _ => format!("({n}.count({}) > 0)", k.s),
                }
            }
            5 => {
                let a = self.str_expr(1).0;
                let b = self.str_expr(1).0;
                let op = *self.src.pick(&["==", "!=", "<", ">", "<=", ">="]);
                format!("({a} {op} {b})")
            }
            6 => {
                let a = self.char_expr(1);
                let b = self.char_expr(1);
                let op = *self.src.pick(&["==", "!=", "<", ">", "<=", ">="]);
                format!("({a} {op} {b})")
            }
            _ => {
                let a = self.boolean(d - 1);
                let b = self.num(N::Int, d - 1);
                let b = self.fit_int(b);
                format!("(({a}) + {} > 1)", b.s)
            }
        }
    }

    /// A `char` in `32..=126`.
    pub fn char_expr(&mut self, d: u32) -> String {
        match self.src.weighted(&[3, 3, 3, 2, 1]) {
            0 => match self.pick_var(&[K::Char]) {
                Some(v) => v.name,
                None => "'a'".into(),
            },
            1 => {
                let c = *self.src.pick(b"abcz07AZ ");
                format!("'{}'", c as char)
            }
            2 => {
                let e = self.num(N::Int, d);
                let e = self.fit_int(e);
                format!("static_cast<char>('a' + ({} % 26 + 26) % 26)", e.s)
            }
            3 => match self.pick_var(&[K::Str]) {
                Some(v) => {
                    let n = v.name;
                    let e = self.num(N::Int, d.saturating_sub(1));
                    let e = self.fit_int(e);
                    format!(
                        "({n}.empty() ? 'a' : {n}[static_cast<size_t>({}) % {n}.size()])",
                        e.s
                    )
                }
                None => "'q'".into(),
            },
            _ => {
                let c = self.char_expr(d.saturating_sub(1));
                if self.src.chance(1, 2) {
                    format!("static_cast<char>(std::toupper({c}))")
                } else {
                    format!("static_cast<char>(std::tolower({c}))")
                }
            }
        }
    }

    /// A `std::string` expression and an upper bound of its length.
    pub fn str_expr(&mut self, d: u32) -> (String, i64) {
        let w = if d == 0 {
            self.src.weighted(&[3, 2, 0, 0, 0, 0, 0])
        } else {
            self.src.weighted(&[3, 2, 2, 2, 1, 2, 1])
        };
        match w {
            0 => match self.pick_var(&[K::Str]) {
                Some(v) => (v.name, v.len),
                None => ("std::string(\"ab\")".into(), 2),
            },
            1 => {
                let w = *self
                    .src
                    .pick(&["", "a", "abc", "hello", "xyz", "banana", "level", "cab"]);
                (format!("std::string(\"{w}\")"), w.len() as i64)
            }
            2 => {
                let (a, la) = self.str_expr(d - 1);
                if self.src.chance(1, 2) {
                    let (b, lb) = self.str_expr(d - 1);
                    (format!("({a} + {b})"), (la + lb).min(LEN_CAP))
                } else {
                    let c = self.char_expr(d - 1);
                    (format!("({a} + {c})"), la + 1)
                }
            }
            3 => {
                let n = self.src.range(0, 4);
                let c = self.char_expr(d - 1);
                (format!("std::string({n}, {c})"), n)
            }
            4 => {
                let e = self.num(N::Int, d - 1);
                let e = self.fit_int(e);
                (format!("std::to_string({})", e.s), 7)
            }
            5 => {
                let (a, la) = self.str_expr(d - 1);
                let e = self.num(N::Int, d - 1);
                let e = self.fit_int(e);
                let tmp = format!("static_cast<size_t>({}) % ({a}.size() + 1)", e.s);
                if self.src.chance(1, 2) {
                    (format!("{a}.substr({tmp})"), la)
                } else {
                    let n = self.src.range(0, 5);
                    (format!("{a}.substr({tmp}, {n})"), n.min(la))
                }
            }
            _ => {
                let c = self.char_expr(d - 1);
                (format!("std::string(1, {c})"), 1)
            }
        }
    }

    /// A `std::vector<int>` expression and an upper bound of its length. Elements stay in `-KI..=KI`.
    pub fn vec_expr(&mut self, d: u32) -> (String, i64) {
        match self.src.weighted(&[3, 3, 2, 1]) {
            0 => match self.pick_var(&[K::Vec]) {
                Some(v) => (v.name, v.len),
                None => ("std::vector<int>()".into(), 0),
            },
            1 => {
                let n = self.src.range(1, 4) as usize;
                let items: Vec<String> = (0..n)
                    .map(|_| {
                        let e = self.num(N::Int, d.saturating_sub(1));
                        self.fit_int(e).s
                    })
                    .collect();
                (
                    format!("std::vector<int>{{{}}}", items.join(", ")),
                    n as i64,
                )
            }
            2 => {
                let n = self.src.range(0, 5);
                let e = self.num(N::Int, d.saturating_sub(1));
                let e = self.fit_int(e);
                (format!("std::vector<int>({n}, {})", e.s), n)
            }
            _ => ("std::vector<int>()".into(), 0),
        }
    }

    /// An argument for a helper parameter of kind `k`.
    pub fn arg_for(&mut self, k: K, d: u32) -> Option<String> {
        match k {
            K::Int | K::Ll => {
                let n = k.num().expect("numeric");
                let e = self.num(n, d);
                let v = Var::new("p".into(), k);
                Some(self.fit_var(e, &v).s)
            }
            K::Str => {
                let (s, l) = self.str_expr(d);
                (l <= LEN_CAP).then_some(s)
            }
            K::Vec => {
                let (s, l) = self.vec_expr(d);
                (l <= LEN_CAP).then_some(s)
            }
            _ => None,
        }
    }

    /// A call of a helper returning a number, or a call of a lambda (`None` when there is none).
    pub fn call_expr(&mut self, d: u32) -> Option<E> {
        let cands: Vec<Helper> = self
            .helpers
            .iter()
            .filter(|h| h.ret.is_some())
            .cloned()
            .collect();
        let nl = self.lambdas.len();
        if cands.is_empty() && nl == 0 {
            return None;
        }
        if nl > 0 && (cands.is_empty() || self.src.chance(1, 2)) {
            let (name, np) = self.lambdas[self.src.below(nl)].clone();
            let mut args = Vec::new();
            for _ in 0..np {
                let e = self.num(N::Int, d);
                args.push(self.fit_int(e).s);
            }
            return Some(E::new(
                format!("{name}({})", args.join(", ")),
                N::Int,
                -KI,
                KI,
            ));
        }
        let h = cands[self.src.below(cands.len())].clone();
        let mut args = Vec::new();
        for (i, k) in h.params.iter().enumerate() {
            if let (0, Some(mask)) = (i, h.rec_mask) {
                let e = self.num(N::Int, d);
                args.push(format!("({} & {mask})", e.s));
            } else {
                args.push(self.arg_for(*k, d)?);
            }
        }
        let r = h.ret.expect("returns");
        let n = r.num().expect("numeric");
        let (lo, hi) = if n == N::Int { (-KI, KI) } else { (-KL, KL) };
        Some(E::new(
            format!("{}({})", h.name, args.join(", ")),
            n,
            lo,
            hi,
        ))
    }
}
