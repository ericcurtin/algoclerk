// SPDX-License-Identifier: Apache-2.0
//! Statement generation: scalars, control flow.

use super::expr::*;
use super::reads::LEN_CAP;

pub fn kind_type(k: K) -> &'static str {
    match k {
        K::Int => "int",
        K::U => "unsigned",
        K::Ll => "long long",
        K::Ull => "unsigned long long",
        K::Char => "char",
        K::Bool => "bool",
        _ => "?",
    }
}

fn prefix(k: K) -> &'static str {
    match k {
        K::Int => "x",
        K::U => "u",
        K::Ll => "l",
        K::Ull => "w",
        K::Char => "c",
        K::Bool => "b",
        _ => "v",
    }
}

impl Gen {
    /// Statements in a new scope, each one `{ ... }`-free (the caller writes the braces).
    pub fn block(&mut self, n: usize, d: u32) {
        self.push_scope();
        self.indent += 1;
        for _ in 0..n {
            self.stmt(d);
        }
        self.indent -= 1;
        self.pop_scope();
    }

    pub fn stmt(&mut self, d: u32) {
        if self.budget <= 0 {
            return;
        }
        self.budget -= 1;
        let nest = if d > 0 && self.loops < 3 { 1 } else { 0 };
        let w = [
            3,          // 0 declare scalar
            5,          // 1 declare container
            5,          // 2 assign scalar
            3,          // 3 compound assign
            1,          // 4 inc/dec
            8,          // 5 container operation
            4,          // 6 algorithm
            3 * nest,   // 7 if
            3 * nest,   // 8 counting for
            3 * nest,   // 9 range for
            nest,       // 10 while / do-while
            nest,       // 11 switch
            if self.in_loop { 2 } else { 0 }, // 12 break / continue
            2,          // 13 lambda
            2,          // 14 structured binding / pairs
            if self.early_return.is_some() { 1 } else { 0 }, // 15
            2,          // 16 swap
            2,          // 17 unordered iteration
            3 * nest,   // 18 drain loop
            if self.void_helpers { 2 } else { 0 }, // 19 void helper call
        ];
        match self.src.weighted(&w) {
            0 => self.decl_scalar(),
            1 => self.decl_container(),
            2 => self.assign_scalar(),
            3 => self.compound(),
            4 => self.incdec(),
            5 => self.container_op(),
            6 => self.algo(),
            7 => self.if_stmt(d),
            8 => self.for_count(d),
            9 => self.range_for(d),
            10 => self.while_stmt(d),
            11 => self.switch_stmt(d),
            12 => {
                let c = self.boolean(1);
                let kw = if self.src.chance(1, 2) { "break" } else { "continue" };
                self.line(&format!("if ({c}) {kw};"));
            }
            13 => self.lambda_decl(),
            14 => self.bindings(),
            15 => {
                let k = self.early_return.expect("early return");
                let c = self.boolean(1);
                let e = self.num(k.num().expect("numeric"), 2);
                let v = Var::new("r".into(), k);
                let e = self.fit_var(e, &v);
                self.line(&format!("if ({c}) return {};", e.s));
            }
            16 => self.swap_stmt(),
            17 => self.unordered_iter(),
            18 => self.drain_loop(d),
            _ => self.void_call(),
        }
    }

    /// An assignable scalar variable that is not read-only.
    fn rw_var(&mut self, ks: &[K]) -> Option<Var> {
        let vs: Vec<Var> = self.find_vars(ks).into_iter().filter(|v| v.rw).collect();
        if vs.is_empty() { None } else { Some(vs[self.src.below(vs.len())].clone()) }
    }

