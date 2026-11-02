// SPDX-License-Identifier: Apache-2.0
//! Structured bindings and range-based `for`.

use super::Resolved;
use super::ctx::{Checker, VarSym};
use super::val::*;
use crate::cpp::ast::*;

/// The parts a structured binding can unpack.
fn components(
    t: &Ty,
    structs: &std::collections::HashMap<String, super::ctx::StructSym>,
) -> Option<Vec<Ty>> {
    Some(match t {
        Ty::Pair(a, b) => vec![(**a).clone(), (**b).clone()],
        Ty::Tuple(ts) => ts.clone(),
        Ty::Array(e, n) => vec![(**e).clone(); *n],
        Ty::Struct(s) => structs.get(s)?.fields.iter().map(|f| f.1.clone()).collect(),
        _ => return None,
    })
}

impl<'a> Checker<'a> {
    /// `auto [a, b] = v;` (also the structured form of range-for): declare the names.
    pub fn bind_names(
        &mut self,
        stmt: NodeId,
        spec: &TypeSpec,
        names: &[String],
        v: &Val,
        node: NodeId,
    ) {
        let mut out: Vec<(String, Ty)> = Vec::new();
        if v.is_unknown() {
            for (i, n) in names.iter().enumerate() {
                let sym = VarSym {
                    ty: Ty::Unknown,
                    konst: false,
                    ic: false,
                    id: Some(Resolved::Binding { stmt, index: i }),
                    cval: None,
                };
                self.declare(node, n, sym);
                out.push((n.clone(), Ty::Unknown));
            }
            self.info.bindings.insert(stmt, out);
            return;
        }
        let Some(comps) = components(&v.ty, &self.structs) else {
            self.err(node, "type.structured", format!("`auto [..]` can only unpack a std::pair, std::tuple, std::array or struct, but this is a {}", show(v)));
            return self.bind_names(stmt, spec, names, &Val::unknown(), node);
        };
        if comps.len() != names.len() {
            self.err(
                node,
                "type.structured",
                format!(
                    "a {} has {} part(s) but you named {}",
                    v.ty,
                    comps.len(),
                    names.len()
                ),
            );
            return self.bind_names(stmt, spec, names, &Val::unknown(), node);
        }
        if spec.is_ref && !spec.konst && !v.lval {
            self.err(
                node,
                "type.temporary",
                "`auto& [..]` cannot refer to a temporary value; use `const auto&` or `auto`",
            );
        }
        let konst = spec.konst || (spec.is_ref && v.konst);
        let first_const = spec.is_ref && v.ic && matches!(v.ty, Ty::Pair(..));
        for (i, (n, t)) in names.iter().zip(comps).enumerate() {
            let sym = VarSym {
                ty: t.clone(),
                konst: konst || (i == 0 && first_const),
                ic: false,
                id: Some(Resolved::Binding { stmt, index: i }),
                cval: None,
            };
            self.declare(node, n, sym);
            out.push((n.clone(), t));
        }
        self.info.bindings.insert(stmt, out);
    }

    /// `for (spec names : range)`: declare the loop variable(s) in the current scope.
    pub fn range_for(
        &mut self,
        stmt: NodeId,
        spec: &TypeSpec,
        names: &[String],
        structured: bool,
        range: &'a Expr,
    ) {
        let rv = self.expr(range);
        let elem = if rv.is_unknown() {
            Val::unknown()
        } else if let (Ty::InitList, Some(list)) = (&rv.ty, &rv.list) {
            match list.split_first() {
                Some((f, rest)) if rest.iter().all(|x| x.ty == f.ty) => Val::lv(f.ty.clone(), true),
                _ => {
                    self.err(
                        range.id,
                        "type.deduce",
                        "the values in the braces must all have the same type to loop over them",
                    );
                    Val::unknown()
                }
            }
        } else {
            match self.range_elem(&rv) {
                Some(e) => e,
                None => {
                    self.err(
                        range.id,
                        "type.range",
                        format!(
                            "you cannot loop over a {} with `for (x : ...)`: it is not a container",
                            show(&rv)
                        ),
                    );
                    Val::unknown()
                }
            }
        };
        if structured {
            self.bind_names(stmt, spec, names, &elem, range.id);
            return;
        }
        let name = names.first().cloned().unwrap_or_default();
        let p = self.spec_param(spec, range.id);
        let sym = if p.ty == Ty::Auto {
            let (ty, konst, ic) =
                self.deduce(range.id, spec, &elem, &name)
                    .unwrap_or((Ty::Unknown, false, false));
            VarSym {
                ty,
                konst,
                ic,
                id: Some(Resolved::Binding { stmt, index: 0 }),
                cval: None,
            }
        } else {
            self.bind_to(range.id, &elem, &p, "the loop variable");
            VarSym {
                ty: p.ty.clone(),
                konst: p.konst,
                ic: false,
                id: Some(Resolved::Binding { stmt, index: 0 }),
                cval: None,
            }
        };
        self.info
            .bindings
            .insert(stmt, vec![(name.clone(), sym.ty.clone())]);
        self.declare(range.id, &name, sym);
    }
}
