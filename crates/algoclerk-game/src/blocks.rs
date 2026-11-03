// SPDX-License-Identifier: Apache-2.0
//! The block (card) view of a program: rows for display and structural edits that keep the program and its
//! text in sync (the text is always the printed syntax tree).
//!
//! Every statement of a function body is one card. Compound statements (`if`, `for`, `while`, range-`for`,
//! `do`-`while`, `switch`) show their header and hold their body as nested cards; simple statements show
//! their one line. Comments above and after a statement travel with its card. What cards cannot express
//! (bare blocks, statements that print on several lines such as lambdas with bodies) is shown as a
//! read-only *raw C++* card that can still be moved and deleted.

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
    /// The closing `while (cond);` of a do-while card.
    Foot,
    /// C++ the cards cannot express (read only text, can be moved and deleted).
    Raw,
    /// A top-level item that is not a function (include, alias, global, struct): read only.
    Item,
}

/// Which part of a statement holds child cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The body of a function.
    Fn,
    /// The `then` branch of an `if`.
    Then,
    /// The `else` branch of an `if`.
    Else,
    /// The body of a loop, a `do`-`while` or a `switch`.
    Body,
}

/// A place cards can be dropped: a statement (or function) and one of its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// The statement or function that owns the block.
    pub owner: NodeId,
    /// Which block of the owner.
    pub part: Part,
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
    /// The block this card sits in and its position there (`None` for headers, items, `else` labels).
    pub at: Option<(Slot, usize)>,
    /// Where the child cards of this row live.
    pub body: Option<Slot>,
    /// Comment lines above the statement.
    pub comments: Vec<String>,
    /// Comment at the end of the statement's line.
    pub trailing: Option<String>,
}

impl Row {
    /// Can the text of this card be edited?
    pub fn editable(&self) -> bool {
        matches!(
            self.kind,
            CardKind::Compound | CardKind::Simple | CardKind::Foot
        )
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
        StmtKind::Return(_) => "right",
        StmtKind::Block(_) | StmtKind::Empty => "code",
    }
}

fn body_slot(k: &StmtKind, id: NodeId) -> Option<Slot> {
    let part = match k {
        StmtKind::If { .. } => Part::Then,
        StmtKind::For { .. }
        | StmtKind::RangeFor { .. }
        | StmtKind::While { .. }
        | StmtKind::DoWhile { .. }
        | StmtKind::Switch { .. } => Part::Body,
        _ => return None,
    };
    Some(Slot { owner: id, part })
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
            | StmtKind::DoWhile { .. }
            | StmtKind::Switch { .. }
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
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => *body = empty_body(),
        StmtKind::Switch { body, .. } => *body = Block::default(),
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
        StmtKind::Block(_) | StmtKind::Empty => true,
        StmtKind::DoWhile { .. } => {
            let printed = print_stmt(&header_clone(s), 0);
            let t = printed.trim_end();
            t.contains('\n') || !t.starts_with("do {} while")
        }
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

/// The closing line of a do-while card: `while (cond);`.
fn foot_text(s: &Stmt) -> String {
    let t = header_text(s);
    t.strip_prefix("do {}")
        .map_or_else(|| t.clone(), |r| r.trim().to_string())
}

fn row_of(s: &Stmt, depth: usize, chained: bool, at: Option<(Slot, usize)>) -> Row {
    let raw = needs_raw(s);
    let text = if raw {
        one_line(print_stmt(&header_clone(s), 0).trim_end())
    } else if chained {
        format!("else {}", header_text(s))
    } else if matches!(s.kind, StmtKind::DoWhile { .. }) {
        "do".to_string()
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
        at,
        body: if raw { None } else { body_slot(&s.kind, s.id) },
        comments: s.comments.clone(),
        trailing: s.trailing.clone(),
    }
}

fn label_row(node: NodeId, depth: usize, text: String, kind: CardKind, body: Option<Slot>) -> Row {
    Row {
        node,
        depth,
        text,
        icon: "signpost",
        kind,
        chained: false,
        at: None,
        body,
        comments: Vec::new(),
        trailing: None,
    }
}

fn rows_stmt(s: &Stmt, depth: usize, chained: bool, at: Option<(Slot, usize)>, out: &mut Vec<Row>) {
    out.push(row_of(s, depth, chained, at));
    if needs_raw(s) {
        return;
    }
    let slot = |part| Slot { owner: s.id, part };
    match &s.kind {
        StmtKind::If { then, els, .. } => {
            rows_body(then, depth + 1, slot(Part::Then), out);
            match els.as_deref() {
                Some(e) if matches!(e.kind, StmtKind::If { .. }) => {
                    rows_stmt(e, depth, true, None, out);
                }
                Some(e) => {
                    out.push(label_row(
                        s.id,
                        depth,
                        "else".into(),
                        CardKind::Else,
                        Some(slot(Part::Else)),
                    ));
                    rows_body(e, depth + 1, slot(Part::Else), out);
                }
                None => {}
            }
        }
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. } => rows_body(body, depth + 1, slot(Part::Body), out),
        StmtKind::DoWhile { body, .. } => {
            rows_body(body, depth + 1, slot(Part::Body), out);
            out.push(label_row(s.id, depth, foot_text(s), CardKind::Foot, None));
        }
        StmtKind::Switch { body, .. } => {
            // statements under a `case` label sit one level deeper, to read like the printed text
            let mut under_case = false;
            for (i, st) in body.stmts.iter().enumerate() {
                let is_case = matches!(st.kind, StmtKind::Case(_));
                under_case = if is_case { true } else { under_case };
                let d = depth + 1 + usize::from(under_case && !is_case);
                rows_stmt(st, d, false, Some((slot(Part::Body), i)), out);
            }
        }
        _ => {}
    }
}

