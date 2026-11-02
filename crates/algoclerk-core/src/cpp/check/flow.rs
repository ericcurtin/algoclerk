// SPDX-License-Identifier: Apache-2.0
//! Structural flow checks: a non-void function must not fall off its end, and recursion is a lock.

use super::ctx::Checker;
use crate::cpp::ast::*;
use std::collections::{HashMap, HashSet};

/// Is the expression a constant that is true (`true`, `1`)?
fn const_true(e: &Option<Expr>) -> bool {
    match e {
        None => true,
        Some(e) => {
            matches!(&e.kind, ExprKind::Bool(true))
                || matches!(&e.kind, ExprKind::Int { value, .. } if *value != 0)
        }
    }
}

/// Does a `break` inside `s` leave the enclosing loop or switch (not an inner one)?
fn has_break(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::Break => true,
        StmtKind::If { then, els, .. } => {
            has_break(then) || els.as_ref().is_some_and(|e| has_break(e))
        }
        StmtKind::Block(b) => b.stmts.iter().any(has_break),
        _ => false,
    }
}

fn has_continue(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::Continue => true,
        StmtKind::If { then, els, .. } => {
            has_continue(then) || els.as_ref().is_some_and(|e| has_continue(e))
        }
        StmtKind::Block(b) => b.stmts.iter().any(has_continue),
        _ => false,
    }
}

/// Can control reach the end of the statement list? `Case` labels make the rest reachable again.
fn ends(stmts: &[Stmt]) -> bool {
    let mut live = true;
    for s in stmts {
        if matches!(s.kind, StmtKind::Case(_)) {
            live = true;
        } else if live && !reach(s) {
            live = false;
        }
    }
    live
}

/// Can control continue after this statement?
fn reach(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::Return(_) => false,
        StmtKind::Block(b) => ends(&b.stmts),
        StmtKind::If {
            then, els: Some(e), ..
        } => reach(then) || reach(e),
        StmtKind::While { cond, body } => {
            let forever = matches!(&cond.kind, ExprKind::Bool(true))
                || matches!(&cond.kind, ExprKind::Int { value, .. } if *value != 0);
            !forever || has_break(body)
        }
        StmtKind::For { cond, body, .. } => !const_true(cond) || has_break(body),
        StmtKind::DoWhile { body, cond } => {
            let forever = matches!(&cond.kind, ExprKind::Bool(true))
                || matches!(&cond.kind, ExprKind::Int { value, .. } if *value != 0);
            if forever {
                has_break(body)
            } else {
                reach(body) || has_break(body) || has_continue(body)
            }
        }
        StmtKind::Switch { body, .. } => {
            let has_default = body
                .stmts
                .iter()
                .any(|s| matches!(s.kind, StmtKind::Case(None)));
            !has_default || body.stmts.iter().any(has_break) || ends(&body.stmts)
        }
        _ => true,
    }
}

impl Checker<'_> {
    /// Missing `return` and recursion, for the player's functions.
    pub fn flow_checks(&mut self) {
        let p = self.p;
        let mut calls: HashMap<&str, HashSet<String>> = HashMap::new();
        for it in p.player_items() {
            let ItemKind::Func(f) = &it.kind else {
                continue;
            };
            let Some(body) = &f.body else { continue };
            if f.ret.ty != Ty::Void && ends(&body.stmts) && f.name != "main" {
                self.err(f.id, "flow.missing-return", format!("`{}` must give back a {}, but some path reaches the closing brace without a `return`", f.name, f.ret.ty));
            }
            let mut set = HashSet::new();
            walk_stmts(body, &mut |s| {
                stmt_exprs(s, &mut |e| {
                    walk_expr(e, &mut |x| {
                        if let ExprKind::Call { callee, .. } = &x.kind
                            && let ExprKind::Var(n) = &callee.kind
                        {
                            set.insert(n.clone());
                        }
                    });
                });
            });
            calls.entry(&f.name).or_default().extend(set);
            if let Some(name) = self.lambda_recursion(body) {
                self.recursion_error(f.id, &f.name, &format!("the lambda `{name}` calls itself"));
            }
        }
        if self.o.recursion {
            return;
        }
        let names: Vec<&str> = calls.keys().copied().collect();
        for f in names {
            if reaches(&calls, f, f) {
                let id = p.player_items().iter().find_map(|i| match &i.kind {
                    ItemKind::Func(d) if d.name == f => Some(d.id),
                    _ => None,
                });
                self.recursion_error(
                    id.unwrap_or(0),
                    f,
                    &format!("`{f}` calls itself, directly or through other functions"),
                );
            }
        }
    }

    fn recursion_error(&mut self, id: NodeId, f: &str, why: &str) {
        if !self.o.recursion {
            self.err(
                id,
                "lock.recursion",
                format!("this floor does not allow recursion (in `{f}`): {why}"),
            );
        }
    }

    /// A variable initialised with a lambda that calls the variable itself (the `std::function` idiom).
    fn lambda_recursion(&self, body: &Block) -> Option<String> {
        let mut found = None;
        walk_stmts(body, &mut |s| {
            if let StmtKind::Decl { vars, .. } = &s.kind {
                for d in vars {
                    let init = match &d.init {
                        Init::Copy(e) => Some(e),
                        _ => None,
                    };
                    if let Some(e) = init {
                        walk_expr(e, &mut |x| {
                            if let ExprKind::Lambda(l) = &x.kind {
                                let mut hit = false;
                                walk_stmts(&l.body, &mut |ls| {
                                    stmt_exprs(ls, &mut |le| {
                                        walk_expr(le, &mut |y| {
                                            if let ExprKind::Call { callee, .. } = &y.kind {
                                                hit |= matches!(&callee.kind, ExprKind::Var(n) if *n == d.name);
                                            }
                                        });
                                    });
                                });
                                if hit {
                                    found = Some(d.name.clone());
                                }
                            }
                        });
                    }
                }
            }
        });
        found
    }
}

/// Can `from` reach `target` through calls?
fn reaches(g: &HashMap<&str, HashSet<String>>, from: &str, target: &str) -> bool {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut stack = vec![from];
    while let Some(n) = stack.pop() {
        let Some(next) = g.get(n) else { continue };
        for m in next {
            if m == target {
                return true;
            }
            if let Some((k, _)) = g.get_key_value(m.as_str())
                && seen.insert(k)
            {
                stack.push(k);
            }
        }
    }
    false
}
