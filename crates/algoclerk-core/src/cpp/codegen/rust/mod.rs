// SPDX-License-Identifier: Apache-2.0
//! The Rust translator: idiomatic Rust 2021 from the checked C++ tree. `Vec`, `String`, `HashMap`,
//! `BTreeMap`, `VecDeque`, `BinaryHeap`, iterators and `for` loops; references become `&` / `&mut`;
//! linked structures (`ListNode*`, `TreeNode*`) become `Copy` handles into a small thread-local arena.

use super::Generated;
use super::Lang;
use super::common::*;
use crate::cpp::ast::*;
use crate::cpp::check::TypeInfo;
use crate::cpp::run::Compiled;
use std::collections::{BTreeSet, HashMap, HashSet};

mod assign;
mod call;
mod expr;
mod lib_free;
mod lib_funcs;
mod lib_methods;
mod lib_str;
mod loops;
mod prelude;
mod stmt;
mod structs;
mod ty;

pub(super) const RESERVED: &[&str] = &[
    "as", "async", "await", "crate", "dyn", "enum", "extern", "fn", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "self", "Self", "super", "trait", "type",
    "unsafe", "use", "where", "box", "final", "macro", "override", "priv", "typeof", "unsized",
    "virtual", "yield", "try", "abstract", "become", "gen",
];

/// A C++ function name as a Rust identifier.
pub fn fn_name(n: &str) -> String {
    ident(n, RESERVED)
}

/// How a variable is held in the generated code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rk {
    /// By value.
    Val,
    /// A shared reference.
    Ref,
    /// A mutable reference.
    RefMut,
    /// An `Option` of a reference (an iterator from `find`); `true` when it is `Option<&mut V>`.
    Found(bool),
    /// A map entry `(&K, &V)` bound by `for (const auto& kv : map)`.
    Entry,
    /// A `&str` parameter.
    StrRef,
}

/// A variable in scope.
#[derive(Clone, Debug)]
pub struct Var {
    /// Value type.
    pub ty: Ty,
    /// How it is held.
    pub rk: Rk,
}

/// The translator state.
#[derive(Debug)]
pub struct Rs<'a> {
    pub(super) prog: &'a Program,
    pub(super) info: &'a TypeInfo,
    pub(super) scopes: Vec<HashMap<String, Var>>,
    pub(super) uses: BTreeSet<&'static str>,
    pub(super) helpers: BTreeSet<&'static str>,
    pub(super) used_structs: Vec<String>,
    pub(super) mutated: HashSet<String>,
    pub(super) ret: Ty,
    pub(super) in_lambda: usize,
    pub(super) cont: Vec<Option<String>>,
    pub(super) tmp: usize,
    pub(super) lams: HashMap<NodeId, &'a Lambda>,
}

/// Translate to Rust.
pub fn generate(comp: &Compiled, entry: &str) -> R<Generated> {
    let mut rs = Rs {
        prog: &comp.program,
        info: &comp.info,
        scopes: vec![HashMap::new()],
        uses: BTreeSet::new(),
        helpers: BTreeSet::new(),
        used_structs: Vec::new(),
        mutated: HashSet::new(),
        ret: Ty::Void,
        in_lambda: 0,
        cont: Vec::new(),
        tmp: 0,
        lams: HashMap::new(),
    };
    rs.collect_lambdas();
    let mut code = String::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for f in rs.prog.fns() {
        if f.body.is_some() && !seen.insert(f.name.as_str()) {
            return no(
                "Rust",
                format!(
                    "the overloaded function `{}` (Rust has no overloading)",
                    f.name
                ),
            );
        }
    }
    if rs.prog.find(entry).is_none() {
        return Err(format!("there is no function named {entry} to translate"));
    }
    for item in rs.prog.player_items() {
        match &item.kind {
            ItemKind::Func(f) if f.body.is_some() => {
                code.push_str(&rs.function(f)?);
                code.push('\n');
            }
            ItemKind::Global { spec, vars, .. } => {
                code.push_str(&rs.global(spec, vars)?);
                code.push('\n');
            }
            ItemKind::Struct(s) => {
                rs.use_struct(&s.name)?;
            }
            _ => {}
        }
    }
    let mut code = code.trim_end().to_string();
    code.push('\n');
    Ok(Generated {
        lang: Lang::Rust,
        prelude: rs.prelude_text()?,
        code,
    })
}

