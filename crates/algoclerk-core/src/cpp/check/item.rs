// SPDX-License-Identifier: Apache-2.0
//! Top-level items: includes, aliases, structs, functions, globals and the entry signature.

use super::ctx::{Checker, FnCtx, FnSym, ParamTy, RetKind, StructSym, VarSym};
use super::val::*;
use super::{EntrySig, Resolved};
use crate::cpp::ast::*;

/// Headers the subset knows about.
const KNOWN_HEADERS: &[&str] = &[
    "vector",
    "string",
    "string_view",
    "unordered_map",
    "map",
    "set",
    "unordered_set",
    "stack",
    "queue",
    "deque",
    "list",
    "array",
    "algorithm",
    "numeric",
    "functional",
    "utility",
    "tuple",
    "optional",
    "iterator",
    "limits",
    "climits",
    "cctype",
    "cstdlib",
    "cstddef",
    "cstring",
    "cmath",
];

impl<'a> Checker<'a> {
    /// Check the whole program.
    pub fn run(&mut self) {
        let p = self.p;
        for (idx, it) in p.items.iter().enumerate() {
            let player = idx >= p.prelude_items;
            match &it.kind {
                ItemKind::Include { path, angle } => {
                    if player {
                        self.include_item(it.id, path, *angle);
                    }
                }
                ItemKind::UsingNamespaceStd => {}
                ItemKind::Alias { name, ty, .. } => {
                    let t = self.fix_ty(ty, it.id);
                    self.aliases.insert(name.clone(), t);
                }
                ItemKind::Struct(sd) => self.struct_item(sd, player),
                ItemKind::Func(f) => self.func_item(f, player),
                ItemKind::Global { spec, vars, .. } => {
                    for d in vars {
                        self.decl_var(spec, d);
                    }
                }
            }
        }
        self.entry_check();
        self.flow_checks();
        self.unseq_checks();
    }

    fn include_item(&mut self, id: NodeId, path: &str, angle: bool) {
        let h = format!("<{path}>");
        if !angle || path == "bits/stdc++.h" || path == "cstdint" || !KNOWN_HEADERS.contains(&path)
        {
            let why = match path {
                "bits/stdc++.h" => "it is not a standard header; include the specific headers you need".to_string(),
                "cstdint" | "stdint.h" => "its sized integer types differ between systems; write `int` or `long long`".to_string(),
                "iostream" | "cstdio" | "fstream" | "sstream" => "there is no input or output in the subset: the clerk hands you the arguments and takes your return value".to_string(),
                _ => "it is outside the subset".to_string(),
            };
            self.err(
                id,
                "unsupported.header",
                format!(
                    "the supply cupboard does not stock `#include {}`: {why}",
                    if angle { h } else { format!("\"{path}\"") }
                ),
            );
            return;
        }
        // a lock wins over everything: show its reason
        if !self.check_locked(id, std::slice::from_ref(&h)) {
            return;
        }
        // including a header the prelude already includes is legal C++ and always accepted
        if let Some(set) = &self.o.unlocked_headers
            && !set.contains(&h)
            && !self.prelude_headers.contains(&h)
        {
            self.err(
                id,
                "lock.header",
                format!("the supply cupboard on this floor does not stock `{h}` yet"),
            );
        }
    }

    fn struct_item(&mut self, sd: &'a StructDecl, player: bool) {
        if player && !self.o.structs {
            self.err(
                sd.id,
                "lock.structs",
                format!(
                    "this floor does not let you declare your own structs (`{}`)",
                    sd.name
                ),
            );
        }
        if self.structs.contains_key(&sd.name) {
            self.err(
                sd.id,
                "name.duplicate",
                format!("the struct `{}` is already declared", sd.name),
            );
            return;
        }
        self.structs.insert(sd.name.clone(), StructSym::default());
        let mut fields: Vec<(String, Ty, bool)> = Vec::new();
        for f in &sd.fields {
            let t = self.spec_param(&f.spec, sd.id).ty;
            if fields.iter().any(|x| x.0 == f.name) {
                self.err(
                    sd.id,
                    "name.duplicate",
                    format!("`{}` has two members called `{}`", sd.name, f.name),
                );
            }
            fields.push((f.name.clone(), t.clone(), f.spec.konst));
            if let Some(d) = &f.default {
                let v = self.expr(d);
                self.convert_to(d.id, &v, &t, &format!("the default value of `{}`", f.name));
            }
        }
        let mut ctors = Vec::new();
        for c in &sd.ctors {
            let ps: Vec<ParamTy> = c
                .params
                .iter()
                .map(|p| self.spec_param(&p.spec, p.id))
                .collect();
            ctors.push(ps);
        }
        if let Some(s) = self.structs.get_mut(&sd.name) {
            s.fields = fields.clone();
            s.ctors = ctors.clone();
        }
        for (c, ps) in sd.ctors.iter().zip(&ctors) {
            self.fctx.push(FnCtx {
                base: self.scopes.len(),
                ret: RetKind::Fixed(ParamTy {
                    ty: Ty::Void,
                    konst: false,
                    is_ref: false,
                }),
                lambda: None,
                rets: Vec::new(),
            });
            self.push_scope();
            for (n, t, k) in &fields {
                self.declare(
                    c.id,
                    n,
                    VarSym {
                        ty: t.clone(),
                        konst: *k,
                        ic: false,
                        id: Some(Resolved::Field(n.clone())),
                        cval: None,
                    },
                );
            }
            self.push_scope();
            for (p, pt) in c.params.iter().zip(ps) {
                self.info.put(p.id, pt.ty.clone(), true, pt.konst);
                self.declare(
                    p.id,
                    &p.name,
                    VarSym {
                        ty: pt.ty.clone(),
                        konst: pt.konst,
                        ic: false,
                        id: Some(Resolved::Var(p.id)),
                        cval: None,
                    },
                );
            }
            for (field, args) in &c.inits {
                let vals: Vec<Val> = args.iter().map(|a| self.expr(a)).collect();
                match fields.iter().find(|f| &f.0 == field) {
                    Some((_, t, _)) => {
                        self.construct(c.id, t, &vals, false);
                    }
                    None => self.err(
                        c.id,
                        "name.member",
                        format!("`{}` has no member `{field}` to initialise", sd.name),
                    ),
                }
            }
            self.block_in_scope(&c.body);
            self.pop_scope();
            self.pop_scope();
            self.fctx.pop();
        }
    }

