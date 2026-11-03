// SPDX-License-Identifier: Apache-2.0
//! Strings, algorithms, container loops, bindings.

use super::expr::*;
use super::reads::LEN_CAP;

const CMPS: [&str; 4] = [
    "a > b",
    "a % 10 != b % 10 ? a % 10 < b % 10 : a < b",
    "std::abs(a) != std::abs(b) ? std::abs(a) < std::abs(b) : a < b",
    "(a & 1) != (b & 1) ? (a & 1) < (b & 1) : a > b",
];

impl Gen {
    pub fn str_op(&mut self, v: &Var, can: bool) {
        let n = v.name.clone();
        let m = self.mult;
        let c = self.char_expr(1);
        let pos = {
            let e = self.fit_int_text(1);
            format!("static_cast<size_t>({e}) % ({n}.size() + 1)")
        };
        let w = [
            3 * can as usize,
            2 * can as usize,
            2 * can as usize,
            2,
            2,
            2 * can as usize,
            2,
            1,
            1,
            can as usize,
            1,
            2,
        ];
        match self.src.weighted(&w) {
            0 => {
                self.line(&format!("{n} += {c};"));
                self.grow(&n, m);
            }
            1 => {
                self.line(&format!("{n}.push_back({c});"));
                self.grow(&n, m);
            }
            2 => {
                let (e, l) = self.str_expr(1);
                if l * m + v.len <= LEN_CAP {
                    self.line(&format!("{n} += {e};"));
                    self.grow(&n, l * m);
                }
            }
            3 => self.line(&format!("if (!{n}.empty()) {n}.pop_back();")),
            4 => {
                let i = format!("static_cast<size_t>({}) % {n}.size()", self.fit_int_text(1));
                self.line(&format!("if (!{n}.empty()) {n}[{i}] = {c};"));
            }
            5 => {
                let (e, l) = self.str_expr(1);
                if l * m + v.len <= LEN_CAP {
                    self.line(&format!("{n}.insert({pos}, {e});"));
                    self.grow(&n, l * m);
                }
            }
            6 => {
                let i = format!("static_cast<size_t>({}) % {n}.size()", self.fit_int_text(1));
                let k = self.src.range(0, 3);
                self.line(&format!("if (!{n}.empty()) {n}.erase({i}, {k});"));
            }
            7 => self.line(&format!("{n}.clear();")),
            8 => {
                let k = self.src.range(0, 6);
                self.line(&format!("{n}.resize({k}, 'z');"));
                if let Some(vv) = self.var_mut(&n) {
                    vv.len = vv.len.max(k);
                }
            }
            9 if super::known("append_n") => self.line(&format!("{n} += {c};")),
            9 => {
                let k = self.src.range(0, 3);
                self.line(&format!("{n}.append({k}, {c});"));
                self.grow(&n, k * m);
            }
            10 => self.line(&format!("if (!{n}.empty()) {n}.back() = {c};")),
            _ => {
                let (e, l) = self.str_expr(2);
                if l <= LEN_CAP {
                    self.line(&format!("{n} = {e};"));
                    if let Some(vv) = self.var_mut(&n) {
                        vv.len = vv.len.max(l);
                    }
                }
            }
        }
    }

