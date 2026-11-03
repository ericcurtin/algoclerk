// SPDX-License-Identifier: Apache-2.0
//! Numeric expression generation (see `expr.rs` for the invariants).

use super::expr::*;

fn paren(s: &str) -> String {
    format!("({s})")
}

impl Gen {
    /// `e % m` with the interval of the remainder.
    pub fn reduce(&mut self, e: E, m: i128) -> E {
        let (lo, hi) = e.conv_iv(e.t);
        let s = paren(&format!("{} % {m}", e.s));
        let rlo = if lo < 0 { (-(m - 1)).max(lo) } else { 0 };
        let rhi = if hi > 0 { (m - 1).min(hi) } else { 0 };
        E::new(s, e.t, rlo, rhi)
    }

    /// A literal of type `t`.
    pub fn lit(&mut self, t: N) -> E {
        let table: &[i64] = &[0, 1, 2, 3, 5, 7, 10, 31, 100, 255, 1000, -1, -2, -7, -100];
        if self.src.chance(1, 24) {
            return match t {
                N::Int => E::new("2147483647".into(), t, i32::MAX as i128, i32::MAX as i128),
                N::Ll => E::new("9223372036854775807LL".into(), t, i64::MAX as i128, i64::MAX as i128),
                N::U => E::new("4294967295u".into(), t, u32::MAX as i128, u32::MAX as i128),
                N::Ull => E::new("18446744073709551615ull".into(), t, u64::MAX as i128, u64::MAX as i128),
            };
        }
        let mut v = *self.src.pick(table);
        if !t.signed() {
            v = v.abs();
        }
        let suffix = match t {
            N::Int => "",
            N::U => "u",
            N::Ll => "LL",
            N::Ull => "ull",
        };
        let s = if v < 0 { format!("(-{}{suffix})", -v) } else { format!("{v}{suffix}") };
        E::new(s, t, v as i128, v as i128)
    }

    /// A leaf of exactly type `t`: variable, literal, or a value read from a container.
    pub fn leaf(&mut self, t: N) -> E {
        let w = self.src.weighted(&[4, 3, 2, 2, 2]);
        match w {
            0 => {
                if let Some(v) = self.pick_var(&[K::of(t)]) {
                    return E::new(v.name.clone(), t, v.lo, v.hi);
                }
                self.lit(t)
            }
            1 => self.lit(t),
            2 if t == N::Int => self.int_read(),
            3 if t == N::Int => {
                if let Some(v) = self.pick_var(&[K::Char]) {
                    // integer promotion: char + 0 is an int
                    return E::new(format!("({} + 0)", v.name), t, v.lo, v.hi);
                }
                self.lit(t)
            }
            4 => match self.call_expr(1) {
                Some(e) if e.t == t => e,
                _ => self.lit(t),
            },
            _ => self.lit(t),
        }
    }

    /// A numeric expression of exactly type `t`.
    pub fn num(&mut self, t: N, d: u32) -> E {
        if d == 0 {
            return self.leaf(t);
        }
        match self.src.weighted(&[4, 8, 3, 3, 3, 2, 2, 3, 2, 2]) {
            0 => self.leaf(t),
            1 => self.arith(t, d),
            2 => self.divmod(t, d),
            3 => self.bitwise(t, d),
            4 => self.shift(t, d),
            5 => self.unary(t, d),
            6 => self.ternary(t, d),
            7 => self.cast_from_other(t, d),
            8 => self.minmax(t, d),
            _ => self.leaf(t),
        }
    }

    /// An operand whose type has rank at most that of `t`.
    fn operand(&mut self, t: N, d: u32) -> E {
        let ts: Vec<N> = ALL_N.iter().copied().filter(|x| (*x as usize) <= (t as usize)).collect();
        let pick = if self.src.chance(1, 3) { *self.src.pick(&ts) } else { t };
        self.num(pick, d.saturating_sub(1))
    }

    fn arith(&mut self, t: N, d: u32) -> E {
        let mut a = self.num(t, d - 1);
        let mut b = self.operand(t, d);
        let op = *self.src.pick(&['+', '-', '*']);
        for round in 0..2 {
            let (alo, ahi) = a.conv_iv(t);
            let (blo, bhi) = b.conv_iv(t);
            let (lo, hi) = match op {
                '+' => (alo + blo, ahi + bhi),
                '-' => (alo - bhi, ahi - blo),
                _ => {
                    let p = [
                        alo.saturating_mul(blo),
                        alo.saturating_mul(bhi),
                        ahi.saturating_mul(blo),
                        ahi.saturating_mul(bhi),
                    ];
                    (*p.iter().min().expect("p"), *p.iter().max().expect("p"))
                }
            };
            let s = paren(&format!("{} {op} {}", a.s, b.s));
            if lo >= t.min() && hi <= t.max() {
                return E::new(s, t, lo, hi);
            }
            if !t.signed() {
                return E::full(s, t);
            }
            if round == 0 {
                a = self.reduce(a, 17);
                b = self.reduce(b, 17);
            }
        }
        unreachable!("reduced operands always fit")
    }

    /// A divisor that is never zero (and never -1): `(b % 7 + 8)`, `(b % 7 - 8)` or `(b | 1)` for unsigned.
    fn divisor(&mut self, t: N, d: u32) -> E {
        if !t.signed() {
            let b = self.operand(t, d);
            let s = paren(&format!("{} | 1", b.s));
            return E::new(s, t, 1, t.max());
        }
        let b = self.num(t, d.saturating_sub(1));
        if self.src.chance(1, 3) {
            let s = paren(&format!("{} % 7 - 8", b.s));
            E::new(s, t, -14, -2)
        } else {
            let s = paren(&format!("{} % 7 + 8", b.s));
            E::new(s, t, 2, 14)
        }
    }

