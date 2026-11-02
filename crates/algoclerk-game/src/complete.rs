// SPDX-License-Identifier: Apache-2.0
//! Autocomplete for the text editor and the card editor chips: `std::` names and members of a receiver,
//! from the library table (`stdlib::MEMBERS`), restricted to the headers the level unlocked and never
//! offering what the level locks. The receiver type of `x.` / `p->` comes from the checker's type
//! information of the last program that compiled (`check::scope_at` and the declarator types).

use crate::session::Session;
use algoclerk_core::cpp::ast::{ItemKind, NodeId, Program, StmtKind, Ty, walk_stmts};
use algoclerk_core::cpp::check::{TypeInfo, scope_at};
use algoclerk_core::cpp::diag::line_col;
use algoclerk_core::cpp::stdlib::{MEMBERS, Member, members_of};
use std::collections::BTreeSet;

/// One suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Shown in the list.
    pub label: String,
    /// Text that replaces the typed prefix.
    pub insert: String,
    /// Signature or hint shown next to the label.
    pub detail: String,
}

/// Where the cursor is and what is being completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ctx {
    /// After `std::`.
    Std,
    /// After `recv.` or `recv->`.
    Member {
        /// Text of the receiver expression.
        recv: String,
        /// `->` was typed.
        arrow: bool,
    },
    /// A plain word.
    Word,
}

/// The completion context at a byte offset: what is around the cursor and where the typed prefix starts.
pub fn context(text: &str, cursor: usize) -> (Ctx, String, usize) {
    let cursor = cursor.min(text.len());
    let b = text.as_bytes();
    let mut start = cursor;
    while start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_') {
        start -= 1;
    }
    let prefix = text[start..cursor].to_string();
    if text[..start].ends_with("std::") {
        return (Ctx::Std, prefix, start);
    }
    let before = &text[..start];
    let (op_len, arrow) = if before.ends_with("->") {
        (2, true)
    } else if before.ends_with('.') {
        (1, false)
    } else {
        return (Ctx::Word, prefix, start);
    };
    let recv = receiver_text(&before[..before.len() - op_len]);
    if recv.is_empty() {
        return (Ctx::Word, prefix, start);
    }
    (Ctx::Member { recv, arrow }, prefix, start)
}

/// The receiver expression that ends the text: identifiers, `.`, `->`, balanced `[..]` and `(..)`.
fn receiver_text(s: &str) -> String {
    let b = s.as_bytes();
    let mut i = b.len();
    let mut depth = 0i32;
    while i > 0 {
        let c = b[i - 1];
        match c {
            b']' | b')' => depth += 1,
            b'[' | b'(' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            _ if depth > 0 => {}
            c if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' => {}
            b'>' if i >= 2 && b[i - 2] == b'-' => {
                i -= 1;
            }
            b'-' if b.get(i) == Some(&b'>') => {}
            _ => break,
        }
        i -= 1;
    }
    s[i..].trim().to_string()
}

/// The headers the player may use right now: those of the prelude, the player's own `#include` lines and
/// the level's unlocked list. `None` means every header.
pub fn allowed_headers(s: &Session) -> Option<BTreeSet<String>> {
    let Some(level) = &s.level else { return None };
    if level.unlocked.iter().any(|h| h == "*") {
        return None;
    }
    let mut set: BTreeSet<String> = level.unlocked.iter().cloned().collect();
    let mut add = |p: &Program| {
        for it in &p.items {
            if let ItemKind::Include { path, angle } = &it.kind {
                set.insert(if *angle {
                    format!("<{path}>")
                } else {
                    format!("\"{path}\"")
                });
            }
        }
    };
    if let Some(p) = &s.good {
        add(p);
    }
    for l in level.prelude.lines() {
        if let Some(h) = l.trim().strip_prefix("#include") {
            set.insert(h.trim().to_string());
        }
    }
    Some(set)
}

fn header_ok(allowed: &Option<BTreeSet<String>>, header: &str) -> bool {
    header.is_empty() || allowed.as_ref().is_none_or(|a| a.contains(header))
}

/// The name the level's `locked` list uses for a library row (`std::sort`, `std::string::substr`).
pub fn lock_name(m: &Member) -> String {
    if m.recv == "free" || m.recv == "iterator" {
        m.name.split('<').next().unwrap_or(m.name).to_string()
    } else {
        let base = m.recv.split('<').next().unwrap_or(m.recv);
        format!("{base}::{}", m.name)
    }
}

