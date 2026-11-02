// SPDX-License-Identifier: Apache-2.0
//! Library metadata table (STUB: replaced by the CHECK work package).

use super::ast::Ty;

/// One supported library member or free function.
#[derive(Clone, Copy, Debug)]
pub struct Member {
    /// Receiver type pattern such as `std::vector<T>`, or `free`.
    pub recv: &'static str,
    /// Name.
    pub name: &'static str,
    /// Header.
    pub header: &'static str,
    /// Display signature.
    pub sig: &'static str,
    /// Cost in the abstract model.
    pub cost: &'static str,
    /// The card editor can express it.
    pub card: bool,
    /// Example statements.
    pub example: &'static str,
}

/// Every supported member.
pub static MEMBERS: &[Member] = &[];

/// Members whose receiver matches the static type.
pub fn members_of(_ty: &Ty) -> Vec<&'static Member> {
    Vec::new()
}

/// The library table as markdown.
pub fn markdown_table() -> String {
    String::new()
}
