// SPDX-License-Identifier: Apache-2.0
//! Control-flow structuring: recover `while` / `if` / `break` / `continue` from jump patterns.
//!
//! The algorithm works directly on the instruction array. Every jump in the program is
//! classified exactly once while building the tree; if any jump does not fit a supported
//! pattern the whole function reports failure and the renderer falls back to a goto /
//! state-machine form that is correct for every program.
#![allow(missing_docs)]

use crate::asm::Program;
use crate::isa::Instr;

/// A test on the hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    /// `hand == 0`
    Zero,
    /// `hand != 0`
    NotZero,
    /// `hand < 0`
    Neg,
    /// `hand >= 0`
    NotNeg,
}

impl Cond {
    /// The opposite condition.
    pub fn negate(self) -> Cond {
        match self {
            Cond::Zero => Cond::NotZero,
            Cond::NotZero => Cond::Zero,
            Cond::Neg => Cond::NotNeg,
            Cond::NotNeg => Cond::Neg,
        }
    }
}

/// A structured statement. `at` fields are instruction indexes used for comments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    /// A simple (non-jump) instruction.
    Ins(usize),
    /// `if cond { then } else { els }`; `at` is the jump instruction.
    If {
        cond: Cond,
        then: Vec<Stmt>,
        els: Vec<Stmt>,
        at: usize,
    },
    /// `while true { body }`; `head` is the first instruction of the loop.
    Loop { body: Vec<Stmt>, head: usize },
    /// `while cond { body }`
    While {
        cond: Cond,
        body: Vec<Stmt>,
        head: usize,
    },
    /// `do { body } while cond`
    DoWhile {
        body: Vec<Stmt>,
        cond: Cond,
        head: usize,
        at: usize,
    },
    /// Leave the innermost loop.
    Break(usize),
    /// Restart the innermost loop.
    Continue(usize),
    /// End the program (return).
    Exit(usize),
    /// A block that `BreakTo` can leave; code after the block starts at instruction `target`.
    Block { target: usize, body: Vec<Stmt> },
    /// Leave the enclosing `Block` whose target is `target` (a forward jump).
    BreakTo { target: usize, at: usize },
}

struct Structurer<'a> {
    instrs: &'a [Instr<usize>],
    n: usize,
    /// Every jump as `(from, to)`.
    jumps: Vec<(usize, usize)>,
    /// Forward-jump targets that are implemented with a labelled block.
    wanted: Vec<usize>,
}

#[derive(Clone, Copy)]
struct LoopCtx {
    head: usize,
    exit: usize,
}

#[derive(Default)]
struct Ctx {
    loops: Vec<LoopCtx>,
    /// Targets of the blocks we are currently inside.
    active: Vec<usize>,
}

/// Why structuring failed.
#[derive(Clone, Copy, Debug)]
enum Fail {
    /// A forward jump to this target would work if a labelled block ended there.
    Fix(usize),
    /// Give up: use the fallback form.
    No,
}

type Res<T> = Result<T, Fail>;

/// Try to structure the whole program. `None` means "use the fallback form".
pub fn structure(program: &Program) -> Option<Vec<Stmt>> {
    let jumps: Vec<(usize, usize)> = program
        .instrs
        .iter()
        .enumerate()
        .filter_map(|(i, ins)| ins.target().map(|t| (i, *t)))
        .collect();
    let mut wanted: Vec<usize> = Vec::new();
    loop {
        let s = Structurer {
            instrs: &program.instrs,
            n: program.instrs.len(),
            jumps: jumps.clone(),
            wanted: wanted.clone(),
        };
        let mut ctx = Ctx::default();
        match s.region(0, s.n, &mut ctx, None) {
            Ok(t) => return Some(t),
            Err(Fail::Fix(t)) if !wanted.contains(&t) && wanted.len() < 16 => wanted.push(t),
            Err(_) => return None,
        }
    }
}

fn cond_of(ins: &Instr<usize>) -> Option<Cond> {
    match ins {
        Instr::JumpZ(_) => Some(Cond::Zero),
        Instr::JumpN(_) => Some(Cond::Neg),
        _ => None,
    }
}

fn contains_continue(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Continue(_) => true,
        Stmt::If { then, els, .. } => contains_continue(then) || contains_continue(els),
        Stmt::Block { body, .. } => contains_continue(body),
        // Nested loops own their `continue`s.
        _ => false,
    })
}