    pub fn decl_scalar(&mut self) {
        let k = *self.src.pick(&[K::Int, K::Int, K::Int, K::Ll, K::Ll, K::U, K::Ull, K::Char, K::Bool]);
        let name = self.fresh(prefix(k));
        let mut v = Var::new(name.clone(), k);
        match k {
            K::Char => {
                let c = self.char_expr(2);
                self.line(&format!("char {name} = {c};"));
            }
            K::Bool => {
                let c = self.boolean(2);
                self.line(&format!("bool {name} = {c};"));
            }
            _ => {
                let n = k.num().expect("numeric");
                let from = if self.src.chance(1, 4) { *self.src.pick(&ALL_N) } else { n };
                let e = self.num(from, 3);
                let e = self.fit_var(e, &v);
                let e = if n.signed() && (e.lo < v.lo || e.hi > v.hi) { self.reduce(e, v.hi + 1) } else { e };
                if self.src.chance(1, 5) && e.t == n {
                    self.line(&format!("auto {name} = {};", e.s));
                } else if self.src.chance(1, 8) && n == N::Int {
                    v.rw = false;
                    self.line(&format!("const int {name} = {};", e.s));
                } else {
                    self.line(&format!("{} {name} = {};", n.name(), e.s));
                }
            }
        }
        self.add(v);
    }

    pub fn assign_scalar(&mut self) {
        let Some(v) = self.rw_var(&[K::Int, K::U, K::Ll, K::Ull, K::Char, K::Bool]) else {
            return self.decl_scalar();
        };
        let rhs = match v.k {
            K::Char => self.char_expr(2),
            K::Bool => self.boolean(2),
            k => {
                let n = k.num().expect("numeric");
                let from = if self.src.chance(1, 4) { *self.src.pick(&ALL_N) } else { n };
                let e = self.num(from, 3);
                self.fit_var(e, &v).s
            }
        };
        self.line(&format!("{} = {rhs};", v.name));
    }

    pub fn compound(&mut self) {
        let Some(v) = self.rw_var(&[K::Int, K::U, K::Ll, K::Ull]) else {
            return self.decl_scalar();
        };
        let n = v.k.num().expect("numeric");
        let name = v.name.clone();
        if !n.signed() {
            let from = *self.src.pick(&ALL_N);
            let e = self.num(from, 2);
            let mask = if n.bits() == 32 { 31 } else { 63 };
            let (op, rhs) = match self.src.below(9) {
                0 => ("+=", e.s),
                1 => ("-=", e.s),
                2 => ("*=", e.s),
                3 => ("^=", e.s),
                4 => ("|=", e.s),
                5 => ("&=", e.s),
                6 => ("<<=", format!("({} & {mask})", e.s)),
                7 => (">>=", format!("({} & {mask})", e.s)),
                _ => (if self.src.chance(1, 2) { "/=" } else { "%=" }, format!("({} | 1)", e.s)),
            };
            return self.line(&format!("{name} {op} {rhs};"));
        }
        let m = v.hi + 1;
        let e = self.num(n, 2);
        match self.src.below(8) {
            0 | 1 => {
                let r = self.reduce(e, 17);
                let op = if self.src.chance(1, 2) { "+=" } else { "-=" };
                self.line(&format!("{name} {op} {};", r.s));
                self.line(&format!("{name} %= {m};"));
            }
            2 => {
                let r = self.reduce(e, 5);
                self.line(&format!("{name} *= {};", r.s));
            }
            3 => {
                let r = self.fit_int(e);
                let op = *self.src.pick(&["^=", "|=", "&="]);
                self.line(&format!("{name} {op} {};", r.s));
                self.line(&format!("{name} %= {m};"));
            }
            4 => {
                let d = self.divisor_text(n);
                let op = if self.src.chance(1, 2) { "/=" } else { "%=" };
                self.line(&format!("{name} {op} {d};"));
            }
            5 => self.line(&format!("{name} >>= ({} & 31);", e.s)),
            6 => {
                let r = self.reduce(e, 17);
                self.line(&format!("{name} = std::max({name}, {});", r.s));
            }
            _ => {
                let r = self.reduce(e, 9);
                self.line(&format!("{name} = {name} * 2 - {};", r.s));
                self.line(&format!("{name} %= {m};"));
            }
        }
    }

    fn divisor_text(&mut self, n: N) -> String {
        let b = self.num(n, 1);
        if self.src.chance(1, 3) { format!("({} % 7 - 8)", b.s) } else { format!("({} % 7 + 8)", b.s) }
    }

