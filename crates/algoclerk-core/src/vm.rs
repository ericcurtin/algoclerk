// SPDX-License-Identifier: Apache-2.0
//! The deterministic, sandboxed virtual machine that runs player programs.
//!
//! There is no I/O, no unsafe code and no unbounded loop: every run has a step limit.
//! The machine can record undo information so front ends can rewind single steps.

use crate::asm::Program;
use crate::i18n::{Diagnostic, english};
use crate::isa::{Addr, Instr, VALUE_LIMIT};
use std::fmt;
use std::sync::Arc;

/// A runtime error ("the clerk got confused").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// An instruction needed a value in the hand but the hand was empty.
    EmptyHand,
    /// An instruction read a floor slot that holds nothing.
    EmptyTile(usize),
    /// The address (possibly computed through a pointer) is not on the floor.
    NoSuchTile(i64),
    /// A value left the range the paper can show.
    Overflow,
    /// Division or remainder by zero.
    DivideByZero,
    /// The program ran for too many steps.
    StepLimit(u64),
}

impl Diagnostic for RuntimeError {
    fn key(&self) -> &'static str {
        match self {
            RuntimeError::EmptyHand => "run.empty_hand",
            RuntimeError::EmptyTile(_) => "run.empty_tile",
            RuntimeError::NoSuchTile(_) => "run.no_such_tile",
            RuntimeError::Overflow => "run.overflow",
            RuntimeError::DivideByZero => "run.divide_by_zero",
            RuntimeError::StepLimit(_) => "run.step_limit",
        }
    }
    fn args(&self) -> Vec<(&'static str, String)> {
        match self {
            RuntimeError::EmptyTile(n) => vec![("slot", n.to_string())],
            RuntimeError::NoSuchTile(n) => vec![("slot", n.to_string())],
            RuntimeError::StepLimit(n) => vec![("limit", n.to_string())],
            _ => vec![],
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(english()))
    }
}

impl std::error::Error for RuntimeError {}

/// Where the machine is in its life.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// More instructions to run.
    Running,
    /// The shift ended normally (inbox exhausted or end of program).
    Finished,
    /// The machine stopped on an error.
    Error(RuntimeError),
}

/// Something visible that happened during a step (drives animations).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// A value moved from the inbox to the hand.
    InboxTake(i32),
    /// The hand's value moved to the outbox.
    OutboxPut(i32),
    /// The hand now holds this.
    HandSet(Option<i32>),
    /// A floor slot was written.
    FloorWrite {
        /// Slot number.
        slot: usize,
        /// New value.
        value: i32,
    },
    /// A floor slot was read (including pointer slots).
    FloorRead(usize),
    /// A jump was taken.
    Jumped(usize),
    /// The inbox was empty, so the shift ended.
    InboxEmpty,
}

/// The result of one `step`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepReport {
    /// Index of the instruction that was (attempted to be) executed.
    pub pc: usize,
    /// What happened.
    pub effects: Vec<Effect>,
    /// Status after the step.
    pub status: Status,
}

#[derive(Clone, Debug)]
struct Undo {
    pc: usize,
    hand: Option<i32>,
    in_pos: usize,
    out_len: usize,
    steps: u64,
    floor: Option<(usize, Option<i32>)>,
}

/// The clerk's office.
#[derive(Clone)]
pub struct Machine {
    program: Arc<Program>,
    inbox: Vec<i32>,
    floor_init: Vec<Option<i32>>,
    /// Number of inbox values taken so far.
    pub in_pos: usize,
    /// Everything put on the outbox so far.
    pub outbox: Vec<i32>,
    /// The floor slots.
    pub floor: Vec<Option<i32>>,
    /// The value in the clerk's hand.
    pub hand: Option<i32>,
    /// Next instruction index.
    pub pc: usize,
    /// Instructions executed so far.
    pub steps: u64,
    /// Current status.
    pub status: Status,
    step_limit: u64,
    history: Option<Vec<Undo>>,
}

impl fmt::Debug for Machine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Machine")
            .field("pc", &self.pc)
            .field("hand", &self.hand)
            .field("steps", &self.steps)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// Maximum number of undo records kept by a machine that records history.
