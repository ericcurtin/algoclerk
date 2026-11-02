// SPDX-License-Identifier: Apache-2.0
//! The state of one play session (a level or the sandbox): the program being edited, the machine, the
//! animated office and the verdict. Logic only; the screen lives in `play.rs`.

use crate::choreo;
use crate::office::{MAX_TILES, Office};
use algoclerk_core::asm::{self, Listing, Program};
use algoclerk_core::i18n::{Diagnostic, Strings};
use algoclerk_core::isa::Allowed;
use algoclerk_core::judge::{Judge, Verdict};
use algoclerk_core::level::{Level, TestCase, TestOrigin};
use algoclerk_core::vm::{Machine, Status};
use algoclerk_media::{ArtBank, Outfit};
use std::sync::Arc;

/// Animation speeds: step 0..=4. Multiplier for the choreography; the last one skips the animation.
pub const SPEEDS: [f32; 5] = [1.0, 2.0, 4.0, 8.0, f32::INFINITY];

/// Largest number of floor tiles the sandbox offers.
pub const SANDBOX_MAX_FLOOR: usize = MAX_TILES;

/// A play session.
#[derive(Debug)]
pub struct Session {
    /// The level (`None` in the sandbox).
    pub level: Option<Level>,
    /// The judge (`None` in the sandbox).
    pub judge: Option<Judge>,
    /// Tests the machine can run (the sandbox has exactly one).
    pub tests: Vec<TestCase>,
    /// Which test is shown in the office.
    pub test_idx: usize,
    /// The program being edited.
    pub listing: Listing,
    /// Text form of the program.
    pub text: String,
    /// Problems found while parsing the text.
    pub text_errors: Vec<String>,
    /// Rule problems of the compiled program.
    pub problems: Vec<String>,
    /// The compiled program, if it compiles.
    pub program: Option<Arc<Program>>,
    /// The machine, once a run has started.
    pub machine: Option<Machine>,
    /// Running automatically.
    pub running: bool,
    /// The animated office.
    pub office: Office,
    /// The last verdict from Check.
    pub verdict: Option<Verdict>,
    /// A message for the status line.
    pub message: Option<String>,
    /// Hints shown so far.
    pub hints_shown: usize,
    /// Sandbox: floor tile count.
    pub sandbox_floor: usize,
    /// Sandbox: inbox values as typed.
    pub sandbox_input: String,
    /// The program changed since it was saved.
    pub draft_dirty: bool,
    /// Instructions allowed.
    pub allowed: Allowed,
    /// Floor tile count.
    pub floor_len: usize,
    /// `Some(true)` when the last full run matched the expected output.
    pub finished_ok: Option<bool>,
    /// Animation speed step 0..=4.
    pub speed_level: u8,
    /// Seconds-per-tick accumulator for headless stepping.
    pub steps_this_run: u64,
}

fn level_floor_names(level: &Level) -> Vec<String> {
    (0..level.machine.floor)
        .map(|i| level.slot_name(i).unwrap_or("").to_string())
        .collect()
}

impl Session {
    /// Start a level, restoring the saved draft if there is one.
    ///
    /// # Errors
    /// A message when the level cannot be judged.
    pub fn for_level(
        level: &Level,
        draft: &str,
        hints_used: u8,
        bank: &ArtBank,
        outfit: &Outfit,
        speed_level: u8,
    ) -> Result<Session, String> {
        let judge = Judge::new(level).map_err(|e| e.to_string())?;
        let tests = judge.tests().to_vec();
        let allowed = level.allowed().map_err(|e| e.to_string())?;
        let floor_len = level.machine.floor;
        let mut office = Office::new(bank, outfit, floor_len);
        office.names = level_floor_names(level);
        office.char_mode = level.display == algoclerk_core::level::Display::Char;
        let mut s = Session {
            level: Some(level.clone()),
            tests,
            judge: Some(judge),
            test_idx: 0,
            listing: Listing::default(),
            text: String::new(),
            text_errors: Vec::new(),
            problems: Vec::new(),
            program: None,
            machine: None,
            running: false,
            office,
            verdict: None,
            message: None,
            hints_shown: usize::from(hints_used),
            sandbox_floor: 16,
            sandbox_input: String::new(),
            draft_dirty: false,
            allowed,
            floor_len,
            finished_ok: None,
            speed_level: speed_level.min(4),
            steps_this_run: 0,
        };
        if let Ok(l) = asm::parse(draft) {
            s.listing = l;
        }
        s.text = s.listing.to_text();
        s.after_edit(false);
        s.reset_machine();
        s.draft_dirty = false;
        Ok(s)
    }

