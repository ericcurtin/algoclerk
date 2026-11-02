// SPDX-License-Identifier: Apache-2.0
//! State of one play session (a level or the sandbox): program, machine, animation, verdict.

use crate::office::OfficeView;
use algoclerk_core::asm::{self, Listing, Program};
use algoclerk_core::i18n::{Diagnostic, Strings};
use algoclerk_core::isa::Allowed;
use algoclerk_core::judge::{Judge, Verdict};
use algoclerk_core::level::{Level, TestCase, TestOrigin};
use algoclerk_core::vm::{Machine, Status, StepReport};
use std::sync::Arc;

/// Tabs of the narrow (phone) layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Brief,
    Program,
    Office,
    Text,
}

/// A play session.
pub struct PlayState {
    pub level: Option<Level>,
    pub judge: Option<Judge>,
    pub tests: Vec<TestCase>,
    pub test_idx: usize,
    pub listing: Listing,
    pub text: String,
    pub text_errors: Vec<String>,
    pub problems: Vec<String>,
    pub selected: Option<usize>,
    pub machine: Option<Machine>,
    pub program: Option<Arc<Program>>,
    pub running: bool,
    pub speed: f32,
    pub accum: f32,
    pub view: OfficeView,
    pub verdict: Option<Verdict>,
    pub message: Option<String>,
    pub error_bubble: Option<String>,
    pub tab: Tab,
    pub show_text: bool,
    pub hints_shown: usize,
    pub confirm_reference: bool,
    pub confirm_peek: bool,
    pub sandbox_floor: usize,
    pub sandbox_input: String,
    pub draft_dirty: bool,
    pub allowed: Allowed,
    pub floor_len: usize,
    pub tile_names: Vec<String>,
    pub finished_ok: Option<bool>,
}

fn level_floor_names(level: &Level) -> Vec<String> {
    (0..level.machine.floor)
        .map(|i| level.slot_name(i).unwrap_or("").to_string())
        .collect()
}

impl PlayState {
    /// Start a level, restoring the saved draft if there is one.
    pub fn for_level(level: &Level, draft: &str, hints_used: u8) -> Result<PlayState, String> {
        let judge = Judge::new(level).map_err(|e| e.to_string())?;
        let tests = judge.tests().to_vec();
        let allowed = level.allowed().map_err(|e| e.to_string())?;
        let mut s = PlayState {
            level: Some(level.clone()),
            tests,
            judge: Some(judge),
            test_idx: 0,
            listing: Listing::default(),
            text: String::new(),
            text_errors: Vec::new(),
            problems: Vec::new(),
            selected: None,
            machine: None,
            program: None,
            running: false,
            speed: 0.45,
            accum: 0.0,
            view: OfficeView::default(),
            verdict: None,
            message: None,
            error_bubble: None,
            tab: Tab::Brief,
            show_text: false,
            hints_shown: usize::from(hints_used),
            confirm_reference: false,
            confirm_peek: false,
            sandbox_floor: 16,
            sandbox_input: String::new(),
            draft_dirty: false,
            allowed,
            floor_len: level.machine.floor,
            tile_names: level_floor_names(level),
            finished_ok: None,
        };
        if let Ok(l) = asm::parse(draft) {
            s.listing = l;
        }
        s.text = s.listing.to_text();
        s.after_edit(false);
        s.reset_machine();
        Ok(s)
    }

    /// Start the sandbox (every instruction, free floor size, free input).
    pub fn sandbox(saved: &str) -> PlayState {
        let floor_len = 16;
        let mut s = PlayState {
            level: None,
            judge: None,
            tests: vec![TestCase {
                inbox: vec![3, 1, 4, 1, 5],
                floor: vec![None; floor_len],
                expected: Vec::new(),
                origin: TestOrigin::Example,
            }],
            test_idx: 0,
            listing: Listing::default(),
            text: String::new(),
            text_errors: Vec::new(),
            problems: Vec::new(),
            selected: None,
            machine: None,
            program: None,
            running: false,
            speed: 0.45,
            accum: 0.0,
            view: OfficeView::default(),
            verdict: None,
            message: None,
            error_bubble: None,
            tab: Tab::Program,
            show_text: true,
            hints_shown: 0,
            confirm_reference: false,
            confirm_peek: false,
            sandbox_floor: floor_len,
            sandbox_input: "3, 1, 4, 1, 5".to_string(),
            draft_dirty: false,
            allowed: Allowed::everything(),
            floor_len,
            tile_names: vec![String::new(); floor_len],
            finished_ok: None,
        };
        if let Ok(l) = asm::parse(saved) {
            s.listing = l;
        }
        s.text = s.listing.to_text();
        s.after_edit(false);
        s.reset_machine();
        s
    }

    /// The level id, or `"sandbox"`.
    pub fn id(&self) -> String {
        self.level
            .as_ref()
            .map_or_else(|| "sandbox".to_string(), |l| l.id.clone())
    }

    /// True when drawing values as characters.
    pub fn char_mode(&self) -> bool {
        self.level
            .as_ref()
            .is_some_and(|l| l.display == algoclerk_core::level::Display::Char)
    }

