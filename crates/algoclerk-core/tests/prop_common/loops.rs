// SPDX-License-Identifier: Apache-2.0
//! Loops over containers, structured bindings, pairs.

use super::expr::*;
use super::reads::LEN_CAP;

impl Gen {
    fn set_frozen(&mut self, name: &str, on: bool) -> bool {
        match self.var_mut(name) {
            Some(v) => std::mem::replace(&mut v.frozen, on),
            None => false,
        }
    }

    fn ro(&mut self, name: String, k: K, lo: i128, hi: i128, capturable: bool) {
        let mut v = Var::new(name, k);
        v.lo = lo;
        v.hi = hi;
        v.rw = false;
        v.capturable = capturable;
        self.add(v);
    }

    pub fn range_for(&mut self, d: u32) {
        let all = [K::Vec, K::Vec, K::Str, K::Map, K::Set, K::Deque, K::Grid];
        let Some(v) = self.pick_var(&all) else {
            return self.decl_container();
        };
        let n = v.name.clone();
        let was = self.set_frozen(&n, true);
        let body = 1 + self.src.below(3);
        self.push_scope();
        let times = v.len.clamp(1, LEN_CAP);
        let x = self.fresh("e");
        match v.k {
            K::Vec | K::Deque | K::Set => {
                let mut form = if v.k == K::Set { self.src.below(2) } else { self.src.below(5) };
                if form == 3 && !v.rw {
                    form = 2;
                }
                match form {
                    0 => {
                        self.line(&format!("for (int {x} : {n}) {{"));
                        let mut ev = Var::new(x, K::Int);
                        ev.capturable = true;
                        self.add(ev);
                    }
                    1 => {
                        self.line(&format!("for (const auto& {x} : {n}) {{"));
                        self.ro(x, K::Int, -KI, KI, true);
                    }
                    2 => {
                        self.line(&format!("for (auto {x} : {n}) {{"));
                        self.add(Var::new(x, K::Int));
                    }
                    3 => {
                        self.line(&format!("for (auto& {x} : {n}) {{"));
                        self.add(Var::new(x, K::Int));
                    }
                    _ => {
                        let i = self.fresh("i");
                        let big = self.src.chance(1, 2);
                        if big && v.k != K::Set {
                            self.line(&format!("for (size_t {i} = 0; {i} < {n}.size(); ++{i}) {{"));
                            self.ro(format!("static_cast<unsigned long long>({i})"), K::Ull, 0, v.len as i128, false);
                        } else if v.k != K::Set {
                            self.line(&format!("for (int {i} = 0; {i} < static_cast<int>({n}.size()); ++{i}) {{"));
                            self.ro(i.clone(), K::Int, 0, v.len as i128, true);
                        } else {
                            self.line(&format!("for (auto it = {n}.begin(); it != {n}.end(); ++it) {{"));
                            self.ro("*it".into(), K::Int, -KI, KI, false);
                        }
                        if v.k != K::Set {
                            self.ro(format!("{n}[{i}]"), K::Int, -KI, KI, false);
                        }
                    }
                }
            }
            K::Str => {
                if self.src.chance(1, 2) || !v.rw {
                    self.line(&format!("for (char {x} : {n}) {{"));
                    self.ro(x, K::Char, 32, 126, true);
                } else {
                    self.line(&format!("for (auto& {x} : {n}) {{"));
                    self.add(Var::new(x, K::Char));
                }
            }
            K::Map => {
                let (kk, vv) = (self.fresh("k"), self.fresh("val"));
                match self.src.below(3) {
                    0 | 1 => {
                        let c = if self.src.chance(1, 2) { "const auto&" } else { "auto&" };
                        self.line(&format!("for ({c} [{kk}, {vv}] : {n}) {{"));
                        self.ro(kk, K::Int, -KI, KI, false);
                        self.ro(vv, K::Int, -KI, KI, false);
                    }
                    _ => {
                        self.line(&format!("for (const auto& {x} : {n}) {{"));
                        self.ro(format!("{x}.first"), K::Int, -KI, KI, false);
                        self.ro(format!("{x}.second"), K::Int, -KI, KI, false);
                    }
                }
            }
            _ => {
                // grid: rows are frozen vectors
                let times = v.rows;
                self.line(&format!("for (const auto& {x} : {n}) {{"));
                let mut rv = Var::new(x, K::Vec);
                rv.len = v.cols;
                rv.frozen = true;
                rv.rw = false;
                self.add(rv);
                self.in_loop_with(times, |g| {
                    g.indent += 1;
                    for _ in 0..body {
                        g.stmt(d - 1);
                    }
                    g.indent -= 1;
                });
                self.line("}");
                self.pop_scope();
                self.set_frozen(&n, was);
                return;
            }
        }
        self.in_loop_with(times, |g| {
            g.indent += 1;
            for _ in 0..body {
                g.stmt(d - 1);
            }
            g.indent -= 1;
        });
        self.line("}");
        self.pop_scope();
        self.set_frozen(&n, was);
    }

