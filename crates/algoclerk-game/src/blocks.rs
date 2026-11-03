// SPDX-License-Identifier: Apache-2.0
//! The block (card) view of a program: rows for display and structural edits that keep the program and its
//! text in sync (the text is always the printed syntax tree).
//!
//! Every statement of a function body is one card. Compound statements (`if`, `for`, `while`, range-`for`)
//! show their header and hold their body as nested cards; simple statements show their one line. What cards
//! cannot express (`switch`, `do`-`while`, bare blocks, statements that print on several lines such as
//! lambdas with bodies) is shown as a read-only *raw C++* card that can still be moved and deleted.

use algoclerk_core::cpp::ast::*;
use algoclerk_core::cpp::parse::{ParseOpts, known_types, parse};
use algoclerk_core::cpp::print::{print_program, print_stmt, print_type};

/// What kind of card a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// Function header (fixed).
    Header,
    /// A statement with a body.
    Compound,
    /// A simple statement.
    Simple,
    /// The `else` label of an if card.
    Else,
    /// C++ the cards cannot express (read only text, can be moved and deleted).
    Raw,
    /// A top-level item that is not a function (include, alias, global, struct): read only.
    Item,
}

/// One row of the block view.
#[derive(Debug, Clone)]
pub struct Row {
    /// Statement, item or function id.
    pub node: NodeId,
    /// Nesting depth.
    pub depth: usize,
    /// Card text (one line).
    pub text: String,
    /// Icon name.
    pub icon: &'static str,
    /// Card kind.
    pub kind: CardKind,
    /// The card is `else if (...)` continuing the chain of the card above (cannot be moved or deleted).
    pub chained: bool,
}

impl Row {
    /// Can the text of this card be edited?
    pub fn editable(&self) -> bool {
        matches!(self.kind, CardKind::Compound | CardKind::Simple)
    }
    /// Can the card be moved, deleted or have cards inserted after it?
    pub fn movable(&self) -> bool {
        matches!(
            self.kind,
            CardKind::Compound | CardKind::Simple | CardKind::Raw
        ) && !self.chained
    }
}

fn icon_of(k: &StmtKind) -> &'static str {
    match k {
        StmtKind::Decl { .. } | StmtKind::Structured { .. } => "label",
        StmtKind::Expr(e) if matches!(e.kind, ExprKind::Assign { .. }) => "pointer",
        StmtKind::Expr(_) => "hand",
        StmtKind::If { .. } | StmtKind::Switch { .. } | StmtKind::Case(_) => "signpost",
        StmtKind::For { .. }
        | StmtKind::RangeFor { .. }
        | StmtKind::While { .. }
        | StmtKind::DoWhile { .. } => "reset",
        StmtKind::Break => "cross",
        StmtKind::Continue => "fast",
        StmtKind::Return(_) => "outbox",
        StmtKind::Block(_) | StmtKind::Empty => "code",
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_compound(k: &StmtKind) -> bool {
    matches!(
        k,
        StmtKind::If { .. }
            | StmtKind::For { .. }
            | StmtKind::RangeFor { .. }
            | StmtKind::While { .. }
    )
}

fn empty_body() -> Box<Stmt> {
    Box::new(Stmt::new(0, StmtKind::Block(Block::default())))
}

/// A copy of a statement without comments, with empty bodies: what the card header shows.
fn header_clone(s: &Stmt) -> Stmt {
    let mut c = s.clone();
    c.comments.clear();
    c.trailing = None;
    match &mut c.kind {
        StmtKind::If { then, els, .. } => {
            *then = empty_body();
            *els = None;
        }
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => *body = empty_body(),
        _ => {}
    }
    c
}

/// The one-line text of a card: the whole statement, or only the header of a compound statement.
pub fn header_text(s: &Stmt) -> String {
    let c = header_clone(s);
    let printed = print_stmt(&c, 0);
    let t = printed.trim_end();
    let t = if is_compound(&c.kind) {
        t.strip_suffix("{}")
            .or_else(|| t.strip_suffix("{\n}"))
            .unwrap_or(t)
    } else {
        t
    };
    one_line(t)
}

/// Does the statement print on several lines (or hide a comment), so a one-line card cannot hold it?
fn needs_raw(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::Switch { .. }
        | StmtKind::Case(_)
        | StmtKind::DoWhile { .. }
        | StmtKind::Block(_)
        | StmtKind::Empty => true,
        k if is_compound(k) => {
            let c = header_clone(s);
            let printed = print_stmt(&c, 0);
            let t = printed.trim_end();
            let t = t.strip_suffix("{}").or_else(|| t.strip_suffix("{\n}"));
            t.is_none_or(|t| t.contains('\n'))
        }
        _ => print_stmt(&header_clone(s), 0).trim_end().contains('\n'),
    }
}

