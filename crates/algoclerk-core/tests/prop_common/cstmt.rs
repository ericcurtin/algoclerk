// SPDX-License-Identifier: Apache-2.0
//! Statements that declare and modify containers and strings.

use super::expr::*;
use super::reads::LEN_CAP;

const CONTAINERS: [K; 15] = [
    K::Vec,
    K::Vec,
    K::Vec,
    K::Str,
    K::Str,
    K::Grid,
    K::Map,
    K::UMap,
    K::Set,
    K::USet,
    K::Stack,
    K::Queue,
    K::Deque,
    K::List,
    K::Pq,
];

impl Gen {
    fn items(&mut self, n: usize) -> String {
        let v: Vec<String> = (0..n)
            .map(|_| {
                let e = self.num(N::Int, 2);
                self.fit_int(e).s
            })
            .collect();
        v.join(", ")
    }

    pub fn decl_container(&mut self) {
        let k = *self.src.pick(&CONTAINERS);
        let mut v;
        match k {
            K::Vec => {
                let name = self.fresh("v");
                v = Var::new(name.clone(), k);
                match self.src.below(5) {
                    0 => self.line(&format!("std::vector<int> {name};")),
                    1 => {
                        let n = self.src.range(1, 5) as usize;
                        let it = self.items(n);
                        v.len = n as i64;
                        self.line(&format!("std::vector<int> {name} = {{{it}}};"));
                    }
                    2 => {
                        let n = self.src.range(1, 5) as usize;
                        let it = self.items(n);
                        v.len = n as i64;
                        self.line(&format!("std::vector<int> {name}{{{it}}};"));
                    }
                    3 => {
                        let n = self.src.range(0, 6);
                        let e = self.items(1);
                        v.len = n;
                        self.line(&format!("std::vector<int> {name}({n}, {e});"));
                    }
                    _ => {
                        let (e, l) = self.vec_expr(1);
                        v.len = l;
                        self.line(&format!("std::vector<int> {name} = {e};"));
                    }
                }
            }
            K::Str => {
                let name = self.fresh("s");
                v = Var::new(name.clone(), k);
                match self.src.below(3) {
                    0 => self.line(&format!("std::string {name};")),
                    1 => {
                        let n = self.src.range(0, 4);
                        let c = self.char_expr(1);
                        v.len = n;
                        self.line(&format!("std::string {name}({n}, {c});"));
                    }
                    _ => {
                        let (e, l) = self.str_expr(2);
                        v.len = l;
                        self.line(&format!("std::string {name} = {e};"));
                    }
                }
            }
            K::Grid => {
                let name = self.fresh("g");
                v = Var::new(name.clone(), k);
                v.rows = self.src.range(1, 4);
                v.cols = self.src.range(1, 4);
                let e = self.src.range(-5, 5);
                v.len = v.rows;
                self.line(&format!(
                    "std::vector<std::vector<int>> {name}({}, std::vector<int>({}, {e}));",
                    v.rows, v.cols
                ));
            }
            K::Map | K::UMap => {
                let (ty, p) = if k == K::Map {
                    ("std::map", "m")
                } else {
                    ("std::unordered_map", "um")
                };
                let name = self.fresh(p);
                v = Var::new(name.clone(), k);
                if self.src.chance(1, 2) {
                    self.line(&format!("{ty}<int, int> {name};"));
                } else {
                    let n = self.src.range(1, 3);
                    let es: Vec<String> = (0..n)
                        .map(|_| {
                            format!("{{{}, {}}}", self.src.range(-9, 9), self.src.range(-99, 99))
                        })
                        .collect();
                    v.len = n;
                    self.line(&format!("{ty}<int, int> {name} = {{{}}};", es.join(", ")));
                }
            }
            K::Set | K::USet => {
                let (ty, p) = if k == K::Set {
                    ("std::set", "st")
                } else {
                    ("std::unordered_set", "us")
                };
                let name = self.fresh(p);
                v = Var::new(name.clone(), k);
                if self.src.chance(1, 2) {
                    self.line(&format!("{ty}<int> {name};"));
                } else {
                    let n = self.src.range(1, 4) as usize;
                    let it = self.items(n);
                    v.len = n as i64;
                    self.line(&format!("{ty}<int> {name} = {{{it}}};"));
                }
            }
            K::Stack => {
                let name = self.fresh("stk");
                v = Var::new(name.clone(), k);
                self.line(&format!("std::stack<int> {name};"));
            }
            K::Queue => {
                let name = self.fresh("q");
                v = Var::new(name.clone(), k);
                self.line(&format!("std::queue<int> {name};"));
            }
            K::List => {
                let name = self.fresh("ls");
                v = Var::new(name.clone(), k);
                if self.src.chance(1, 2) {
                    self.line(&format!("std::list<int> {name};"));
                } else {
                    let n = self.src.range(1, 4) as usize;
                    let it = self.items(n);
                    v.len = n as i64;
                    self.line(&format!("std::list<int> {name} = {{{it}}};"));
                }
            }
            K::Deque => {
                let name = self.fresh("dq");
                v = Var::new(name.clone(), k);
                if self.src.chance(1, 2) {
                    self.line(&format!("std::deque<int> {name};"));
                } else {
                    let n = self.src.range(1, 4) as usize;
                    let it = self.items(n);
                    v.len = n as i64;
                    self.line(&format!("std::deque<int> {name} = {{{it}}};"));
                }
            }
            _ => {
                let name = self.fresh("pq");
                v = Var::new(name.clone(), K::Pq);
                match self.src.below(4) {
                    0 => self.line(&format!("std::priority_queue<int> {name};")),
                    1 => self.line(&format!(
                        "std::priority_queue<int, std::vector<int>, std::greater<int>> {name};"
                    )),
                    2 => {
                        let cmp = self.fresh("cmp");
                        self.line(&format!(
                            "auto {cmp} = [](int a, int b) {{ return a > b; }};"
                        ));
                        self.line(&format!(
                            "std::priority_queue<int, std::vector<int>, decltype({cmp})> {name}({cmp});"
                        ));
                    }
                    _ => {
                        let cmp = self.fresh("cmp");
                        self.line(&format!(
                            "auto {cmp} = [](int a, int b) {{ return std::abs(a) != std::abs(b) ? std::abs(a) > std::abs(b) : a > b; }};"
                        ));
                        self.line(&format!(
                            "std::priority_queue<int, std::vector<int>, decltype({cmp})> {name}({cmp});"
                        ));
                    }
                }
            }
        }
        self.add(v);
    }

