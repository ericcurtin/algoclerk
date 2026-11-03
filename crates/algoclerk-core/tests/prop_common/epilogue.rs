// SPDX-License-Identifier: Apache-2.0
//! The end of the entry function: fold every top-level variable into a digest and return a value of a
//! random supported return type, so that the final state of everything is observed.

use super::expr::*;

const M: i64 = 1_000_003;

impl Gen {
    /// Emit the digest code and the return statement; returns the C++ return type.
    pub fn epilogue(&mut self) -> &'static str {
        let vars: Vec<Var> = self.scopes[0].clone();
        let mut digests: Vec<String> = Vec::new();
        for v in &vars {
            let d = self.fresh("d");
            let n = &v.name;
            let fold = |term: &str| format!("{d} = ({d} * 31 + {term}) % {M};");
            let mut lines: Vec<String> = Vec::new();
            match v.k {
                K::Int => lines.push(format!("int {d} = (({n} % {M}) + {M}) % {M};")),
                K::Ll => lines.push(format!(
                    "int {d} = static_cast<int>((({n} % {M}) + {M}) % {M});"
                )),
                K::U | K::Ull => lines.push(format!("int {d} = static_cast<int>({n} % {M});")),
                K::Char => lines.push(format!("int {d} = static_cast<int>({n});")),
                K::Bool => lines.push(format!("int {d} = {n} ? 1 : 0;")),
                K::Str => {
                    lines.push(format!("int {d} = 7;"));
                    lines.push(format!("for (char c : {n}) {}", fold("c")));
                }
                K::Vec => {
                    lines.push(format!("int {d} = 7;"));
                    lines.push(format!("for (int e : {n}) {}", fold("(e % 1000 + 1000)")));
                }
                K::Grid => {
                    lines.push(format!("int {d} = 7;"));
                    lines.push(format!(
                        "for (const auto& row : {n}) for (int e : row) {}",
                        fold("(e % 1000 + 1000)")
                    ));
                }
                K::Map => {
                    lines.push(format!("int {d} = 7;"));
                    lines.push(format!(
                        "for (const auto& [k, x] : {n}) {}",
                        fold("(k % 1000 + 1000) * 3 + (x % 1000 + 1000)")
                    ));
                }
                K::Set => {
                    lines.push(format!("int {d} = 7;"));
                    lines.push(format!("for (int e : {n}) {}", fold("(e % 1000 + 1000)")));
                }
                K::UMap => {
                    lines.push(format!("long long {d}a = 0;"));
                    lines.push(format!(
                        "for (const auto& [k, x] : {n}) {d}a += static_cast<long long>(k) * 7 + x;"
                    ));
                    lines.push(format!(
                        "int {d} = static_cast<int>((({d}a % {M}) + {M}) % {M});"
                    ));
                }
                K::USet => {
                    lines.push(format!("long long {d}a = 0;"));
                    lines.push(format!("for (int e : {n}) {d}a += e;"));
                    lines.push(format!(
                        "int {d} = static_cast<int>((({d}a % {M}) + {M}) % {M});"
                    ));
                }
                K::Stack | K::Queue | K::Deque | K::Pq => {
                    let t = format!("{d}t");
                    lines.push(format!("int {d} = 7;"));
                    if v.k == K::Deque {
                        lines.push(format!("for (int e : {n}) {}", fold("(e % 1000 + 1000)")));
                    } else {
                        lines.push(format!("auto {t} = {n};"));
                        let get = match v.k {
                            K::Queue => format!("{t}.front()"),
                            _ => format!("{t}.top()"),
                        };
                        lines.push(format!(
                            "while (!{t}.empty()) {{ {} {t}.pop(); }}",
                            fold(&format!("({get} % 1000 + 1000)"))
                        ));
                    }
                }
            }
            for l in lines {
                self.line(&l);
            }
            digests.push(d);
        }
        let ds = &digests;
        match self.src.below(9) {
            0 | 1 => {
                self.line("int h = 17;");
                for d in ds {
                    self.line(&format!("h = (h * 31 + {d}) % {M};"));
                }
                self.line("return h;");
                "int"
            }
            2 => {
                self.line("long long h = 17;");
                for d in ds {
                    self.line(&format!("h = (h * 31 + {d}) % 1000000007LL;"));
                }
                self.line("return h * 3 - 5;");
                "long long"
            }
            3 => {
                self.line("unsigned h = 17u;");
                for d in ds {
                    self.line(&format!("h = h * 31u + static_cast<unsigned>({d});"));
                }
                self.line("return h;");
                "unsigned"
            }
            4 => {
                self.line("unsigned long long h = 17;");
                for d in ds {
                    self.line(&format!(
                        "h = h * 1000003ull + static_cast<unsigned long long>({d});"
                    ));
                }
                self.line("return h;");
                "unsigned long long"
            }
            5 => {
                let t = ds.first().map_or("0".to_string(), Clone::clone);
                self.line(&format!("return {t} % 2 == 0;"));
                "bool"
            }
            6 => {
                let t = ds.first().map_or("0".to_string(), Clone::clone);
                self.line(&format!("return static_cast<char>('a' + {t} % 26);"));
                "char"
            }
            7 => {
                self.line("std::string out;");
                for d in ds {
                    self.line(&format!("out += std::to_string({d});"));
                    self.line("out += ',';");
                }
                for v in vars.iter().filter(|v| v.k == K::Str) {
                    self.line(&format!("out += {};", v.name));
                }
                self.line("return out;");
                "std::string"
            }
            _ => {
                self.line("std::vector<int> out;");
                for d in ds {
                    self.line(&format!("out.push_back({d});"));
                }
                self.line("return out;");
                "std::vector<int>"
            }
        }
    }
}
