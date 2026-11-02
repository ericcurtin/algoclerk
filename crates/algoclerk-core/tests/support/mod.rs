// SPDX-License-Identifier: Apache-2.0
//! Random program strategies shared by the integration tests.

#![allow(dead_code)]

use algoclerk_core::asm::Program;
use algoclerk_core::isa::{Addr, Instr};
use proptest::prelude::*;

/// Floor size used by random programs.
pub const FLOOR: usize = 6;

fn addr(indirect: bool) -> impl Strategy<Value = Addr> {
    (
        0..FLOOR,
        prop::bool::weighted(if indirect { 0.15 } else { 0.0 }),
    )
        .prop_map(|(n, ind)| {
            if ind {
                Addr::Indirect(n)
            } else {
                Addr::Direct(n)
            }
        })
}

/// A hand-preserving, non-jump operation.
fn op() -> impl Strategy<Value = Instr<usize>> {
    prop_oneof![
        4 => addr(true).prop_map(Instr::CopyTo),
        4 => addr(true).prop_map(Instr::CopyFrom),
        3 => addr(true).prop_map(Instr::Add),
        3 => addr(true).prop_map(Instr::Sub),
        2 => addr(false).prop_map(Instr::BumpUp),
        2 => addr(false).prop_map(Instr::BumpDn),
        1 => addr(false).prop_map(Instr::Mul),
        1 => addr(false).prop_map(Instr::Div),
        1 => addr(false).prop_map(Instr::Mod),
    ]
}

/// Structured building blocks.
#[derive(Clone, Debug)]
pub enum Node {
    Op(Instr<usize>),
    /// `OUTBOX; INBOX` (keeps the hand full).
    Emit,
    IfZ(Vec<Node>, Vec<Node>),
    IfNeg(Vec<Node>, Vec<Node>),
    /// `if hand != 0 { .. }` shaped as `JUMPZ skip`.
    Guard(Vec<Node>),
    /// loop { pre; if hand == 0 break; post }
    LoopZ(Vec<Node>, Vec<Node>),
    /// do { body } while (hand < 0)
    DoNeg(Vec<Node>),
    /// loop { pre; JUMPN exit; post; JUMP loop } with a `continue` in the middle sometimes.
    LoopNeg(Vec<Node>, Vec<Node>),
}

fn nodes(depth: u32) -> BoxedStrategy<Vec<Node>> {
    let leaf = prop_oneof![8 => op().prop_map(Node::Op), 3 => Just(Node::Emit)];
    let node: BoxedStrategy<Node> = if depth == 0 {
        leaf.boxed()
    } else {
        let inner = nodes(depth - 1);
        prop_oneof![
            6 => leaf,
            1 => (inner.clone(), inner.clone()).prop_map(|(a, b)| Node::IfZ(a, b)),
            1 => (inner.clone(), inner.clone()).prop_map(|(a, b)| Node::IfNeg(a, b)),
            1 => inner.clone().prop_map(Node::Guard),
            1 => (inner.clone(), inner.clone()).prop_map(|(a, b)| Node::LoopZ(a, b)),
            1 => inner.clone().prop_map(Node::DoNeg),
            1 => (inner.clone(), inner).prop_map(|(a, b)| Node::LoopNeg(a, b)),
        ]
        .boxed()
    };
    prop::collection::vec(node, 0..4).boxed()
}

const PLACEHOLDER: usize = usize::MAX;

fn emit(ns: &[Node], out: &mut Vec<Instr<usize>>) {
    for n in ns {
        match n {
            Node::Op(i) => out.push(i.clone()),
            Node::Emit => {
                out.push(Instr::Outbox);
                out.push(Instr::Inbox);
            }
            Node::IfZ(a, b) | Node::IfNeg(a, b) => {
                let jump_at = out.len();
                out.push(Instr::Jump(PLACEHOLDER));
                emit(a, out);
                let skip_at = out.len();
                out.push(Instr::Jump(PLACEHOLDER));
                let else_start = out.len();
                emit(b, out);
                let end = out.len();
                out[jump_at] = if matches!(n, Node::IfZ(..)) {
                    Instr::JumpZ(else_start)
                } else {
                    Instr::JumpN(else_start)
                };
                out[skip_at] = Instr::Jump(end);
            }
            Node::Guard(a) => {
                let at = out.len();
                out.push(Instr::Jump(PLACEHOLDER));
                emit(a, out);
                let end = out.len();
                out[at] = Instr::JumpZ(end);
            }
            Node::LoopZ(pre, post) | Node::LoopNeg(pre, post) => {
                let head = out.len();
                emit(pre, out);
                let exit_at = out.len();
                out.push(Instr::Jump(PLACEHOLDER));
                emit(post, out);
                out.push(Instr::Jump(head));
                let end = out.len();
                out[exit_at] = if matches!(n, Node::LoopZ(..)) {
                    Instr::JumpZ(end)
                } else {
                    Instr::JumpN(end)
                };
            }
            Node::DoNeg(body) => {
                let head = out.len();
                emit(body, out);
                out.push(Instr::JumpN(head));
            }
        }
    }
}

/// A program that is well structured by construction (always starts with INBOX).
pub fn structured_program() -> impl Strategy<Value = Program> {
    nodes(3).prop_map(|ns| {
        let mut v = vec![Instr::Inbox];
        emit(&ns, &mut v);
        Program::from_instrs(v).expect("targets in range")
    })
}

/// A program with arbitrary jumps (often irreducible).
pub fn wild_program() -> impl Strategy<Value = Program> {
    (4usize..14)
        .prop_flat_map(|n| {
            let unit = prop_oneof![
                10 => op().prop_map(|i| vec![i]),
                2 => Just(vec![Instr::Inbox]),
                5 => Just(vec![Instr::Outbox, Instr::Inbox]),
                2 => (0..=n + 4).prop_map(|t| vec![Instr::Jump(t)]),
                2 => (0..=n + 4).prop_map(|t| vec![Instr::JumpZ(t)]),
                2 => (0..=n + 4).prop_map(|t| vec![Instr::JumpN(t)]),
            ];
            prop::collection::vec(unit, n)
        })
        .prop_map(|units| {
            let mut v = vec![Instr::Inbox];
            v.extend(units.into_iter().flatten());
            let n = v.len();
            for i in &mut v {
                if let Instr::Jump(t) | Instr::JumpZ(t) | Instr::JumpN(t) = i {
                    *t = (*t).min(n);
                }
            }
            Program::from_instrs(v).expect("targets in range")
        })
}

/// Random inbox contents.
pub fn inbox() -> impl Strategy<Value = Vec<i32>> {
    prop::collection::vec(-5i32..=5, 0..7)
}

/// The initial floor for random programs: every tile holds a small valid address.
pub fn floor() -> Vec<Option<i32>> {
    (0..FLOOR).map(|i| Some(((i + 1) % FLOOR) as i32)).collect()
}
