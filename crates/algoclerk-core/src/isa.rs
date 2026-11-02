// SPDX-License-Identifier: Apache-2.0
//! The AlgoClerk instruction set.
//!
//! See `docs/ISA.md` for the human readable specification.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Largest absolute value the clerk can hold (six digits fit on the paper).
pub const VALUE_LIMIT: i32 = 999_999;

/// Largest floor slot index accepted by the assembler.
pub const MAX_SLOT: usize = 4095;

/// Operand addressing mode for instructions that touch the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Addr {
    /// `n`: the floor slot with number `n`.
    Direct(usize),
    /// `[n]`: the floor slot whose number is stored in slot `n`.
    Indirect(usize),
}

impl Addr {
    /// The slot number written in the source (the pointer slot for indirect).
    pub fn slot(self) -> usize {
        match self {
            Addr::Direct(n) | Addr::Indirect(n) => n,
        }
    }

    /// True for `[n]`.
    pub fn is_indirect(self) -> bool {
        matches!(self, Addr::Indirect(_))
    }
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Addr::Direct(n) => write!(f, "{n}"),
            Addr::Indirect(n) => write!(f, "[{n}]"),
        }
    }
}

/// A machine instruction. `L` is the jump target type: a label name in
/// source listings, an instruction index in compiled programs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Instr<L> {
    /// Take the next value from the inbox into the hand (ends the shift if the inbox is empty).
    Inbox,
    /// Put the hand on the outbox; the hand becomes empty.
    Outbox,
    /// Copy the hand onto a floor slot.
    CopyTo(Addr),
    /// Copy a floor slot into the hand.
    CopyFrom(Addr),
    /// hand = hand + slot.
    Add(Addr),
    /// hand = hand - slot.
    Sub(Addr),
    /// slot = slot + 1, hand = slot.
    BumpUp(Addr),
    /// slot = slot - 1, hand = slot.
    BumpDn(Addr),
    /// hand = hand * slot.
    Mul(Addr),
    /// hand = hand / slot (truncating toward zero).
    Div(Addr),
    /// hand = hand % slot (remainder, sign follows the hand).
    Mod(Addr),
    /// Continue at the target.
    Jump(L),
    /// Continue at the target if the hand is zero.
    JumpZ(L),
    /// Continue at the target if the hand is negative.
    JumpN(L),
}

/// Operand shape of an opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandKind {
    /// No operand.
    None,
    /// A floor address (`n` or `[n]`).
    Address,
    /// A label.
    Label,
}

/// Opcode without operands, used for level instruction sets and parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Opcode {
    /// `INBOX`
    Inbox,
    /// `OUTBOX`
    Outbox,
    /// `COPYTO`
    CopyTo,
    /// `COPYFROM`
    CopyFrom,
    /// `ADD`
    Add,
    /// `SUB`
    Sub,
    /// `BUMPUP`
    BumpUp,
    /// `BUMPDN`
    BumpDn,
    /// `MUL`
    Mul,
    /// `DIV`
    Div,
    /// `MOD`
    Mod,
    /// `JUMP`
    Jump,
    /// `JUMPZ`
    JumpZ,
    /// `JUMPN`
    JumpN,
}

impl Opcode {
    /// Every opcode in documentation order.
    pub const ALL: [Opcode; 14] = [
        Opcode::Inbox,
        Opcode::Outbox,
        Opcode::CopyTo,
        Opcode::CopyFrom,
        Opcode::Add,
        Opcode::Sub,
        Opcode::BumpUp,
        Opcode::BumpDn,
        Opcode::Mul,
        Opcode::Div,
        Opcode::Mod,
        Opcode::Jump,
        Opcode::JumpZ,
        Opcode::JumpN,
    ];

