// SPDX-License-Identifier: Apache-2.0
//! Library calls: overload choice among `MEMBERS` rows, const-correctness of methods, callable arguments
//! (lambdas, functors) checked against what the algorithm passes them.

use super::Resolved;
use super::ctx::Checker;
use super::libcall::{Bind, Fail, deref_val, elem_of};
use super::sigpat::{PT, parse_sig};
use super::val::*;
use crate::cpp::ast::*;
use crate::cpp::stdlib::{MEMBERS, Member, members_of};

/// Methods that also exist as `const` overloads (the table lists only the mutable one).
const CONST_OVERLOADED: &[&str] = &[
    "at",
    "back",
    "front",
    "operator[]",
    "find",
    "lower_bound",
    "upper_bound",
    "begin",
    "end",
    "rbegin",
    "rend",
    "value",
    "operator*",
    "operator->",
    "data",
    "top",
];

/// `std::vector<T>` -> `std::vector`.
fn base_name(recv: &str) -> &str {
    recv.split('<').next().unwrap_or(recv)
}

impl Checker<'_> {
    /// Choose the best row that accepts the arguments (most exact parameter matches, table order on ties).
    pub fn lib_pick(
        &self,
        cands: &[&'static Member],
        recv: Option<&Val>,
        args: &[Val],
        explicit: &[Ty],
    ) -> Result<(&'static Member, Val, Bind), (&'static Member, Fail)> {
        let mut best: Option<(&'static Member, Val, Bind)> = None;
        let mut fails: Vec<(&'static Member, Fail)> = Vec::new();
        for m in cands {
            match self.try_sig(m, recv, args, explicit) {
                Ok((v, b)) => {
                    if best.as_ref().is_none_or(|(_, _, bb)| b.exact > bb.exact) {
                        best = Some((m, v, b));
                    }
                }
                Err(f) => fails.push((m, f)),
            }
        }
        if let Some(b) = best {
            return Ok(b);
        }
        let i = fails
            .iter()
            .position(|(_, f)| f.code != "type.arity")
            .unwrap_or(0);
        Err(fails.swap_remove(i.min(fails.len().saturating_sub(1))))
    }

    /// The type of `recv.name(args)` for a library receiver.
    pub fn lib_method(
        &mut self,
        node: NodeId,
        recv: &Val,
        name: &str,
        args: &[Val],
        who: &str,
    ) -> Val {
        let cands: Vec<&'static Member> = members_of(&recv.ty)
            .into_iter()
            .filter(|m| m.name == name)
            .collect();
        let Some(first) = cands.first() else {
            self.err(
                node,
                "name.member",
                format!(
                    "{} has no member called `{name}` in the supply cupboard; try `{who}` with a name from its list",
                    recv.ty
                ),
            );
            return Val::unknown();
        };
        let key = format!("{}::{name}", base_name(first.recv));
        if !self.check_locked(node, &[key]) {
            return Val::unknown();
        }
        match self.lib_pick(&cands, Some(recv), args, &[]) {
            Err((m, f)) => {
                self.err(
                    node,
                    f.code,
                    format!("`{name}` on {} does not accept these arguments: {}; it is declared as `{}`", recv.ty, f.msg, m.sig),
                );
                Val::unknown()
            }
            Ok((m, ret, bind)) => {
                self.info.refs.insert(node, Resolved::Lib(m));
                self.method_const(node, recv, m, who);
                self.check_callables(node, m, &bind, args);
                if name.starts_with("emplace") {
                    self.emplace_check(node, &recv.ty, args);
                }
                ret
            }
        }
    }

    fn method_const(&mut self, node: NodeId, recv: &Val, m: &Member, who: &str) {
        if !recv.konst || recv.ty.is_integral() {
            return;
        }
        let sig = parse_sig(m.sig);
        let map = matches!(recv.ty, Ty::Map(..) | Ty::UnorderedMap(..));
        let overloaded = CONST_OVERLOADED.contains(&m.name) && !(m.name == "operator[]" && map);
        if !sig.konst_method && !overloaded && !sig.is_data {
            self.err(
                node,
                "const.method",
                format!("`{who}` is const here, so `{}` (which changes it) cannot be used on it; drop the `const` or work on a copy", m.name),
            );
        }
    }

    /// Resolve a free library function `key` (such as `std::sort`) called with `args`; `None` when the
    /// library has no such function.
    pub fn lib_free(
        &mut self,
        node: NodeId,
        key: &str,
        args: &[Val],
        explicit: &[Ty],
    ) -> Option<Val> {
        let cands: Vec<&'static Member> = MEMBERS
            .iter()
            .filter(|m| m.recv == "free" && m.name == key)
            .collect();
        if cands.is_empty() {
            return None;
        }
        Some(match self.lib_pick(&cands, None, args, explicit) {
            Err((m, f)) => {
                self.need_header(node, m.header, key);
                self.err(
                    node,
                    f.code,
                    format!(
                        "`{key}` does not accept these arguments: {}; it is declared as `{}`",
                        f.msg, m.sig
                    ),
                );
                Val::unknown()
            }
            Ok((m, ret, bind)) => {
                if self.need_header(node, m.header, key) {
                    self.check_locked(node, &[m.header.to_string(), key.to_string()]);
                }
                self.info.refs.insert(node, Resolved::Lib(m));
                self.check_callables(node, m, &bind, args);
                ret
            }
        })
    }

    /// Check lambda and functor arguments against what the algorithm passes them; this also instantiates
    /// generic lambdas with the element type.
    pub fn check_callables(&mut self, node: NodeId, m: &Member, b: &Bind, args: &[Val]) {
        let sig = parse_sig(m.sig);
        let iter_elem = b
            .deduced
            .values()
            .find(|v| matches!(v.ty, Ty::Iter { .. } | Ty::Ptr(_)))
            .and_then(deref_val);
        let elem = iter_elem.or_else(|| b.fixed.get("T").map(|t| Val::lv(t.clone(), false)));
        for (i, p) in sig.params.iter().enumerate() {
            let PT::Callable(n) = &p.pt else { continue };
            let (Some(a), Some(e)) = (args.get(i), elem.clone()) else {
                continue;
            };
            let acc = b.deduced.get("T").map(|v| Val::lv(v.ty.clone(), false));
            let (call_args, ret): (Vec<Val>, Option<Ty>) = match n.as_str() {
                "Compare" | "Cmp" => (vec![e.clone(), e], Some(Ty::Bool)),
                "Pred" => (vec![e], Some(Ty::Bool)),
                "BinaryOp" => match acc {
                    Some(t) => (vec![t.clone(), e], Some(t.ty)),
                    None => continue,
                },
                _ => (vec![e], None),
            };
            let got = self.invoke(node, a, &call_args);
            if let (Some(want), false) = (ret, got.is_unknown())
                && (!self.convertible(&got, &want) || (want == Ty::Bool && !self.truthy(&got.ty)))
            {
                self.err(
                    node,
                    "type.callable",
                    format!("the function you pass to `{}` must give back a {want}, but yours gives a {}", m.name, got.ty),
                );
            }
        }
    }

    /// Element type of a range (container) value, for `for (x : range)` and algorithms.
    pub fn range_elem(&self, r: &Val) -> Option<Val> {
        let e = elem_of(&r.ty)?;
        let set = matches!(r.ty, Ty::Set(_) | Ty::UnorderedSet(_) | Ty::StrView);
        let map = matches!(r.ty, Ty::Map(..) | Ty::UnorderedMap(..));
        Some(Val::lv(e, r.konst || set).with_ic(map))
    }
}
