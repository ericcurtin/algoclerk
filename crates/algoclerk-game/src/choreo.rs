// SPDX-License-Identifier: Apache-2.0
//! Turns an executed machine step into *beats*: small timed actions (walk to a tile, grab, put down,
//! sparkle ...) that the office plays back at the player's chosen speed.

use crate::clerk::{Dir, Pose};
use crate::office::{BELT_SLOTS, CRATE_H, CRATE_W, Geometry};
use algoclerk_core::i18n::Strings;
use algoclerk_core::isa::{Addr, Instr};
use algoclerk_core::vm::{Effect, RuntimeError, Status, StepReport};

/// What happens when a crate in flight arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Land {
    /// It ends up over the clerk's head.
    Hand,
    /// It slides onto the outbox belt, in this slot.
    Out(usize),
    /// A copy lands on a floor tile with this value.
    Tile(usize, i32),
}

/// One timed piece of choreography. Tick counts are for the slowest speed; the office divides them.
#[derive(Debug, Clone, PartialEq)]
pub enum Beat {
    /// Walk to a position (centre x, feet y).
    Walk(f32, f32),
    /// Turn.
    Face(Dir),
    /// Strike a pose for some ticks.
    Pose(Pose, u32),
    /// A crate flies along a path (crate top-left corners).
    Fly {
        /// Waypoints.
        path: Vec<(f32, f32)>,
        /// Value on the label.
        value: i32,
        /// Duration.
        ticks: u32,
        /// What happens on arrival.
        land: Land,
    },
    /// Instantly change the value in the hand.
    Hand(Option<i32>),
    /// Instantly change a tile's value.
    Tile(usize, i32),
    /// The next inbox crate leaves the belt (the belt slides up to fill the gap).
    TakeInbox,
    /// A sparkle with a symbol at a scene position.
    Sparkle(char, f32, f32),
    /// Show the pointer arrow on a tile.
    PointerAt(usize),
    /// A speech bubble for some ticks.
    Bubble(String, u32),
    /// A bubble that stays until the machine is reset.
    StickyBubble(String),
    /// Just wait.
    Wait(u32),
    /// Flash a tile.
    Flash(usize),
    /// Remove the bubble.
    ClearBubble,
}

fn hand_pos(stand: (f32, f32)) -> (f32, f32) {
    (stand.0 - CRATE_W as f32 / 2.0, stand.1 - 20.0 - CRATE_H as f32 + 2.0)
}

fn op_symbol(i: &Instr<usize>) -> char {
    match i {
        Instr::Add(_) => '+',
        Instr::Sub(_) => '-',
        Instr::Mul(_) => 'x',
        Instr::Div(_) => '/',
        Instr::Mod(_) => '%',
        Instr::BumpUp(_) => '+',
        Instr::BumpDn(_) => '-',
        _ => '?',
    }
}

/// The friendly, clerk-voiced line for a runtime error (`clerk.*` keys in the language file).
pub fn error_line(e: &RuntimeError, tr: &Strings) -> String {
    match e {
        RuntimeError::EmptyHand => tr.t("clerk.empty_hand"),
        RuntimeError::EmptyTile(n) => tr.format("clerk.empty_tile", &[("slot", n.to_string())]),
        RuntimeError::NoSuchTile(n) => tr.format("clerk.no_such_tile", &[("slot", n.to_string())]),
        RuntimeError::Overflow => tr.t("clerk.overflow"),
        RuntimeError::DivideByZero => tr.t("clerk.divide_by_zero"),
        RuntimeError::StepLimit(_) => tr.t("clerk.step_limit"),
    }
}