    fn func_item(&mut self, f: &'a FnDecl, player: bool) {
        if player && !self.o.helpers && self.o.entry.as_ref().is_some_and(|e| e.name != f.name) {
            self.err(f.id, "lock.helpers", format!("this floor does not allow helper functions: `{}` is not the function on the form", f.name));
        }
        let params: Vec<ParamTy> = f
            .params
            .iter()
            .map(|p| self.spec_param(&p.spec, p.id))
            .collect();
        let ret = self.spec_param(&f.ret, f.id);
        if params.iter().any(|p| p.ty == Ty::Auto) || ret.ty == Ty::Auto {
            self.err(f.id, "unsupported.auto", format!("`{}`: `auto` parameters and return types are not in the subset; write the type", f.name));
        }
        self.info.put(f.id, ret.ty.clone(), false, false);
        let list = self.funcs.entry(f.name.clone()).or_default();
        match list.iter().position(|s| s.params == params) {
            Some(i) => {
                if list[i].ret != ret {
                    self.err(
                        f.id,
                        "type.redeclare",
                        format!(
                            "`{}` was declared before with a different return type",
                            f.name
                        ),
                    );
                } else if f.body.is_some() && self.defined(&f.name, &params) {
                    self.err(
                        f.id,
                        "name.duplicate",
                        format!("`{}` is defined twice; keep one definition", f.name),
                    );
                }
                if f.body.is_some()
                    && let Some(l) = self.funcs.get_mut(&f.name)
                {
                    l[i].id = f.id;
                }
            }
            None => list.push(FnSym {
                id: f.id,
                params: params.clone(),
                ret: ret.clone(),
            }),
        }
        let Some(body) = &f.body else { return };
        self.mark_defined(&f.name, &params);
        self.fctx.push(FnCtx {
            base: self.scopes.len(),
            ret: RetKind::Fixed(ret),
            lambda: None,
            rets: Vec::new(),
        });
        self.push_scope();
        for (p, pt) in f.params.iter().zip(&params) {
            self.info.put(p.id, pt.ty.clone(), true, pt.konst);
            self.declare(
                p.id,
                &p.name,
                VarSym {
                    ty: pt.ty.clone(),
                    konst: pt.konst,
                    ic: false,
                    id: Some(Resolved::Var(p.id)),
                    cval: None,
                },
            );
        }
        self.block_in_scope(body);
        self.pop_scope();
        self.fctx.pop();
    }

    fn defined(&self, name: &str, params: &[ParamTy]) -> bool {
        self.defs.iter().any(|(n, p)| n == name && p == params)
    }

    fn mark_defined(&mut self, name: &str, params: &[ParamTy]) {
        self.defs.push((name.to_string(), params.to_vec()));
    }

    /// The player's function must match the signature on the form exactly.
    fn entry_check(&mut self) {
        let Some(entry) = self.o.entry.clone() else {
            return;
        };
        let p = self.p;
        let Some(f) = p.player_items().iter().find_map(|i| match &i.kind {
            ItemKind::Func(f) if f.name == entry.name && f.body.is_some() => Some(f),
            _ => None,
        }) else {
            let node = p.items.last().map_or(0, |i| i.id);
            self.err(node, "name.entry", format!("the form on this desk wants `{}`, and I cannot find a function called `{}` in your code", entry.text(), entry.name));
            return;
        };
        let norm = |c: &mut Self, s: &TypeSpec| TypeSpec {
            ty: c.fix_ty(&s.ty, f.id),
            spelled: None,
            ..s.clone()
        };
        let same = f.params.len() == entry.params.len()
            && f.params
                .iter()
                .zip(&entry.params)
                .all(|(a, b)| norm(self, &a.spec) == norm(self, &b.spec))
            && norm(self, &f.ret) == norm(self, &entry.ret);
        if !same {
            let mine = EntrySig {
                name: f.name.clone(),
                params: f.params.clone(),
                ret: f.ret.clone(),
            };
            self.err(
                f.id,
                "type.entry",
                format!(
                    "the form on this desk wants `{}`, but you wrote `{}`",
                    entry.text(),
                    mine.text()
                ),
            );
        }
    }
}
