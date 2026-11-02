// SPDX-License-Identifier: Apache-2.0
//! The block (card) view of a program: rows for display and structural edits that keep the program
//! and its text in sync (the text is always the printed syntax tree).

use algoclerk_core::lang::ast::*;
use algoclerk_core::lang::parse::parse;
use algoclerk_core::lang::print::{expr_text, pat_text, print};

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
}

/// One row of the block view.
#[derive(Debug, Clone)]
pub struct Row {
    /// Statement or function id.
    pub node: NodeId,
    /// Nesting depth.
    pub depth: usize,
    /// Card text.
    pub text: String,
    /// Icon name.
    pub icon: &'static str,
    /// Card kind.
    pub kind: CardKind,
}

fn icon_of(k: &StmtKind) -> &'static str {
    match k {
        StmtKind::Let { .. } => "label",
        StmtKind::Assign { .. } => "pointer",
        StmtKind::If { .. } => "signpost",
        StmtKind::While { .. } | StmtKind::ForRange { .. } | StmtKind::ForEach { .. } => "reset",
        StmtKind::Break => "cross",
        StmtKind::Continue => "fast",
        StmtKind::Return(_) => "outbox",
        StmtKind::Expr(_) => "hand",
    }
}

/// The header text of a statement (what the Edit button changes).
pub fn header_text(s: &Stmt) -> String {
    match &s.kind {
        StmtKind::Let { pat, ty, init } => {
            let t = ty.as_ref().map(|t| format!(": {t}")).unwrap_or_default();
            format!("let {}{t} = {};", pat_text(pat), expr_text(init))
        }
        StmtKind::Assign { target, op, value } => {
            format!("{} {}= {};", expr_text(target), op.map_or(String::new(), |o| o.text().to_string()), expr_text(value))
        }
        StmtKind::If { cond, .. } => format!("if {}", expr_text(cond)),
        StmtKind::While { cond, .. } => format!("while {}", expr_text(cond)),
        StmtKind::ForRange { var, start, end, step, .. } => {
            let mut a = Vec::new();
            if let Some(x) = start {
                a.push(expr_text(x));
            } else if step.is_some() {
                a.push("0".into());
            }
            a.push(expr_text(end));
            if let Some(x) = step {
                a.push(expr_text(x));
            }
            format!("for {var} in range({})", a.join(", "))
        }
        StmtKind::ForEach { pat, iter, .. } => format!("for {} in {}", pat_text(pat), expr_text(iter)),
        StmtKind::Break => "break;".into(),
        StmtKind::Continue => "continue;".into(),
        StmtKind::Return(Some(e)) => format!("return {};", expr_text(e)),
        StmtKind::Return(None) => "return;".into(),
        StmtKind::Expr(e) => format!("{};", expr_text(e)),
    }
}

fn rows_block(b: &Block, depth: usize, out: &mut Vec<Row>) {
    for s in &b.stmts {
        let compound = matches!(s.kind, StmtKind::If { .. } | StmtKind::While { .. } | StmtKind::ForRange { .. } | StmtKind::ForEach { .. });
        out.push(Row { node: s.id, depth, text: header_text(s), icon: icon_of(&s.kind), kind: if compound { CardKind::Compound } else { CardKind::Simple } });
        match &s.kind {
            StmtKind::If { then, els, .. } => {
                rows_block(then, depth + 1, out);
                if let Some(e) = els {
                    out.push(Row { node: s.id, depth, text: "else".into(), icon: "signpost", kind: CardKind::Else });
                    rows_block(e, depth + 1, out);
                }
            }
            StmtKind::While { body, .. } | StmtKind::ForRange { body, .. } | StmtKind::ForEach { body, .. } => rows_block(body, depth + 1, out),
            _ => {}
        }
    }
}

/// All rows of a program.
pub fn rows(p: &Program) -> Vec<Row> {
    let mut out = Vec::new();
    for f in &p.fns {
        out.push(Row { node: f.id, depth: 0, text: algoclerk_core::lang::print::signature_text(f), icon: "code", kind: CardKind::Header });
        rows_block(&f.body, 1, &mut out);
    }
    out
}

