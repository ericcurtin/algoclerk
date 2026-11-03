// SPDX-License-Identifier: Apache-2.0
//! Autocomplete for the text editor and the card editor chips: `std::` names and members of a receiver,
//! from the library table (`stdlib::MEMBERS`), restricted to the headers the level unlocked and never
//! offering what the level locks. The receiver type of `x.` / `p->` comes from the checker's type
//! information of the last program that compiled (`check::scope_at` and the declarator types).

use crate::session::{Session, parse_with_prelude};
use algoclerk_core::cpp::ast::{ItemKind, NodeId, Program, StmtKind, StructDecl, Ty, walk_stmts};
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
    std_filtered(s, prefix, false)
}

/// Like [`std_items`], but only the functions cards can express (`card: true`).
pub fn card_std_items(s: &Session, prefix: &str) -> Vec<Item> {
    std_filtered(s, prefix, true)
}

fn std_filtered(s: &Session, prefix: &str, card_only: bool) -> Vec<Item> {
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
            || (card_only && !m.card)
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
    members_filtered(s, ty, arrow, prefix, false)
}

/// Like [`member_items`], but only the members cards can express (rows of the table with `card: true`).
pub fn card_member_items(s: &Session, ty: &Ty, arrow: bool, prefix: &str) -> Vec<Item> {
    members_filtered(s, ty, arrow, prefix, true)
}