    pub fn incdec(&mut self) {
        let Some(v) = self.rw_var(&[K::Int, K::U, K::Ll, K::Ull]) else {
            return self.decl_scalar();
        };
        let op = *self.src.pick(&["++", "--"]);
        let name = v.name.clone();
        if self.src.chance(1, 2) {
            self.line(&format!("{op}{name};"));
        } else {
            self.line(&format!("{name}{op};"));
        }
        if v.k.num().is_some_and(|n| n.signed()) && self.mult > 1 {
            self.line(&format!("{name} %= {};", v.hi + 1));
        }
    }

    pub fn swap_stmt(&mut self) {
        let k = *self.src.pick(&[K::Int, K::Ll, K::U, K::Char]);
        let vs: Vec<Var> = self.find_vars(&[k]).into_iter().filter(|v| v.rw).collect();
        if vs.len() < 2 {
            return self.decl_scalar();
        }
        let a = self.src.below(vs.len());
        let b = self.src.below(vs.len());
        self.line(&format!("std::swap({}, {});", vs[a].name, vs[b].name));
    }

    pub fn lambda_decl(&mut self) {
        let name = self.fresh("lam");
        let np = self.src.below(3) + 1;
        let generic = self.src.chance(1, 3);
        let caps: Vec<Var> = self
            .find_vars(&[K::Int, K::Ll])
            .into_iter()
            .filter(|v| v.capturable)
            .collect();
        let mut sc = Vec::new();
        let mut params = Vec::new();
        for i in 0..np {
            let pn = format!("{}{i}", ["p", "q", "r"][i % 3]);
            params.push(format!("{} {pn}", if generic { "auto" } else { "int" }));
            let mut v = Var::new(pn, K::Int);
            v.rw = false;
            sc.push(v);
        }
        if !generic {
            sc.extend(caps);
        }
        let saved = std::mem::replace(&mut self.scopes, vec![sc]);
        let saved_l = std::mem::take(&mut self.lambdas);
        let saved_h = std::mem::take(&mut self.helpers);
        let e = self.num(N::Int, 3);
        let e = self.fit_int(e);
        self.scopes = saved;
        self.lambdas = saved_l;
        self.helpers = saved_h;
        let cap = if generic { "[]" } else if self.src.chance(1, 2) { "[&]" } else { "[=]" };
        self.line(&format!("auto {name} = {cap}({}) {{ return {}; }};", params.join(", "), e.s));
        self.lambdas.push((name, np));
    }

    fn scoped_var(&mut self, mut v: Var, rw: bool) -> Var {
        v.rw = rw;
        v
    }

    pub fn if_stmt(&mut self, d: u32) {
        let mut head = format!("if ({})", self.boolean(2));
        let mut frozen_map: Option<(String, bool)> = None;
        let mut then_var: Option<Var> = None;
        self.push_scope();
        if self.src.chance(1, 4) {
            // C++17 init statement
            let name = self.fresh("t");
            let e = self.num(N::Int, 2);
            let e = self.fit_int(e);
            let v = self.scoped_var(Var::new(name.clone(), K::Int), false);
            self.add(v);
            let op = *self.src.pick(&["<", ">", "!=", "=="]);
            let k = self.lit_int(-5, 5);
            head = format!("if (int {name} = {}; {name} {op} {})", e.s, k.s);
        } else if let Some(m) = self.pick_var(&[K::Map]).filter(|_| self.src.chance(1, 4)) {
            let k = self.num(N::Int, 1);
            let name = self.fresh("it");
            let n = &m.name;
            head = format!("if (auto {name} = {n}.find({}); {name} != {n}.end())", k.s);
            frozen_map = Some((m.name.clone(), m.frozen));
            if let Some(v) = self.var_mut(&m.name) {
                v.frozen = true;
            }
            let mut v = Var::new(format!("{name}->second"), K::Int);
            v.rw = false;
            v.capturable = false;
            then_var = Some(v);
        }
        self.line(&format!("{head} {{"));
        let n = 1 + self.src.below(3);
        self.push_scope();
        if let Some(v) = then_var {
            // the iterator may only be read in the branch where it is known not to be end()
            self.add(v);
        }
        self.block(n, d - 1);
        self.pop_scope();
        if self.src.chance(1, 3) {
            self.line("} else if (false || (true && true)) {");
            let n = 1 + self.src.below(2);
            self.block(n, d - 1);
        }
        if self.src.chance(1, 2) {
            self.line("} else {");
            let n = 1 + self.src.below(2);
            self.block(n, d - 1);
        }
        self.line("}");
        self.pop_scope();
        if let Some((n, was)) = frozen_map {
            if let Some(v) = self.var_mut(&n) {
                v.frozen = was;
            }
        }
    }