    /// Start the sandbox (every instruction, free floor size, free input).
    pub fn sandbox(saved: &str, bank: &ArtBank, outfit: &Outfit, speed_level: u8) -> Session {
        let floor_len = 16;
        let mut s = Session {
            level: None,
            judge: None,
            tests: Vec::new(),
            test_idx: 0,
            listing: Listing::default(),
            text: String::new(),
            text_errors: Vec::new(),
            problems: Vec::new(),
            program: None,
            machine: None,
            running: false,
            office: Office::new(bank, outfit, floor_len),
            verdict: None,
            message: None,
            hints_shown: 0,
            sandbox_floor: floor_len,
            sandbox_input: "3, 1, 4, 1, 5".to_string(),
            draft_dirty: false,
            allowed: Allowed::everything(),
            floor_len,
            finished_ok: None,
            speed_level: speed_level.min(4),
            steps_this_run: 0,
        };
        s.apply_sandbox_settings(bank, outfit);
        if let Ok(l) = asm::parse(saved) {
            s.listing = l;
        }
        s.text = s.listing.to_text();
        s.after_edit(false);
        s.reset_machine();
        s.draft_dirty = false;
        s
    }

    /// The level id, or `"sandbox"`.
    pub fn id(&self) -> String {
        self.level
            .as_ref()
            .map_or_else(|| "sandbox".to_string(), |l| l.id.clone())
    }

    /// Is this the sandbox?
    pub fn is_sandbox(&self) -> bool {
        self.level.is_none()
    }

    /// Parse the sandbox inbox text (numbers or single characters separated by commas, spaces or semicolons).
    pub fn parse_inbox(text: &str) -> Vec<i32> {
        text.split([',', ' ', ';'])
            .filter(|t| !t.is_empty())
            .filter_map(|t| {
                t.parse::<i32>()
                    .ok()
                    .or_else(|| (t.chars().count() == 1).then(|| t.chars().next().map_or(0, |c| c as i32)))
            })
            .collect()
    }

    /// Apply the sandbox input fields to the single sandbox test.
    pub fn apply_sandbox_settings(&mut self, bank: &ArtBank, outfit: &Outfit) {
        if self.level.is_some() {
            return;
        }
        let inbox = Session::parse_inbox(&self.sandbox_input);
        self.floor_len = self.sandbox_floor.clamp(1, SANDBOX_MAX_FLOOR);
        self.sandbox_floor = self.floor_len;
        self.tests = vec![TestCase {
            inbox,
            floor: vec![None; self.floor_len],
            expected: Vec::new(),
            origin: TestOrigin::Example,
        }];
        self.test_idx = 0;
        let mut office = Office::new(bank, outfit, self.floor_len);
        office.reduce_motion = self.office.reduce_motion;
        self.office = office;
        self.after_edit(false);
        self.reset_machine();
    }

    /// Called after any change of the listing. `regenerate_text` is false when the text itself was the source.
    pub fn after_edit(&mut self, regenerate_text: bool) {
        if regenerate_text {
            self.text = self.listing.to_text();
            self.text_errors.clear();
        }
        self.running = false;
        self.machine = None;
        self.verdict = None;
        self.message = None;
        self.finished_ok = None;
        self.draft_dirty = true;
        self.problems.clear();
        self.program = None;
        match self.listing.compile() {
            Ok(p) => {
                let probs = p.check(&self.allowed, self.floor_len);
                for pr in probs {
                    self.problems.push(pr.message(algoclerk_core::i18n::english()));
                }
                self.program = Some(Arc::new(p));
            }
            Err(es) => {
                for e in es {
                    self.problems.push(e.message(algoclerk_core::i18n::english()));
                }
            }
        }
        self.sync_office();
    }

    /// Replace the listing from edited text. Keeps the old listing when the text has errors.
    pub fn set_text(&mut self, tr: &Strings) {
        match asm::parse(&self.text) {
            Ok(l) => {
                self.text_errors.clear();
                self.listing = l;
                self.after_edit(false);
            }
            Err(es) => {
                self.text_errors = es.iter().map(|e| e.message(tr)).collect();
            }
        }
    }

    /// Localised problems for display.
    pub fn problems_text(&self, tr: &Strings) -> Vec<String> {
        match self.listing.compile() {
            Ok(p) => p.check(&self.allowed, self.floor_len).iter().map(|x| x.message(tr)).collect(),
            Err(es) => es.iter().map(|e| e.message(tr)).collect(),
        }
    }

    /// The test used for the animated run.
    pub fn current_test(&self) -> &TestCase {
        &self.tests[self.test_idx.min(self.tests.len().saturating_sub(1))]
    }