/// Plan the beats for one executed instruction.
///
/// `outbox_len` is the length of the outbox *after* the step. `names` is only used to bound slots.
pub fn plan(
    instr: &Instr<usize>,
    report: &StepReport,
    geom: &Geometry,
    outbox_len: usize,
    tr: &Strings,
) -> Vec<Beat> {
    let mut beats = Vec::new();
    let ptr_read = |first: bool| -> Option<usize> {
        // For an indirect operand the VM reports the pointer tile's read first.
        let reads: Vec<usize> = report
            .effects
            .iter()
            .filter_map(|e| if let Effect::FloorRead(s) = e { Some(*s) } else { None })
            .collect();
        if first { reads.first().copied() } else { reads.last().copied() }
    };
    let tile_ok = |s: usize| s < geom.n;
    // visit the pointer tile first (indirect addressing), then the real target
    let go_to_tile = |beats: &mut Vec<Beat>, a: Addr, target: Option<usize>| {
        if a.is_indirect() {
            let p = a.slot();
            if tile_ok(p) {
                let st = geom.stand(p);
                beats.push(Beat::Walk(st.0, st.1));
                beats.push(Beat::PointerAt(p));
                beats.push(Beat::Flash(p));
                beats.push(Beat::Wait(26));
            }
        }
        if let Some(t) = target.filter(|t| tile_ok(*t)) {
            let st = geom.stand(t);
            beats.push(Beat::Walk(st.0, st.1));
            beats.push(Beat::Flash(t));
        }
    };
    let target_of = |a: Addr| -> Option<usize> {
        match a {
            Addr::Direct(n) => Some(n),
            Addr::Indirect(_) => report
                .effects
                .iter()
                .rev()
                .find_map(|e| match e {
                    Effect::FloorWrite { slot, .. } => Some(*slot),
                    Effect::FloorRead(s) => Some(*s),
                    _ => None,
                })
                .filter(|_| report.effects.len() > 1 || matches!(report.status, Status::Running)),
        }
    };
    let _ = ptr_read;
    let error = match &report.status {
        Status::Error(e) => Some(e.clone()),
        _ => None,
    };
    match instr {
        Instr::Inbox => {
            let st = geom.inbox_station();
            beats.push(Beat::Walk(st.0, st.1));
            beats.push(Beat::Face(Dir::Left));
            let taken = report.effects.iter().find_map(|e| match e {
                Effect::InboxTake(v) => Some(*v),
                _ => None,
            });
            if let Some(v) = taken {
                let from = geom.in_slot(0);
                beats.push(Beat::Pose(Pose::Grab, 14));
                beats.push(Beat::TakeInbox);
                beats.push(Beat::Fly {
                    path: vec![(from.x as f32, from.y as f32), hand_pos(st)],
                    value: v,
                    ticks: 14,
                    land: Land::Hand,
                });
            } else {
                beats.push(Beat::Pose(Pose::Shrug, 36));
            }
        }
        Instr::Outbox => {
            let st = geom.outbox_station();
            beats.push(Beat::Walk(st.0, st.1));
            beats.push(Beat::Face(Dir::Right));
            if let Some(v) = report.effects.iter().find_map(|e| match e {
                Effect::OutboxPut(v) => Some(*v),
                _ => None,
            }) {
                let idx = outbox_len.saturating_sub(1);
                let w0 = (idx + 1).saturating_sub(BELT_SLOTS - 1);
                let j = idx - w0;
                let drop = geom.out_slot(BELT_SLOTS - 1);
                let end = geom.out_slot(j);
                beats.push(Beat::Pose(Pose::Grab, 10));
                beats.push(Beat::Fly {
                    path: vec![hand_pos(st), (drop.x as f32, drop.y as f32), (end.x as f32, end.y as f32)],
                    value: v,
                    ticks: 24 + 6 * (BELT_SLOTS - 1 - j) as u32,
                    land: Land::Out(j),
                });
            }
        }
        Instr::CopyTo(a) => {
            let target = target_of(*a);
            go_to_tile(&mut beats, *a, target);
            if let (Some(t), Some(v)) = (
                target.filter(|t| tile_ok(*t)),
                report.effects.iter().find_map(|e| match e {
                    Effect::FloorWrite { value, .. } => Some(*value),
                    _ => None,
                }),
            ) {
                let st = geom.stand(t);
                let to = geom.tile_crate(t);
                beats.push(Beat::Pose(Pose::Grab, 10));
                beats.push(Beat::Fly {
                    path: vec![hand_pos(st), (to.x as f32, to.y as f32)],
                    value: v,
                    ticks: 14,
                    land: Land::Tile(t, v),
                });
            }
        }
        Instr::CopyFrom(a) => {
            let target = target_of(*a);
            go_to_tile(&mut beats, *a, target);
            if let (Some(t), Some(v)) = (
                target.filter(|t| tile_ok(*t)),
                report.effects.iter().find_map(|e| match e {
                    Effect::HandSet(Some(v)) => Some(*v),
                    _ => None,
                }),
            ) {
                let st = geom.stand(t);
                let from = geom.tile_crate(t);
                beats.push(Beat::Pose(Pose::Grab, 10));
                beats.push(Beat::Fly {
                    path: vec![(from.x as f32, from.y as f32), hand_pos(st)],
                    value: v,
                    ticks: 14,
                    land: Land::Hand,
                });
            }
        }
        Instr::Add(a) | Instr::Sub(a) | Instr::Mul(a) | Instr::Div(a) | Instr::Mod(a) => {
            let target = target_of(*a);
            go_to_tile(&mut beats, *a, target);
            if let Some(t) = target.filter(|t| tile_ok(*t)) {
                let st = geom.stand(t);
                if error.is_none() {
                    beats.push(Beat::Pose(Pose::Work, 18));
                    beats.push(Beat::Sparkle(op_symbol(instr), st.0, st.1 - 28.0));
                    if let Some(v) = report.effects.iter().find_map(|e| match e {
                        Effect::HandSet(Some(v)) => Some(*v),
                        _ => None,
                    }) {
                        beats.push(Beat::Hand(Some(v)));
                    }
                    beats.push(Beat::Wait(8));
                }
            }
        }
        Instr::BumpUp(a) | Instr::BumpDn(a) => {
            let target = target_of(*a);
            go_to_tile(&mut beats, *a, target);
            if let Some(t) = target.filter(|t| tile_ok(*t)) {
                let st = geom.stand(t);
                if error.is_none() {
                    beats.push(Beat::Pose(Pose::Work, 14));
                    beats.push(Beat::Sparkle(op_symbol(instr), st.0, st.1 - 28.0));
                    if let Some(v) = report.effects.iter().find_map(|e| match e {
                        Effect::HandSet(Some(v)) => Some(*v),
                        _ => None,
                    }) {
                        beats.push(Beat::Tile(t, v));
                        beats.push(Beat::Hand(Some(v)));
                    }
                    beats.push(Beat::Wait(8));
                }
            }
        }
        Instr::Jump(_) => {
            beats.push(Beat::Pose(Pose::Think, 14));
        }
        Instr::JumpZ(_) | Instr::JumpN(_) => {
            beats.push(Beat::Pose(Pose::Think, 14));
            if error.is_none() {
                let jumped = report.effects.iter().any(|e| matches!(e, Effect::Jumped(_)));
                let key = if jumped { "clerk.jump_yes" } else { "clerk.jump_no" };
                beats.push(Beat::Bubble(tr.t(key), 26));
            }
        }
    }
    if let Some(e) = error {
        // walk to the offending tile (if there is one), then droop and complain
        let slot = match &e {
            RuntimeError::EmptyTile(s) => Some(*s),
            _ => None,
        };
        if let Some(s) = slot.filter(|s| tile_ok(*s)) {
            let st = geom.stand(s);
            if !beats.iter().any(|b| matches!(b, Beat::Walk(x, y) if (*x, *y) == st)) {
                beats.push(Beat::Walk(st.0, st.1));
            }
            beats.push(Beat::Flash(s));
        }
        beats.push(Beat::Pose(Pose::Sad, 20));
        beats.push(Beat::StickyBubble(error_line(&e, tr)));
    }
    beats
}