    pub fn grow(&mut self, name: &str, by: i64) {
        if let Some(v) = self.var_mut(name) {
            v.len += by;
        }
    }

    pub fn container_op(&mut self) {
        let all = [
            K::Vec,
            K::Vec,
            K::Vec,
            K::Str,
            K::Str,
            K::Grid,
            K::Map,
            K::UMap,
            K::Set,
            K::USet,
            K::Stack,
            K::Queue,
            K::Deque,
            K::List,
            K::Pq,
        ];
        let Some(v) = self.pick_var(&all) else {
            return self.decl_container();
        };
        if v.frozen {
            return self.assign_scalar();
        }
        if self.src.chance(1, 6) && !matches!(v.k, K::Grid | K::Pq) {
            return self.alias_op(v);
        }
        self.container_op_on(v);
    }

    /// Operate on a container through a reference alias: `{ T& al = v; ...ops on al... }`.
    fn alias_op(&mut self, v: Var) {
        let al = self.fresh("al");
        let ty = self.type_of(&v);
        self.line("{");
        self.indent += 1;
        self.push_scope();
        self.line(&format!("{ty}& {al} = {};", v.name));
        let mut a = v.clone();
        a.name = al.clone();
        a.capturable = false;
        self.add(a.clone());
        for _ in 0..1 + self.src.below(3) {
            self.container_op_on(a.clone());
            // later operations see the alias with its grown length
            if let Some(cur) = self.find_vars(&[a.k]).into_iter().find(|x| x.name == al) {
                a = cur;
            }
        }
        let grown = a.len;
        self.pop_scope();
        self.indent -= 1;
        self.line("}");
        if let Some(o) = self.var_mut(&v.name) {
            o.len = o.len.max(grown);
        }
    }