fn members_filtered(s: &Session, ty: &Ty, arrow: bool, prefix: &str, card_only: bool) -> Vec<Item> {
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
    if let Some(st) = struct_name.and_then(|n| find_struct(s, n)) {
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
        if m.name == "ctor"
            || m.name.starts_with("operator")
            || !m.name.starts_with(prefix)
            || (card_only && !m.card)
        {
            continue;
        }
        // a member needs no header of its own: the receiver exists, so its type's header is in reach
        if is_locked(s, m) || out.iter().any(|o| o.label == m.name) {
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

/// A struct of the prelude or of the player's program.
fn find_struct(s: &Session, name: &str) -> Option<StructDecl> {
    if let Some(st) = s.good.as_ref().and_then(|p| p.find_struct(name)) {
        return Some(st.clone());
    }
    parse_with_prelude(s.prelude(), "")
        .ok()?
        .find_struct(name)
        .cloned()
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

/// The text with the completion site cut out (`seen.pu` becomes `seen;`) so that it parses even while the
/// player is half way through typing, parsed together with the prelude.
type Probe = (Program, String);

fn probe(s: &Session, text: &str, keep_to: usize, cursor: usize) -> Option<Probe> {
    // the rest of the word under the cursor would break the statement
    let rest = &text[cursor..];
    let skip = rest
        .bytes()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
        .count();
    let t = format!("{};{}", &text[..keep_to], &rest[skip..]);
    let p = parse_with_prelude(s.prelude(), &t).ok()?;
    Some((p, t))
}

/// Variables with a spelled type (`std::unordered_map<int, int> seen;`, parameters, `for (int x : v)`,
/// `for (auto x : v)` over a known container) declared on or before `line` in a parsed program.
fn spelled_names(p: &Program, text: &str, line: usize) -> Vec<(String, Ty)> {
    let mut out: Vec<(String, Ty)> = Vec::new();
    let put = |out: &mut Vec<(String, Ty)>, n: &str, t: Ty| {
        out.retain(|(m, _)| m != n);
        out.push((n.to_string(), t));
    };
    for it in p.player_items() {
        let ItemKind::Func(f) = &it.kind else {
            continue;
        };
        let Some(body) = &f.body else { continue };
        for pa in &f.params {
            put(&mut out, &pa.name, pa.spec.ty.clone());
        }
        walk_stmts(body, &mut |st| {
            let at = p.span(st.id).map(|sp| line_col(text, sp.start).0);
            if !at.is_some_and(|l| l <= line) {
                return;
            }
            match &st.kind {
                StmtKind::Decl { spec, vars } if spec.ty != Ty::Auto => {
                    for v in vars {
                        let t = if v.array.is_some() {
                            Ty::Array(Box::new(spec.ty.clone()), 0)
                        } else {
                            spec.ty.clone()
                        };
                        put(&mut out, &v.name, t);
                    }
                }
                StmtKind::RangeFor {
                    spec,
                    names,
                    structured: false,
                    range,
                    ..
                } if names.len() == 1 => {
                    let t = if spec.ty == Ty::Auto {
                        let key = algoclerk_core::cpp::print::print_expr(range);
                        out.iter()
                            .find(|(n, _)| *n == key)
                            .and_then(|(_, t)| value_of(t))
                    } else {
                        Some(spec.ty.clone())
                    };
                    if let Some(t) = t {
                        put(&mut out, &names[0], t);
                    }
                }
                _ => {}
            }
        });
    }
    out
}

/// Variables visible around a line, with their types. Spelled types come from `spelled` (the probe or the
/// parsed program); `auto` variables and everything else from the checker's `scope_at` of the last program
/// that compiled.
pub fn names_at(s: &Session, line: usize, probe: Option<&Probe>) -> Vec<(String, Ty)> {
    let text = s.edit.text();
    let mut out: Vec<(String, Ty)> = Vec::new();
    if let Some((p, t)) = probe {
        out = spelled_names(p, t, line);
    } else if let Some(p) = &s.good {
        out = spelled_names(p, &text, line);
    }
    let Some(c) = &s.last_ok else {
        return out;
    };
    let mut best: Option<NodeId> = None;
    let mut checked: Vec<(String, Ty)> = Vec::new();
    for it in c.program.player_items() {
        let ItemKind::Func(f) = &it.kind else {
            continue;
        };
        let Some(body) = &f.body else { continue };
        for p in &f.params {
            checked.push((p.name.clone(), c.info.decl_ty(p.id).clone()));
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
            checked.retain(|(m, _)| *m != n);
            checked.push((n, t));
        }
        declared_by(&c.program, &c.info, id, &mut checked);
    }
    for (n, t) in checked {
        if !out.iter().any(|(m, _)| *m == n) {
            out.push((n, t));
        }
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
pub fn receiver_type(s: &Session, recv: &str, line: usize, probe: Option<&Probe>) -> Option<Ty> {
    let names = names_at(s, line, probe);
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
pub fn word_items(s: &Session, prefix: &str, line: usize, probe: Option<&Probe>) -> Vec<Item> {
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
    for (n, t) in names_at(s, line, probe).into_iter().rev() {
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
        Ctx::Member { recv, arrow } => {
            let op = if arrow { 2 } else { 1 };
            let pr = probe(s, text, start - op, cursor);
            receiver_type(s, &recv, line, pr.as_ref())
                .map(|t| member_items(s, &t, arrow, &prefix))
                .unwrap_or_default()
        }
        Ctx::Word => {
            let pr = probe(s, text, cursor, cursor);
            word_items(s, &prefix, line, pr.as_ref())
        }
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

    use algoclerk_levels::Campaign;

    fn session(id: &str, text: &str) -> Session {
        let campaign = Campaign::shipped();
        let level = campaign.find(id).expect("level").clone();
        Session::for_level(&level, text).expect("session")
    }

    fn labels(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_str()).collect()
    }

    const FIRST_UNIQUE: &str = "#include <unordered_map>\n\nint firstUnique(const std::string& s) {\n    std::unordered_map<char, int> counts;\n    for (char c : s) {\n        ++counts[c];\n    }\n    COMMENT\n    return -1;\n}\n";

    #[test]
    fn members_of_a_declared_variable_even_when_the_line_does_not_parse() {
        let text = FIRST_UNIQUE.replace("COMMENT", "counts.fi");
        let s = session(
            "first-unique-char",
            &text.replace("counts.fi", "counts.fi\n"),
        );
        // the whole text is broken (no `;`), the type still comes from the declaration
        let text = s.text();
        let cursor = text.find("counts.fi").unwrap() + "counts.fi".len();
        let (items, start) = suggest(&s, &text, cursor);
        assert_eq!(labels(&items), ["find"], "{items:?}");
        assert_eq!(&text[start..cursor], "fi");
        let (items, _) = suggest(&s, &text, text.find("counts.fi").unwrap() + "counts.".len());
        let l = labels(&items);
        assert!(
            l.contains(&"insert") && l.contains(&"erase") && l.contains(&"size"),
            "{l:?}"
        );
        assert!(!l.contains(&"ctor") && !l.iter().any(|x| x.starts_with("operator")));
        assert_eq!(
            items.iter().find(|i| i.label == "size").unwrap().insert,
            "size("
        );
    }

    #[test]
    fn string_and_vector_members_and_element_access() {
        let text = "int countVowels(const std::string& s) {\n    int n = 0;\n    n = s.\n}\n";
        let s = session("count-vowels", text);
        let (items, _) = suggest(&s, &s.text(), s.text().find("s.").unwrap() + 2);
        assert!(labels(&items).contains(&"substr") || labels(&items).contains(&"size"));
        // `grid[i].` is a vector
        let text = "int countIslands(const std::vector<std::vector<char>>& grid) {\n    int n = grid[0].\n}\n";
        let s = session("count-islands", text);
        let (items, _) = suggest(&s, &s.text(), s.text().find("].").unwrap() + 2);
        assert!(labels(&items).contains(&"size"), "{:?}", labels(&items));
    }

    #[test]
    fn iterator_arrow_uses_the_checkers_types() {
        let ok = FIRST_UNIQUE.replace("COMMENT", "auto it = counts.find('a');");
        let mut s = session("first-unique-char", &ok);
        assert!(s.last_ok.is_some(), "{:?}", s.problem());
        let broken = ok.replace("return -1;", "it->\n    return -1;");
        s.set_text(&broken);
        assert!(s.problem().is_some());
        let at = broken.find("it->").unwrap() + 4;
        let (items, start) = suggest(&s, &broken, at);
        assert!(labels(&items).contains(&"first") && labels(&items).contains(&"second"));
        assert_eq!(start, at);
    }

    #[test]
    fn std_names_follow_the_unlocked_headers_and_the_locks() {
        // sum-the-bills unlocks nothing and locks std::accumulate
        let s = session(
            "sum-the-bills",
            "int sumTheBills(const std::vector<int>& bills) {\n    std::so\n}\n",
        );
        let (items, _) = suggest(&s, &s.text(), s.text().find("std::so").unwrap() + 7);
        assert!(items.is_empty(), "sort needs <algorithm>: {items:?}");
        let s = session(
            "sum-the-bills",
            "int sumTheBills(const std::vector<int>& bills) {\n    std::acc\n}\n",
        );
        let (items, _) = suggest(&s, &s.text(), s.text().find("std::acc").unwrap() + 8);
        assert!(items.is_empty(), "accumulate is locked: {items:?}");
        let s = session(
            "sum-the-bills",
            "int sumTheBills(const std::vector<int>& bills) {\n    std::\n}\n",
        );
        let (items, _) = suggest(&s, &s.text(), s.text().find("std::").unwrap() + 5);
        assert!(labels(&items).contains(&"vector"));
        assert!(!labels(&items).contains(&"unordered_map") && !labels(&items).contains(&"sort"));
        // first-unique-char unlocks <algorithm> and <unordered_map>
        let s = session(
            "first-unique-char",
            "int firstUnique(const std::string& s) {\n    std::so\n}\n",
        );
        let (items, _) = suggest(&s, &s.text(), s.text().find("std::so").unwrap() + 7);
        assert_eq!(labels(&items), ["sort"]);
        assert_eq!(items[0].insert, "sort(");
        let text = s.text();
        let (items, _) = suggest(&s, &text, text.find("std::so").unwrap() + 5);
        assert!(labels(&items).contains(&"unordered_map") && labels(&items).contains(&"max"));
    }

    #[test]
    fn words_offer_variables_and_keywords() {
        let s = session(
            "sum-the-bills",
            "int sumTheBills(const std::vector<int>& bills) {\n    int total = 0;\n    to\n}\n",
        );
        let text = s.text();
        let (items, start) = suggest(&s, &text, text.find("    to\n").unwrap() + 6);
        assert_eq!(labels(&items), ["total"], "{items:?}");
        assert_eq!(items[0].detail, "int");
        assert_eq!(&text[start..start + 2], "to");
        let (items, _) = suggest(&s, &text, text.find("    to\n").unwrap() + 4);
        assert!(items.is_empty(), "an empty prefix offers nothing");
    }

    #[test]
    fn struct_fields_through_a_pointer() {
        let text =
            "ListNode* reverseList(ListNode* head) {\n    ListNode* cur = head;\n    cur->\n}\n";
        let s = session("reverse-list", text);
        let t = s.text();
        let (items, _) = suggest(&s, &t, t.find("cur->").unwrap() + 5);
        assert_eq!(labels(&items), ["val", "next"], "{items:?}");
    }
}
