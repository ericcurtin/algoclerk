// SPDX-License-Identifier: Apache-2.0
//! Whole programs: helpers, the entry function, its arguments and the observing epilogue.

use super::expr::*;
use super::reads::LEN_CAP;

pub const INCLUDES: &str = "#include <algorithm>
#include <cctype>
#include <cstddef>
#include <cstdlib>
#include <deque>
#include <functional>
#include <iterator>
#include <map>
#include <numeric>
#include <queue>
#include <set>
#include <stack>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <tuple>
#include <vector>
";

/// A generated program.
#[derive(Clone, Debug)]
pub struct Prog {
    /// Everything after the includes: helpers and `f`.
    pub body: String,
    /// Arguments of `f` in the brace notation.
    pub args: Vec<String>,
    /// The C++ return type of `f`.
    pub ret: &'static str,
}

impl Prog {
    pub fn source(&self) -> String {
        format!("{INCLUDES}{}", self.body)
    }
}

const VOID_HELPERS: &str = "void hbump(std::vector<int>& v, int x) {
    for (size_t i = 0; i < v.size(); ++i) v[i] = (v[i] + x) % 30001;
}
void hrot(std::vector<int>& v) {
    if (v.size() > 1) std::rotate(v.begin(), v.begin() + 1, v.end());
}
void hdedup(std::vector<int>& v) {
    std::sort(v.begin(), v.end());
    v.erase(std::unique(v.begin(), v.end()), v.end());
}
";

impl Gen {
    /// Call one of the void helpers on a vector.
    pub fn void_call(&mut self) {
        let Some(v) = self.pick_var(&[K::Vec]).filter(|v| !v.frozen) else {
            return self.decl_container();
        };
        match self.src.below(3) {
            0 => {
                let e = self.fit_int_text(2);
                self.line(&format!("hbump({}, {e});", v.name));
            }
            1 => self.line(&format!("hrot({});", v.name)),
            _ => self.line(&format!("hdedup({});", v.name)),
        }
    }

    fn arg_literal(&mut self, k: K) -> String {
        match k {
            K::Int | K::Ll => {
                let v = if self.src.chance(1, 8) {
                    *self.src.pick(&[30000, -30000, 0, 1, -1])
                } else {
                    self.src.range(-50, 50)
                };
                v.to_string()
            }
            K::Str => {
                let n = self.src.range(0, 8) as usize;
                let s: String = (0..n)
                    .map(|_| (b'a' + self.src.range(0, 5) as u8) as char)
                    .collect();
                format!("\"{s}\"")
            }
            _ => {
                let n = self.src.range(0, 8);
                let v: Vec<String> = (0..n)
                    .map(|_| {
                        if self.src.chance(1, 10) {
                            "30000".into()
                        } else {
                            self.src.range(-20, 20).to_string()
                        }
                    })
                    .collect();
                format!("{{{}}}", v.join(", "))
            }
        }
    }

