// SPDX-License-Identifier: Apache-2.0
//! The C translator. The generated program is C11 in two parts: a helper prelude (typed vectors, strings,
//! hash maps and friends instantiated from small macro templates, collapsed by default in the game) and the
//! player's functions written plainly against it. References become pointers, copies of containers are
//! explicit (`Vec_int_copy`), `ListNode*` and `TreeNode*` are plain structs, there is no `goto`.

use super::Generated;
use super::Lang;
use super::common::*;
use crate::cpp::ast::*;
use crate::cpp::check::TypeInfo;
use crate::cpp::run::Compiled;
use std::collections::{HashMap, HashSet};

mod assign;
mod call;
mod expr;
mod lib_funcs;
mod lib_methods;
mod loops;
mod stmt;
mod structs;
mod tmpl_base;
mod tmpl_more;
mod types;

pub(super) use tmpl_base::STR as STR_TEMPLATE;

const RESERVED: &[&str] = &[
    "auto", "char", "const", "double", "enum", "extern", "float", "int", "long", "register",
    "restrict", "short", "signed", "sizeof", "static", "struct", "typedef", "union", "unsigned",
    "void", "volatile", "inline", "bool", "true", "false", "NULL", "main", "free", "exit", "abs",
    "div", "rand", "time", "index", "printf", "puts", "read", "write", "round", "floor", "ceil",
    "pow", "log", "exp", "sqrt", "signal", "remove", "rename", "system", "malloc", "calloc",
    "realloc", "memcpy", "memset", "strlen", "strcmp", "isdigit", "isalpha", "min", "max", "swap",
    "Str", "SWAP", "uint", "uchar", "ll", "ull",
];

/// A C function name for a C++ one.
pub fn fn_name(n: &str) -> String {
    ident(n, RESERVED)
}

/// How a variable is held in the generated code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rk {
    /// By value.
    Val,
    /// A pointer to the value (reference parameters and variables).
    Ref,
    /// A native C array (`int a[26]`).
    Native,
    /// An index into the container `cont` (an iterator from `find`).
    Found {
        /// Container place text.
        cont: String,
    },
    /// A lifted lambda: the name of the C function.
    Lambda(String),
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
pub struct Cx<'a> {
    pub(super) prog: &'a Program,
    pub(super) info: &'a TypeInfo,
    pub(super) scopes: Vec<HashMap<String, Var>>,
    pub(super) types: Vec<(Ty, String)>,
    pub(super) used_structs: Vec<String>,
    pub(super) ret: Ty,
    pub(super) tmp: usize,
    pub(super) lifted: Vec<String>,
    pub(super) allow_generic: bool,
}

impl<'a> Cx<'a> {
    pub(super) fn new(prog: &'a Program, info: &'a TypeInfo) -> Cx<'a> {
        Cx {
            prog,
            info,
            scopes: vec![HashMap::new()],
            types: Vec::new(),
            used_structs: Vec::new(),
            ret: Ty::Void,
            tmp: 0,
            lifted: Vec::new(),
            allow_generic: false,
        }
    }
}

/// Translate to C.
pub fn generate(comp: &Compiled, entry: &str) -> R<Generated> {
    let mut cx = Cx::new(&comp.program, &comp.info);
    let mut seen: HashSet<&str> = HashSet::new();
    for f in cx.prog.fns() {
        if f.body.is_some() && !seen.insert(f.name.as_str()) {
            return no(
                "C",
                format!(
                    "the overloaded function `{}` (C has no overloading)",
                    f.name
                ),
            );
        }
    }
    if cx.prog.find(entry).is_none() {
        return Err(format!("there is no function named {entry} to translate"));
    }
    let mut code = String::new();
    for item in cx.prog.player_items() {
        match &item.kind {
            ItemKind::Func(f) => {
                let text = cx.function(f)?;
                for l in std::mem::take(&mut cx.lifted) {
                    if !code.contains(&l) {
                        code.push_str(&l);
                        code.push('\n');
                    }
                }
                code.push_str(&text);
                code.push('\n');
            }
            ItemKind::Global { vars, .. } => {
                for v in vars {
                    code.push_str(&cx.global(v)?);
                }
                code.push('\n');
            }
            ItemKind::Struct(s) => cx.use_struct(&s.name)?,
            _ => {}
        }
    }
    let prelude = cx.prelude_text()?;
    Ok(Generated {
        lang: Lang::C,
        prelude,
        code: code.trim_end().to_string() + "\n",
    })
}