    /// Show the machine's state (or the untouched test) in the office without animation.
    pub fn sync_office(&mut self) {
        let Some(t) = self.tests.get(self.test_idx.min(self.tests.len().saturating_sub(1))) else {
            return;
        };
        let mut floor = t.floor.clone();
        floor.resize(self.floor_len, None);
        let expected = if self.level.is_some() { t.expected.clone() } else { Vec::new() };
        self.office.speed = SPEEDS[usize::from(self.speed_level)];
        match &self.machine {
            Some(m) => {
                let (inbox, taken) = (m.inbox().to_vec(), m.in_pos);
                self.office.sync(&inbox, taken, &m.floor, m.hand, &m.outbox, m.steps);
            }
            None => self.office.sync(&t.inbox, 0, &floor, None, &[], 0),
        }
        self.office.set_expected(&expected);
    }

    /// Create a fresh machine for the current test (paused at the start).
    pub fn reset_machine(&mut self) {
        self.running = false;
        self.message = None;
        self.finished_ok = None;
        self.steps_this_run = 0;
        let limit = self.level.as_ref().map_or(100_000, |l| l.tests.step_limit);
        let Some(program) = self.program.clone().filter(|_| self.problems.is_empty()) else {
            self.machine = None;
            self.sync_office();
            return;
        };
        let t = self.current_test().clone();
        let mut floor = t.floor.clone();
        floor.resize(self.floor_len, None);
        self.machine = Some(Machine::new(program, t.inbox, floor, limit).with_history());
        self.sync_office();
    }

    /// Choose another test for the office (wraps) and restart.
    pub fn cycle_test(&mut self, delta: i32) {
        let n = self.tests.len().max(1) as i32;
        self.test_idx = (self.test_idx as i32 + delta).rem_euclid(n) as usize;
        self.reset_machine();
    }

    /// Execute one instruction (creating the machine if needed) and queue its choreography.
    pub fn step(&mut self, tr: &Strings) -> bool {
        if self.machine.is_none() {
            self.reset_machine();
        }
        self.office.settle();
        let Some(m) = self.machine.as_mut() else {
            return false;
        };
        if m.is_done() {
            return false;
        }
        let pc = m.pc;
        let instr = m.program().instrs.get(pc).cloned();
        let report = m.step();
        self.steps_this_run += 1;
        self.office.steps = m.steps;
        let outbox_len = m.outbox.len();
        if let Some(instr) = instr {
            let beats = choreo::plan(&instr, &report, &self.office.geom, outbox_len, tr);
            if self.office.speed.is_finite() {
                self.office.enqueue(beats);
            } else {
                self.apply_error_bubble(&report.status, tr);
            }
        }
        self.after_step(tr);
        true
    }

    fn apply_error_bubble(&mut self, status: &Status, tr: &Strings) {
        if let Status::Error(e) = status {
            self.office.set_bubble(Some(choreo::error_line(e, tr)));
        }
    }

    fn after_step(&mut self, tr: &Strings) {
        let Some(m) = self.machine.as_ref() else {
            return;
        };
        let instant = !self.office.speed.is_finite();
        match &m.status {
            Status::Running => {}
            Status::Finished => {
                self.running = false;
                let exp = &self.current_test().expected;
                let ok = self.level.is_none() || m.outbox == *exp;
                self.finished_ok = Some(ok);
                let (line, msg) = if self.level.is_none() {
                    (tr.t("clerk.done_sandbox"), tr.t("play.finished"))
                } else if ok {
                    (tr.t("clerk.done_ok"), tr.t("play.test_ok"))
                } else {
                    (tr.t("clerk.wrong_output"), tr.t("play.test_wrong"))
                };
                self.message = Some(msg);
                if instant {
                    self.office.settle();
                    self.sync_machine_view();
                }
                self.office.enqueue(choreo::finish_beats(ok, line));
                if instant {
                    self.office.settle();
                }
            }
            Status::Error(e) => {
                self.running = false;
                self.message = Some(e.message(tr));
                self.finished_ok = Some(false);
                if instant {
                    self.sync_machine_view();
                }
            }
        }
    }

    fn sync_machine_view(&mut self) {
        if let Some(m) = &self.machine {
            let (inbox, taken) = (m.inbox().to_vec(), m.in_pos);
            let bubble = self.office.bubble.clone();
            let (x, y, dir) = (self.office.clerk.x, self.office.clerk.y, self.office.clerk.dir);
            self.office.sync(&inbox, taken, &m.floor, m.hand, &m.outbox, m.steps);
            self.office.bubble = bubble;
            self.office.clerk.x = x;
            self.office.clerk.y = y;
            self.office.clerk.dir = dir;
        }
    }

    /// Step back one instruction.
    pub fn step_back(&mut self) {
        self.running = false;
        if let Some(m) = self.machine.as_mut() {
            m.step_back();
            self.message = None;
            self.finished_ok = None;
            self.sync_office();
        }
    }

