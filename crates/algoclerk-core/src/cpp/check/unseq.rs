// SPDX-License-Identifier: Apache-2.0
//! Unsequenced modifications (`i + i++`, `f(i++, i)`): undefined behaviour that needs no execution to see.
//! C++17 orders the right operand of an assignment before the left, the operands of `&&`, `||`, `,`,
//! `<<`, `>>`, `?:` and `[]`, so those are fine; operands of the other binary operators and the arguments
//! of a call have no order.

use super::ctx::Checker;
use crate::cpp::ast::*;

/// Simple variables read and modified by an evaluation.
#[derive(Default)]
struct Eff {
    reads: Vec<String>,
    writes: Vec<String>,
}

impl Eff {
    fn absorb(&mut self, o: Eff) {
        self.reads.extend(o.reads);
        self.writes.extend(o.writes);
    }
    /// A variable that one side modifies and the other side reads or modifies.
    fn clash(&self, o: &Eff) -> Option<String> {
        self.writes
            .iter()
            .find(|w| o.writes.contains(w) || o.reads.contains(w))
            .or_else(|| o.writes.iter().find(|w| self.reads.contains(w)))
            .cloned()
    }
}

fn eff(e: &Expr, found: &mut Vec<(NodeId, String)>) -> Eff {
    let mut out = Eff::default();
    match &e.kind {
        ExprKind::Var(n) => out.reads.push(n.clone()),
        ExprKind::Unary(op, a) => {
            let step = matches!(
                op,
                UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec
            );
            match (&a.kind, step) {
                (ExprKind::Var(n), true) => out.writes.push(n.clone()),
                _ => out.absorb(eff(a, found)),
            }
        }
        ExprKind::Binary(op, a, b) => {
            let (ea, eb) = (eff(a, found), eff(b, found));
            let ordered = matches!(
                op,
                BinOp::And | BinOp::Or | BinOp::Comma | BinOp::Shl | BinOp::Shr
            );
            if !ordered && let Some(v) = ea.clash(&eb) {
                found.push((e.id, v));
            }
            out.absorb(ea);
            out.absorb(eb);
        }
        ExprKind::Assign { lhs, rhs, .. } => {
            out.absorb(eff(rhs, found));
            match &lhs.kind {
                ExprKind::Var(n) => out.writes.push(n.clone()),
                _ => out.absorb(eff(lhs, found)),
            }
        }
        ExprKind::Ternary(c, a, b) => {
            for x in [c, a, b] {
                out.absorb(eff(x, found));
            }
        }
        ExprKind::Index(a, i) => {
            out.absorb(eff(a, found));
            out.absorb(eff(i, found));
        }
        ExprKind::Call { callee, args } => {
            out.absorb(eff(callee, found));
            unordered(e.id, args, &mut out, found);
        }
        ExprKind::Construct {
            args, brace: false, ..
        }
        | ExprKind::New {
            args, brace: false, ..
        } => {
            unordered(e.id, args, &mut out, found);
        }
        ExprKind::Member { obj, .. } => out.absorb(eff(obj, found)),
        ExprKind::Cast { e: a, .. } | ExprKind::Delete(a) => out.absorb(eff(a, found)),
        ExprKind::InitList(items)
        | ExprKind::Construct { args: items, .. }
        | ExprKind::New { args: items, .. } => {
            for x in items {
                out.absorb(eff(x, found));
            }
        }
        _ => {}
    }
    out
}

/// Arguments with no order between them.
fn unordered(id: NodeId, args: &[Expr], out: &mut Eff, found: &mut Vec<(NodeId, String)>) {
    let effs: Vec<Eff> = args.iter().map(|a| eff(a, found)).collect();
    for (i, a) in effs.iter().enumerate() {
        for b in &effs[i + 1..] {
            if let Some(v) = a.clash(b) {
                found.push((id, v));
            }
        }
    }
    for x in effs {
        out.absorb(x);
    }
}

impl Checker<'_> {
    /// Look for unsequenced modifications in every full expression of the player's functions.
    pub fn unseq_checks(&mut self) {
        let p = self.p;
        let mut found: Vec<(NodeId, String)> = Vec::new();
        for it in p.player_items() {
            if let ItemKind::Func(f) = &it.kind
                && let Some(b) = &f.body
            {
                scan_block(b, &mut found);
            }
        }
        for (id, v) in found {
            self.err(
                id,
                "unseq.modify",
                format!("`{v}` is changed and used again in the same expression with no order between the two: C++ does not say which happens first; split it into two statements"),
            );
        }
    }
}

fn scan_block(b: &Block, found: &mut Vec<(NodeId, String)>) {
    walk_stmts(b, &mut |s| {
        stmt_exprs(s, &mut |e| {
            eff(e, found);
            // lambda bodies are full expressions of their own
            walk_expr(e, &mut |x| {
                if let ExprKind::Lambda(l) = &x.kind {
                    scan_lambda_body(&l.body, found);
                }
            });
        });
    });
}

fn scan_lambda_body(b: &Block, found: &mut Vec<(NodeId, String)>) {
    walk_stmts(b, &mut |s| {
        stmt_exprs(s, &mut |e| {
            eff(e, found);
        });
    });
}
