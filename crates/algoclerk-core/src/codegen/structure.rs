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
}

struct Structurer<'a> {
    instrs: &'a [Instr<usize>],
    n: usize,
    /// Every jump as `(from, to)`.
    jumps: Vec<(usize, usize)>,
}

#[derive(Clone, Copy)]
struct LoopCtx {
    head: usize,
    exit: usize,
}

type Res<T> = Result<T, ()>;

/// Try to structure the whole program. `None` means "use the fallback form".
pub fn structure(program: &Program) -> Option<Vec<Stmt>> {
    let jumps = program
        .instrs
        .iter()
        .enumerate()
        .filter_map(|(i, ins)| ins.target().map(|t| (i, *t)))
        .collect();
    let s = Structurer {
        instrs: &program.instrs,
        n: program.instrs.len(),
        jumps,
    };
    let mut ctx = Vec::new();
    s.region(0, s.n, &mut ctx, None).ok()
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

    /// What a jump to `t` from instruction `i` inside a region ending at `hi` means.
    fn special(&self, t: usize, ctx: &[LoopCtx], i: usize) -> Option<Stmt> {
        if let Some(l) = ctx.last() {
            if t == l.head {
                return Some(Stmt::Continue(i));
            }
            if t == l.exit {
                return Some(Stmt::Break(i));
            }
        }
        (t == self.n).then_some(Stmt::Exit(i))
    }

    fn region(
        &self,
        lo: usize,
        hi: usize,
        ctx: &mut Vec<LoopCtx>,
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
                    if j >= hi {
                        return Err(());
                    }
                    if self.entered_from_outside(i, j + 1) {
                        return Err(());
                    }
                    ctx.push(LoopCtx {
                        head: i,
                        exit: j + 1,
                    });
                    let body = self.region(i, j, ctx, Some(i));
                    ctx.pop();
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
                    } else if let Some(s) = self.special(t, ctx, i) {
                        out.push(s);
                    } else {
                        return Err(());
                    }
                    i += 1;
                }
                Instr::JumpZ(t) | Instr::JumpN(t) => {
                    let t = *t;
                    let cond = cond_of(ins).ok_or(())?;
                    if t == i + 1 {
                        i += 1;
                        continue;
                    }
                    if let Some(s) = self.special(t, ctx, i) {
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
                        let else_end = match self.instrs.get(t - 1) {
                            Some(Instr::Jump(u))
                                if t - 1 > i
                                    && *u > t
                                    && *u <= hi
                                    && self.special(*u, ctx, t - 1).is_none() =>
                            {
                                Some(*u)
                            }
                            _ => None,
                        };
                        if let Some(u) = else_end {
                            if self.entered_from_outside(i, u) {
                                return Err(());
                            }
                            let then = self.region(i + 1, t - 1, ctx, None)?;
                            let els = self.region(t, u, ctx, None)?;
                            out.push(Stmt::If {
                                cond: cond.negate(),
                                then,
                                els,
                                at: i,
                            });
                            i = u;
                        } else {
                            if self.entered_from_outside(i, t) {
                                return Err(());
                            }
                            let then = self.region(i + 1, t, ctx, None)?;
                            out.push(Stmt::If {
                                cond: cond.negate(),
                                then,
                                els: Vec::new(),
                                at: i,
                            });
                            i = t;
                        }
                        continue;
                    }
                    return Err(());
                }
                _ => {
                    out.push(Stmt::Ins(i));
                    i += 1;
                }
            }
        }
        Ok(out)
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
        assert!(matches!(&t[1], Stmt::While { cond: Cond::NotZero, .. }));
    }

    #[test]
    fn if_else_recovered() {
        let t = s("INBOX\nJUMPN neg\nCOPYTO 0\nJUMP end\nneg:\nCOPYTO 1\nend:\nOUTBOX").unwrap();
        assert!(t.iter().any(|x| matches!(x, Stmt::If { els, .. } if !els.is_empty())));
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
