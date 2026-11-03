// SPDX-License-Identifier: Apache-2.0
//! Rust translation of member functions of strings, maps and sets.

use super::Rs;
use crate::cpp::ast::*;
use crate::cpp::codegen::common::*;

impl Rs<'_> {
    /// `s.find(x[, pos])` as a Rust `Option<usize>` (C++ returns `npos` when absent).
    pub(super) fn str_find(&mut self, obj: &Expr, args: &[Expr], rfind: bool) -> R<String> {
        let s = self.str_ref(obj)?;
        let a = args.first().ok_or("find needs an argument")?;
        let pat = if self.ty_of(a) == Ty::Char {
            let c = self.ex(a, false)?;
            format!("{} as char", c.at(16))
        } else {
            self.str_ref(a)?.s
        };
        let f = if rfind { "rfind" } else { "find" };
        match args.get(1) {
            None => Ok(format!("{}.{f}({pat})", s.at(19))),
            Some(p) if !rfind => {
                let pos = self.conv_val(p, &Ty::Size)?;
                Ok(format!(
                    "{}[{}..].find({pat}).map(|i| i + {})",
                    s.at(19),
                    pos.s,
                    pos.at(12)
                ))
            }
            Some(_) => no("Rust", "rfind with a start position"),
        }
    }

    pub(super) fn str_method(&mut self, obj: &Expr, name: &str, args: &[Expr], m: bool) -> R<Ex> {
        let mutating = matches!(
            name,
            "push_back" | "pop_back" | "clear" | "append" | "insert" | "erase" | "resize" | "swap"
        );
        let val = |s: String| Ok(Ex::new(s, 19, Kind::Val));
        match (name, args) {
            ("size" | "length", []) => {
                let o = self.recv(obj, false)?;
                val(format!("{o}.len()"))
            }
            ("empty", []) => {
                let o = self.recv(obj, false)?;
                val(format!("{o}.is_empty()"))
            }
            ("push_back", [c]) => {
                let o = self.recv(obj, true)?;
                let c = self.ex(c, false)?;
                val(format!("{o}.push({} as char)", c.at(16)))
            }
            ("pop_back", []) => {
                let o = self.recv(obj, true)?;
                val(format!("{o}.pop()"))
            }
            ("back", []) if !m => {
                let o = self.recv(obj, false)?;
                val(format!("{o}.bytes().last().unwrap()"))
            }
            ("front", []) if !m => {
                let o = self.recv(obj, false)?;
                val(format!("{o}.as_bytes()[0]"))
            }
            ("at", [i]) if !m => {
                let o = self.recv(obj, false)?;
                let t = self.ty_of(i);
                val(format!("{o}.as_bytes()[{}]", self.index_text(i, &t)?))
            }
            ("clear", []) => {
                let o = self.recv(obj, true)?;
                val(format!("{o}.clear()"))
            }
            ("append", [x]) => {
                let o = self.recv(obj, true)?;
                let y = self.str_ref(x)?;
                val(format!("{o}.push_str({})", y.s))
            }
            ("substr", [p]) | ("substr", [p, _]) => {
                self.helpers.insert("substr");
                let s = self.str_ref(obj)?;
                let pos = self.conv_val(p, &Ty::Size)?;
                let len = match args.get(1) {
                    Some(l) => self.conv_val(l, &Ty::Size)?.s,
                    None => "usize::MAX".to_string(),
                };
                val(format!("substr({}, {}, {len})", s.s, pos.s))
            }
            ("find" | "rfind", [_, ..]) => {
                let t = self.str_find(obj, args, name == "rfind")?;
                val(format!("{t}.unwrap_or(usize::MAX)"))
            }
            ("compare", [x]) => {
                let (a, b) = (self.str_ref(obj)?, self.str_ref(x)?);
                val(format!("({}.cmp({}) as i32)", a.at(19), b.s))
            }
            ("insert", [p, x]) if self.ty_of(p).is_integral() => {
                let o = self.recv(obj, true)?;
                let pos = self.conv_val(p, &Ty::Size)?;
                let y = self.str_ref(x)?;
                val(format!("{o}.insert_str({}, {})", pos.s, y.s))
            }
            ("erase", [p]) => {
                let o = self.recv(obj, true)?;
                val(format!("{o}.truncate({})", self.conv_val(p, &Ty::Size)?.s))
            }
            ("erase", [p, l]) if self.ty_of(p).is_integral() => {
                let o = self.recv(obj, true)?;
                let (pos, len) = (self.conv_val(p, &Ty::Size)?, self.conv_val(l, &Ty::Size)?);
                val(format!(
                    "{o}.replace_range({}..({} + {}).min({o}.len()), \"\")",
                    pos.s,
                    pos.at(12),
                    len.at(13)
                ))
            }
            ("resize", [n]) | ("resize", [n, _]) => {
                self.helpers.insert("str_resize");
                let o = self.ex(obj, true)?;
                let amp = if o.kind == Kind::Ref {
                    o.s.clone()
                } else {
                    format!("&mut {}", o.at(17))
                };
                let c = match args.get(1) {
                    Some(c) => self.ex(c, false)?.s,
                    None => "0".to_string(),
                };
                val(format!(
                    "str_resize({amp}, {}, {c})",
                    self.conv_val(n, &Ty::Size)?.s
                ))
            }
            ("swap", [other]) => self.swap_with(obj, other),
            _ => {
                let _ = mutating;
                no("Rust", format!("the member `{name}` of a string"))
            }
        }
    }

    pub(super) fn assoc_method(
        &mut self,
        obj: &Expr,
        ot: &Ty,
        name: &str,
        args: &[Expr],
        m: bool,
    ) -> R<Ex> {
        let (k, v) = match ot {
            Ty::Map(k, v) | Ty::UnorderedMap(k, v) => ((**k).clone(), Some((**v).clone())),
            Ty::Set(k) | Ty::UnorderedSet(k) => ((**k).clone(), None),
            _ => return no("Rust", "this container"),
        };
        let val = |s: String| Ok(Ex::new(s, 19, Kind::Val));
        let mutating = matches!(name, "insert" | "emplace" | "erase" | "clear");
        let o = self.recv(obj, mutating || (m && name == "at"))?;
        match (name, args) {
            ("size", []) => val(format!("{o}.len()")),
            ("empty", []) => val(format!("{o}.is_empty()")),
            ("clear", []) => val(format!("{o}.clear()")),
            ("count", [x]) => {
                let key = self.conv_val(x, &k)?;
                let f = if v.is_some() {
                    "contains_key"
                } else {
                    "contains"
                };
                val(format!("usize::from({o}.{f}(&{}))", key.at(17)))
            }
            ("erase", [x]) if !matches!(self.ty_of(x), Ty::Iter { .. }) => {
                let key = self.conv_val(x, &k)?;
                val(format!("{o}.remove(&{})", key.at(17)))
            }
            ("at", [x]) => {
                let key = self.conv_val(x, &k)?;
                if m {
                    Ok(Ex::new(
                        format!("{o}.get_mut(&{}).unwrap()", key.at(17)),
                        19,
                        Kind::Ref,
                    ))
                } else {
                    Ok(Ex::new(format!("{o}[&{}]", key.at(17)), 19, Kind::Place))
                }
            }
            ("insert" | "emplace", _) if v.is_none() && args.len() == 1 => {
                let key = self.conv_val(&args[0], &k)?;
                val(format!("{o}.insert({})", key.s))
            }
            ("insert" | "emplace", _) if v.is_some() => {
                let (ke, ve) = self.pair_args(name, args)?;
                let vt = v.unwrap_or(Ty::Unknown);
                let key = self.conv_val(ke, &k)?;
                let value = self.conv_val(ve, &vt)?;
                val(format!("{o}.entry({}).or_insert({})", key.s, value.s))
            }
            _ => no("Rust", format!("the member `{name}` of a map or set")),
        }
    }

    /// The key and value expressions of `insert({k, v})`, `insert(std::make_pair(k, v))`, `emplace(k, v)`.
    fn pair_args<'e>(&self, name: &str, args: &'e [Expr]) -> R<(&'e Expr, &'e Expr)> {
        let two = |items: &'e [Expr]| match items {
            [a, b] => Ok((a, b)),
            _ => no("Rust", "this insert"),
        };
        match (name, args) {
            ("emplace", [_, _]) => two(args),
            (_, [a]) => match &a.kind {
                ExprKind::InitList(items) => two(items),
                ExprKind::Construct { args: items, .. } => two(items),
                ExprKind::Call { args: items, .. } => two(items),
                _ => no("Rust", "this insert"),
            },
            _ => no("Rust", "this insert"),
        }
    }

    /// `s.insert(x).second` on sets and unordered maps (a `bool` in Rust's `insert`).
    pub(super) fn insert_second(&mut self, obj: &Expr) -> Option<R<Ex>> {
        let ExprKind::Call { callee, args } = &obj.kind else {
            return None;
        };
        let ExprKind::Member { obj: c, name, .. } = &callee.kind else {
            return None;
        };
        if !matches!(name.as_str(), "insert" | "emplace") {
            return None;
        }
        let ct = self.ty_of(c);
        let r = (|| match &ct {
            Ty::Set(k) | Ty::UnorderedSet(k) if args.len() == 1 => {
                let o = self.recv(c, true)?;
                let key = self.conv_val(&args[0], k)?;
                Ok(Ex::new(format!("{o}.insert({})", key.s), 19, Kind::Val))
            }
            Ty::UnorderedMap(k, v) => {
                let (ke, ve) = self.pair_args(name, args)?;
                self.helpers.insert("insert_new");
                self.uses.insert("HashMap");
                let o = self.ex(c, true)?;
                let amp = if o.kind == Kind::Ref {
                    o.s.clone()
                } else {
                    format!("&mut {}", o.at(17))
                };
                let key = self.conv_val(ke, k)?;
                let value = self.conv_val(ve, v)?;
                Ok(Ex::new(
                    format!("insert_new({amp}, {}, {})", key.s, value.s),
                    19,
                    Kind::Val,
                ))
            }
            _ => no("Rust", "the result of this insert"),
        })();
        Some(r)
    }
}