pub const HISTORY_LIMIT: usize = 200_000;

impl Machine {
    /// Create a machine. `floor` gives the initial floor (its length is the floor size).
    pub fn new(
        program: Arc<Program>,
        inbox: Vec<i32>,
        floor: Vec<Option<i32>>,
        step_limit: u64,
    ) -> Machine {
        let status = if program.instrs.is_empty() {
            Status::Finished
        } else {
            Status::Running
        };
        Machine {
            program,
            inbox,
            floor_init: floor.clone(),
            in_pos: 0,
            outbox: Vec::new(),
            floor,
            hand: None,
            pc: 0,
            steps: 0,
            status,
            step_limit,
            history: None,
        }
    }

    /// Record undo information so `step_back` works.
    pub fn with_history(mut self) -> Machine {
        self.history = Some(Vec::new());
        self
    }

    /// The program being run.
    pub fn program(&self) -> &Arc<Program> {
        &self.program
    }

    /// All inbox values (including consumed ones).
    pub fn inbox(&self) -> &[i32] {
        &self.inbox
    }

    /// The initial floor.
    pub fn floor_init(&self) -> &[Option<i32>] {
        &self.floor_init
    }

    /// Values still waiting on the inbox belt.
    pub fn inbox_remaining(&self) -> &[i32] {
        &self.inbox[self.in_pos.min(self.inbox.len())..]
    }

    /// True once the machine stopped (finished or errored).
    pub fn is_done(&self) -> bool {
        self.status != Status::Running
    }

    /// Whether `step_back` has something to undo.
    pub fn can_step_back(&self) -> bool {
        self.history.as_ref().is_some_and(|h| !h.is_empty())
    }

    /// Back to the beginning.
    pub fn reset(&mut self) {
        self.in_pos = 0;
        self.outbox.clear();
        self.floor = self.floor_init.clone();
        self.hand = None;
        self.pc = 0;
        self.steps = 0;
        self.status = if self.program.instrs.is_empty() {
            Status::Finished
        } else {
            Status::Running
        };
        if let Some(h) = &mut self.history {
            h.clear();
        }
    }

    /// Undo the most recent step. Returns false if there was nothing to undo.
    pub fn step_back(&mut self) -> bool {
        let Some(u) = self.history.as_mut().and_then(Vec::pop) else {
            return false;
        };
        self.pc = u.pc;
        self.hand = u.hand;
        self.in_pos = u.in_pos;
        self.outbox.truncate(u.out_len);
        self.steps = u.steps;
        if let Some((slot, old)) = u.floor {
            self.floor[slot] = old;
        }
        self.status = Status::Running;
        true
    }

    fn resolve(&self, a: Addr, fx: &mut Vec<Effect>) -> Result<usize, RuntimeError> {
        match a {
            Addr::Direct(n) => {
                if n < self.floor.len() {
                    Ok(n)
                } else {
                    Err(RuntimeError::NoSuchTile(n as i64))
                }
            }
            Addr::Indirect(n) => {
                let cell = self
                    .floor
                    .get(n)
                    .ok_or(RuntimeError::NoSuchTile(n as i64))?;
                let v = cell.ok_or(RuntimeError::EmptyTile(n))?;
                fx.push(Effect::FloorRead(n));
                usize::try_from(v)
                    .ok()
                    .filter(|p| *p < self.floor.len())
                    .ok_or(RuntimeError::NoSuchTile(i64::from(v)))
            }
        }
    }

    fn tile(&self, slot: usize) -> Result<i32, RuntimeError> {
        self.floor[slot].ok_or(RuntimeError::EmptyTile(slot))
    }

    fn checked(v: i64) -> Result<i32, RuntimeError> {
        if v.abs() > i64::from(VALUE_LIMIT) {
            Err(RuntimeError::Overflow)
        } else {
            Ok(v as i32)
        }
    }