/// The text a card editor starts with (`None` for cards that cannot be edited).
pub fn card_text(p: &Program, id: NodeId) -> Option<String> {
    let mut found = None;
    for it in p.player_items() {
        if let ItemKind::Func(f) = &it.kind
            && let Some(b) = &f.body
        {
            walk_stmts(b, &mut |s| {
                if s.id == id && !needs_raw(s) {
                    found = Some(header_text(s));
                }
            });
        }
    }
    found
}

/// The signature of a function as one line.
pub fn signature_text(f: &FnDecl) -> String {
    let ps: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            format!("{} {}", print_type(&p.spec), p.name)
                .trim()
                .to_string()
        })
        .collect();
    format!("{} {}({})", print_type(&f.ret), f.name, ps.join(", "))
}

fn then_block(s: &Stmt) -> Option<&Block> {
    match &s.kind {
        StmtKind::Block(b) => Some(b),
        _ => None,
    }
}

fn row_of(s: &Stmt, depth: usize, chained: bool) -> Row {
    let raw = needs_raw(s);
    let text = if raw {
        one_line(print_stmt(&header_clone(s), 0).trim_end())
    } else if chained {
        format!("else {}", header_text(s))
    } else {
        header_text(s)
    };
    Row {
        node: s.id,
        depth,
        text,
        icon: icon_of(&s.kind),
        kind: if raw {
            CardKind::Raw
        } else if is_compound(&s.kind) {
            CardKind::Compound
        } else {
            CardKind::Simple
        },
        chained,
    }
}

fn rows_stmt(s: &Stmt, depth: usize, chained: bool, out: &mut Vec<Row>) {
    out.push(row_of(s, depth, chained));
    if needs_raw(s) {
        return;
    }
    match &s.kind {
        StmtKind::If { then, els, .. } => {
            rows_body(then, depth + 1, out);
            match els.as_deref() {
                Some(e) if matches!(e.kind, StmtKind::If { .. }) => {
                    rows_stmt(e, depth, true, out);
                }
                Some(e) => {
                    out.push(Row {
                        node: s.id,
                        depth,
                        text: "else".into(),
                        icon: "signpost",
                        kind: CardKind::Else,
                        chained: false,
                    });
                    rows_body(e, depth + 1, out);
                }
                None => {}
            }
        }
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => rows_body(body, depth + 1, out),
        _ => {}
    }
}

fn rows_body(b: &Stmt, depth: usize, out: &mut Vec<Row>) {
    match then_block(b) {
        Some(blk) => rows_block(blk, depth, out),
        None => rows_stmt(b, depth, false, out),
    }
}

fn rows_block(b: &Block, depth: usize, out: &mut Vec<Row>) {
    for s in &b.stmts {
        rows_stmt(s, depth, false, out);
    }
}