    /// Run `f` with `mult` multiplied and `in_loop` set.
    pub fn in_loop_with(&mut self, times: i64, f: impl FnOnce(&mut Gen)) {
        let (m, l, il) = (self.mult, self.loops, self.in_loop);
        self.mult = m.saturating_mul(times.max(1)).min(1_000_000);
        self.loops = l + 1;
        self.in_loop = true;
        f(self);
        self.mult = m;
        self.loops = l;
        self.in_loop = il;
    }

    pub fn for_count(&mut self, d: u32) {
        let i = self.fresh("i");
        let k = self.src.range(1, 5);
        let mut iv = Var::new(i.clone(), K::Int);
        iv.rw = false;
        iv.lo = 0;
        iv.hi = k as i128;
        self.push_scope();
        let head = match self.src.below(5) {
            0 => {
                let j = self.fresh("j");
                let mut jv = Var::new(j.clone(), K::Int);
                jv.rw = false;
                jv.lo = 0;
                jv.hi = k as i128;
                self.add(jv);
                format!("for (int {i} = 0, {j} = {k}; {i} < {j}; ++{i}, --{j})")
            }
            1 => format!("for (int {i} = {k}; {i} > 0; --{i})"),
            2 => format!("for (int {i} = {}; {i} >= 0; {i}--)", k - 1),
            3 => format!("for (int {i} = 0; {i} < {}; {i} += 2)", k + 1),
            _ => format!("for (int {i} = 0; {i} < {k}; ++{i})"),
        };
        self.add(iv);
        self.line(&format!("{head} {{"));
        let n = 1 + self.src.below(3);
        self.in_loop_with(k, |g| g.block(n, d - 1));
        self.line("}");
        self.pop_scope();
    }

    pub fn while_stmt(&mut self, d: u32) {
        let w = self.fresh("w");
        let k = self.src.range(1, 4);
        let mut v = Var::new(w.clone(), K::Int);
        v.rw = false;
        v.lo = 0;
        v.hi = k as i128;
        self.push_scope();
        self.add(v);
        self.line(&format!("int {w} = 0;"));
        let n = 1 + self.src.below(3);
        if self.src.chance(1, 2) {
            self.line(&format!("while ({w} < {k}) {{"));
            self.line(&format!("    ++{w};"));
            self.in_loop_with(k, |g| g.block(n, d - 1));
            self.line("}");
        } else {
            self.line("do {");
            self.line(&format!("    ++{w};"));
            self.in_loop_with(k, |g| g.block(n, d - 1));
            self.line(&format!("}} while ({w} < {k});"));
        }
        self.pop_scope();
    }

    pub fn switch_stmt(&mut self, d: u32) {
        let on_char = self.src.chance(1, 4) && self.pick_var(&[K::Char]).is_some();
        let labels: Vec<String> = if on_char {
            let c = self.pick_var(&[K::Char]).expect("char").name;
            self.line(&format!("switch ({c}) {{"));
            vec!["'a'".into(), "'b'".into(), "'z'".into()]
        } else {
            let e = self.num(N::Int, 2);
            self.line(&format!("switch ({} & 3) {{", e.s));
            vec!["0".into(), "1".into(), "2".into(), "3".into()]
        };
        let ncase = 1 + self.src.below(labels.len());
        for l in labels.iter().take(ncase) {
            self.line(&format!("case {l}: {{"));
            let n = 1 + self.src.below(2);
            self.block(n, d - 1);
            if self.src.chance(3, 4) {
                self.line("    break;");
            }
            self.line("}");
        }
        if self.src.chance(2, 3) {
            self.line("default: {");
            let n = 1 + self.src.below(2);
            self.block(n, d - 1);
            self.line("}");
        }
        self.line("}");
    }

    /// Cap for container growth: true when `v` can take `extra` more elements.
    pub fn room(&self, v: &Var) -> bool {
        v.len.saturating_add(self.mult) <= LEN_CAP
    }
}