impl Cx<'_> {
    /// Every type instantiated so far with its prelude name, contained types first.
    pub(in crate::cpp::codegen) fn registry(&self) -> Vec<(Ty, String)> {
        self.types.clone()
    }

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

    /// A C parameter declaration and the variable kind for a C++ parameter.
    pub(super) fn param(&mut self, spec: &TypeSpec, name: &str) -> R<(String, Rk)> {
        let t = &spec.ty;
        if matches!(t, Ty::Function(..) | Ty::Lambda(_) | Ty::Auto) {
            return no("C", "a function taking a lambda or std::function");
        }
        let ct = self.ctype(t)?;
        let sp = if ct.ends_with('*') { "" } else { " " };
        let n = self.name(name);
        let by_ptr = spec.is_ref && !(spec.konst && self.is_plain(t));
        if by_ptr {
            return Ok((format!("{ct}{sp}*{n}"), Rk::Ref));
        }
        Ok((format!("{ct}{sp}{n}"), Rk::Val))
    }

    fn function(&mut self, f: &FnDecl) -> R<String> {
        self.ret = f.ret.ty.clone();
        self.push_scope();
        let mut params = Vec::new();
        for p in &f.params {
            let (decl, rk) = self.param(&p.spec, &p.name)?;
            params.push(decl);
            self.declare(&p.name, p.spec.ty.clone(), rk);
        }
        let rt = self.ctype(&f.ret.ty)?;
        let sp = if rt.ends_with('*') { "" } else { " " };
        let ps = if params.is_empty() {
            "void".to_string()
        } else {
            params.join(", ")
        };
        let mut out = String::new();
        for c in &f.comments {
            out.push_str(&format!("//{c}\n"));
        }
        let head = format!("{rt}{sp}{}({ps})", self.name(&f.name));
        let Some(body) = &f.body else {
            self.pop_scope();
            return Ok(format!("{out}{head};\n"));
        };
        out.push_str(&format!("{head} {{\n"));
        let mut b = String::new();
        self.stmts(&body.stmts, 1, &mut b)?;
        out.push_str(&b);
        out.push_str("}\n");
        self.pop_scope();
        Ok(out)
    }

    fn global(&mut self, v: &Declarator) -> R<String> {
        let ty = self.info.decl_ty(v.id).clone();
        if !ty.is_integral() || v.array.is_some() {
            return no("C", "a global that is not an integer constant");
        }
        let Init::Copy(e) = &v.init else {
            return no("C", "a global without a value");
        };
        let ct = self.ctype(&ty)?;
        let x = self.conv_val(e, &ty)?;
        self.declare(&v.name, ty, Rk::Val);
        Ok(format!(
            "static const {ct} {} = {};\n",
            self.name(&v.name),
            x.s
        ))
    }
}

/// `(*p).x` as `p->x` (the translation works with `(*p)` places and tidies the text afterwards).
pub(super) fn tidy(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'(' && i + 1 < b.len() && b[i + 1] == b'*' {
            let mut j = i + 2;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j > i + 2
                && j + 1 < b.len()
                && b[j] == b')'
                && b[j + 1] == b'.'
                && !b[i + 2].is_ascii_digit()
            {
                out.push_str(&s[i + 2..j]);
                out.push_str("->");
                i = j + 2;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}