    /// Advance the session by one tick (60 per second): animation, and automatic stepping when running.
    pub fn tick(&mut self, tr: &Strings) {
        self.office.tick();
        if !self.running {
            return;
        }
        if self.office.speed.is_finite() {
            if !self.office.busy() && !self.step(tr) {
                self.running = false;
            }
        } else {
            // "no animation": run as many instructions as fit in a frame, then show the result
            let mut n = 0;
            while self.running && n < 4000 {
                n += 1;
                if !self.step(tr) {
                    self.running = false;
                }
            }
            if self.running {
                self.sync_machine_view();
            }
        }
    }

    /// Start or pause the run.
    pub fn toggle_run(&mut self) {
        if self.machine.as_ref().is_none_or(Machine::is_done) {
            self.reset_machine();
        }
        if self.machine.is_some() {
            self.running = !self.running;
        }
    }

    /// Change the animation speed step.
    pub fn set_speed(&mut self, level: u8) {
        self.speed_level = level.min(4);
        self.office.speed = SPEEDS[usize::from(self.speed_level)];
    }

    /// Judge the program on all tests. Returns true if it passed.
    pub fn check(&mut self) -> bool {
        self.running = false;
        let (Some(judge), Some(program)) = (self.judge.as_ref(), self.program.clone()) else {
            return false;
        };
        let v = judge.judge(&program);
        let passed = v.passed;
        self.verdict = Some(v);
        passed
    }

    /// Whether the pending program can be run.
    pub fn runnable(&self) -> bool {
        self.problems.is_empty() && self.program.as_ref().is_some_and(|p| p.size() > 0)
    }

    /// Listing index of the instruction the machine will execute next (or just failed on).
    pub fn executing_line(&self) -> Option<usize> {
        let m = self.machine.as_ref()?;
        let p = m.program();
        if m.status == Status::Running || matches!(m.status, Status::Error(_)) {
            if m.steps == 0 && m.status == Status::Running && !self.running && !self.office.busy() {
                return None;
            }
            p.listing_idx.get(m.pc).copied()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoclerk_core::i18n::english;
    use algoclerk_media::Assets;

    fn bank() -> ArtBank {
        ArtBank::load(&Assets::embedded()).expect("art loads")
    }

    fn echo() -> Level {
        let c = algoclerk_levels::Campaign::shipped();
        c.find("echo").expect("echo level").clone()
    }

    #[test]
    fn solving_a_level_through_the_session() {
        let b = bank();
        let level = echo();
        let text = level.solution.as_ref().unwrap().asm.clone();
        let mut s = Session::for_level(&level, &text, 0, &b, &Outfit::default(), 4).unwrap();
        assert!(s.runnable());
        s.toggle_run();
        for _ in 0..50 {
            s.tick(english());
        }
        assert_eq!(s.finished_ok, Some(true), "{:?}", s.message);
        assert!(s.check());
    }

    #[test]
    fn stepping_animates_and_rewinding_resets_the_picture() {
        let b = bank();
        let level = echo();
        let text = level.solution.as_ref().unwrap().asm.clone();
        let mut s = Session::for_level(&level, &text, 0, &b, &Outfit::default(), 0).unwrap();
        assert!(s.step(english()));
        assert!(s.office.busy(), "the clerk has something to do");
        for _ in 0..400 {
            s.office.tick();
        }
        assert!(!s.office.busy());
        assert!(s.office.vis.hand.is_some());
        s.step_back();
        assert!(s.office.vis.hand.is_none());
        assert_eq!(s.office.vis.taken, 0);
    }

    #[test]
    fn errors_show_a_bubble() {
        let b = bank();
        let level = echo();
        let mut s = Session::for_level(&level, "    OUTBOX\n", 0, &b, &Outfit::default(), 0).unwrap();
        assert!(s.step(english()));
        for _ in 0..300 {
            s.office.tick();
        }
        let (text, _) = s.office.bubble.clone().expect("bubble");
        assert!(text.contains("hand is empty"));
        assert!(matches!(s.machine.as_ref().unwrap().status, Status::Error(_)));
    }

    #[test]
    fn sandbox_parses_inbox_and_limits_the_floor() {
        assert_eq!(Session::parse_inbox("3, 1 4;a"), vec![3, 1, 4, 97]);
        let b = bank();
        let mut s = Session::sandbox("", &b, &Outfit::default(), 1);
        s.sandbox_floor = 500;
        s.apply_sandbox_settings(&b, &Outfit::default());
        assert_eq!(s.floor_len, SANDBOX_MAX_FLOOR);
    }
}