    /// The upper-case assembly mnemonic.
    pub fn mnemonic(self) -> &'static str {
        match self {
            Opcode::Inbox => "INBOX",
            Opcode::Outbox => "OUTBOX",
            Opcode::CopyTo => "COPYTO",
            Opcode::CopyFrom => "COPYFROM",
            Opcode::Add => "ADD",
            Opcode::Sub => "SUB",
            Opcode::BumpUp => "BUMPUP",
            Opcode::BumpDn => "BUMPDN",
            Opcode::Mul => "MUL",
            Opcode::Div => "DIV",
            Opcode::Mod => "MOD",
            Opcode::Jump => "JUMP",
            Opcode::JumpZ => "JUMPZ",
            Opcode::JumpN => "JUMPN",
        }
    }

    /// Parse a mnemonic, case-insensitively.
    pub fn from_mnemonic(s: &str) -> Option<Opcode> {
        Opcode::ALL
            .into_iter()
            .find(|o| o.mnemonic().eq_ignore_ascii_case(s))
    }

    /// Shape of the operand.
    pub fn operand_kind(self) -> OperandKind {
        match self {
            Opcode::Inbox | Opcode::Outbox => OperandKind::None,
            Opcode::Jump | Opcode::JumpZ | Opcode::JumpN => OperandKind::Label,
            _ => OperandKind::Address,
        }
    }

    /// Stable lower-case key used in level files and translations.
    pub fn key(self) -> &'static str {
        match self {
            Opcode::Inbox => "inbox",
            Opcode::Outbox => "outbox",
            Opcode::CopyTo => "copyto",
            Opcode::CopyFrom => "copyfrom",
            Opcode::Add => "add",
            Opcode::Sub => "sub",
            Opcode::BumpUp => "bumpup",
            Opcode::BumpDn => "bumpdn",
            Opcode::Mul => "mul",
            Opcode::Div => "div",
            Opcode::Mod => "mod",
            Opcode::Jump => "jump",
            Opcode::JumpZ => "jumpz",
            Opcode::JumpN => "jumpn",
        }
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mnemonic())
    }
}

impl<L> Instr<L> {
    /// The opcode of this instruction.
    pub fn opcode(&self) -> Opcode {
        match self {
            Instr::Inbox => Opcode::Inbox,
            Instr::Outbox => Opcode::Outbox,
            Instr::CopyTo(_) => Opcode::CopyTo,
            Instr::CopyFrom(_) => Opcode::CopyFrom,
            Instr::Add(_) => Opcode::Add,
            Instr::Sub(_) => Opcode::Sub,
            Instr::BumpUp(_) => Opcode::BumpUp,
            Instr::BumpDn(_) => Opcode::BumpDn,
            Instr::Mul(_) => Opcode::Mul,
            Instr::Div(_) => Opcode::Div,
            Instr::Mod(_) => Opcode::Mod,
            Instr::Jump(_) => Opcode::Jump,
            Instr::JumpZ(_) => Opcode::JumpZ,
            Instr::JumpN(_) => Opcode::JumpN,
        }
    }

    /// The floor operand, if the instruction has one.
    pub fn addr(&self) -> Option<Addr> {
        match self {
            Instr::CopyTo(a)
            | Instr::CopyFrom(a)
            | Instr::Add(a)
            | Instr::Sub(a)
            | Instr::BumpUp(a)
            | Instr::BumpDn(a)
            | Instr::Mul(a)
            | Instr::Div(a)
            | Instr::Mod(a) => Some(*a),
            _ => None,
        }
    }

    /// The jump target, if the instruction is a jump.
    pub fn target(&self) -> Option<&L> {
        match self {
            Instr::Jump(l) | Instr::JumpZ(l) | Instr::JumpN(l) => Some(l),
            _ => None,
        }
    }

    /// Build an instruction from an opcode and an address (ignored when not needed).
    pub fn with_addr(op: Opcode, a: Addr, label: L) -> Instr<L> {
        match op {
            Opcode::Inbox => Instr::Inbox,
            Opcode::Outbox => Instr::Outbox,
            Opcode::CopyTo => Instr::CopyTo(a),
            Opcode::CopyFrom => Instr::CopyFrom(a),
            Opcode::Add => Instr::Add(a),
            Opcode::Sub => Instr::Sub(a),
            Opcode::BumpUp => Instr::BumpUp(a),
            Opcode::BumpDn => Instr::BumpDn(a),
            Opcode::Mul => Instr::Mul(a),
            Opcode::Div => Instr::Div(a),
            Opcode::Mod => Instr::Mod(a),
            Opcode::Jump => Instr::Jump(label),
            Opcode::JumpZ => Instr::JumpZ(label),
            Opcode::JumpN => Instr::JumpN(label),
        }
    }