    pub fn algo(&mut self) {
        let Some(v) = self.pick_var(&[K::Vec, K::Vec, K::Vec, K::Str, K::Deque]) else {
            return self.decl_container();
        };
        if v.frozen {
            return self.assign_scalar();
        }
        let n = v.name.clone();
        let (b, e) = (format!("{n}.begin()"), format!("{n}.end()"));
        if v.k == K::Str {
            let c = self.char_expr(1);
            match self.src.below(4) {
                0 => self.line(&format!("std::sort({b}, {e});")),
                1 => self.line(&format!("std::reverse({b}, {e});")),
                2 => self.line(&format!("{n}.erase(std::remove({b}, {e}, {c}), {e});")),
                _ => self.line(&format!("{n}.erase(std::unique({b}, {e}), {e});")),
            }
            return;
        }
        if v.k == K::Deque {
            match self.src.below(2) {
                0 => self.line(&format!("std::sort({b}, {e});")),
                _ => self.line(&format!("std::reverse({b}, {e});")),
            }
            return;
        }
        let cmp = *self.src.pick(&CMPS);
        let k = self.fit_int_text(1);
        let off = format!("static_cast<size_t>({k}) % ({n}.size() + 1)");
        match self.src.below(17) {
            0 => self.line(&format!("std::sort({b}, {e});")),
            1 => self.line(&format!("std::sort({b}, {e}, std::greater<int>());")),
            2 => self.line(&format!(
                "std::sort({b}, {e}, [](int a, int b) {{ return {cmp}; }});"
            )),
            3 => self.line(&format!(
                "std::stable_sort({b}, {e}, [](int a, int b) {{ return a % 7 < b % 7; }});"
            )),
            4 => self.line(&format!("std::reverse({b}, {e});")),
            5 => {
                self.line(&format!("std::sort({b}, {e});"));
                self.line(&format!("{n}.erase(std::unique({b}, {e}), {e});"));
            }
            6 => {
                let s = self.src.range(-100, 100);
                self.line(&format!("std::iota({b}, {e}, {s});"));
            }
            7 => self.line(&format!("std::fill({b}, {e}, {k});")),
            8 => self.line(&format!(
                "std::rotate({b}, {b} + static_cast<int>({off}), {e});"
            )),
            9 => {
                let f = if self.src.chance(1, 2) {
                    "next_permutation"
                } else {
                    "prev_permutation"
                };
                self.line(&format!("std::{f}({b}, {e});"));
            }
            10 => self.line(&format!("{n}.erase(std::remove({b}, {e}, {k}), {e});")),
            11 => self.line(&format!(
                "{n}.erase(std::remove_if({b}, {e}, [](int a) {{ return a % 3 == 0; }}), {e});"
            )),
            12 if super::known("partial_sum") => self.line(&format!("std::reverse({b}, {e});")),
            12 => {
                self.line(&format!("std::partial_sum({b}, {e}, {b});"));
                self.line(&format!("for (int& q : {n}) q %= {};", KI + 1));
            }
            13 if super::known("for_each") => self.line(&format!("std::reverse({b}, {e});")),
            13 => self.line(&format!(
                "std::for_each({b}, {e}, [](int& a) {{ a = a % 100; }});"
            )),
            14 => {
                // searches on a sorted copy
                let t = self.fresh("t");
                self.line(&format!("std::vector<int> {t} = {n};"));
                self.line(&format!("std::sort({t}.begin(), {t}.end());"));
                let f = *self.src.pick(&["lower_bound", "upper_bound"]);
                let q = self.fresh("x");
                let kk = self.fit_int_text(1);
                self.line(&format!(
                    "int {q} = static_cast<int>(std::distance({t}.begin(), std::{f}({t}.begin(), {t}.end(), {kk})));"
                ));
                let mut nv = Var::new(q, K::Int);
                nv.lo = 0;
                nv.hi = LEN_CAP as i128;
                self.add(nv);
                let q2 = self.fresh("b");
                self.line(&format!(
                    "bool {q2} = std::binary_search({t}.begin(), {t}.end(), {kk});"
                ));
                self.add(Var::new(q2, K::Bool));
            }
            15 => {
                let q = self.fresh("x");
                let f = *self.src.pick(&["count_if", "all_of", "any_of", "none_of"]);
                let ty = if f == "count_if" { "int" } else { "bool" };
                let cast = if f == "count_if" {
                    "static_cast<int>"
                } else {
                    ""
                };
                self.line(&format!(
                    "{ty} {q} = {cast}(std::{f}({b}, {e}, [](int a) {{ return a % 2 == 0; }}));"
                ));
                let mut nv = Var::new(q, if f == "count_if" { K::Int } else { K::Bool });
                nv.lo = 0;
                nv.hi = LEN_CAP as i128;
                self.add(nv);
            }
            _ => self.line(&format!(
                "if (!{n}.empty()) std::sort({b}, {b} + static_cast<int>({off}));"
            )),
        }
    }
}
