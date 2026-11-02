// SPDX-License-Identifier: Apache-2.0
//! Friendly diagnostics shared by the parser, the checker and the interpreter.

use super::ast::{NodeId, Span};
use std::fmt;

/// A problem found in the player's program, worded the way a clerk would say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    /// Where in the text (parse errors) when known.
    pub span: Option<Span>,
    /// Which node (check errors) when known.
    pub node: Option<NodeId>,
    /// Stable machine-readable code such as `type.mismatch`.
    pub code: &'static str,
    /// The message shown to the player.
    pub msg: String,
}

impl Diag {
    /// A diagnostic attached to a text range.
    pub fn at(span: Span, code: &'static str, msg: impl Into<String>) -> Diag {
        Diag { span: Some(span), node: None, code, msg: msg.into() }
    }
    /// A diagnostic attached to a node.
    pub fn node(node: NodeId, code: &'static str, msg: impl Into<String>) -> Diag {
        Diag { span: None, node: Some(node), code, msg: msg.into() }
    }
    /// A diagnostic with no location.
    pub fn plain(code: &'static str, msg: impl Into<String>) -> Diag {
        Diag { span: None, node: None, code, msg: msg.into() }
    }
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for Diag {}

/// 1-based line and column of a byte offset.
pub fn line_col(text: &str, offset: u32) -> (usize, usize) {
    let off = (offset as usize).min(text.len());
    let before = &text.as_bytes()[..off];
    let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
    let col = off - before.iter().rposition(|&b| b == b'\n').map_or(0, |p| p + 1) + 1;
    (line, col)
}