impl<'a> Rs<'a> {
    fn collect_lambdas(&mut self) {
        let prog = self.prog;
        let mut lams: HashMap<NodeId, &'a Lambda> = HashMap::new();
        for item in &prog.items {
            if let ItemKind::Func(f) = &item.kind
                && let Some(b) = &f.body
            {
                walk_stmts(b, &mut |s| {
                    stmt_exprs(s, &mut |e| {
                        walk_expr(e, &mut |x| {
                            if let ExprKind::Lambda(l) = &x.kind {
                                lams.insert(x.id, &**l);
                            }
                        });
                    });
                });
            }
        }
        self.lams = lams;
    }
}

impl Rs<'_> {
    pub(super) fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    pub(super) fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    pub(super) fn declare(&mut self, name: &str, ty: Ty, rk: Rk) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), Var { ty, rk });
        }
    }
    pub(super) fn lookup(&self, name: &str) -> Option<&Var> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    pub(super) fn fresh(&mut self, base: &str) -> String {
        self.tmp += 1;
        format!("{base}{}", self.tmp)
    }
    pub(super) fn name(&self, n: &str) -> String {
        ident(n, RESERVED)
    }

    /// The Rust parameter type and variable kind for a C++ parameter.
    fn param_ty(&mut self, spec: &TypeSpec) -> R<(String, Rk)> {
        let t = &spec.ty;
        if matches!(t, Ty::Function(..) | Ty::Lambda(_) | Ty::Auto) {
            return no("Rust", "a function taking a lambda or std::function");
        }
        if spec.is_ref && spec.konst {
            return Ok(match t {
                Ty::Str | Ty::StrView => ("&str".to_string(), Rk::StrRef),
                _ => (format!("&{}", self.rty(t)?), Rk::Ref),
            });
        }
        if spec.is_ref {
            return Ok((format!("&mut {}", self.rty(t)?), Rk::RefMut));
        }
        if *t == Ty::StrView {
            return Ok(("&str".to_string(), Rk::StrRef));
        }
        Ok((self.rty(t)?, Rk::Val))
    }

    fn function(&mut self, f: &FnDecl) -> R<String> {
        let Some(body) = &f.body else {
            return Ok(String::new());
        };
        self.mutated = mutated_names(self.prog, self.info, body);
        self.ret = f.ret.ty.clone();
        self.push_scope();
        let mut params = Vec::new();
        for p in &f.params {
            let (t, rk) = self.param_ty(&p.spec)?;
            let n = self.name(&p.name);
            let m = if rk == Rk::Val && self.mutated.contains(&p.name) {
                "mut "
            } else {
                ""
            };
            params.push(format!("{m}{n}: {t}"));
            let ty = if p.spec.ty == Ty::StrView {
                Ty::Str
            } else {
                p.spec.ty.clone()
            };
            self.declare(&p.name, ty, rk);
        }
        let ret = match &f.ret.ty {
            Ty::Void => String::new(),
            t => format!(" -> {}", self.rty(t)?),
        };
        let mut out = String::new();
        for c in &f.comments {
            out.push_str(&format!("//{c}\n"));
        }
        out.push_str(&format!(
            "fn {}({}){ret} {{\n",
            self.name(&f.name),
            params.join(", ")
        ));
        let b = self.fn_body(body, &f.ret.ty)?;
        out.push_str(&b);
        out.push_str("}\n");
        self.pop_scope();
        Ok(out)
    }

    fn global(&mut self, spec: &TypeSpec, vars: &[Declarator]) -> R<String> {
        let mut out = String::new();
        for v in vars {
            let ty = self.info.decl_ty(v.id).clone();
            if !ty.is_integral() || v.array.is_some() {
                return no("Rust", "a global that is not an integer constant");
            }
            let Init::Copy(e) = &v.init else {
                return no("Rust", "a global without a value");
            };
            let t = self.rty(&ty)?;
            let x = self.conv_val(e, &ty)?;
            out.push_str(&format!("const {}: {t} = {};\n", self.name(&v.name), x.s));
            self.declare(&v.name, ty, Rk::Val);
        }
        let _ = spec;
        Ok(out)
    }
}
