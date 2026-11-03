// SPDX-License-Identifier: Apache-2.0
//! Rust translation of member functions of sequences, adaptors, priority queues and optionals.

use super::Rs;

use crate::cpp::ast::*;
use crate::cpp::check::Resolved;
use crate::cpp::codegen::common::*;
use crate::cpp::print::print_expr;

impl Rs<'_> {
    pub(super) fn method(
        &mut self,
        e: &Expr,
        obj: &Expr,
        name: &str,
        args: &[Expr],
        m: bool,
    ) -> R<Ex> {
        let ot = self.ty_of(obj);
        let _ = e;
        match &ot {
            Ty::Vector(el) | Ty::Array(el, _) => self.seq_method(obj, el, name, args, m),
            Ty::Stack(el) | Ty::Queue(el) | Ty::Deque(el) | Ty::List(el) => {
                self.adaptor_method(obj, &ot, el, name, args, m)
            }
            Ty::PriorityQueue(el, cmp) => self.pq_method(obj, el, cmp, name, args),
            Ty::Optional(el) => self.opt_method(obj, el, name, args, m),
            Ty::Str | Ty::StrView => self.str_method(obj, name, args, m),
            Ty::Map(..) | Ty::UnorderedMap(..) | Ty::Set(_) | Ty::UnorderedSet(_) => {
                self.assoc_method(obj, &ot, name, args, m)
            }
            t => no("Rust", format!("the member `{name}` of {t}")),
        }
    }

    pub(super) fn recv(&mut self, obj: &Expr, mutating: bool) -> R<String> {
        Ok(self.ex(obj, mutating)?.at(19))
    }

    /// The index an iterator argument (`begin()`, `begin() + k`, `end()`) of `obj` stands for.
    pub(super) fn pos_idx(&mut self, obj: &Expr, p: &Expr) -> R<String> {
        match iter_pos(p) {
            Some(Pos::Begin(o, k, false)) if print_expr(o) == print_expr(obj) => match k {
                None => Ok("0".into()),
                Some(k) => {
                    let t = self.ty_of(k);
                    self.index_text(k, &t)
                }
            },
            Some(Pos::End(o, None, false)) if print_expr(o) == print_expr(obj) => {
                Ok(format!("{}.len()", self.recv(obj, false)?))
            }
            _ => no("Rust", "this iterator position"),
        }
    }

    fn call_val(s: String) -> R<Ex> {
        Ok(Ex::new(s, 19, Kind::Val))
    }

    fn seq_method(&mut self, obj: &Expr, el: &Ty, name: &str, args: &[Expr], m: bool) -> R<Ex> {
        let mutating = matches!(
            name,
            "push_back"
                | "emplace_back"
                | "pop_back"
                | "clear"
                | "insert"
                | "erase"
                | "resize"
                | "assign"
                | "swap"
                | "fill"
        );
        let o = self.recv(obj, mutating || m)?;
        match (name, args) {
            ("size", []) => Self::call_val(format!("{o}.len()")),
            ("empty", []) => Self::call_val(format!("{o}.is_empty()")),
            ("push_back", [x]) => Self::call_val(format!("{o}.push({})", self.conv_val(x, el)?.s)),
            ("emplace_back", _) => {
                Self::call_val(format!("{o}.push({})", self.construct(el, args, false)?.s))
            }
            ("pop_back", []) => Self::call_val(format!("{o}.pop()")),
            ("back", []) => Ok(Ex::new(
                format!("{o}.{}().unwrap()", if m { "last_mut" } else { "last" }),
                19,
                Kind::Ref,
            )),
            ("front", []) => Ok(Ex::new(
                format!("{o}.{}().unwrap()", if m { "first_mut" } else { "first" }),
                19,
                Kind::Ref,
            )),
            ("clear", []) => Self::call_val(format!("{o}.clear()")),
            ("at", [i]) => {
                let t = self.ty_of(i);
                Ok(Ex::new(
                    format!("{o}[{}]", self.index_text(i, &t)?),
                    19,
                    Kind::Place,
                ))
            }
            ("fill", [x]) => Self::call_val(format!("{o}.fill({})", self.conv_val(x, el)?.s)),
            ("reserve" | "shrink_to_fit", _) => Ok(Ex::val("")),
            ("resize", [n]) => {
                let d = self.default_val(el)?;
                Self::call_val(format!(
                    "{o}.resize({}, {d})",
                    self.conv_val(n, &Ty::Size)?.s
                ))
            }
            ("resize", [n, x]) => Self::call_val(format!(
                "{o}.resize({}, {})",
                self.conv_val(n, &Ty::Size)?.s,
                self.conv_val(x, el)?.s
            )),
            ("assign", [n, x]) => {
                let p = self.place_text(obj)?;
                let v = format!(
                    "vec![{}; {}]",
                    self.conv_val(x, el)?.s,
                    self.conv_val(n, &Ty::Size)?.s
                );
                Ok(Ex::new(format!("{p} = {v}"), 1, Kind::Val))
            }
            ("swap", [other]) => self.swap_with(obj, other),
            ("insert", [p, x]) if !matches!(self.ty_of(x), Ty::Iter { .. }) => {
                let i = self.pos_idx(obj, p)?;
                Self::call_val(format!("{o}.insert({i}, {})", self.conv_val(x, el)?.s))
            }
            ("insert", [p, a, b]) => {
                let Some(r) = self.range(a, b)? else {
                    return no("Rust", "this insert");
                };
                let i = self.pos_idx(obj, p)?;
                let it = self.range_iter(&r);
                Self::call_val(format!("{o}.splice({i}..{i}, {it})"))
            }
            ("erase", [p]) => {
                let i = self.pos_idx(obj, p)?;
                Self::call_val(format!("{o}.remove({i})"))
            }
            ("erase", [a, b]) => self.erase_range(obj, &o, a, b),
            _ => no("Rust", format!("the member `{name}` of a vector")),
        }
    }

    /// `v.erase(first, last)`, including the `erase(unique(..), end())` and `erase(remove(..), end())` idioms.
    fn erase_range(&mut self, obj: &Expr, o: &str, a: &Expr, b: &Expr) -> R<Ex> {
        if let ExprKind::Call { callee, args } = &a.kind
            && let Some(Resolved::Lib(mem)) = self.info.resolved(callee.id)
            && matches!(iter_pos(b), Some(Pos::End(_, None, false)))
        {
            match (mem.name, args.as_slice()) {
                ("std::unique", [x, y]) => {
                    let Some(r) = self.range(x, y)? else {
                        return no("Rust", "this idiom");
                    };
                    return Self::call_val(format!("{}.dedup()", self.range_slice(&r)));
                }
                ("std::remove", [x, y, v]) => {
                    let Some(r) = self.range(x, y)? else {
                        return no("Rust", "this idiom");
                    };
                    let t = self.elem_of(&r.ty);
                    let v = self.conv_val(v, &t)?;
                    return Self::call_val(format!(
                        "{}.retain(|e| *e != {})",
                        self.range_slice(&r),
                        v.at(8)
                    ));
                }
                _ => {}
            }
        }
        let lo = self.pos_idx(obj, a)?;
        let hi = self.pos_idx(obj, b)?;
        Self::call_val(format!("{o}.drain({lo}..{hi})"))
    }

    pub(super) fn swap_with(&mut self, obj: &Expr, other: &Expr) -> R<Ex> {
        let side = |this: &mut Self, x: &Expr| -> R<String> {
            let v = this.ex(x, true)?;
            match v.kind {
                Kind::Place => Ok(format!("&mut {}", v.at(17))),
                Kind::Ref => Ok(v.s),
                Kind::Val => no("Rust", "swapping a temporary"),
            }
        };
        let (a, b) = (side(self, obj)?, side(self, other)?);
        Self::call_val(format!("std::mem::swap({a}, {b})"))
    }

    fn adaptor_method(
        &mut self,
        obj: &Expr,
        ot: &Ty,
        el: &Ty,
        name: &str,
        args: &[Expr],
        m: bool,
    ) -> R<Ex> {
        let stack = matches!(ot, Ty::Stack(_));
        let mutating = matches!(
            name,
            "push"
                | "pop"
                | "emplace"
                | "push_back"
                | "push_front"
                | "pop_back"
                | "pop_front"
                | "emplace_back"
                | "emplace_front"
                | "clear"
                | "insert"
                | "erase"
        );
        let o = self.recv(obj, mutating || m)?;
        let (push, pop, front, back) = if stack {
            ("push", "pop", "first", "last")
        } else {
            ("push_back", "pop_front", "front", "back")
        };
        let acc = |f: &str| if m { format!("{f}_mut") } else { f.to_string() };
        match (name, args) {
            ("size", []) => Self::call_val(format!("{o}.len()")),
            ("empty", []) => Self::call_val(format!("{o}.is_empty()")),
            ("clear", []) => Self::call_val(format!("{o}.clear()")),
            ("push", [x]) => Self::call_val(format!("{o}.{push}({})", self.conv_val(x, el)?.s)),
            ("emplace", _) => Self::call_val(format!(
                "{o}.{push}({})",
                self.construct(el, args, false)?.s
            )),
            ("pop", []) => Self::call_val(format!("{o}.{pop}()")),
            ("top", []) if stack => Ok(Ex::new(
                format!("{o}.{}().unwrap()", acc("last")),
                19,
                Kind::Ref,
            )),
            ("front", []) if !stack => Ok(Ex::new(
                format!("{o}.{}().unwrap()", acc(front)),
                19,
                Kind::Ref,
            )),
            ("back", []) if !stack => Ok(Ex::new(
                format!("{o}.{}().unwrap()", acc(back)),
                19,
                Kind::Ref,
            )),
            ("push_back", [x]) if !stack => {
                Self::call_val(format!("{o}.push_back({})", self.conv_val(x, el)?.s))
            }
            ("push_front", [x]) if !stack => {
                Self::call_val(format!("{o}.push_front({})", self.conv_val(x, el)?.s))
            }
            ("emplace_back", _) if !stack => Self::call_val(format!(
                "{o}.push_back({})",
                self.construct(el, args, false)?.s
            )),
            ("emplace_front", _) if !stack => Self::call_val(format!(
                "{o}.push_front({})",
                self.construct(el, args, false)?.s
            )),
            ("pop_back", []) if !stack => Self::call_val(format!("{o}.pop_back()")),
            ("pop_front", []) if !stack => Self::call_val(format!("{o}.pop_front()")),
            ("at", [i]) if !stack => {
                let t = self.ty_of(i);
                Ok(Ex::new(
                    format!("{o}[{}]", self.index_text(i, &t)?),
                    19,
                    Kind::Place,
                ))
            }
            ("insert", [p, x]) if !stack => {
                let i = self.pos_idx(obj, p)?;
                Self::call_val(format!("{o}.insert({i}, {})", self.conv_val(x, el)?.s))
            }
            ("erase", [p]) if !stack => {
                let i = self.pos_idx(obj, p)?;
                Self::call_val(format!("{o}.remove({i})"))
            }
            _ => no("Rust", format!("the member `{name}` of this container")),
        }
    }

    fn pq_method(&mut self, obj: &Expr, el: &Ty, cmp: &Ty, name: &str, args: &[Expr]) -> R<Ex> {
        let greater = matches!(cmp, Ty::Functor(Functor::Greater, _));
        let o = self.recv(obj, true)?;
        let wrap = |s: String| if greater { format!("Reverse({s})") } else { s };
        match (name, args) {
            ("size", []) => Self::call_val(format!("{o}.len()")),
            ("empty", []) => Self::call_val(format!("{o}.is_empty()")),
            ("push", [x]) => {
                let v = self.conv_val(x, el)?.s;
                Self::call_val(format!("{o}.push({})", wrap(v)))
            }
            ("emplace", _) => {
                let v = self.construct(el, args, false)?.s;
                Self::call_val(format!("{o}.push({})", wrap(v)))
            }
            ("pop", []) => Self::call_val(format!("{o}.pop()")),
            ("top", []) if greater => {
                Ok(Ex::new(format!("{o}.peek().unwrap().0"), 19, Kind::Place))
            }
            ("top", []) => Ok(Ex::new(format!("{o}.peek().unwrap()"), 19, Kind::Ref)),
            _ => no("Rust", format!("the member `{name}` of a priority_queue")),
        }
    }

    fn opt_method(&mut self, obj: &Expr, el: &Ty, name: &str, args: &[Expr], m: bool) -> R<Ex> {
        let o = self.recv(obj, m || name == "reset")?;
        match (name, args) {
            ("has_value", []) => Self::call_val(format!("{o}.is_some()")),
            ("value", []) => Ok(Ex::new(
                format!("{o}.{}().unwrap()", if m { "as_mut" } else { "as_ref" }),
                19,
                Kind::Ref,
            )),
            ("value_or", [x]) => {
                let v = self.conv_val(x, el)?.s;
                let base = if self.is_copy(el) {
                    o
                } else {
                    format!("{o}.clone()")
                };
                Self::call_val(format!("{base}.unwrap_or({v})"))
            }
            ("reset", []) => {
                let p = self.place_text(obj)?;
                Ok(Ex::new(format!("{p} = None"), 1, Kind::Val))
            }
            _ => no("Rust", format!("the member `{name}` of an optional")),
        }
    }
}