fn locate<'a>(b: &'a mut Block, id: NodeId) -> Option<(&'a mut Block, usize)> {
    if let Some(i) = b.stmts.iter().position(|s| s.id == id) {
        return Some((b, i));
    }
    for s in &mut b.stmts {
        let found = match &mut s.kind {
            StmtKind::If { then, els, .. } => {
                if then.stmts.iter().any(|x| contains(x, id)) {
                    locate(then, id)
                } else if let Some(e) = els {
                    locate(e, id)
                } else {
                    None
                }
            }
            StmtKind::While { body, .. } | StmtKind::ForRange { body, .. } | StmtKind::ForEach { body, .. } => locate(body, id),
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

fn contains(s: &Stmt, id: NodeId) -> bool {
    if s.id == id {
        return true;
    }
    match &s.kind {
        StmtKind::If { then, els, .. } => then.stmts.iter().any(|x| contains(x, id)) || els.as_ref().is_some_and(|e| e.stmts.iter().any(|x| contains(x, id))),
        StmtKind::While { body, .. } | StmtKind::ForRange { body, .. } | StmtKind::ForEach { body, .. } => body.stmts.iter().any(|x| contains(x, id)),
        _ => false,
    }
}

fn locate_in_prog(p: &mut Program, id: NodeId) -> Option<(&mut Block, usize)> {
    for f in &mut p.fns {
        if f.body.stmts.iter().any(|s| contains(s, id)) {
            return locate(&mut f.body, id);
        }
    }
    None
}

fn find_stmt<'a>(b: &'a mut Block, id: NodeId) -> Option<&'a mut Stmt> {
    let (blk, i) = locate(b, id)?;
    blk.stmts.get_mut(i)
}

fn reprint(p: &Program) -> String {
    print(p)
}

/// Move a statement one place up (or down) among its siblings.
pub fn move_stmt(p: &mut Program, id: NodeId, up: bool) -> Option<String> {
    let (blk, i) = locate_in_prog(p, id)?;
    let j = if up { i.checked_sub(1)? } else { i + 1 };
    if j >= blk.stmts.len() {
        return None;
    }
    blk.stmts.swap(i, j);
    Some(reprint(p))
}

/// Delete a statement.
pub fn delete_stmt(p: &mut Program, id: NodeId) -> Option<String> {
    let (blk, i) = locate_in_prog(p, id)?;
    blk.stmts.remove(i);
    Some(reprint(p))
}

/// Parse a statement from header text (compound statements get an empty body).
pub fn parse_stmt(text: &str) -> Result<Stmt, String> {
    let t = text.trim();
    let wrapped = if t.ends_with(';') || t.ends_with('}') { format!("fn w() {{ {t} }}") } else { format!("fn w() {{ {t} {{ }} }}") };
    let p = parse(&wrapped).map_err(|d| d.msg)?;
    let f = p.fns.into_iter().next().ok_or("empty")?;
    if f.body.stmts.len() != 1 {
        return Err("A card holds exactly one sentence.".into());
    }
    f.body.stmts.into_iter().next().ok_or_else(|| "empty".to_string())
}

/// Replace the header of a statement, keeping its body.
pub fn edit_stmt(p: &mut Program, id: NodeId, text: &str) -> Result<String, String> {
    let new = parse_stmt(text)?;
    let mut done = false;
    for f in &mut p.fns {
        if let Some(s) = find_stmt(&mut f.body, id) {
            let old = std::mem::replace(&mut s.kind, StmtKind::Break);
            s.kind = match (old, new.kind) {
                (StmtKind::If { then, els, .. }, StmtKind::If { cond, .. }) => StmtKind::If { cond, then, els },
                (StmtKind::While { body, .. }, StmtKind::While { cond, .. }) => StmtKind::While { cond, body },
                (StmtKind::ForRange { body, .. } | StmtKind::ForEach { body, .. }, StmtKind::ForRange { var, start, end, step, .. }) => StmtKind::ForRange { var, start, end, step, body },
                (StmtKind::ForRange { body, .. } | StmtKind::ForEach { body, .. }, StmtKind::ForEach { pat, iter, .. }) => StmtKind::ForEach { pat, iter, body },
                (_, other) => other,
            };
            done = true;
            break;
        }
    }
    if done { Ok(reprint(p)) } else { Err("That card is gone.".into()) }
}

/// Insert a new statement after `id` (or at the start of its body when `inside`).
pub fn insert_stmt(p: &mut Program, anchor: Option<NodeId>, inside: bool, text: &str) -> Result<String, String> {
    let mut new = parse_stmt(text)?;
    new.id = p.fresh_id();
    match anchor {
        None => {
            let f = p.fns.first_mut().ok_or("no function")?;
            f.body.stmts.push(new);
        }
        Some(id) => {
            if inside {
                let mut placed = false;
                for f in &mut p.fns {
                    if let Some(s) = find_stmt(&mut f.body, id) {
                        match &mut s.kind {
                            StmtKind::If { then, .. } => then.stmts.insert(0, new.clone()),
                            StmtKind::While { body, .. } | StmtKind::ForRange { body, .. } | StmtKind::ForEach { body, .. } => body.stmts.insert(0, new.clone()),
                            _ => return Err("That card has no inside.".into()),
                        }
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    return Err("That card is gone.".into());
                }
            } else {
                let (blk, i) = locate_in_prog(p, id).ok_or("That card is gone.")?;
                blk.stmts.insert(i + 1, new);
            }
        }
    }
    Ok(reprint(p))
}

/// The template cards of the palette. `{n}` is replaced with a fresh name.
pub const PALETTE: &[(&str, &str)] = &[
    ("let", "let {n} = 0;"),
    ("set", "{n} = {n};"),
    ("if", "if true"),
    ("while", "while true"),
    ("for i", "for i in range(len(items))"),
    ("for each", "for item in items"),
    ("push", "items.push(0);"),
    ("return", "return 0;"),
    ("break", "break;"),
    ("continue", "continue;"),
];

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "fn f(a: vec<int>) -> int {\n    let t = 0;\n    for x in a {\n        if x > 2 {\n            t += x;\n        } else {\n            t -= 1;\n        }\n    }\n    return t;\n}\n";

    #[test]
    fn rows_follow_the_text() {
        let p = parse(SRC).unwrap();
        let r = rows(&p);
        assert_eq!(r[0].kind, CardKind::Header);
        assert!(r.iter().any(|x| x.text == "else"));
        assert!(r.iter().any(|x| x.text == "for x in a" && x.depth == 1));
        assert!(r.iter().any(|x| x.text == "t += x;" && x.depth == 3));
    }

    #[test]
    fn structural_edits_round_trip_through_text() {
        let mut p = parse(SRC).unwrap();
        let ret = p.fns[0].body.stmts[2].id;
        let text = move_stmt(&mut p, ret, true).unwrap();
        assert!(text.find("return t;").unwrap() < text.find("for x in a").unwrap());
        let mut p = parse(&text).unwrap();
        let t0 = p.fns[0].body.stmts[0].id;
        let text = edit_stmt(&mut p, t0, "let total = 1;").unwrap();
        assert!(text.contains("let total = 1;"));
        let mut p = parse(&text).unwrap();
        let lp = p.fns[0].body.stmts[2].id;
        let text = edit_stmt(&mut p, lp, "for x in a").unwrap();
        assert!(text.contains("else {"));
        let mut p = parse(&text).unwrap();
        let first = p.fns[0].body.stmts[0].id;
        let text = insert_stmt(&mut p, Some(first), false, "let n = len(a);").unwrap();
        assert!(parse(&text).is_ok() && text.contains("let n = len(a);"));
        let mut p = parse(&text).unwrap();
        let id = p.fns[0].body.stmts[1].id;
        let text = delete_stmt(&mut p, id).unwrap();
        assert!(!text.contains("let n"));
    }

    #[test]
    fn bad_card_text_is_reported_not_applied() {
        let mut p = parse(SRC).unwrap();
        let id = p.fns[0].body.stmts[0].id;
        assert!(edit_stmt(&mut p, id, "let = ;").is_err());
        assert!(parse_stmt("a; b;").is_err());
    }
}
