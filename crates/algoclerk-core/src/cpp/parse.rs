// SPDX-License-Identifier: Apache-2.0
//! Parser (STUB: replaced by the PARSE work package).

use super::ast::*;
use super::diag::Diag;

/// Options for [`parse`].
#[derive(Clone, Debug, Default)]
pub struct ParseOpts {
    /// First node id to hand out (so prelude and player ids do not collide).
    pub first_id: NodeId,
    /// Struct and alias names defined by the prelude.
    pub known_types: Vec<String>,
}

/// Parse a source text.
pub fn parse(_src: &str, _opts: &ParseOpts) -> Result<Program, Vec<Diag>> {
    Err(vec![Diag::plain("parse.stub", "parser not written yet")])
}

/// Names of the structs and aliases a program defines.
pub fn known_types(p: &Program) -> Vec<String> {
    p.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Struct(s) => Some(s.name.clone()),
            ItemKind::Alias { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}