    /// Execute one instruction.
    pub fn step(&mut self) -> StepReport {
        let pc = self.pc;
        let mut fx = Vec::new();
        if self.status != Status::Running {
            return StepReport {
                pc,
                effects: fx,
                status: self.status.clone(),
            };
        }
        if self.steps >= self.step_limit {
            self.status = Status::Error(RuntimeError::StepLimit(self.step_limit));
            return StepReport {
                pc,
                effects: fx,
                status: self.status.clone(),
            };
        }
        let before = Undo {
            pc,
            hand: self.hand,
            in_pos: self.in_pos,
            out_len: self.outbox.len(),
            steps: self.steps,
            floor: None,
        };
        let mut undo = before;
        match self.exec(pc, &mut fx, &mut undo) {
            Ok(()) => {
                self.steps += 1;
                if self.status == Status::Running && self.pc >= self.program.instrs.len() {
                    self.status = Status::Finished;
                }
                if let Some(h) = &mut self.history {
                    if h.len() >= HISTORY_LIMIT {
                        h.remove(0);
                    }
                    h.push(undo);
                }
            }
            Err(e) => {
                // Nothing was modified before an error is raised.
                self.status = Status::Error(e);
            }
        }
        StepReport {
            pc,
            effects: fx,
            status: self.status.clone(),
        }
    }

    fn write_floor(&mut self, slot: usize, v: i32, fx: &mut Vec<Effect>, undo: &mut Undo) {
        if undo.floor.is_none() {
            undo.floor = Some((slot, self.floor[slot]));
        }
        self.floor[slot] = Some(v);
        fx.push(Effect::FloorWrite { slot, value: v });
    }

    fn exec(
        &mut self,
        pc: usize,
        fx: &mut Vec<Effect>,
        undo: &mut Undo,
    ) -> Result<(), RuntimeError> {
        let instr = self.program.instrs[pc].clone();
        let mut next = pc + 1;
        match instr {
            Instr::Inbox => {
                if self.in_pos >= self.inbox.len() {
                    fx.push(Effect::InboxEmpty);
                    self.status = Status::Finished;
                } else {
                    let v = self.inbox[self.in_pos];
                    self.in_pos += 1;
                    self.hand = Some(v);
                    fx.push(Effect::InboxTake(v));
                }
            }
            Instr::Outbox => {
                let v = self.hand.ok_or(RuntimeError::EmptyHand)?;
                self.outbox.push(v);
                self.hand = None;
                fx.push(Effect::OutboxPut(v));
                fx.push(Effect::HandSet(None));
            }
            Instr::CopyTo(a) => {
                let v = self.hand.ok_or(RuntimeError::EmptyHand)?;
                let s = self.resolve(a, fx)?;
                self.write_floor(s, v, fx, undo);
            }
            Instr::CopyFrom(a) => {
                let s = self.resolve(a, fx)?;
                let v = self.tile(s)?;
                fx.push(Effect::FloorRead(s));
                self.hand = Some(v);
                fx.push(Effect::HandSet(Some(v)));
            }
            Instr::Add(a) | Instr::Sub(a) | Instr::Mul(a) | Instr::Div(a) | Instr::Mod(a) => {
                let h = self.hand.ok_or(RuntimeError::EmptyHand)?;
                let s = self.resolve(a, fx)?;
                let t = self.tile(s)?;
                fx.push(Effect::FloorRead(s));
                let (h, t) = (i64::from(h), i64::from(t));
                let r = match instr {
                    Instr::Add(_) => h + t,
                    Instr::Sub(_) => h - t,
                    Instr::Mul(_) => h * t,
                    Instr::Div(_) => {
                        if t == 0 {
                            return Err(RuntimeError::DivideByZero);
                        }
                        h / t
                    }
                    _ => {
                        if t == 0 {
                            return Err(RuntimeError::DivideByZero);
                        }
                        h % t
                    }
                };
                let r = Self::checked(r)?;
                self.hand = Some(r);
                fx.push(Effect::HandSet(Some(r)));
            }
            Instr::BumpUp(a) | Instr::BumpDn(a) => {
                let s = self.resolve(a, fx)?;
                let t = self.tile(s)?;
                let r = Self::checked(
                    i64::from(t)
                        + if matches!(instr, Instr::BumpUp(_)) {
                            1
                        } else {
                            -1
                        },
                )?;
                self.write_floor(s, r, fx, undo);
                self.hand = Some(r);
                fx.push(Effect::HandSet(Some(r)));
            }
            Instr::Jump(t) => {
                next = t;
                fx.push(Effect::Jumped(t));
            }
            Instr::JumpZ(t) => {
                let h = self.hand.ok_or(RuntimeError::EmptyHand)?;
                if h == 0 {
                    next = t;
                    fx.push(Effect::Jumped(t));
                }
            }
            Instr::JumpN(t) => {
                let h = self.hand.ok_or(RuntimeError::EmptyHand)?;
                if h < 0 {
                    next = t;
                    fx.push(Effect::Jumped(t));
                }
            }
        }
        self.pc = next;
        Ok(())
    }