    fn type_of(&self, v: &Var) -> &'static str {
        match v.k {
            K::Vec => "std::vector<int>",
            K::Str => "std::string",
            K::Map => "std::map<int, int>",
            K::UMap => "std::unordered_map<int, int>",
            K::Set => "std::set<int>",
            K::USet => "std::unordered_set<int>",
            K::Stack => "std::stack<int>",
            K::Queue => "std::queue<int>",
            K::Deque => "std::deque<int>",
            K::List => "std::list<int>",
            _ => "std::priority_queue<int>",
        }
    }

    fn container_op_on(&mut self, v: Var) {
        let can = if self.room(&v) { 1 } else { 0 };
        let n = v.name.clone();
        let m = self.mult;
        let e = self.num(N::Int, 2);
        let e = self.fit_int(e).s;
        match v.k {
            K::Vec => {
                let i = {
                    let x = self.num(N::Int, 1);
                    format!("static_cast<size_t>({}) % {n}.size()", self.fit_int(x).s)
                };
                let other = self
                    .pick_var(&[K::Vec])
                    .filter(|o| o.name != n && o.len * m + v.len <= LEN_CAP);
                let w = [
                    3 * can,
                    can,
                    2,
                    3,
                    2 * can,
                    2,
                    1,
                    1,
                    1,
                    3,
                    2,
                    2 * can,
                    can,
                    2,
                ];
                match self.src.weighted(&w) {
                    0 => {
                        self.line(&format!("{n}.push_back({e});"));
                        self.grow(&n, m);
                    }
                    1 => {
                        self.line(&format!("{n}.emplace_back({e});"));
                        self.grow(&n, m);
                    }
                    2 => self.line(&format!("if (!{n}.empty()) {n}.pop_back();")),
                    3 => {
                        let at = if self.src.chance(1, 2) {
                            format!("{n}[{i}]")
                        } else {
                            format!("{n}.at({i})")
                        };
                        self.line(&format!("if (!{n}.empty()) {at} = {e};"));
                    }
                    4 => {
                        self.line(&format!("{n}.insert({n}.begin() + static_cast<int>(static_cast<size_t>({e}) % ({n}.size() + 1)), {e});"));
                        self.grow(&n, m);
                    }
                    5 => self.line(&format!(
                        "if (!{n}.empty()) {n}.erase({n}.begin() + static_cast<int>({i}));"
                    )),
                    6 => {
                        if self.src.chance(1, 3) {
                            self.line(&format!("{n}.clear();"));
                        } else {
                            self.line(&format!("if (!{n}.empty()) {n}.resize({n}.size() / 2);"));
                        }
                    }
                    7 => {
                        let k = self.src.range(0, 6);
                        self.line(&format!("{n}.resize({k});"));
                        if let Some(vv) = self.var_mut(&n) {
                            vv.len = vv.len.max(k);
                        }
                    }
                    8 => {
                        let k = self.src.range(0, 6);
                        self.line(&format!("{n}.assign({k}, {e});"));
                        if let Some(vv) = self.var_mut(&n) {
                            vv.len = vv.len.max(k);
                        }
                    }
                    9 => {
                        let r = self.num(N::Int, 1);
                        let r = self.reduce(r, 17).s;
                        self.line(&format!(
                            "if (!{n}.empty()) {n}[{i}] = ({n}[{i}] + {r}) % {};",
                            KI + 1
                        ));
                    }
                    10 => {
                        let j =
                            format!("static_cast<size_t>({}) % {n}.size()", self.fit_int_text(2));
                        self.line(&format!("if (!{n}.empty()) std::swap({n}[{i}], {n}[{j}]);"));
                    }
                    11 => {
                        if let Some(o) = other {
                            self.line(&format!(
                                "{n}.insert({n}.end(), {0}.begin(), {0}.end());",
                                o.name
                            ));
                            self.grow(&n, o.len * m);
                        } else {
                            self.line(&format!("if (!{n}.empty()) {n}.push_back({n}.back());"));
                            self.grow(&n, m);
                        }
                    }
                    12 => {
                        self.line(&format!("if (!{n}.empty()) {n}.push_back({n}.back());"));
                        self.grow(&n, m);
                    }
                    _ => {
                        // a reference to an element, written through
                        let r = self.num(N::Int, 1);
                        let r = self.reduce(r, 17).s;
                        let rf = self.fresh("r");
                        self.line(&format!(
                            "if (!{n}.empty()) {{ int& {rf} = {n}[{i}]; {rf} = ({rf} + {r}) % {}; }}",
                            KI + 1
                        ));
                    }
                }
            }
            K::Str => self.str_op(&v, can == 1),
            K::Grid => {
                let r = self.idx_mod(v.rows, 1);
                let c = self.idx_mod(v.cols, 1);
                self.line(&format!("{n}[{r}][{c}] = {e};"));
            }
            K::Map | K::UMap => {
                let k = self.fit_int_text(1);
                match self.src.weighted(&[3, 2, 2, 2 * can, 2 * can, 1]) {
                    0 => self.line(&format!("{n}[{k}] = {e};")),
                    1 => {
                        let r = self.num(N::Int, 1);
                        let r = self.reduce(r, 17).s;
                        self.line(&format!("{n}[{k}] = ({n}[{k}] + {r}) % {};", KI + 1));
                    }
                    2 => self.line(&format!("{n}.erase({k});")),
                    3 => {
                        self.line(&format!("{n}.insert({{{k}, {e}}});"));
                        self.grow(&n, m);
                    }
                    4 => {
                        self.line(&format!("{n}.emplace({k}, {e});"));
                        self.grow(&n, m);
                    }
                    _ => {
                        if m <= 25 {
                            self.line(&format!("++{n}[{k}];"));
                        } else {
                            self.line(&format!("{n}.clear();"));
                        }
                    }
                }
                if self.src.chance(1, 3) {
                    self.grow(&n, m);
                }
            }
            K::Set | K::USet => {
                let k = self.fit_int_text(1);
                match self.src.weighted(&[3 * can, 2, 1, 2, 1]) {
                    0 => {
                        self.line(&format!("{n}.insert({e});"));
                        self.grow(&n, m);
                    }
                    1 => self.line(&format!("{n}.erase({k});")),
                    2 => self.line(&format!("{n}.clear();")),
                    3 => {
                        // A fresh name: `k` may mention an enclosing loop's `it`, and
                        // `auto it = ...find(*it)` would read the variable being declared.
                        let fi = self.fresh("fi");
                        self.line(&format!(
                            "if (auto {fi} = {n}.find({k}); {fi} != {n}.end()) {n}.erase({fi});"
                        ));
                    }
                    _ => {
                        self.line(&format!("{n}.emplace({e});"));
                        self.grow(&n, m);
                    }
                }
            }
            K::Stack | K::Queue | K::Pq => match self.src.weighted(&[3 * can, 3, 1, can]) {
                0 => {
                    self.line(&format!("{n}.push({e});"));
                    self.grow(&n, m);
                }
                1 => self.line(&format!("if (!{n}.empty()) {n}.pop();")),
                2 if v.k == K::Stack => self.line(&format!("if (!{n}.empty()) {n}.top() = {e};")),
                2 if v.k == K::Queue => self.line(&format!("if (!{n}.empty()) {n}.front() = {e};")),
                _ => {
                    self.line(&format!("{n}.emplace({e});"));
                    self.grow(&n, m);
                }
            },
            K::List => match self.src.weighted(&[2 * can, 2 * can, 2, 2, can, 1, 1]) {
                0 => {
                    self.line(&format!("{n}.push_back({e});"));
                    self.grow(&n, m);
                }
                1 => {
                    self.line(&format!("{n}.push_front({e});"));
                    self.grow(&n, m);
                }
                2 => self.line(&format!("if (!{n}.empty()) {n}.pop_back();")),
                3 => self.line(&format!("if (!{n}.empty()) {n}.pop_front();")),
                4 => {
                    let k = self.fit_int_text(1);
                    self.line(&format!(
                        "{n}.insert(std::next({n}.begin(), static_cast<int>(static_cast<size_t>({k}) % ({n}.size() + 1))), {e});"
                    ));
                    self.grow(&n, m);
                }
                5 => self.line(&format!("if (!{n}.empty()) {n}.erase({n}.begin());")),
                _ => self.line(&format!("if (!{n}.empty()) {n}.front() = {e};")),
            },
            _ => {
                let i = {
                    let x = self.num(N::Int, 1);
                    format!("static_cast<size_t>({}) % {n}.size()", self.fit_int(x).s)
                };
                match self.src.weighted(&[2 * can, 2 * can, 2, 2, 2, 1, 1]) {
                    0 => {
                        self.line(&format!("{n}.push_back({e});"));
                        self.grow(&n, m);
                    }
                    1 => {
                        self.line(&format!("{n}.push_front({e});"));
                        self.grow(&n, m);
                    }
                    2 => self.line(&format!("if (!{n}.empty()) {n}.pop_back();")),
                    3 => self.line(&format!("if (!{n}.empty()) {n}.pop_front();")),
                    4 => self.line(&format!("if (!{n}.empty()) {n}[{i}] = {e};")),
                    5 => self.line(&format!(
                        "if (!{n}.empty()) {n}.erase({n}.begin() + static_cast<int>({i}));"
                    )),
                    _ => self.line(&format!("if (!{n}.empty()) {n}.front() = {e};")),
                }
            }
        }
    }

    /// A reduced `int` expression as text.
    pub fn fit_int_text(&mut self, d: u32) -> String {
        let e = self.num(N::Int, d);
        self.fit_int(e).s
    }
}