impl Structurer<'_> {
    /// Jumps from outside `[s, e)` that land strictly inside `(s, e)`.
    fn entered_from_outside(&self, s: usize, e: usize) -> bool {
        self.jumps
            .iter()
            .any(|&(k, t)| !(s..e).contains(&k) && t > s && t < e)
    }

    /// What a jump to `t` from instruction `i` means when it leaves the current construct.
    fn special(&self, t: usize, ctx: &Ctx, i: usize, blocks: bool) -> Option<Stmt> {
        if let Some(l) = ctx.loops.last() {
            if t == l.head {
                return Some(Stmt::Continue(i));
            }
            if t == l.exit {
                return Some(Stmt::Break(i));
            }
        }
        if blocks && ctx.active.contains(&t) {
            return Some(Stmt::BreakTo { target: t, at: i });
        }
        (t == self.n).then_some(Stmt::Exit(i))
    }

    fn region(
        &self,
        lo: usize,
        hi: usize,
        ctx: &mut Ctx,
        no_loop_at: Option<usize>,
    ) -> Res<Vec<Stmt>> {
        // Labelled blocks requested for forward jumps: the outermost (largest target) first.
        let mut cands: Vec<usize> = self
            .wanted
            .iter()
            .copied()
            .filter(|t| *t > lo && *t <= hi && !ctx.active.contains(t))
            .collect();
        cands.sort_unstable_by(|a, b| b.cmp(a));
        for t in cands {
            ctx.active.push(t);
            let first = self.region(lo, t, ctx, no_loop_at);
            ctx.active.pop();
            if let Ok(first) = first
                && let Ok(rest) = self.region(t, hi, ctx, None)
            {
                let mut out = vec![Stmt::Block {
                    target: t,
                    body: first,
                }];
                out.extend(rest);
                return Ok(out);
            }
        }
        self.region_plain(lo, hi, ctx, no_loop_at)
    }

    fn region_plain(
        &self,
        lo: usize,
        hi: usize,
        ctx: &mut Ctx,
        no_loop_at: Option<usize>,
    ) -> Res<Vec<Stmt>> {
        let mut out = Vec::new();
        let mut i = lo;
        while i < hi {
            // Loop head?
            if no_loop_at != Some(i) || i != lo {
                let back = self
                    .jumps
                    .iter()
                    .filter(|&&(k, t)| t == i && k >= i)
                    .map(|&(k, _)| k)
                    .max();
                if let Some(j) = back {
                    if j >= hi || self.entered_from_outside(i, j + 1) {
                        return Err(Fail::No);
                    }
                    ctx.loops.push(LoopCtx {
                        head: i,
                        exit: j + 1,
                    });
                    let body = self.region(i, j, ctx, Some(i));
                    ctx.loops.pop();
                    let body = body?;
                    out.push(self.make_loop(i, j, body));
                    i = j + 1;
                    continue;
                }
            }
            let ins = &self.instrs[i];
            match ins {
                Instr::Jump(t) => {
                    let t = *t;
                    if t == i + 1 || (t == hi && i + 1 == hi) {
                        // Jump to the next instruction: nothing to do.
                    } else if let Some(s) = self.special(t, ctx, i, true) {
                        out.push(s);
                    } else if t > i {
                        return Err(Fail::Fix(t));
                    } else {
                        return Err(Fail::No);
                    }
                    i += 1;
                }
                Instr::JumpZ(t) | Instr::JumpN(t) => {
                    let t = *t;
                    let cond = cond_of(ins).ok_or(Fail::No)?;
                    if t == i + 1 {
                        i += 1;
                        continue;
                    }
                    if let Some(s) = self.special(t, ctx, i, false) {
                        out.push(Stmt::If {
                            cond,
                            then: vec![s],
                            els: Vec::new(),
                            at: i,
                        });
                        i += 1;
                        continue;
                    }
                    if t > i && t <= hi {
                        // if-then, possibly with else.
                        if let Some((stmt, next)) = self.if_construct(i, t, hi, cond, ctx)? {
                            out.push(stmt);
                            i = next;
                            continue;
                        }
                    }
                    if let Some(s) = self.special(t, ctx, i, true) {
                        out.push(Stmt::If {
                            cond,
                            then: vec![s],
                            els: Vec::new(),
                            at: i,
                        });
                        i += 1;
                        continue;
                    }
                    return Err(if t > i { Fail::Fix(t) } else { Fail::No });
                }
                _ => {
                    out.push(Stmt::Ins(i));
                    i += 1;
                }
            }
        }
        Ok(out)
    }

    /// `if` / `if-else` for a forward conditional jump at `i` to `t <= hi`.
    /// `Ok(None)` means the pattern does not apply (something jumps into the middle).
    fn if_construct(
        &self,
        i: usize,
        t: usize,
        hi: usize,
        cond: Cond,
        ctx: &mut Ctx,
    ) -> Res<Option<(Stmt, usize)>> {
        let else_end = match self.instrs.get(t - 1) {
            Some(Instr::Jump(u))
                if t - 1 > i
                    && *u > t
                    && *u <= hi
                    && self.special(*u, ctx, t - 1, true).is_none() =>
            {
                Some(*u)
            }
            _ => None,
        };
        if let Some(u) = else_end {
            if self.entered_from_outside(i, u) {
                return Ok(None);
            }
            let then = self.region(i + 1, t - 1, ctx, None)?;
            let els = self.region(t, u, ctx, None)?;
            let stmt = Stmt::If {
                cond: cond.negate(),
                then,
                els,
                at: i,
            };
            return Ok(Some((stmt, u)));
        }
        if self.entered_from_outside(i, t) {
            return Ok(None);
        }
        let then = self.region(i + 1, t, ctx, None)?;
        let stmt = Stmt::If {
            cond: cond.negate(),
            then,
            els: Vec::new(),
            at: i,
        };
        Ok(Some((stmt, t)))
    }

    fn make_loop(&self, head: usize, j: usize, mut body: Vec<Stmt>) -> Stmt {
        match &self.instrs[j] {
            Instr::Jump(_) => {
                // while (cond) { ... } when the loop starts with `if cond { break }`.
                if let Some(Stmt::If {
                    cond,
                    then,
                    els,
                    at: _,
                }) = body.first()
                    && els.is_empty()
                    && matches!(then.as_slice(), [Stmt::Break(_)])
                {
                    let cond = cond.negate();
                    body.remove(0);
                    return Stmt::While { cond, body, head };
                }
                Stmt::Loop { body, head }
            }
            ins => {
                let cond = cond_of(ins).unwrap_or(Cond::Zero);
                if contains_continue(&body) {
                    // do-while would turn `continue` into "test the condition"; use an explicit break.
                    body.push(Stmt::If {
                        cond: cond.negate(),
                        then: vec![Stmt::Break(j)],
                        els: Vec::new(),
                        at: j,
                    });
                    Stmt::Loop { body, head }
                } else {
                    Stmt::DoWhile {
                        body,
                        cond,
                        head,
                        at: j,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;

    fn s(src: &str) -> Option<Vec<Stmt>> {
        structure(&assemble(src).unwrap())
    }

    #[test]
    fn echo_loop() {
        let t = s("a:\nINBOX\nOUTBOX\nJUMP a").unwrap();
        assert!(matches!(&t[..], [Stmt::Loop { .. }]));
    }

    #[test]
    fn while_loop_recovered() {
        let t = s("INBOX\nl:\nJUMPZ e\nOUTBOX\nINBOX\nJUMP l\ne:").unwrap();
        assert!(matches!(
            &t[1],
            Stmt::While {
                cond: Cond::NotZero,
                ..
            }
        ));
    }

    #[test]
    fn if_else_recovered() {
        let t = s("INBOX\nJUMPN neg\nCOPYTO 0\nJUMP end\nneg:\nCOPYTO 1\nend:\nOUTBOX").unwrap();
        assert!(
            t.iter()
                .any(|x| matches!(x, Stmt::If { els, .. } if !els.is_empty()))
        );
    }

    #[test]
    fn do_while_recovered() {
        let t = s("a:\nBUMPDN 0\nJUMPZ a").unwrap();
        assert!(matches!(&t[..], [Stmt::DoWhile { .. }]));
    }

    #[test]
    fn irreducible_falls_back() {
        // Two entries into one loop.
        assert!(s("INBOX\nJUMPZ b\na:\nBUMPUP 0\nb:\nBUMPDN 0\nJUMPN a").is_none());
        // Jump out of two loops at once.
        assert!(s("a:\nINBOX\nb:\nJUMPZ end\nJUMPN a\nJUMP b\nend:").is_none());
    }

    #[test]
    fn continue_inside_do_while_uses_loop() {
        let t = s("a:\nINBOX\nJUMPZ a\nBUMPDN 0\nJUMPN a").unwrap();
        assert!(matches!(&t[..], [Stmt::Loop { .. }]));
    }
}