fn is_locked(s: &Session, m: &Member) -> bool {
    let Some(l) = &s.level else { return false };
    let n = lock_name(m);
    l.locked.iter().any(|k| k.name == n || k.name == m.header)
}

const STD_TYPES: &[(&str, &str)] = &[
    ("vector", "<vector>"),
    ("string", "<string>"),
    ("string_view", "<string_view>"),
    ("array", "<array>"),
    ("pair", "<utility>"),
    ("tuple", "<tuple>"),
    ("optional", "<optional>"),
    ("unordered_map", "<unordered_map>"),
    ("map", "<map>"),
    ("unordered_set", "<unordered_set>"),
    ("set", "<set>"),
    ("stack", "<stack>"),
    ("queue", "<queue>"),
    ("priority_queue", "<queue>"),
    ("deque", "<deque>"),
    ("list", "<list>"),
];

/// Names after `std::` that start with `prefix`, for the headers in reach.
pub fn std_items(s: &Session, prefix: &str) -> Vec<Item> {
    let allowed = allowed_headers(s);
    let mut out: Vec<Item> = Vec::new();
    for (name, header) in STD_TYPES {
        if name.starts_with(prefix) && header_ok(&allowed, header) {
            out.push(Item {
                label: (*name).to_string(),
                insert: format!("{name}<"),
                detail: (*header).to_string(),
            });
        }
    }
    for m in MEMBERS.iter().filter(|m| m.recv == "free") {
        let Some(name) = m.name.strip_prefix("std::") else {
            continue;
        };
        let short = name.split('<').next().unwrap_or(name);
        if !short.starts_with(prefix)
            || !header_ok(&allowed, m.header)
            || is_locked(s, m)
            || out.iter().any(|o| o.label == short)
        {
            continue;
        }
        let call = !m.sig.contains("constexpr nullopt");
        out.push(Item {
            label: short.to_string(),
            insert: if name.contains('<') {
                format!("{short}<")
            } else if call {
                format!("{short}(")
            } else {
                short.to_string()
            },
            detail: m.sig.to_string(),
        });
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Members of a value of type `ty` that start with `prefix` (`arrow`: reached through `->`).
pub fn member_items(s: &Session, ty: &Ty, arrow: bool, prefix: &str) -> Vec<Item> {
    let allowed = allowed_headers(s);
    let mut out: Vec<Item> = Vec::new();
    // `p->x` on a pointer to a struct, `x.f` on a struct: its data members
    let struct_name = match ty {
        Ty::Ptr(inner) if arrow => match &**inner {
            Ty::Struct(n) => Some(n.as_str()),
            _ => None,
        },
        Ty::Struct(n) if !arrow => Some(n.as_str()),
        _ => None,
    };
    if let (Some(n), Some(p)) = (struct_name, &s.good)
        && let Some(st) = p.find_struct(n)
    {
        for f in &st.fields {
            if f.name.starts_with(prefix) {
                out.push(Item {
                    label: f.name.clone(),
                    insert: f.name.clone(),
                    detail: format!(
                        "{} {}",
                        algoclerk_core::cpp::print::print_type(&f.spec),
                        f.name
                    ),
                });
            }
        }
        return out;
    }
    let ty = if arrow { deref(ty) } else { Some(ty.clone()) };
    let Some(ty) = ty else { return out };
    for m in members_of(&ty) {
        if m.name == "ctor" || m.name.starts_with("operator") || !m.name.starts_with(prefix) {
            continue;
        }
        if !header_ok(&allowed, m.header)
            || is_locked(s, m)
            || out.iter().any(|o| o.label == m.name)
        {
            continue;
        }
        let call = m.sig.contains('(');
        out.push(Item {
            label: m.name.to_string(),
            insert: if call {
                format!("{}(", m.name)
            } else {
                m.name.to_string()
            },
            detail: m.sig.to_string(),
        });
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// What `*x` or `x->` reaches.
fn deref(t: &Ty) -> Option<Ty> {
    match t {
        Ty::Iter { container, .. } => value_of(container),
        Ty::Ptr(i) => Some((**i).clone()),
        Ty::Optional(i) => Some((**i).clone()),
        _ => None,
    }
}

/// The element type an iterator over `c` yields.
fn value_of(c: &Ty) -> Option<Ty> {
    Some(match c {
        Ty::Map(k, v) | Ty::UnorderedMap(k, v) => Ty::Pair(k.clone(), v.clone()),
        Ty::Vector(t)
        | Ty::Array(t, _)
        | Ty::Deque(t)
        | Ty::List(t)
        | Ty::Set(t)
        | Ty::UnorderedSet(t) => (**t).clone(),
        Ty::Str | Ty::StrView => Ty::Char,
        _ => return None,
    })
}

/// The type of `x[i]`.
fn index_of(c: &Ty) -> Option<Ty> {
    Some(match c {
        Ty::Map(_, v) | Ty::UnorderedMap(_, v) => (**v).clone(),
        Ty::Vector(t) | Ty::Array(t, _) | Ty::Deque(t) => (**t).clone(),
        Ty::Str | Ty::StrView => Ty::Char,
        _ => return None,
    })
}

/// Variables visible around a line of the last program that compiled, with their types: the checker's
/// `scope_at` of the last statement that starts on or before the line, plus the parameters.
pub fn names_at(s: &Session, line: usize) -> Vec<(String, Ty)> {
    let Some(c) = &s.last_ok else {
        return Vec::new();
    };
    let text = s.edit.text();
    let mut best: Option<NodeId> = None;
    let mut out: Vec<(String, Ty)> = Vec::new();
    for it in c.program.player_items() {
        let ItemKind::Func(f) = &it.kind else {
            continue;
        };
        let Some(body) = &f.body else { continue };
        for p in &f.params {
            out.push((p.name.clone(), c.info.decl_ty(p.id).clone()));
        }
        walk_stmts(body, &mut |st| {
            let at = c.program.span(st.id).map(|sp| line_col(&text, sp.start).0);
            if at.is_some_and(|l| l <= line) {
                best = Some(st.id);
            }
        });
    }
    if let Some(id) = best {
        for (n, t) in scope_at(&c.program, &c.info, id) {
            out.retain(|(m, _)| *m != n);
            out.push((n, t));
        }
        // names the chosen statement itself declares
        declared_by(&c.program, &c.info, id, &mut out);
    }
    out
}

fn declared_by(p: &Program, info: &TypeInfo, id: NodeId, out: &mut Vec<(String, Ty)>) {
    for it in p.player_items() {
        if let ItemKind::Func(f) = &it.kind
            && let Some(b) = &f.body
        {
            walk_stmts(b, &mut |s| {
                if s.id != id {
                    return;
                }
                match &s.kind {
                    StmtKind::Decl { vars, .. } => {
                        for v in vars {
                            out.retain(|(m, _)| *m != v.name);
                            out.push((v.name.clone(), info.decl_ty(v.id).clone()));
                        }
                    }
                    StmtKind::Structured { .. } => {
                        for (n, t) in info.bindings(s.id) {
                            out.retain(|(m, _)| m != n);
                            out.push((n.clone(), t.clone()));
                        }
                    }
                    _ => {}
                }
            });
        }
    }
}

/// The static type of a receiver expression written as text (`seen`, `grid[i]`, `it->second`, `m.find(k)`),
/// or `None` when it cannot be worked out.
pub fn receiver_type(s: &Session, recv: &str, line: usize) -> Option<Ty> {
    let names = names_at(s, line);
    let b = recv.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
        i += 1;
    }
    let head = &recv[..i];
    let mut ty = names.iter().rev().find(|(n, _)| n == head)?.1.clone();
    let mut rest = &recv[i..];
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('[') {
            let close = matching(r, b'[', b']')?;
            ty = index_of(&ty)?;
            rest = &r[close + 1..];
        } else if rest.starts_with('.') || rest.starts_with("->") {
            let arrow = rest.starts_with("->");
            let r = &rest[if arrow { 2 } else { 1 }..];
            let n = r
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            let name = &r[..n];
            rest = &r[n..];
            let recv_ty = if arrow { deref(&ty)? } else { ty.clone() };
            if let Some(r2) = rest.strip_prefix('(') {
                let close = matching(r2, b'(', b')')?;
                rest = &r2[close + 1..];
                ty = call_result(&recv_ty, name)?;
            } else {
                ty = field_of(s, &recv_ty, name)?;
            }
        } else {
            return None;
        }
    }
    Some(ty)
}

fn matching(s: &str, open: u8, close: u8) -> Option<usize> {
    let mut depth = 1;
    for (i, c) in s.bytes().enumerate() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

fn field_of(s: &Session, t: &Ty, name: &str) -> Option<Ty> {
    match (t, name) {
        (Ty::Pair(a, _), "first") => Some((**a).clone()),
        (Ty::Pair(_, b), "second") => Some((**b).clone()),
        (Ty::Struct(n), f) => {
            let p = s.good.as_ref()?;
            let st = p.find_struct(n)?;
            st.fields
                .iter()
                .find(|x| x.name == f)
                .map(|x| x.spec.ty.clone())
        }
        _ => None,
    }
}

fn call_result(t: &Ty, name: &str) -> Option<Ty> {
    let iter = |c: &Ty| Ty::Iter {
        container: Box::new(c.clone()),
        reverse: false,
    };
    match name {
        "begin" | "end" | "find" | "lower_bound" | "upper_bound" => Some(iter(t)),
        "front" | "back" | "top" | "at" => match t {
            Ty::Stack(e) | Ty::Queue(e) | Ty::PriorityQueue(e, _) => Some((**e).clone()),
            _ => index_of(t).or_else(|| value_of(t)),
        },
        "value" => deref(t),
        _ => None,
    }
}

/// Words offered for a plain prefix: variables in scope, keywords and the `std::` entry.
pub fn word_items(s: &Session, prefix: &str, line: usize) -> Vec<Item> {
    const KEYWORDS: &[&str] = &[
        "auto",
        "bool",
        "break",
        "char",
        "const",
        "continue",
        "else",
        "false",
        "for",
        "if",
        "int",
        "nullptr",
        "return",
        "static_cast",
        "true",
        "unsigned",
        "while",
    ];
    let mut out: Vec<Item> = Vec::new();
    if prefix.is_empty() {
        return out;
    }
    for (n, t) in names_at(s, line).into_iter().rev() {
        if n.starts_with(prefix) && n != prefix && !out.iter().any(|o| o.label == n) {
            out.push(Item {
                insert: n.clone(),
                detail: t.to_string(),
                label: n,
            });
        }
    }
    for k in KEYWORDS {
        if k.starts_with(prefix) && *k != prefix {
            out.push(Item {
                label: (*k).to_string(),
                insert: (*k).to_string(),
                detail: "keyword".into(),
            });
        }
    }
    if "std".starts_with(prefix) && prefix != "std" {
        out.push(Item {
            label: "std::".into(),
            insert: "std::".into(),
            detail: "library".into(),
        });
    }
    out
}

/// Suggestions for the text with the cursor at byte offset `cursor`: the items and the byte offset where
/// the typed prefix starts (what an accepted item replaces).
pub fn suggest(s: &Session, text: &str, cursor: usize) -> (Vec<Item>, usize) {
    let (ctx, prefix, start) = context(text, cursor);
    let line = line_col(text, cursor as u32).0;
    let items = match ctx {
        Ctx::Std => std_items(s, &prefix),
        Ctx::Member { recv, arrow } => receiver_type(s, &recv, line)
            .map(|t| member_items(s, &t, arrow, &prefix))
            .unwrap_or_default(),
        Ctx::Word => word_items(s, &prefix, line),
    };
    (items, start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contexts() {
        let t = "int x = std::so";
        assert_eq!(context(t, t.len()), (Ctx::Std, "so".into(), t.len() - 2));
        let t = "    seen.pu";
        let (c, p, st) = context(t, t.len());
        assert_eq!(
            c,
            Ctx::Member {
                recv: "seen".into(),
                arrow: false
            }
        );
        assert_eq!((p.as_str(), st), ("pu", t.len() - 2));
        let t = "if (it->";
        let (c, p, _) = context(t, t.len());
        assert_eq!(
            c,
            Ctx::Member {
                recv: "it".into(),
                arrow: true
            }
        );
        assert!(p.is_empty());
        let t = "x = grid[i][j].si";
        let (c, _, _) = context(t, t.len());
        assert_eq!(
            c,
            Ctx::Member {
                recv: "grid[i][j]".into(),
                arrow: false
            }
        );
        assert_eq!(context("ret", 3).0, Ctx::Word);
    }
}
