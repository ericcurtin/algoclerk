// SPDX-License-Identifier: Apache-2.0
//! The checker's state: scopes, symbol tables, diagnostics, headers and locks.

use super::val::*;
use super::{CheckOpts, Resolved, TypeInfo};
use crate::cpp::ast::*;
use crate::cpp::diag::Diag;
use std::collections::{BTreeSet, HashMap, HashSet};

/// A parameter or return type as declared: type plus `const` and `&`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParamTy {
    pub ty: Ty,
    pub konst: bool,
    pub is_ref: bool,
}

/// A user function (one overload).
#[derive(Clone, Debug)]
pub(super) struct FnSym {
    pub id: NodeId,
    pub params: Vec<ParamTy>,
    pub ret: ParamTy,
}

/// A user struct.
#[derive(Clone, Debug, Default)]
pub(super) struct StructSym {
    pub fields: Vec<(String, Ty, bool)>,
    pub ctors: Vec<Vec<ParamTy>>,
}

/// Scopes and function contexts at a lambda's definition.
pub(super) type LamEnv = (Vec<Vec<(String, VarSym)>>, Vec<FnCtx>);

/// A lambda seen so far.
#[derive(Clone, Debug)]
pub(super) struct LamInfo<'a> {
    pub ast: &'a Lambda,
    pub params: Vec<ParamTy>,
    pub ret: Option<Ty>,
    pub generic: bool,
    /// Scopes and function contexts at the definition (generic lambdas are re-checked per call).
    pub env: Option<LamEnv>,
}

/// A variable in scope.
#[derive(Clone, Debug)]
pub(super) struct VarSym {
    pub ty: Ty,
    pub konst: bool,
    pub ic: bool,
    pub id: Option<Resolved>,
    pub cval: Option<i128>,
}

/// How `return e;` is checked in the current function.
#[derive(Clone, Debug)]
pub(super) enum RetKind {
    Fixed(ParamTy),
    Deduce,
}

/// Capture rules of the lambda being checked.
#[derive(Clone, Debug)]
pub(super) struct LamCap {
    pub default: CaptureDefault,
    pub items: Vec<(String, bool)>,
    pub mutable: bool,
}

/// One function body (or lambda body) being checked.
#[derive(Clone, Debug)]
pub(super) struct FnCtx {
    pub base: usize,
    pub ret: RetKind,
    pub lambda: Option<LamCap>,
    pub rets: Vec<Ty>,
}

/// The checker state.
pub(super) struct Checker<'a> {
    pub p: &'a Program,
    pub o: &'a CheckOpts,
    pub info: TypeInfo,
    pub diags: Vec<Diag>,
    pub scopes: Vec<Vec<(String, VarSym)>>,
    pub funcs: HashMap<String, Vec<FnSym>>,
    pub fn_names: HashSet<String>,
    pub structs: HashMap<String, StructSym>,
    pub aliases: HashMap<String, Ty>,
    pub lams: HashMap<NodeId, LamInfo<'a>>,
    pub fctx: Vec<FnCtx>,
    pub headers: BTreeSet<String>,
    /// Headers the level prelude includes (the player may repeat them).
    pub prelude_headers: BTreeSet<String>,
    pub using_std: bool,
    pub depth: u32,
    /// Functions with a body seen so far (name and parameter types).
    pub defs: Vec<(String, Vec<ParamTy>)>,
    /// Case values seen so far in each enclosing `switch`.
    pub cases: Vec<Vec<i128>>,
    seen: HashSet<(NodeId, &'static str)>,
}