fn rows_body(b: &Stmt, depth: usize, slot: Slot, out: &mut Vec<Row>) {
    match then_block(b) {
        Some(blk) => rows_block(blk, depth, slot, out),
        None => rows_stmt(b, depth, false, Some((slot, 0)), out),
    }
}

fn rows_block(b: &Block, depth: usize, slot: Slot, out: &mut Vec<Row>) {
    for (i, s) in b.stmts.iter().enumerate() {
        rows_stmt(s, depth, false, Some((slot, i)), out);
    }
}

/// All rows of the player's part of a program.
pub fn rows(p: &Program) -> Vec<Row> {
    let mut out = Vec::new();
    for it in p.player_items() {
        match &it.kind {
            ItemKind::Func(f) if f.body.is_some() => {
                let slot = Slot {
                    owner: f.id,
                    part: Part::Fn,
                };
                let mut head = label_row(f.id, 0, signature_text(f), CardKind::Header, Some(slot));
                head.icon = "code";
                out.push(head);
                if let Some(b) = &f.body {
                    rows_block(b, 1, slot, &mut out);
                }
            }
            _ => {
                let mut one = Program::default();
                one.items.push(it.clone());
                let mut item = label_row(
                    it.id,
                    0,
                    one_line(print_program(&one, false).trim_end()),
                    CardKind::Item,
                    None,
                );
                item.icon = "code";
                out.push(item);
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
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => {
            wrap_block(next, body);
            norm_stmt(next, body);
        }
        StmtKind::Switch { body, .. } => norm_block(next, body),
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
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => match &mut body.kind {
            StmtKind::Block(b) => visit_block(b, id, f),
            _ => false,
        },
        StmtKind::Switch { body, .. } => visit_block(body, id, f),
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

/// One step up or down: swap with the neighbour, or at the edge of a block step out of it (before or after
/// the card that owns the block). Returns the new text.
pub fn step_stmt(p: &mut Program, id: NodeId, up: bool) -> Option<String> {
    let mut probe = p.clone();
    if let Some(t) = move_stmt(&mut probe, id, up) {
        *p = probe;
        return Some(t);
    }
    normalise(p);
    let (slot, i) = locate(p, id)?;
    let len = block_len(p, slot)?;
    let (parent, at) = locate(p, slot.owner)?;
    match (up, i, i + 1 == len) {
        (true, 0, _) => move_to(p, id, parent, at),
        (false, _, true) => move_to(p, id, parent, at + 1),
        _ => None,
    }
}

fn block_len(p: &mut Program, slot: Slot) -> Option<usize> {
    let mut n = None;
    with_slot(p, slot, &mut |b| n = Some(b.stmts.len()));
    n
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

fn with_stmt_block(b: &mut Block, id: NodeId, f: &mut dyn FnMut(&mut Stmt)) -> bool {
    for s in &mut b.stmts {
        if s.id == id {
            f(s);
            return true;
        }
        if with_stmt_in(s, id, f) {
            return true;
        }
    }
    false
}

fn with_stmt_in(s: &mut Stmt, id: NodeId, f: &mut dyn FnMut(&mut Stmt)) -> bool {
    match &mut s.kind {
        StmtKind::If { then, els, .. } => {
            if let StmtKind::Block(b) = &mut then.kind
                && with_stmt_block(b, id, f)
            {
                return true;
            }
            match els {
                Some(e) if e.id == id => {
                    f(e);
                    true
                }
                Some(e) => match &mut e.kind {
                    StmtKind::Block(b) => with_stmt_block(b, id, f),
                    StmtKind::If { .. } => with_stmt_in(e, id, f),
                    _ => false,
                },
                None => false,
            }
        }
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => match &mut body.kind {
            StmtKind::Block(b) => with_stmt_block(b, id, f),
            _ => false,
        },
        StmtKind::Switch { body, .. } => with_stmt_block(body, id, f),
        _ => false,
    }
}

/// Run `f` on the block of a slot.
fn with_slot(p: &mut Program, slot: Slot, f: &mut dyn FnMut(&mut Block)) -> bool {
    let skip = p.prelude_items;
    if slot.part == Part::Fn {
        for it in p.items.iter_mut().skip(skip) {
            if let ItemKind::Func(func) = &mut it.kind
                && func.id == slot.owner
                && let Some(b) = &mut func.body
            {
                f(b);
                return true;
            }
        }
        return false;
    }
    let mut found = false;
    for it in p.items.iter_mut().skip(skip) {
        if let ItemKind::Func(func) = &mut it.kind
            && let Some(b) = &mut func.body
            && with_stmt_block(b, slot.owner, &mut |s: &mut Stmt| {
                let block = match (&mut s.kind, slot.part) {
                    (StmtKind::If { then, .. }, Part::Then) => match &mut then.kind {
                        StmtKind::Block(b) => Some(b),
                        _ => None,
                    },
                    (StmtKind::If { els: Some(e), .. }, Part::Else) => match &mut e.kind {
                        StmtKind::Block(b) => Some(b),
                        _ => None,
                    },
                    (
                        StmtKind::For { body, .. }
                        | StmtKind::RangeFor { body, .. }
                        | StmtKind::While { body, .. }
                        | StmtKind::DoWhile { body, .. },
                        Part::Body,
                    ) => match &mut body.kind {
                        StmtKind::Block(b) => Some(b),
                        _ => None,
                    },
                    (StmtKind::Switch { body, .. }, Part::Body) => Some(body),
                    _ => None,
                };
                if let Some(b) = block {
                    f(b);
                    found = true;
                }
            })
        {
            return found;
        }
    }
    found
}

/// Ask a question of the statement with this id.
fn find_stmt<R>(p: &Program, id: NodeId, f: impl Fn(&Stmt) -> R) -> Option<R> {
    let mut out = None;
    for it in p.player_items() {
        if let ItemKind::Func(func) = &it.kind
            && let Some(b) = &func.body
        {
            walk_stmts(b, &mut |s| {
                if s.id == id {
                    out = Some(f(s));
                }
            });
        }
    }
    out
}

fn contains_id(s: &Stmt, id: NodeId) -> bool {
    let mut hit = false;
    walk_stmts_in(s, &mut |x| hit |= x.id == id);
    hit
}

fn walk_stmts_in(s: &Stmt, f: &mut dyn FnMut(&Stmt)) {
    f(s);
    match &s.kind {
        StmtKind::If { then, els, .. } => {
            walk_stmts_in(then, f);
            if let Some(e) = els {
                walk_stmts_in(e, f);
            }
        }
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => walk_stmts_in(body, f),
        StmtKind::Block(b) | StmtKind::Switch { body: b, .. } => {
            for st in &b.stmts {
                walk_stmts_in(st, f);
            }
        }
        _ => {}
    }
}

/// Where a statement sits: its block and position (not for `else if` chains, which are not in a block).
pub fn locate(p: &Program, id: NodeId) -> Option<(Slot, usize)> {
    let mut found = None;
    for r in rows(p) {
        if r.node == id && r.kind != CardKind::Else && r.kind != CardKind::Foot && r.at.is_some() {
            found = r.at;
        }
    }
    found
}

/// Move a statement to position `index` of the block `slot` (before the statement that is there now).
/// Returns the new text, or `None` when the move is not possible (an `else if`, into itself, no such
/// block) or would change nothing.
pub fn move_to(p: &mut Program, id: NodeId, slot: Slot, index: usize) -> Option<String> {
    normalise(p);
    // `case` and `default` labels only make sense directly inside a switch
    let is_label = find_stmt(p, id, |s| matches!(s.kind, StmtKind::Case(_)))?;
    let into_switch =
        find_stmt(p, slot.owner, |s| matches!(s.kind, StmtKind::Switch { .. })).unwrap_or(false);
    if is_label && !into_switch {
        return None;
    }
    let (from, i) = locate(p, id)?;
    // a card cannot move into itself or into one of its own children
    let mut inside = false;
    let skip = p.prelude_items;
    for it in p.items.iter().skip(skip) {
        if let ItemKind::Func(f) = &it.kind
            && let Some(b) = &f.body
        {
            for st in &b.stmts {
                walk_stmts_in(st, &mut |x| {
                    if x.id == id && contains_id(x, slot.owner) {
                        inside = true;
                    }
                });
            }
        }
    }
    if inside {
        return None;
    }
    let mut index = index;
    if from == slot {
        if index == i || index == i + 1 {
            return None;
        }
        if i < index {
            index -= 1;
        }
    }
    let mut taken: Option<Stmt> = None;
    with_slot(p, from, &mut |b| {
        if i < b.stmts.len() {
            taken = Some(b.stmts.remove(i));
        }
    });
    let stmt = taken?;
    let mut slot_ok = false;
    let mut stmt = Some(stmt);
    with_slot(p, slot, &mut |b| {
        if let Some(s) = stmt.take() {
            let at = index.min(b.stmts.len());
            b.stmts.insert(at, s);
            slot_ok = true;
        }
    });
    slot_ok.then(|| reprint(p))
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
    let label = t.starts_with("case ") || t.starts_with("default");
    let body = if t.ends_with(';') || t.ends_with('}') || label {
        t.to_string()
    } else {
        format!("{t} {{}}")
    };
    // the card is read in the context of the program: `using namespace std;` makes `stack<char>` a type and
    // a global `const` makes `std::array<int, N>` a legal size
    let mut ctx = String::new();
    for it in &p.items {
        match &it.kind {
            ItemKind::UsingNamespaceStd => ctx.push_str("using namespace std;\n"),
            ItemKind::Global { .. } => {
                let mut one = Program::default();
                one.items.push(it.clone());
                ctx.push_str(&print_program(&one, false));
            }
            _ => {}
        }
    }
    let using = ctx;
    let wrapped = if label {
        format!("{using}void w() {{\nswitch (0) {{\n{body}\n}}\n}}\n")
    } else {
        format!("{using}void w() {{\n{body}\n}}\n")
    };
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
    }) = parsed
        .items
        .into_iter()
        .find(|i| matches!(i.kind, ItemKind::Func(_)))
    else {
        return Err("I cannot read that card.".into());
    };
    let mut stmts = f.body.map(|b| b.stmts).unwrap_or_default();
    if label && let Some(StmtKind::Switch { body, .. }) = stmts.first().map(|s| &s.kind) {
        stmts = body.stmts.clone();
    }
    if stmts.len() != 1 {
        return Err("A card holds exactly one sentence.".into());
    }
    Ok(stmts.remove(0))
}

/// Can the body of `old` be carried over to a statement of the shape `new`?
fn same_family(old: &StmtKind, new: &StmtKind) -> bool {
    let switch = |k: &StmtKind| matches!(k, StmtKind::Switch { .. });
    // a switch body is a list of labels and statements, a loop body is a block: they cannot swap
    !((switch(old) && !switch(new)) || (switch(new) && has_body(old) && !switch(old)))
}

fn has_body(k: &StmtKind) -> bool {
    matches!(
        k,
        StmtKind::If { .. }
            | StmtKind::For { .. }
            | StmtKind::RangeFor { .. }
            | StmtKind::While { .. }
            | StmtKind::DoWhile { .. }
            | StmtKind::Switch { .. }
    )
}

fn transplant(old: StmtKind, new: &mut StmtKind) {
    if let (StmtKind::Switch { body: ob, .. }, StmtKind::Switch { body: nb, .. }) =
        (&old, &mut *new)
    {
        *nb = ob.clone();
        return;
    }
    let (body, old_els) = match old {
        StmtKind::If { then, els, .. } => (Some(then), els),
        StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => (Some(body), None),
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
        | StmtKind::While { body: b, .. }
        | StmtKind::DoWhile { body: b, .. } => *b = body,
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
    let mut refused = false;
    let done = with_target(p, id, &mut |t| {
        let s = match t {
            Target::InBlock(b, i) => &mut b.stmts[i],
            Target::Chained(s) => s,
        };
        if let Some(n) = slot.take() {
            if !same_family(&s.kind, &n.kind) {
                refused = true;
                return;
            }
            let old = std::mem::replace(&mut s.kind, StmtKind::Empty);
            let mut kind = n.kind;
            transplant(old, &mut kind);
            s.kind = kind;
        }
    });
    if refused {
        return Err(
            "A switch or do card keeps its shape; delete it and add the new card instead.".into(),
        );
    }
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
        | StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. } => body,
        StmtKind::Switch { body, .. } => return Some(body),
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
    ("do while", "do {} while (false);"),
    ("switch", "switch (0)"),
    ("case", "case 0:"),
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
        let src = "int g(int n) {\n    switch (n) {\n        case 1:\n            return 1;\n        default:\n            break;\n    }\n    do {\n        --n;\n    } while (n > 0);\n    {\n        n += 2;\n    }\n    return n;\n}\n";
        let mut p = prog(src);
        let r = rows(&p);
        let raws = r.iter().filter(|x| x.kind == CardKind::Raw).count();
        assert_eq!(raws, 1, "the bare block is raw: {r:?}");
        let sw = r.iter().find(|x| x.text == "switch (n)").unwrap();
        assert_eq!(sw.kind, CardKind::Compound, "a switch holds nested cards");
        assert!(
            r.iter()
                .any(|x| x.kind == CardKind::Foot && x.text == "while (n > 0);")
        );
        assert!(r.iter().all(|x| !x.text.contains('\n')));
        let id = id_of(&p, "return n;");
        let text = move_stmt(&mut p, id, true).unwrap();
        assert!(text.contains("switch (n)") && text.contains("} while (n > 0);"));
        assert!(text.find("return n;").unwrap() < text.find("n += 2;").unwrap());
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

    #[test]
    fn cards_move_into_and_out_of_blocks() {
        let mut p = prog(SRC);
        // drop `return t;` into the for loop, at its start
        let ret = id_of(&p, "return t;");
        let lp = id_of(&p, "for (int x : a)");
        let body = rows(&p)
            .into_iter()
            .find(|r| r.node == lp)
            .unwrap()
            .body
            .unwrap();
        let text = move_to(&mut p, ret, body, 0).unwrap();
        assert!(
            text.find("return t;").unwrap() < text.find("if (x > 2)").unwrap(),
            "{text}"
        );
        assert!(text.find("return t;").unwrap() > text.find("for (int x").unwrap());
        assert!(parse_with_prelude(PRE, &text).is_ok());
        // a card cannot go into itself
        let mut p = prog(SRC);
        let lp = id_of(&p, "for (int x : a)");
        let inner = id_of(&p, "if (x > 2)");
        let inner_body = rows(&p)
            .into_iter()
            .find(|r| r.node == inner)
            .unwrap()
            .body
            .unwrap();
        assert!(move_to(&mut p, lp, inner_body, 0).is_none());
        // stepping up at the edge of a block leaves it
        let mut p = prog(SRC);
        let first = id_of(&p, "if (x > 2)");
        let text = step_stmt(&mut p, first, true).unwrap();
        assert!(
            text.find("if (x > 2)").unwrap() < text.find("for (int x").unwrap(),
            "{text}"
        );
        assert!(parse_with_prelude(PRE, &text).is_ok());
        let mut p = prog(SRC);
        let last = id_of(&p, "int t = 0;");
        let text = step_stmt(&mut p, last, false).unwrap();
        assert!(text.find("int t = 0;").unwrap() > text.find("for (int x").unwrap());
    }

    #[test]
    fn comments_travel_with_their_cards() {
        let src = "int f(int n) {\n    // start here\n    int t = n; // copy\n    return t;\n}\n";
        let mut p = prog(src);
        let r = rows(&p);
        let t = r.iter().find(|x| x.text == "int t = n;").unwrap();
        assert_eq!(t.comments, vec![" start here".to_string()]);
        assert_eq!(t.trailing.as_deref(), Some(" copy"));
        let id = t.node;
        let text = edit_stmt(&mut p, id, "int t = n + 1;").unwrap();
        assert!(
            text.contains("// start here") && text.contains("// copy"),
            "{text}"
        );
        let mut p = prog(&text);
        let id = id_of(&p, "int t = n + 1;");
        let text = move_stmt(&mut p, id, false).unwrap();
        assert!(
            text.find("return t;").unwrap() < text.find("// start here").unwrap(),
            "{text}"
        );
    }
}