    /// Run until the machine stops. The step limit guarantees termination.
    pub fn run(&mut self) -> &Status {
        while self.status == Status::Running {
            self.step();
        }
        &self.status
    }
}

/// Result of a complete run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunResult {
    /// The outbox contents.
    pub outbox: Vec<i32>,
    /// Instructions executed.
    pub steps: u64,
    /// How the run ended.
    pub status: Status,
}

/// Run a program to completion (or error / step limit).
pub fn run_program(
    program: &Arc<Program>,
    inbox: &[i32],
    floor: &[Option<i32>],
    step_limit: u64,
) -> RunResult {
    let mut m = Machine::new(program.clone(), inbox.to_vec(), floor.to_vec(), step_limit);
    m.run();
    RunResult {
        outbox: m.outbox,
        steps: m.steps,
        status: m.status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;

    fn run(src: &str, inbox: &[i32], floor: &[Option<i32>]) -> RunResult {
        run_program(&Arc::new(assemble(src).unwrap()), inbox, floor, 10_000)
    }

    #[test]
    fn echo() {
        let r = run("a:\nINBOX\nOUTBOX\nJUMP a", &[1, 2, 3], &[]);
        assert_eq!(r.outbox, vec![1, 2, 3]);
        assert_eq!(r.status, Status::Finished);
        assert_eq!(r.steps, 10);
    }

    #[test]
    fn empty_hand_errors() {
        let r = run("OUTBOX", &[], &[]);
        assert_eq!(r.status, Status::Error(RuntimeError::EmptyHand));
        assert_eq!(r.status, Status::Error(RuntimeError::EmptyHand));
        assert!(r.outbox.is_empty());
    }

    #[test]
    fn arithmetic() {
        let f = [Some(7), Some(3), Some(0)];
        let r = run(
            "INBOX\nADD 0\nOUTBOX\nINBOX\nSUB 1\nOUTBOX\nINBOX\nMUL 1\nOUTBOX\nINBOX\nDIV 1\nOUTBOX\nINBOX\nMOD 1\nOUTBOX",
            &[10, 10, 10, -10, -10],
            &f,
        );
        assert_eq!(r.outbox, vec![17, 7, 30, -3, -1]);
    }

    #[test]
    fn bump_updates_slot_and_hand() {
        let r = run(
            "BUMPUP 0\nOUTBOX\nBUMPDN 0\nBUMPDN 0\nOUTBOX",
            &[],
            &[Some(5)],
        );
        assert_eq!(r.outbox, vec![6, 4]);
    }

    #[test]
    fn indirect() {
        let f = [Some(2), None, Some(42)];
        let r = run(
            "COPYFROM [0]\nOUTBOX\nINBOX\nCOPYTO [0]\nCOPYFROM 2\nOUTBOX",
            &[9],
            &f,
        );
        assert_eq!(r.outbox, vec![42, 9]);
        let bad = run("COPYFROM [1]", &[], &f);
        assert_eq!(bad.status, Status::Error(RuntimeError::EmptyTile(1)));
        let bad = run("COPYFROM [0]", &[], &[Some(9), None]);
        assert_eq!(bad.status, Status::Error(RuntimeError::NoSuchTile(9)));
        let bad = run("COPYFROM [0]", &[], &[Some(-1), None]);
        assert_eq!(bad.status, Status::Error(RuntimeError::NoSuchTile(-1)));
    }

    #[test]
    fn errors() {
        let f = [Some(0), None];
        assert_eq!(
            run("INBOX\nDIV 0", &[1], &f).status,
            Status::Error(RuntimeError::DivideByZero)
        );
        assert_eq!(
            run("INBOX\nMOD 0", &[1], &f).status,
            Status::Error(RuntimeError::DivideByZero)
        );
        assert_eq!(
            run("INBOX\nADD 1", &[1], &f).status,
            Status::Error(RuntimeError::EmptyTile(1))
        );
        assert_eq!(
            run("INBOX\nADD 5", &[1], &f).status,
            Status::Error(RuntimeError::NoSuchTile(5))
        );
        let big = [Some(VALUE_LIMIT)];
        assert_eq!(
            run("INBOX\nADD 0", &[1], &big).status,
            Status::Error(RuntimeError::Overflow)
        );
        assert_eq!(
            run("COPYFROM 0\nMUL 0", &[], &big).status,
            Status::Error(RuntimeError::Overflow)
        );
    }

    #[test]
    fn jumps() {
        let r = run(
            "INBOX\nJUMPN neg\nOUTBOX\nJUMP end\nneg:\nBUMPUP 0\nOUTBOX\nend:",
            &[-5],
            &[Some(100)],
        );
        assert_eq!(r.outbox, vec![101]);
        let r = run("INBOX\nJUMPZ z\nBUMPUP 0\nz:\nOUTBOX", &[0], &[Some(100)]);
        assert_eq!(r.outbox, vec![0]);
    }

    #[test]
    fn step_limit() {
        let r = run_program(&Arc::new(assemble("a:\nJUMP a").unwrap()), &[], &[], 50);
        assert_eq!(r.status, Status::Error(RuntimeError::StepLimit(50)));
        assert_eq!(r.steps, 50);
    }

    #[test]
    fn empty_program_finishes() {
        let r = run("", &[1], &[]);
        assert_eq!(r.status, Status::Finished);
        assert_eq!(r.steps, 0);
    }

    #[test]
    fn rewind_restores_everything() {
        let p = Arc::new(assemble("a:\nINBOX\nCOPYTO 0\nBUMPUP 0\nOUTBOX\nJUMP a").unwrap());
        let mut m = Machine::new(p, vec![5, 6], vec![None], 1000).with_history();
        let mut snaps = vec![(
            m.hand,
            m.pc,
            m.floor.clone(),
            m.outbox.clone(),
            m.in_pos,
            m.steps,
        )];
        for _ in 0..7 {
            m.step();
            snaps.push((
                m.hand,
                m.pc,
                m.floor.clone(),
                m.outbox.clone(),
                m.in_pos,
                m.steps,
            ));
        }
        while snaps.len() > 1 {
            snaps.pop();
            assert!(m.step_back());
            let s = snaps.last().unwrap();
            assert_eq!(
                (
                    m.hand,
                    m.pc,
                    m.floor.clone(),
                    m.outbox.clone(),
                    m.in_pos,
                    m.steps
                ),
                s.clone()
            );
        }
        assert!(!m.step_back());
    }

    #[test]
    fn rewind_after_finish_and_error() {
        let p = Arc::new(assemble("INBOX\nOUTBOX\nOUTBOX").unwrap());
        let mut m = Machine::new(p, vec![1], vec![], 100).with_history();
        m.run();
        assert!(matches!(m.status, Status::Error(RuntimeError::EmptyHand)));
        assert!(m.step_back());
        assert_eq!(m.status, Status::Running);
        assert_eq!(m.pc, 1);
        m.reset();
        assert_eq!(m.pc, 0);
        assert!(!m.can_step_back());
    }

    #[test]
    fn effects_reported() {
        let p = Arc::new(assemble("INBOX\nCOPYTO 0\nOUTBOX").unwrap());
        let mut m = Machine::new(p, vec![3], vec![None], 100);
        assert_eq!(m.step().effects, vec![Effect::InboxTake(3)]);
        assert_eq!(
            m.step().effects,
            vec![Effect::FloorWrite { slot: 0, value: 3 }]
        );
        assert!(m.step().effects.contains(&Effect::OutboxPut(3)));
        assert_eq!(m.status, Status::Finished);
    }
}