impl<'a> Checker<'a> {
    /// A fresh checker: reads includes and `using namespace std` from every item.
    pub fn new(p: &'a Program, o: &'a CheckOpts) -> Checker<'a> {
        let mut headers = BTreeSet::new();
        let mut prelude_headers = BTreeSet::new();
        let mut using_std = false;
        let mut fn_names = HashSet::new();
        for (idx, it) in p.items.iter().enumerate() {
            match &it.kind {
                ItemKind::Include { path, angle: true } => {
                    headers.insert(format!("<{path}>"));
                    if idx < p.prelude_items {
                        prelude_headers.insert(format!("<{path}>"));
                    }
                }
                ItemKind::UsingNamespaceStd => using_std = true,
                ItemKind::Func(f) => {
                    fn_names.insert(f.name.clone());
                }
                _ => {}
            }
        }
        Checker {
            p,
            o,
            info: TypeInfo::default(),
            diags: Vec::new(),
            scopes: vec![Vec::new()],
            funcs: HashMap::new(),
            fn_names,
            structs: HashMap::new(),
            aliases: HashMap::new(),
            lams: HashMap::new(),
            fctx: Vec::new(),
            headers,
            prelude_headers,
            using_std,
            depth: 0,
            defs: Vec::new(),
            cases: Vec::new(),
            seen: HashSet::new(),
        }
    }

    /// The result: the type tables, or the first five diagnostics.
    pub fn finish(mut self) -> Result<TypeInfo, Vec<Diag>> {
        if self.diags.is_empty() {
            Ok(self.info)
        } else {
            self.diags.truncate(5);
            Err(self.diags)
        }
    }

    /// Report a problem (the same code on the same node is reported once).
    pub fn err(&mut self, node: NodeId, code: &'static str, msg: impl Into<String>) {
        let msg: String = msg.into();
        if self.seen.insert((node, code)) && self.diags.len() < 50 {
            self.diags.push(Diag::node(node, code, fix_articles(&msg)));
        }
    }

    /// Record the type of an expression node.
    pub fn set(&mut self, id: NodeId, v: &Val) {
        self.info.put(id, v.ty.clone(), v.lval, v.konst);
    }

    // ---- scopes -------------------------------------------------------------------------------

    /// Open a block scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    /// Close a block scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Declare a name in the innermost scope; a second declaration in the same scope is an error.
    pub fn declare(&mut self, node: NodeId, name: &str, sym: VarSym) {
        if name.is_empty() {
            return;
        }
        let top = self.scopes.len() - 1;
        if self.scopes[top].iter().any(|(n, _)| n == name) {
            self.err(
                node,
                "name.duplicate",
                format!("`{name}` is already declared in this block; pick another name or reuse the old one"),
            );
            return;
        }
        self.scopes[top].push((name.to_string(), sym));
    }

    /// Remember the variables visible before a statement (for `scope_at`).
    pub fn snapshot(&mut self, stmt: NodeId) {
        let mut seen: Vec<(String, Ty)> = Vec::new();
        for (n, s) in self.scopes.iter().skip(1).flatten() {
            seen.retain(|(m, _)| m != n);
            seen.push((n.clone(), s.ty.clone()));
        }
        for (n, s) in &self.scopes[0] {
            if !seen.iter().any(|(m, _)| m == n) {
                seen.insert(0, (n.clone(), s.ty.clone()));
            }
        }
        self.info.scopes.insert(stmt, seen);
    }

    /// Find a variable. Crossing a lambda boundary applies the capture rules (error when not captured,
    /// `const` inside a non-`mutable` lambda when captured by copy).
    pub fn lookup(&mut self, name: &str, node: NodeId) -> Option<VarSym> {
        let (idx, mut sym) = self.scopes.iter().enumerate().rev().find_map(|(i, sc)| {
            sc.iter()
                .rev()
                .find(|(n, _)| n == name)
                .map(|(_, s)| (i, s.clone()))
        })?;
        if idx == 0 {
            return Some(sym);
        }
        let mut errors = Vec::new();
        for f in self.fctx.iter().rev() {
            if f.base <= idx {
                break;
            }
            let Some(cap) = &f.lambda else { continue };
            let item = cap.items.iter().find(|(n, _)| n == name);
            let by_ref = match (item, cap.default) {
                (Some((_, r)), _) => *r,
                (None, CaptureDefault::Ref) => true,
                (None, CaptureDefault::Copy) => false,
                (None, CaptureDefault::None) => {
                    errors.push(name.to_string());
                    true
                }
            };
            if !by_ref && !cap.mutable {
                sym.konst = true;
            }
        }
        for n in errors {
            self.err(
                node,
                "name.capture",
                format!("`{n}` lives outside this lambda and its capture list `[...]` does not mention it; write `[&]`, `[=]` or `[{n}]`"),
            );
        }
        Some(sym)
    }

    // ---- headers and locks --------------------------------------------------------------------

    /// Is the header (or an equivalent one that provides the same names with every standard library) included?
    pub fn has_header(&self, h: &str) -> bool {
        if self.headers.contains(h) {
            return true;
        }
        let alts: &[&str] = match h {
            "<utility>" => &["<tuple>", "<map>", "<unordered_map>", "<algorithm>"],
            // corpus/61 uses std::abs with <algorithm> only, which both libstdc++ and libc++ accept
            "<cstdlib>" => &["<cmath>", "<algorithm>"],
            "<cstddef>" => &[
                "<vector>",
                "<string>",
                "<cstdlib>",
                "<cstring>",
                "<array>",
                "<string_view>",
                "<map>",
                "<set>",
                "<unordered_map>",
                "<unordered_set>",
                "<deque>",
                "<list>",
                "<queue>",
                "<stack>",
                "<algorithm>",
                "<numeric>",
            ],
            _ => &[],
        };
        alts.iter().any(|a| self.headers.contains(*a))
    }

    /// The header must be included; otherwise report `name.header` and return false.
    pub fn need_header(&mut self, node: NodeId, h: &str, what: &str) -> bool {
        if h.is_empty() || self.has_header(h) {
            return true;
        }
        self.err(
            node,
            "name.header",
            format!("`{what}` lives in the header `{h}`, and this file does not `#include {h}`"),
        );
        false
    }

    /// Is the name locked on this floor? Reports `lock.name` when it is.
    pub fn check_locked(&mut self, node: NodeId, keys: &[String]) -> bool {
        for k in keys {
            if let Some(why) = self.o.locked.get(k) {
                let (k, why) = (k.clone(), why.clone());
                self.err(
                    node,
                    "lock.name",
                    format!("the supply cupboard on this floor does not stock `{k}`: {why}"),
                );
                return false;
            }
        }
        true
    }
}

/// "a int" -> "an int" (and `unsigned`, `iterator`) in a finished message.
fn fix_articles(s: &str) -> String {
    let padded = format!(" {s}");
    let mut out = String::with_capacity(padded.len() + 4);
    let mut rest = padded.as_str();
    while let Some(i) = rest.find(" a ") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 3..];
        let vowel = ["int", "unsigned", "iterator"].iter().any(|w| {
            after.starts_with(w)
                && !after[w.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
        });
        out.push_str(if vowel { " an " } else { " a " });
        rest = after;
    }
    out.push_str(rest);
    out[1..].to_string()
}