    /// `auto [a, b] = std::make_pair(..)`, tuples, `std::pair` variables, vectors of pairs.
    pub fn bindings(&mut self) {
        match self.src.below(4) {
            0 => {
                let (a, b) = (self.fresh("pa"), self.fresh("pb"));
                let ta = *self.src.pick(&ALL_N);
                let tb = *self.src.pick(&ALL_N);
                let ea = self.num(ta, 2);
                let eb = self.num(tb, 2);
                self.line(&format!("auto [{a}, {b}] = std::make_pair({}, {});", ea.s, eb.s));
                self.ro(a, K::of(ta), ea.lo, ea.hi, false);
                self.ro(b, K::of(tb), eb.lo, eb.hi, false);
            }
            1 => {
                let names = [self.fresh("ta"), self.fresh("tb"), self.fresh("tc")];
                let ts = [*self.src.pick(&ALL_N), *self.src.pick(&ALL_N), *self.src.pick(&ALL_N)];
                let es: Vec<E> = ts.iter().map(|t| self.num(*t, 2)).collect();
                let txt: Vec<&str> = es.iter().map(|e| e.s.as_str()).collect();
                self.line(&format!("auto [{}] = std::make_tuple({});", names.join(", "), txt.join(", ")));
                for i in 0..3 {
                    self.ro(names[i].clone(), K::of(ts[i]), es[i].lo, es[i].hi, false);
                }
            }
            2 => {
                let p = self.fresh("pr");
                let a = self.fit_int_text(2);
                let b = self.fit_int_text(2);
                self.line(&format!("std::pair<int, int> {p} = {{{a}, {b}}};"));
                for f in ["first", "second"] {
                    let mut v = Var::new(format!("{p}.{f}"), K::Int);
                    v.capturable = false;
                    self.add(v);
                }
            }
            _ => self.pair_block(),
        }
    }

    /// A local vector of pairs: fill, sort, fold with a structured binding.
    fn pair_block(&mut self) {
        let k = self.src.range(1, 5);
        let (pv, i, acc, a, b) = (self.fresh("pv"), self.fresh("i"), self.fresh("acc"), self.fresh("a"), self.fresh("b"));
        self.line("{");
        self.indent += 1;
        self.push_scope();
        self.line(&format!("std::vector<std::pair<int, int>> {pv};"));
        self.line(&format!("for (int {i} = 0; {i} < {k}; ++{i}) {{"));
        self.push_scope();
        self.ro(i.clone(), K::Int, 0, k as i128, true);
        let (x, y) = (self.fit_int_text(2), self.fit_int_text(2));
        self.pop_scope();
        self.line(&format!("    {pv}.push_back({{{x}, {y}}});"));
        self.line("}");
        match self.src.below(3) {
            0 => self.line(&format!("std::sort({pv}.begin(), {pv}.end());")),
            1 => self.line(&format!(
                "std::sort({pv}.begin(), {pv}.end(), [](const std::pair<int, int>& p, const std::pair<int, int>& q) {{ return p.second != q.second ? p.second < q.second : p.first > q.first; }});"
            )),
            _ => self.line(&format!("std::sort({pv}.begin(), {pv}.end(), std::greater<std::pair<int, int>>());")),
        }
        self.line(&format!("long long {acc} = 0;"));
        self.line(&format!("for (const auto& [{a}, {b}] : {pv}) {acc} = ({acc} * 31 + {a} * 7 + {b}) % 1000003;"));
        let target = format!("static_cast<int>({acc} % {})", KI + 1);
        self.assign_or_decl(&target, N::Int);
        self.pop_scope_keep_outer_effects();
        self.indent -= 1;
        self.line("}");
    }