/// All rows of the player's part of a program.
pub fn rows(p: &Program) -> Vec<Row> {
    let mut out = Vec::new();
    for it in p.player_items() {
        match &it.kind {
            ItemKind::Func(f) if f.body.is_some() => {
                out.push(Row {
                    node: f.id,
                    depth: 0,
                    text: signature_text(f),
                    icon: "code",
                    kind: CardKind::Header,
                    chained: false,
                });
                if let Some(b) = &f.body {
                    rows_block(b, 1, &mut out);
                }
            }
            _ => {
                let mut one = Program::default();
                one.items.push(it.clone());
                out.push(Row {
                    node: it.id,
                    depth: 0,
                    text: one_line(print_program(&one, false).trim_end()),
                    icon: "code",
                    kind: CardKind::Item,
                    chained: false,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------
// structural edits
// ---------------------------------------------------------------------------------------------------

fn wrap_block(next: &mut NodeId, s: &mut Stmt) {
    if !matches!(s.kind, StmtKind::Block(_)) {
        let inner = std::mem::replace(s, Stmt::new(0, StmtKind::Empty));
        let id = *next;
        *next += 1;
        *s = Stmt::new(
            id,
            StmtKind::Block(Block {
                stmts: vec![inner],
                tail_comments: Vec::new(),
            }),
        );
    }
}

fn norm_block(next: &mut NodeId, b: &mut Block) {
    for s in &mut b.stmts {
        norm_stmt(next, s);
    }
}

/// Give every body of an `if`, `else`, `for`, `while` its own braces (so cards can address it).
fn norm_stmt(next: &mut NodeId, s: &mut Stmt) {
    match &mut s.kind {
        StmtKind::If { then, els, .. } => {
            wrap_block(next, then);
            norm_stmt(next, then);
            if let Some(e) = els {
                if !matches!(e.kind, StmtKind::If { .. }) {
                    wrap_block(next, e);
                }
                norm_stmt(next, e);
            }
        }
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => {
            wrap_block(next, body);
            norm_stmt(next, body);
        }
        StmtKind::Block(b) => norm_block(next, b),
        _ => {}
    }
}

/// Add braces around single-statement bodies in the player's functions.
pub fn normalise(p: &mut Program) {
    let mut next = p.next_id;
    let skip = p.prelude_items;
    for it in p.items.iter_mut().skip(skip) {
        if let ItemKind::Func(f) = &mut it.kind
            && let Some(b) = &mut f.body
        {
            norm_block(&mut next, b);
        }
    }
    p.next_id = next;
}

enum Target<'a> {
    InBlock(&'a mut Block, usize),
    Chained(&'a mut Stmt),
}

fn visit_block(b: &mut Block, id: NodeId, f: &mut dyn FnMut(Target<'_>)) -> bool {
    if let Some(i) = b.stmts.iter().position(|s| s.id == id) {
        f(Target::InBlock(b, i));
        return true;
    }
    b.stmts.iter_mut().any(|s| visit_stmt(s, id, f))
}

fn visit_stmt(s: &mut Stmt, id: NodeId, f: &mut dyn FnMut(Target<'_>)) -> bool {
    match &mut s.kind {
        StmtKind::If { then, els, .. } => {
            if let StmtKind::Block(b) = &mut then.kind
                && visit_block(b, id, f)
            {
                return true;
            }
            match els {
                Some(e) if matches!(e.kind, StmtKind::If { .. }) => {
                    if e.id == id {
                        f(Target::Chained(e));
                        true
                    } else {
                        visit_stmt(e, id, f)
                    }
                }
                Some(e) => match &mut e.kind {
                    StmtKind::Block(b) => visit_block(b, id, f),
                    _ => false,
                },
                None => false,
            }
        }
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => match &mut body.kind {
            StmtKind::Block(b) => visit_block(b, id, f),
            _ => false,
        },
        _ => false,
    }
}

fn with_target(p: &mut Program, id: NodeId, f: &mut dyn FnMut(Target<'_>)) -> bool {
    let skip = p.prelude_items;
    for it in p.items.iter_mut().skip(skip) {
        if let ItemKind::Func(func) = &mut it.kind
            && let Some(b) = &mut func.body
            && visit_block(b, id, f)
        {
            return true;
        }
    }
    false
}

fn reprint(p: &Program) -> String {
    print_program(p, true)
}

/// Move a statement one place up (or down) among its siblings. Returns the new text.
pub fn move_stmt(p: &mut Program, id: NodeId, up: bool) -> Option<String> {
    normalise(p);
    let mut moved = false;
    with_target(p, id, &mut |t| {
        if let Target::InBlock(b, i) = t {
            let j = if up { i.checked_sub(1) } else { Some(i + 1) };
            if let Some(j) = j.filter(|j| *j < b.stmts.len()) {
                b.stmts.swap(i, j);
                moved = true;
            }
        }
    });
    moved.then(|| reprint(p))
}

/// Delete a statement. Returns the new text.
pub fn delete_stmt(p: &mut Program, id: NodeId) -> Option<String> {
    normalise(p);
    let mut done = false;
    with_target(p, id, &mut |t| {
        if let Target::InBlock(b, i) = t {
            b.stmts.remove(i);
            done = true;
        }
    });
    done.then(|| reprint(p))
}

/// Parse one statement from card text (compound headers such as `if (x > 2)` get an empty body).
///
/// # Errors
/// The parser's message, or "A card holds exactly one sentence."
pub fn parse_stmt(p: &mut Program, text: &str) -> Result<Stmt, String> {
    let t = text.trim();
    let t = t.strip_prefix("else ").map_or(t, str::trim_start);
    if t.is_empty() {
        return Err("The card is empty.".into());
    }
    let body = if t.ends_with(';') || t.ends_with('}') {
        t.to_string()
    } else {
        format!("{t} {{}}")
    };
    let wrapped = format!("void w() {{\n{body}\n}}\n");
    let opts = ParseOpts {
        first_id: p.next_id,
        known_types: known_types(p),
    };
    let parsed = parse(&wrapped, &opts).map_err(|d| {
        d.first()
            .map_or_else(|| "I cannot read that card.".to_string(), |d| d.msg.clone())
    })?;
    p.next_id = p.next_id.max(parsed.next_id);
    let Some(Item {
        kind: ItemKind::Func(f),
        ..
    }) = parsed.items.into_iter().next()
    else {
        return Err("I cannot read that card.".into());
    };
    let mut stmts = f.body.map(|b| b.stmts).unwrap_or_default();
    if stmts.len() != 1 {
        return Err("A card holds exactly one sentence.".into());
    }
    Ok(stmts.remove(0))
}

fn transplant(old: StmtKind, new: &mut StmtKind) {
    let (body, old_els) = match old {
        StmtKind::If { then, els, .. } => (Some(then), els),
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => (Some(body), None),
        _ => (None, None),
    };
    let Some(body) = body else { return };
    match new {
        StmtKind::If { then, els, .. } => {
            *then = body;
            *els = old_els;
        }
        StmtKind::For { body: b, .. }
        | StmtKind::RangeFor { body: b, .. }
        | StmtKind::While { body: b, .. } => *b = body,
        _ => {}
    }
}

/// Replace the header of a statement, keeping its body. Returns the new text.
///
/// # Errors
/// The parser's message, or "That card is gone."
pub fn edit_stmt(p: &mut Program, id: NodeId, text: &str) -> Result<String, String> {
    normalise(p);
    let new = parse_stmt(p, text)?;
    let mut slot = Some(new);
    let done = with_target(p, id, &mut |t| {
        let s = match t {
            Target::InBlock(b, i) => &mut b.stmts[i],
            Target::Chained(s) => s,
        };
        if let Some(n) = slot.take() {
            let old = std::mem::replace(&mut s.kind, StmtKind::Empty);
            let mut kind = n.kind;
            transplant(old, &mut kind);
            s.kind = kind;
        }
    });
    if done {
        Ok(reprint(p))
    } else {
        Err("That card is gone.".into())
    }
}

fn first_body(s: &mut Stmt) -> Option<&mut Block> {
    let body = match &mut s.kind {
        StmtKind::If { then, .. } => then,
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => body,
        _ => return None,
    };
    match &mut body.kind {
        StmtKind::Block(b) => Some(b),
        _ => None,
    }
}

/// Insert a new statement after `anchor` (or at the start of its body when `inside`); without an anchor it
/// goes to the end of the function called `entry` (or the last function). Returns the new text.
///
/// # Errors
/// The parser's message, "That card is gone." or "That card has no inside."
pub fn insert_stmt(
    p: &mut Program,
    entry: &str,
    anchor: Option<NodeId>,
    inside: bool,
    text: &str,
) -> Result<String, String> {
    normalise(p);
    let new = parse_stmt(p, text)?;
    match anchor {
        None => {
            let skip = p.prelude_items;
            let mut funcs: Vec<&mut FnDecl> = p
                .items
                .iter_mut()
                .skip(skip)
                .filter_map(|i| match &mut i.kind {
                    ItemKind::Func(f) if f.body.is_some() => Some(f),
                    _ => None,
                })
                .collect();
            let at = funcs
                .iter()
                .position(|f| f.name == entry)
                .or(funcs.len().checked_sub(1))
                .ok_or("There is no function to put the card in.")?;
            if let Some(b) = &mut funcs[at].body {
                b.stmts.push(new);
            }
        }
        Some(id) => {
            let mut slot = Some(new);
            let mut err: Option<&str> = None;
            let done = with_target(p, id, &mut |t| {
                let Some(n) = slot.take() else { return };
                match (t, inside) {
                    (Target::InBlock(b, i), false) => b.stmts.insert(i + 1, n),
                    (Target::InBlock(b, i), true) => match first_body(&mut b.stmts[i]) {
                        Some(body) => body.stmts.insert(0, n),
                        None => err = Some("That card has no inside."),
                    },
                    (Target::Chained(s), true) => match first_body(s) {
                        Some(body) => body.stmts.insert(0, n),
                        None => err = Some("That card has no inside."),
                    },
                    (Target::Chained(_), false) => err = Some("Put the card inside instead."),
                }
            });
            if let Some(e) = err {
                return Err(e.into());
            }
            if !done {
                return Err("That card is gone.".into());
            }
        }
    }
    Ok(reprint(p))
}

/// Names the palette templates are filled with.
#[derive(Debug, Clone)]
pub struct Fill {
    /// A fresh variable name.
    pub fresh: String,
    /// The first container parameter (or a fallback name).
    pub container: String,
    /// Is `container` a real parameter?
    pub has_container: bool,
    /// A `return` value of the function's return type.
    pub ret: String,
}

impl Fill {
    /// Fill values for a function: its first container parameter and a default return value.
    pub fn for_function(fresh: String, f: Option<&FnDecl>) -> Fill {
        let container = f.and_then(|f| {
            f.params.iter().find(|p| {
                matches!(
                    p.spec.ty,
                    Ty::Vector(_) | Ty::Str | Ty::StrView | Ty::Array(..) | Ty::Deque(_)
                )
            })
        });
        let ret = match f.map(|f| &f.ret.ty) {
            Some(Ty::Bool) => "false",
            Some(Ty::Ptr(_)) => "nullptr",
            Some(Ty::Void) => "",
            Some(t) if t.is_integral() => "0",
            Some(Ty::Str) => "\"\"",
            Some(_) => "{}",
            None => "0",
        };
        Fill {
            fresh,
            container: container.map_or_else(|| "items".to_string(), |p| p.name.clone()),
            has_container: container.is_some(),
            ret: ret.to_string(),
        }
    }

    /// Instantiate a palette template.
    pub fn apply(&self, template: &str) -> String {
        let len = if self.has_container {
            format!("static_cast<int>({}.size())", self.container)
        } else {
            "10".to_string()
        };
        let t = template
            .replace("{n}", &self.fresh)
            .replace("{c}", &self.container)
            .replace("{len}", &len)
            .replace("{ret}", &self.ret);
        t.replace("return ;", "return;")
            .replace("return  ;", "return;")
    }
}

/// The template cards of the palette: label and C++ text (`{n}` fresh name, `{c}` a container, ...).
pub const PALETTE: &[(&str, &str)] = &[
    ("int", "int {n} = 0;"),
    ("set", "{n} = {n};"),
    ("if", "if (true)"),
    ("while", "while (true)"),
    ("for i", "for (int i = 0; i < {len}; ++i)"),
    ("for each", "for (const auto& x : {c})"),
    ("push", "{c}.push_back(0);"),
    ("return", "return {ret};"),
    ("break", "break;"),
    ("continue", "continue;"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::parse_with_prelude;

    const PRE: &str = "#include <vector>\n";
    const SRC: &str = "int f(const std::vector<int>& a) {\n    int t = 0;\n    for (int x : a) {\n        if (x > 2) {\n            t += x;\n        } else if (x < 0) {\n            t -= 1;\n        } else {\n            t += 1;\n        }\n    }\n    return t;\n}\n";

    fn prog(src: &str) -> Program {
        parse_with_prelude(PRE, src).unwrap()
    }

    fn texts(p: &Program) -> Vec<(usize, String)> {
        rows(p).into_iter().map(|r| (r.depth, r.text)).collect()
    }

    fn id_of(p: &Program, text: &str) -> NodeId {
        rows(p).into_iter().find(|r| r.text == text).unwrap().node
    }

    #[test]
    fn rows_follow_the_text() {
        let p = prog(SRC);
        let r = rows(&p);
        assert_eq!(r[0].kind, CardKind::Header);
        assert_eq!(r[0].text, "int f(const std::vector<int>& a)");
        assert!(
            r.iter()
                .any(|x| x.text == "else" && x.kind == CardKind::Else)
        );
        let t = texts(&p);
        assert!(t.contains(&(1, "for (int x : a)".to_string())));
        assert!(t.contains(&(3, "t += x;".to_string())));
        let chained = r.iter().find(|x| x.text == "else if (x < 0)").unwrap();
        assert!(chained.chained && chained.kind == CardKind::Compound);
        assert_eq!(chained.depth, 2, "an else-if sits at the depth of its if");
        assert!(!chained.movable() && chained.editable());
    }

    #[test]
    fn unexpressible_statements_are_raw_cards_and_round_trip() {
        let src = "int g(int n) {\n    switch (n) {\n        case 1:\n            return 1;\n        default:\n            break;\n    }\n    do {\n        --n;\n    } while (n > 0);\n    return n;\n}\n";
        let mut p = prog(src);
        let r = rows(&p);
        let raws = r.iter().filter(|x| x.kind == CardKind::Raw).count();
        assert_eq!(raws, 2, "{r:?}");
        assert!(r.iter().all(|x| !x.text.contains('\n')));
        let id = id_of(&p, "return n;");
        let text = move_stmt(&mut p, id, true).unwrap();
        assert!(text.contains("switch (n)") && text.contains("} while (n > 0);"));
        assert!(text.find("return n;").unwrap() < text.find("} while").unwrap());
        assert!(parse_with_prelude(PRE, &text).is_ok());
    }

    #[test]
    fn structural_edits_round_trip_through_text() {
        let mut p = prog(SRC);
        let ret = id_of(&p, "return t;");
        let text = move_stmt(&mut p, ret, true).unwrap();
        assert!(text.find("return t;").unwrap() < text.find("for (int x").unwrap());
        let mut p = prog(&text);
        let t0 = id_of(&p, "int t = 0;");
        let text = edit_stmt(&mut p, t0, "int total = 1;").unwrap();
        assert!(text.contains("int total = 1;"));
        let mut p = prog(&text);
        let lp = id_of(&p, "for (int x : a)");
        let text = edit_stmt(&mut p, lp, "for (int y : a)").unwrap();
        assert!(
            text.contains("for (int y : a) {") && text.contains("else {"),
            "{text}"
        );
        let mut p = prog(&text);
        let first = id_of(&p, "int total = 1;");
        let text = insert_stmt(&mut p, "f", Some(first), false, "int n = 5;").unwrap();
        assert!(parse_with_prelude(PRE, &text).is_ok() && text.contains("int n = 5;"));
        let mut p = prog(&text);
        let id = id_of(&p, "int n = 5;");
        let text = delete_stmt(&mut p, id).unwrap();
        assert!(!text.contains("int n"));
    }

    #[test]
    fn single_statement_bodies_get_braces_when_edited() {
        let mut p = prog("int f(int n) {\n    if (n > 0) return 1;\n    return 0;\n}\n");
        let id = id_of(&p, "if (n > 0)");
        let text = insert_stmt(&mut p, "f", Some(id), true, "n = 3;").unwrap();
        assert!(
            text.contains("if (n > 0) {\n        n = 3;\n        return 1;\n    }"),
            "{text}"
        );
    }

    #[test]
    fn else_if_cards_can_be_edited_but_not_moved() {
        let mut p = prog(SRC);
        let id = id_of(&p, "else if (x < 0)");
        assert!(move_stmt(&mut p, id, true).is_none());
        assert!(delete_stmt(&mut p, id).is_none());
        let text = edit_stmt(&mut p, id, "else if (x < 5)").unwrap();
        assert!(
            text.contains("else if (x < 5)") && text.contains("t -= 1;"),
            "{text}"
        );
        let text2 = insert_stmt(&mut p, "f", Some(id), true, "t = 9;").unwrap();
        assert!(text2.contains("t = 9;"));
    }

    #[test]
    fn card_text_is_one_line_and_bad_text_is_reported() {
        let mut p = prog(SRC);
        let lp = id_of(&p, "for (int x : a)");
        assert_eq!(card_text(&p, lp).as_deref(), Some("for (int x : a)"));
        let id = id_of(&p, "int t = 0;");
        assert!(edit_stmt(&mut p, id, "int = ;").is_err());
        assert!(parse_stmt(&mut p, "a; b;").is_err());
        assert!(parse_stmt(&mut p, "  ").is_err());
        // a statement written without its semicolon is reported by the parser, not applied
        assert!(edit_stmt(&mut p, id, "int t = 1").is_err());
    }

    #[test]
    fn palette_templates_use_the_function_names() {
        let p = prog(SRC);
        let f = p.find("f");
        let fill = Fill::for_function("v1".into(), f);
        assert_eq!(fill.apply("{c}.push_back(0);"), "a.push_back(0);");
        assert_eq!(fill.apply("return {ret};"), "return 0;");
        assert!(
            fill.apply("for (int i = 0; i < {len}; ++i)")
                .contains("a.size()")
        );
        for (_, t) in PALETTE {
            let mut p = prog(SRC);
            let text = fill.apply(t);
            assert!(parse_stmt(&mut p, &text).is_ok(), "{t} -> {text}");
        }
    }
}