    /// Apply the sandbox input fields to the single sandbox test.
    pub fn apply_sandbox_settings(&mut self) {
        if self.level.is_some() {
            return;
        }
        let inbox: Vec<i32> = self
            .sandbox_input
            .split([',', ' ', ';'])
            .filter(|t| !t.is_empty())
            .filter_map(|t| {
                t.parse::<i32>().ok().or_else(|| {
                    (t.chars().count() == 1).then(|| t.chars().next().map_or(0, |c| c as i32))
                })
            })
            .collect();
        self.floor_len = self.sandbox_floor.clamp(1, 64);
        self.tile_names = vec![String::new(); self.floor_len];
        self.tests = vec![TestCase {
            inbox,
            floor: vec![None; self.floor_len],
            expected: Vec::new(),
            origin: TestOrigin::Example,
        }];
        self.test_idx = 0;
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
        self.error_bubble = None;
        self.finished_ok = None;
        self.draft_dirty = true;
        self.problems.clear();
        self.program = None;
        match self.listing.compile() {
            Ok(p) => {
                let probs = p.check(&self.allowed, self.floor_len);
                for pr in probs {
                    self.problems
                        .push(pr.message(algoclerk_core::i18n::english()));
                }
                self.program = Some(Arc::new(p));
            }
            Err(es) => {
                for e in es {
                    self.problems
                        .push(e.message(algoclerk_core::i18n::english()));
                }
            }
        }
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
            Ok(p) => p
                .check(&self.allowed, self.floor_len)
                .iter()
                .map(|x| x.message(tr))
                .collect(),
            Err(es) => es.iter().map(|e| e.message(tr)).collect(),
        }
    }

    /// The test used for the animated run.
    pub fn current_test(&self) -> &TestCase {
        &self.tests[self.test_idx.min(self.tests.len().saturating_sub(1))]
    }

    /// Create a fresh machine for the current test (paused at the start).
    pub fn reset_machine(&mut self) {
        self.running = false;
        self.accum = 0.0;
        self.view.reset();
        self.message = None;
        self.error_bubble = None;
        self.finished_ok = None;
        let limit = self.level.as_ref().map_or(100_000, |l| l.tests.step_limit);
        let Some(program) = self.program.clone().filter(|_| self.problems.is_empty()) else {
            self.machine = None;
            return;
        };
        let t = self.current_test().clone();
        let mut floor = t.floor.clone();
        floor.resize(self.floor_len, None);
        self.machine = Some(Machine::new(program, t.inbox, floor, limit).with_history());
    }

    /// Execute one instruction (creating the machine if needed).
    pub fn step(&mut self, tr: &Strings) -> Option<StepReport> {
        if self.machine.is_none() {
            self.reset_machine();
        }
        let sps = self.steps_per_sec();
        let m = self.machine.as_mut()?;
        if m.is_done() {
            return None;
        }
        let report = m.step();
        self.view.on_report(&report, sps);
        self.after_step(tr);
        Some(report)
    }

    fn after_step(&mut self, tr: &Strings) {
        let Some(m) = self.machine.as_ref() else {
            return;
        };
        match &m.status {
            Status::Running => {}
            Status::Finished => {
                self.running = false;
                let exp = &self.current_test().expected;
                let ok = self.level.is_none() || m.outbox == *exp;
                self.finished_ok = Some(ok);
                self.message = Some(if self.level.is_none() {
                    tr.t("play.finished")
                } else if ok {
                    tr.t("play.test_ok")
                } else {
                    tr.t("play.test_wrong")
                });
            }
            Status::Error(e) => {
                self.running = false;
                let msg = e.message(tr);
                self.error_bubble = Some(msg.clone());
                self.message = Some(msg);
                self.finished_ok = Some(false);
            }
        }
    }

    /// Step back one instruction.
    pub fn step_back(&mut self) {
        self.running = false;
        if let Some(m) = self.machine.as_mut() {
            m.step_back();
            self.error_bubble = None;
            self.message = None;
            self.finished_ok = None;
            self.view.reset();
        }
    }

    /// Animation speed in instructions per second (the top of the slider means "as fast as possible").
    pub fn steps_per_sec(&self) -> f32 {
        1.5 * 2f32.powf(self.speed * 8.0)
    }

    /// Advance a running machine by `dt` seconds.
    pub fn tick(&mut self, dt: f32, tr: &Strings) {
        if !self.running {
            return;
        }
        self.accum += dt * self.steps_per_sec();
        let max_per_frame = if self.speed >= 0.99 { 5000 } else { 400 };
        let mut n = 0;
        while self.accum >= 1.0 && n < max_per_frame && self.running {
            self.accum -= 1.0;
            n += 1;
            if self.step(tr).is_none() {
                self.running = false;
            }
        }
        if self.accum > 50.0 {
            self.accum = 0.0;
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

    /// Whether the pending program can be judged.
    pub fn runnable(&self) -> bool {
        self.problems.is_empty() && self.program.as_ref().is_some_and(|p| p.size() > 0)
    }

    /// Instruction highlighted in the list.
    pub fn executing_line(&self) -> Option<usize> {
        let m = self.machine.as_ref()?;
        let p = m.program();
        if m.status == Status::Running || matches!(m.status, Status::Error(_)) {
            p.listing_idx.get(m.pc).copied()
        } else {
            None
        }
    }
}