    fn pop_scope_keep_outer_effects(&mut self) {
        self.pop_scope();
    }

    /// Store `rhs` (an `int` or `long long` of a small range) into an existing variable or a new one.
    pub fn assign_or_decl(&mut self, rhs: &str, n: N) {
        let k = K::of(n);
        let vs: Vec<Var> = self.find_vars(&[k]).into_iter().filter(|v| v.rw).collect();
        if vs.is_empty() {
            let name = self.fresh("x");
            self.line(&format!("{} {name} = {rhs};", n.name()));
            self.add(Var::new(name, k));
        } else {
            let v = vs[self.src.below(vs.len())].clone();
            self.line(&format!("{} = {rhs};", v.name));
        }
    }

    /// Order-independent folds over unordered containers.
    pub fn unordered_iter(&mut self) {
        let Some(v) = self.pick_var(&[K::UMap, K::USet]) else {
            return self.decl_container();
        };
        let acc = self.fresh("acc");
        let n = &v.name;
        self.line("{");
        self.indent += 1;
        self.push_scope();
        self.line(&format!("long long {acc} = 0;"));
        if v.k == K::UMap {
            let (k, x) = (self.fresh("k"), self.fresh("val"));
            let body = match self.src.below(4) {
                0 => format!("{acc} += static_cast<long long>({k}) * {x};"),
                1 => format!("if ({x} > {k}) ++{acc};"),
                2 => format!("{acc} = std::max({acc}, static_cast<long long>({k}) + {x});"),
                _ => format!("{acc} += {k} ^ {x};"),
            };
            self.line(&format!("for (const auto& [{k}, {x}] : {n}) {body}"));
        } else {
            let x = self.fresh("e");
            let body = match self.src.below(3) {
                0 => format!("{acc} += {x};"),
                1 => format!("if ({x} % 2 == 0) ++{acc};"),
                _ => format!("{acc} = std::min({acc}, static_cast<long long>({x}));"),
            };
            self.line(&format!("for (int {x} : {n}) {body}"));
        }
        let target = format!("static_cast<int>({acc} % {})", KI + 1);
        self.assign_or_decl(&target, N::Int);
        self.pop_scope_keep_outer_effects();
        self.indent -= 1;
        self.line("}");
    }

    /// `while (!c.empty()) { auto t = c.top(); c.pop(); ... }`
    pub fn drain_loop(&mut self, d: u32) {
        let Some(v) = self.pick_var(&[K::Stack, K::Queue, K::Deque, K::Pq]) else {
            return self.decl_container();
        };
        if v.frozen {
            return self.assign_scalar();
        }
        let n = v.name.clone();
        let t = self.fresh("t");
        let (get, pop) = match v.k {
            K::Stack | K::Pq => (format!("{n}.top()"), format!("{n}.pop()")),
            K::Queue => (format!("{n}.front()"), format!("{n}.pop()")),
            _ if self.src.chance(1, 2) => (format!("{n}.front()"), format!("{n}.pop_front()")),
            _ => (format!("{n}.back()"), format!("{n}.pop_back()")),
        };
        let was = self.set_frozen(&n, true);
        self.line(&format!("while (!{n}.empty()) {{"));
        self.line(&format!("    int {t} = {get};"));
        self.line(&format!("    {pop};"));
        self.push_scope();
        self.ro(t, K::Int, -KI, KI, true);
        let body = 1 + self.src.below(2);
        let times = v.len.clamp(1, LEN_CAP);
        self.in_loop_with(times, |g| {
            g.indent += 1;
            for _ in 0..body {
                g.stmt(d - 1);
            }
            g.indent -= 1;
        });
        self.pop_scope();
        self.line("}");
        self.set_frozen(&n, was);
    }
}