    /// Map the jump target type.
    pub fn map_target<M>(&self, f: impl FnOnce(&L) -> M) -> Instr<M> {
        match self {
            Instr::Inbox => Instr::Inbox,
            Instr::Outbox => Instr::Outbox,
            Instr::CopyTo(a) => Instr::CopyTo(*a),
            Instr::CopyFrom(a) => Instr::CopyFrom(*a),
            Instr::Add(a) => Instr::Add(*a),
            Instr::Sub(a) => Instr::Sub(*a),
            Instr::BumpUp(a) => Instr::BumpUp(*a),
            Instr::BumpDn(a) => Instr::BumpDn(*a),
            Instr::Mul(a) => Instr::Mul(*a),
            Instr::Div(a) => Instr::Div(*a),
            Instr::Mod(a) => Instr::Mod(*a),
            Instr::Jump(l) => Instr::Jump(f(l)),
            Instr::JumpZ(l) => Instr::JumpZ(f(l)),
            Instr::JumpN(l) => Instr::JumpN(f(l)),
        }
    }

    /// True for the three jump instructions.
    pub fn is_jump(&self) -> bool {
        matches!(self, Instr::Jump(_) | Instr::JumpZ(_) | Instr::JumpN(_))
    }
}

impl<L: fmt::Display> fmt::Display for Instr<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.opcode().mnemonic();
        if let Some(a) = self.addr() {
            write!(f, "{m} {a}")
        } else if let Some(t) = self.target() {
            write!(f, "{m} {t}")
        } else {
            f.write_str(m)
        }
    }
}

/// The set of instructions a level allows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowed {
    ops: Vec<Opcode>,
    indirect: bool,
}

impl Allowed {
    /// The core eight-ish instructions of the tutorial tier (no arithmetic extensions).
    pub fn basic() -> Allowed {
        Allowed {
            ops: vec![
                Opcode::Inbox,
                Opcode::Outbox,
                Opcode::CopyTo,
                Opcode::CopyFrom,
                Opcode::Add,
                Opcode::Sub,
                Opcode::BumpUp,
                Opcode::BumpDn,
                Opcode::Jump,
                Opcode::JumpZ,
                Opcode::JumpN,
            ],
            indirect: false,
        }
    }

    /// Everything, including indirect addressing (used by the sandbox).
    pub fn everything() -> Allowed {
        Allowed {
            ops: Opcode::ALL.to_vec(),
            indirect: true,
        }
    }

    /// Build from level-file keys (`"inbox"`, ..., plus the pseudo key `"indirect"`).
    /// Unknown keys are returned as errors.
    pub fn from_keys<S: AsRef<str>>(keys: &[S]) -> Result<Allowed, String> {
        let mut ops = Vec::new();
        let mut indirect = false;
        for k in keys {
            let k = k.as_ref();
            if k.eq_ignore_ascii_case("indirect") {
                indirect = true;
            } else if let Some(op) = Opcode::from_mnemonic(k) {
                if !ops.contains(&op) {
                    ops.push(op);
                }
            } else {
                return Err(k.to_string());
            }
        }
        ops.sort();
        Ok(Allowed { ops, indirect })
    }

    /// Whether the opcode may be used.
    pub fn allows(&self, op: Opcode) -> bool {
        self.ops.contains(&op)
    }

    /// Whether `[n]` addressing may be used.
    pub fn allows_indirect(&self) -> bool {
        self.indirect
    }

    /// Allowed opcodes in documentation order.
    pub fn opcodes(&self) -> Vec<Opcode> {
        Opcode::ALL
            .into_iter()
            .filter(|o| self.allows(*o))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics_roundtrip() {
        for op in Opcode::ALL {
            assert_eq!(Opcode::from_mnemonic(op.mnemonic()), Some(op));
            assert_eq!(
                Opcode::from_mnemonic(&op.mnemonic().to_lowercase()),
                Some(op)
            );
            assert_eq!(Opcode::from_mnemonic(op.key()), Some(op));
        }
        assert_eq!(Opcode::from_mnemonic("NOPE"), None);
    }

    #[test]
    fn display_instr() {
        let i: Instr<&str> = Instr::CopyTo(Addr::Indirect(3));
        assert_eq!(i.to_string(), "COPYTO [3]");
        let j: Instr<&str> = Instr::JumpZ("end");
        assert_eq!(j.to_string(), "JUMPZ end");
        assert_eq!(Instr::<&str>::Inbox.to_string(), "INBOX");
    }

    #[test]
    fn allowed_parsing() {
        let a = Allowed::from_keys(&["inbox", "outbox", "indirect"]).unwrap();
        assert!(a.allows(Opcode::Inbox));
        assert!(!a.allows(Opcode::Add));
        assert!(a.allows_indirect());
        assert!(Allowed::from_keys(&["bogus"]).is_err());
    }
}
