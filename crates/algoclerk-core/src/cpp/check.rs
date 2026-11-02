// SPDX-License-Identifier: Apache-2.0
//! Static checker (STUB: replaced by the CHECK work package).

use super::ast::*;
use super::diag::Diag;
use std::collections::{BTreeMap, BTreeSet};

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

/// Semantic information about a checked program.
#[derive(Clone, Debug, Default)]
pub struct TypeInfo {
    types: Vec<Ty>,
}

impl TypeInfo {
    /// Type of an expression node.
    pub fn ty(&self, id: NodeId) -> &Ty {
        self.types.get(id as usize).unwrap_or(&Ty::Unknown)
    }
    /// Resolved type of a declared variable or parameter.
    pub fn decl_ty(&self, id: NodeId) -> &Ty {
        self.ty(id)
    }
    /// Is the expression an lvalue.
    pub fn is_lvalue(&self, _id: NodeId) -> bool {
        false
    }
}

/// Check a program.
pub fn check(_p: &Program, _o: &CheckOpts) -> Result<TypeInfo, Vec<Diag>> {
    Ok(TypeInfo::default())
}

/// Variables visible just before statement `at`.
pub fn scope_at(_p: &Program, _info: &TypeInfo, _at: NodeId) -> Vec<(String, Ty)> {
    Vec::new()
}