/// Beats for the end of a shift: cheer when the order was filled, droop when it was not.
pub fn finish_beats(ok: bool, message: String) -> Vec<Beat> {
    if ok {
        vec![Beat::Pose(Pose::Cheer, 30), Beat::StickyBubble(message)]
    } else {
        vec![Beat::Pose(Pose::Sad, 20), Beat::StickyBubble(message)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoclerk_core::i18n::english;

    fn rep(effects: Vec<Effect>, status: Status) -> StepReport {
        StepReport {
            pc: 0,
            effects,
            status,
        }
    }

    #[test]
    fn inbox_walks_to_the_belt_and_grabs() {
        let g = Geometry::new(4);
        let b = plan(
            &Instr::Inbox,
            &rep(vec![Effect::InboxTake(7), Effect::HandSet(Some(7))], Status::Running),
            &g,
            0,
            english(),
        );
        assert!(matches!(b[0], Beat::Walk(..)));
        assert!(b.iter().any(|x| matches!(x, Beat::TakeInbox)));
        assert!(b.iter().any(|x| matches!(x, Beat::Fly { land: Land::Hand, value: 7, .. })));
    }

    #[test]
    fn outbox_lands_in_the_next_slot() {
        let g = Geometry::new(4);
        let b = plan(
            &Instr::Outbox,
            &rep(vec![Effect::OutboxPut(3), Effect::HandSet(None)], Status::Running),
            &g,
            2,
            english(),
        );
        assert!(b.iter().any(|x| matches!(x, Beat::Fly { land: Land::Out(1), value: 3, .. })));
    }

    #[test]
    fn indirect_addressing_visits_the_pointer_tile_first() {
        let g = Geometry::new(8);
        let b = plan(
            &Instr::CopyFrom(Addr::Indirect(2)),
            &rep(
                vec![Effect::FloorRead(2), Effect::FloorRead(5), Effect::HandSet(Some(9))],
                Status::Running,
            ),
            &g,
            0,
            english(),
        );
        let walks: Vec<_> = b.iter().filter(|x| matches!(x, Beat::Walk(..))).collect();
        assert_eq!(walks.len(), 2);
        assert!(b.iter().any(|x| matches!(x, Beat::PointerAt(2))));
        assert!(b.iter().any(|x| matches!(x, Beat::Fly { land: Land::Hand, value: 9, .. })));
    }

    #[test]
    fn errors_end_with_a_sad_clerk_and_a_friendly_bubble() {
        let g = Geometry::new(4);
        let b = plan(
            &Instr::Outbox,
            &rep(vec![], Status::Error(RuntimeError::EmptyHand)),
            &g,
            0,
            english(),
        );
        assert!(b.iter().any(|x| matches!(x, Beat::Pose(Pose::Sad, _))));
        let Some(Beat::StickyBubble(t)) = b.last() else {
            panic!("no bubble");
        };
        assert!(t.contains("hand is empty"), "{t}");
    }
}