    /// A random helper function; its text goes to `out` (which must be empty on entry).
    fn helper(&mut self, idx: usize) -> String {
        let name = format!("h{idx}");
        let ret = if self.src.chance(1, 4) { K::Ll } else { K::Int };
        let recursive = self.src.chance(1, 3);
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![Vec::new()]);
        let saved_out = std::mem::take(&mut self.out);
        let saved_lam = std::mem::take(&mut self.lambdas);
        let saved_budget = self.budget;
        let saved_indent = self.indent;
        self.indent = 1;
        self.budget = 12;
        self.early_return = Some(ret);
        let rt = ret.num().expect("numeric");
        let rv = Var::new("r".into(), ret);
        let (head, helper) = if recursive {
            let double = self.src.chance(1, 2);
            let mask = if double { 7 } else { 31 };
            let head = format!("{} {name}(int n, int a)", rt.name());
            let mut n = Var::new("n".into(), K::Int);
            n.lo = 0;
            n.hi = mask;
            n.rw = false;
            let mut a = Var::new("a".into(), K::Int);
            a.rw = false;
            self.add(n);
            self.add(a);
            self.early_return = None;
            let e0 = self.num(rt, 2);
            let e0 = self.fit_var(e0, &rv);
            self.line(&format!("if (n <= 0) return {};", e0.s));
            let arg = self.fit_int_text(2);
            let mut r = Var::new("r1".into(), ret);
            r.rw = false;
            self.line(&format!("{} r1 = {name}(n - 1, {arg});", rt.name()));
            self.add(r);
            if double {
                let arg = self.fit_int_text(2);
                let mut r = Var::new("r2".into(), ret);
                r.rw = false;
                self.line(&format!("{} r2 = {name}(n - 2, {arg});", rt.name()));
                self.add(r);
            }
            let e = self.num(rt, 3);
            let e = self.fit_var(e, &rv);
            self.line(&format!("return {};", e.s));
            let helper = Helper {
                name: name.clone(),
                params: vec![K::Int, K::Int],
                ret: Some(ret),
                rec_mask: Some(mask as i64),
            };
            (head, helper)
        } else {
            let np = 1 + self.src.below(3);
            let mut params = Vec::new();
            let mut decl = Vec::new();
            for i in 0..np {
                let k = *self.src.pick(&[K::Int, K::Int, K::Ll, K::Str, K::Vec]);
                let pn = format!("p{i}");
                let mut v = Var::new(pn.clone(), k);
                match k {
                    K::Int | K::Ll => decl.push(format!("{} {pn}", kind_name(k))),
                    K::Str => {
                        decl.push(format!("const std::string& {pn}"));
                        v.len = LEN_CAP;
                        v.frozen = true;
                        v.rw = false;
                    }
                    _ => {
                        decl.push(format!("const std::vector<int>& {pn}"));
                        v.len = LEN_CAP;
                        v.frozen = true;
                        v.rw = false;
                    }
                }
                params.push(k);
                self.add(v);
            }
            let head = format!("{} {name}({})", rt.name(), decl.join(", "));
            let n = 1 + self.src.below(4);
            for _ in 0..n {
                self.stmt(2);
            }
            let e = self.num(rt, 3);
            let e = self.fit_var(e, &rv);
            self.line(&format!("return {};", e.s));
            let helper = Helper {
                name: name.clone(),
                params,
                ret: Some(ret),
                rec_mask: None,
            };
            (head, helper)
        };
        let body = std::mem::replace(&mut self.out, saved_out);
        self.scopes = saved_scopes;
        self.lambdas = saved_lam;
        self.budget = saved_budget;
        self.indent = saved_indent;
        self.early_return = None;
        self.helpers.push(helper);
        format!("{head} {{\n{body}}}\n")
    }

    pub fn program(&mut self) -> Prog {
        let mut text = String::new();
        let uses_void = self.src.chance(1, 3);
        self.void_helpers = uses_void;
        if uses_void {
            text.push_str(VOID_HELPERS);
        }
        for i in 0..self.src.below(3) {
            let h = self.helper(i);
            text.push_str(&h);
        }
        // entry parameters
        let np = self.src.below(4);
        let mut decl = Vec::new();
        let mut args = Vec::new();
        for i in 0..np {
            let k = *self.src.pick(&[K::Int, K::Ll, K::Str, K::Vec, K::Vec]);
            let pn = format!("a{i}");
            let mut v = Var::new(pn.clone(), k);
            let by_ref = matches!(k, K::Str | K::Vec) && self.src.chance(1, 2);
            match k {
                K::Int | K::Ll => decl.push(format!("{} {pn}", kind_name(k))),
                K::Str => {
                    v.len = 8;
                    decl.push(if by_ref {
                        format!("const std::string& {pn}")
                    } else {
                        format!("std::string {pn}")
                    });
                }
                _ => {
                    v.len = 8;
                    decl.push(if by_ref {
                        format!("const std::vector<int>& {pn}")
                    } else {
                        format!("std::vector<int> {pn}")
                    });
                }
            }
            if by_ref {
                v.frozen = true;
                v.rw = false;
            }
            args.push(self.arg_literal(k));
            self.add(v);
        }
        self.budget = 30;
        self.early_return = None;
        let nstmts = 4 + self.src.below(8);
        for _ in 0..nstmts {
            self.stmt(3);
        }
        let ret = self.epilogue();
        text.push_str(&format!(
            "{ret} f({}) {{\n{}}}\n",
            decl.join(", "),
            self.out
        ));
        Prog {
            body: text,
            args,
            ret,
        }
    }
}

pub fn kind_name(k: K) -> &'static str {
    match k {
        K::Int => "int",
        K::Ll => "long long",
        _ => "?",
    }
}