    fn divmod(&mut self, t: N, d: u32) -> E {
        let a = self.num(t, d - 1);
        let dv = self.divisor(t, d);
        let is_div = self.src.chance(1, 2);
        let s = paren(&format!("{} {} {}", a.s, if is_div { '/' } else { '%' }, dv.s));
        let (alo, ahi) = a.conv_iv(t);
        let m = ahi.abs().max(alo.abs());
        if is_div {
            if alo >= 0 && dv.lo > 0 { E::new(s, t, 0, ahi) } else { E::new(s, t, -m, m) }
        } else {
            let dm = dv.lo.abs().max(dv.hi.abs());
            let top = (dm - 1).min(m);
            E::new(s, t, if alo >= 0 { 0 } else { -top }, top)
        }
    }

    fn bitwise(&mut self, t: N, d: u32) -> E {
        let a = self.num(t, d - 1);
        let b = self.operand(t, d);
        let op = *self.src.pick(&['&', '|', '^']);
        let s = paren(&format!("{} {op} {}", a.s, b.s));
        let (alo, ahi) = a.conv_iv(t);
        let (blo, bhi) = b.conv_iv(t);
        if !t.signed() {
            return E::full(s, t);
        }
        let m = alo.abs().max(ahi.abs()).max(blo.abs()).max(bhi.abs());
        let p = ((m + 1) as u128).next_power_of_two() as i128;
        if alo >= 0 && blo >= 0 {
            E::new(s, t, 0, p - 1)
        } else {
            E::new(s, t, -p, p - 1)
        }
    }

    fn shift(&mut self, t: N, d: u32) -> E {
        let mask = if t.bits() == 32 { 31 } else { 63 };
        if t.signed() && self.src.chance(1, 2) {
            // ((a & M) << (b & C)) stays inside the type
            let a = self.num(t, d - 1);
            let b = self.num(N::Int, d - 1);
            let (mbits, cmax) = if t == N::Int { (8, 15) } else { (16, 31) };
            let m = (1i128 << mbits) - 1;
            let s = paren(&format!("({} & {m}) << ({} & {cmax})", a.s, b.s));
            return E::new(s, t, 0, m << cmax);
        }
        let a = self.num(t, d - 1);
        let b = self.num(N::Int, d - 1);
        let left = self.src.chance(1, 2) && !t.signed();
        let s = paren(&format!("{} {} ({} & {mask})", a.s, if left { "<<" } else { ">>" }, b.s));
        let (alo, ahi) = a.conv_iv(t);
        if left {
            E::full(s, t)
        } else {
            E::new(s, t, alo.min(0), ahi.max(0))
        }
    }

    fn unary(&mut self, t: N, d: u32) -> E {
        let mut a = self.num(t, d - 1);
        let neg = self.src.chance(1, 2);
        if t.signed() && neg && a.lo <= t.min() {
            a = self.reduce(a, 17);
        }
        let s = format!("({}{})", if neg { '-' } else { '~' }, a.s);
        let (lo, hi) = a.conv_iv(t);
        match (neg, t.signed()) {
            (true, true) => E::new(s, t, -hi, -lo),
            (false, true) => E::new(s, t, -hi - 1, -lo - 1),
            (false, false) => E::new(s, t, t.max() - hi, t.max() - lo),
            (true, false) => E::full(s, t),
        }
    }

    fn ternary(&mut self, t: N, d: u32) -> E {
        let c = self.boolean(d - 1);
        let a = self.num(t, d - 1);
        let b = self.num(t, d - 1);
        E::new(paren(&format!("{c} ? {} : {}", a.s, b.s)), t, a.lo.min(b.lo), a.hi.max(b.hi))
    }

    fn cast_from_other(&mut self, t: N, d: u32) -> E {
        let from = *self.src.pick(&ALL_N);
        let a = self.num(from, d - 1);
        let (lo, hi) = a.conv_iv(t);
        let s = match self.src.below(3) {
            0 => format!("static_cast<{}>({})", t.name(), a.s),
            1 => format!("(({}){})", t.name(), a.s),
            _ if t == N::Int || t == N::U => format!("{}({})", t.name(), a.s),
            _ => format!("static_cast<{}>({})", t.name(), a.s),
        };
        if from == t { E::new(s, t, a.lo, a.hi) } else { E::new(s, t, lo, hi) }
    }

    fn minmax(&mut self, t: N, d: u32) -> E {
        let a = self.num(t, d - 1);
        if t.signed() && self.src.chance(1, 4) && a.lo > t.min() {
            let (lo, hi) = (a.lo, a.hi);
            let m = lo.abs().max(hi.abs());
            let low = if lo <= 0 && hi >= 0 { 0 } else { lo.abs().min(hi.abs()) };
            return E::new(format!("std::abs({})", a.s), t, low, m);
        }
        let b = self.num(t, d - 1);
        let is_min = self.src.chance(1, 2);
        let s = format!("std::{}({}, {})", if is_min { "min" } else { "max" }, a.s, b.s);
        E::new(s, t, a.lo.min(b.lo), a.hi.max(b.hi))
    }
}
