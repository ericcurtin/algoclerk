// SPDX-License-Identifier: Apache-2.0
//! Static checker for the C++ subset: types every expression and declaration with the C++17 rules
//! (integer promotion, usual arithmetic conversions, brace narrowing, `auto` deduction, const-correctness,
//! lvalues and temporaries, library calls resolved through [`super::stdlib::MEMBERS`]) and reports friendly
//! diagnostics with codes `type.*`, `name.*`, `const.*`, `lock.*`.
//!
//! The work is split over `check/`: `val` (typed values, arithmetic rules), `ctx` (scopes, diagnostics,
//! headers), `sigpat` (library signatures), `libcall` (library calls), `ctor` (construction and
//! initialisation), `expr`, `ops`, `call`, `stmt`, `item`.

use super::ast::*;
use super::diag::Diag;
use super::stdlib::Member;
use std::collections::{BTreeMap, BTreeSet, HashMap};

mod assign;
mod call;
mod ctor;
mod ctx;
mod expr;
mod flow;
mod item;
mod lambda;
mod libcall;
mod libres;
mod ops;
mod range;
mod sigpat;
mod stmt;
mod types;
mod unseq;
mod val;

/// The fixed header of the function the player writes.
#[derive(Clone, Debug, PartialEq)]
pub struct EntrySig {
    /// Function name.
    pub name: String,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type.
    pub ret: TypeSpec,
}

impl EntrySig {
    /// The signature as C++ text (no body).
    pub fn text(&self) -> String {
        let ps: Vec<String> = self
            .params
            .iter()
            .map(|p| format!("{} {}", super::print::print_type(&p.spec), p.name))
            .collect();
        format!(
            "{} {}({})",
            super::print::print_type(&self.ret),
            self.name,
            ps.join(", ")
        )
    }
}

/// What a level unlocks.
#[derive(Clone, Debug, Default)]
pub struct CheckOpts {
    /// The function the player must define.
    pub entry: Option<EntrySig>,
    /// Unlocked headers such as `<vector>`; `None` means everything.
    pub unlocked_headers: Option<BTreeSet<String>>,
    /// Locked names with the reason.
    pub locked: BTreeMap<String, String>,
    /// Helper functions allowed.
    pub helpers: bool,
    /// Recursion allowed.
    pub recursion: bool,
    /// Player-defined structs allowed.
    pub structs: bool,
}

impl CheckOpts {
    /// Everything unlocked, no entry required.
    pub fn all() -> CheckOpts {
        CheckOpts {
            entry: None,
            unlocked_headers: None,
            locked: BTreeMap::new(),
            helpers: true,
            recursion: true,
            structs: true,
        }
    }
}

/// What a name or call expression was resolved to (for code generation and the interpreter).
#[derive(Clone, Debug)]
pub enum Resolved {
    /// A variable or parameter: the id of its declarator or parameter.
    Var(NodeId),
    /// The `index`-th name of the structured binding statement `stmt`.
    Binding {
        /// The statement.
        stmt: NodeId,
        /// Position in the name list.
        index: usize,
    },
    /// A user function (its [`FnDecl::id`]); for overloads, the chosen one.
    Func(NodeId),
    /// A library member, free function, constant or constructor row.
    Lib(&'static Member),
    /// A data member of a struct, by name.
    Field(String),
    /// The `index`-th user constructor of a struct.
    Ctor(usize),
    /// A call through a lambda or `std::function` variable.
    Callable,
}

/// Semantic information about a checked program.
///
/// Every expression node has a type ([`TypeInfo::ty`]); every declarator and parameter has the type of the
/// variable it declares ([`TypeInfo::decl_ty`], `auto` resolved, references and `const` stripped from the
/// type; see the flags below). Names and calls carry a [`Resolved`] target ([`TypeInfo::resolved`]).
/// Structured bindings and range-for names have no node of their own: see [`TypeInfo::bindings`].
#[derive(Clone, Debug, Default)]
pub struct TypeInfo {
    types: Vec<Ty>,
    flags: Vec<u8>,
    refs: HashMap<NodeId, Resolved>,
    bindings: HashMap<NodeId, Vec<(String, Ty)>>,
    scopes: HashMap<NodeId, Vec<(String, Ty)>>,
}

const F_LVALUE: u8 = 1;
const F_CONST: u8 = 2;

impl TypeInfo {
    /// Type of an expression node (value type, references stripped).
    pub fn ty(&self, id: NodeId) -> &Ty {
        self.types.get(id as usize).unwrap_or(&Ty::Unknown)
    }
    /// Resolved type of a declared variable or parameter (`auto` resolved).
    pub fn decl_ty(&self, id: NodeId) -> &Ty {
        self.ty(id)
    }
    /// Is the expression an lvalue.
    pub fn is_lvalue(&self, id: NodeId) -> bool {
        self.flags
            .get(id as usize)
            .is_some_and(|f| f & F_LVALUE != 0)
    }
    /// Is the expression (or variable) `const`-qualified.
    pub fn is_const(&self, id: NodeId) -> bool {
        self.flags
            .get(id as usize)
            .is_some_and(|f| f & F_CONST != 0)
    }
    /// What a name, member access or call expression refers to.
    pub fn resolved(&self, id: NodeId) -> Option<&Resolved> {
        self.refs.get(&id)
    }
    /// Names and types bound by a structured binding statement or a structured range-for (by statement id),
    /// or the single loop variable of a range-for.
    pub fn bindings(&self, stmt: NodeId) -> &[(String, Ty)] {
        self.bindings.get(&stmt).map_or(&[], |v| v.as_slice())
    }

    fn put(&mut self, id: NodeId, ty: Ty, lvalue: bool, konst: bool) {
        let i = id as usize;
        if i >= self.types.len() {
            self.types.resize(i + 1, Ty::Unknown);
            self.flags.resize(i + 1, 0);
        }
        self.types[i] = ty;
        self.flags[i] = (u8::from(lvalue) * F_LVALUE) | (u8::from(konst) * F_CONST);
    }
}

/// Check a program. At most five diagnostics are returned, the first matters most.
pub fn check(p: &Program, o: &CheckOpts) -> Result<TypeInfo, Vec<Diag>> {
    let mut c = ctx::Checker::new(p, o);
    c.run();
    c.finish()
}

/// Variables visible just before statement `at`, innermost last, with their types.
pub fn scope_at(_p: &Program, info: &TypeInfo, at: NodeId) -> Vec<(String, Ty)> {
    info.scopes.get(&at).cloned().unwrap_or_default()
}
