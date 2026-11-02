// SPDX-License-Identifier: Apache-2.0
//! The play screen: the office scene, run controls and the program editor.

use crate::brief::{Memo, MemoOut};
use crate::editor::{EditorPanel, Tab};
use crate::gfx::{Gfx, Kind, State, clip};
use crate::input::Key;
use crate::layout::{Hit, Viewport};
use crate::modal::{Answer, Modal, Purpose};
use crate::office::{Office, SCENE_H, SCENE_W};
use crate::session::Session;
use crate::ui::{Ctx, GameEvent, Nav, View, widget_state};
use algoclerk_core::judge::{FailKind, Verdict};
use algoclerk_media::{ArtBank, Font, Outfit, Rect};

/// Ticks per recorded step at each speed level (0 means instant).
pub const STEP_TICKS: [u32; 5] = [48, 28, 16, 7, 0];

/// The screen.
pub struct Play {
    /// Program and run state.
    pub session: Session,
    /// The animated scene.
    pub office: Office,
    /// The editor panel.
    pub panel: EditorPanel,
    /// Playing automatically.
    pub playing: bool,
    timer: u32,
    /// The memo overlay.
    pub memo: Option<Memo>,
    /// A confirm dialog.
    pub modal: Option<Modal>,
    /// The last Check improved the record.
    pub improved: bool,
    /// Solved by the latest Check.
    pub solved: bool,
    /// Status line under the controls: text and whether it is a problem.
    pub status: Option<(String, bool)>,
    memo_starting: bool,
    shown_hints: usize,
}

/// Screen rectangles.
#[derive(Debug, Clone, Copy)]
pub struct PlayRects {
    /// Top bar.
    pub top: Rect,
    /// Scene.
    pub scene: Rect,
    /// Run controls and status.
    pub controls: Rect,
    /// Editor panel.
    pub editor: Rect,
}

impl PlayRects {
    /// Lay the screen out.
    pub fn new(vp: &Viewport) -> PlayRects {
        let top = Rect::new(0, 0, vp.w, 18);
        if vp.portrait {
            let bh = vp.btn_h();
            let ch = bh * 2 + 8 + if vp.touch { 4 } else { 14 };
            let scene = Rect::new((vp.w - SCENE_W) / 2, 20, SCENE_W, SCENE_H);
            let controls = Rect::new(2, scene.bottom() + 2, vp.w - 4, ch);
            let editor = Rect::new(2, controls.bottom() + 2, vp.w - 4, vp.h - controls.bottom() - 4);
            PlayRects { top, scene, controls, editor }
        } else {
            let scene = Rect::new(2, 20, SCENE_W, SCENE_H);
            let controls = Rect::new(2, scene.bottom() + 3, SCENE_W, vp.h - scene.bottom() - 5);
            let editor = Rect::new(SCENE_W + 6, 20, vp.w - SCENE_W - 8, vp.h - 22);
            PlayRects { top, scene, controls, editor }
        }
    }
}

/// The outfit chosen in settings.
pub fn outfit_of(p: &algoclerk_core::progress::Progress, bank: &ArtBank) -> Outfit {
    Outfit::from_code(&p.settings.outfit).filter(|o| bank.has_template(&format!("clerk/body_{}", o.body))).unwrap_or_default()
}

/// A tap rectangle at least `min` big, kept inside `bounds`.
pub fn tap_rect(r: Rect, min: i32, bounds: Rect) -> Rect {
    let w = r.w.max(min);
    let h = r.h.max(min);
    let x = (r.x - (w - r.w) / 2).clamp(bounds.x, (bounds.right() - w).max(bounds.x));
    let y = (r.y - (h - r.h) / 2).clamp(bounds.y, (bounds.bottom() - h).max(bounds.y));
    Rect::new(x, y, w, h)
}

/// The memo panel rectangle.
pub fn memo_rect(vp: &Viewport) -> Rect {
    Memo::panel(vp)
}

struct Controls {
    reset: Rect,
    back: Rect,
    play: Rect,
    step: Rect,
    speed: Rect,
    check: Rect,
    cases: Vec<Rect>,
    status: Rect,
}

fn controls(vp: &Viewport, c: Rect, ncases: usize) -> Controls {
    let bh = vp.btn_h();
    let row = Rect::new(c.x, c.y, c.w, bh);
    let cols = crate::layout::columns(row, 6, 2);
    let cases_y = c.y + bh + 3;
    let chip_h = if vp.touch { 24 } else { 14 };
    let cw = if vp.touch { 40 } else { 34 };
    let cases = (0..ncases.min(5)).map(|i| Rect::new(c.x + i as i32 * (cw + 2), cases_y, cw, chip_h)).collect();
    let status = Rect::new(c.x, cases_y + chip_h + 3, c.w, (c.bottom() - cases_y - chip_h - 3).max(8));
    Controls { reset: cols[0], back: cols[1], play: cols[2], step: cols[3], speed: cols[4], check: cols[5], cases, status }
}

impl Play {
    /// Start a level; `brief` opens the memo first.
    ///
    /// # Errors
    /// The level could not be prepared (broken level file).
    pub fn level(cx: &mut Ctx<'_>, id: &str, brief: bool) -> Result<Play, String> {
        let level = cx.campaign.find(id).ok_or_else(|| format!("no level {id}"))?.clone();
        let rec = cx.progress.record(id);
        let session = Session::for_level(&level, &rec.draft)?;
        let outfit = outfit_of(cx.progress, cx.bank);
        cx.progress.last_level = id.to_string();
        cx.touch_progress();
        let mut p = Play::from_session(cx, session, &outfit);
        p.shown_hints = usize::from(rec.hints_used).min(level.hints.len());
        if brief {
            p.memo = Some(Memo::default());
            p.memo_starting = true;
        }
        Ok(p)
    }

    /// The sandbox.
    pub fn sandbox(cx: &mut Ctx<'_>) -> Play {
        let session = Session::sandbox(&cx.progress.sandbox);
        let outfit = outfit_of(cx.progress, cx.bank);
        Play::from_session(cx, session, &outfit)
    }

    fn from_session(cx: &Ctx<'_>, session: Session, outfit: &Outfit) -> Play {
        let mut office = Office::new(cx.bank, outfit);
        office.reduce_motion = cx.progress.settings.reduce_motion;
        Play {
            session,
            office,
            panel: EditorPanel::default(),
            playing: false,
            timer: 0,
            memo: None,
            modal: None,
            improved: false,
            solved: false,
            status: None,
            memo_starting: false,
            shown_hints: 0,
        }
    }

    /// Hit rectangles for layout tests.
    pub fn hits(&self, vp: &Viewport) -> Vec<Hit> {
        let r = PlayRects::new(vp);
        let c = controls(vp, r.controls, self.session.case_count());
        let mut v = vec![Hit::area("scene", r.scene), Hit::area("controls", r.controls), Hit::tap("reset", c.reset), Hit::tap("back", c.back), Hit::tap("play", c.play), Hit::tap("step", c.step), Hit::tap("speed", c.speed), Hit::tap("check", c.check)];
        v.extend(self.panel.hits(vp, r.editor));
        v.push(Hit::tap("brief", Rect::new(vp.w - 50, 0, 50, 18)));
        v
    }

    fn speed_level(cx: &Ctx<'_>) -> usize {
        usize::from(cx.progress.settings.speed_level.min(4))
    }

    /// Jump to a recorded step and let the clerk arrive (screenshots and tests).
    pub fn jump(&mut self, pos: usize) {
        if self.ensure_run() {
            if let Some(r) = &mut self.session.run {
                r.pos = pos.min(r.frames.len().saturating_sub(1));
            }
            self.show();
            self.office.settle();
        }
    }

    /// Reveal this many hints (screenshots).
    pub fn show_hints(&mut self, n: usize) {
        self.shown_hints = n;
    }

    /// Run to the end instantly (screenshots and tests).
    pub fn run_to_end(&mut self) {
        if self.ensure_run() {
            if let Some(run) = &mut self.session.run {
                run.pos = run.frames.len().saturating_sub(1);
            }
            self.show();
            self.office.settle();
            if let Some(r) = &self.session.run {
                if let Err(e) = &r.result {
                    let m = e.msg.clone();
                    self.status = Some((m.clone(), true));
                    self.office.oops(&m);
                }
            }
        }
    }

    /// Judge the program as if Check was pressed; marks the level solved when it passes.
    pub fn solve_now(&mut self) {
        if let Some(v) = self.session.check() {
            self.solved = v.passed;
        }
    }

    /// Open the card editor on the selected card.
    pub fn open_card_editor(&mut self) {
        let rows = self.session.rows();
        if let (Some(i), Some(p)) = (self.panel.sel, &self.session.good) {
            if let Some(row) = rows.get(i) {
                let mut found = None;
                for f in &p.fns {
                    algoclerk_core::lang::ast::walk_stmts(&f.body, &mut |s| {
                        if s.id == row.node {
                            found = Some(crate::blocks::header_text(s));
                        }
                    });
                }
                if let Some(t) = found {
                    self.panel.card = Some(crate::editor::CardEdit { node: row.node, line: crate::textedit::TextEdit::new(&t, false), pad: crate::editor::Pad::Names, last_var: None, msg: None });
                }
            }
        }
    }

    /// Load the level's reference solution into the editor.
    pub fn load_reference(&mut self) {
        if let Some(l) = &self.session.level {
            let t = l.reference.clone();
            self.session.set_text(&t);
        }
    }

    fn rewind(&mut self) {
        self.playing = false;
        self.timer = 0;
        if let Some(r) = &mut self.session.run {
            r.pos = 0;
        }
        self.office.clear();
        self.status = None;
    }

    fn show(&mut self) {
        let Some(r) = &self.session.run else { return };
        let Some(f) = r.frames.get(r.pos) else { return };
        let nparams = self.session.level.as_ref().map_or_else(|| self.session.good.as_ref().and_then(|p| p.fns.last().map(|f| f.params.len())).unwrap_or(0), |l| l.signature.params.len());
        let exp = (!r.expected.is_empty()).then_some((r.expected.as_str(), &r.ret));
        let got = match &r.result {
            Ok(t) => t.clone(),
            Err(_) => String::new(),
        };
        let nodes = r.nodes.clone();
        let f = f.clone();
        match exp {
            Some((text, ty)) => self.office.show(&f, &nodes, nparams, Some((text, ty, &nodes, &got))),
            None => self.office.show(&f, &nodes, nparams, None),
        }
    }

    fn ensure_run(&mut self) -> bool {
        if self.session.run.as_ref().is_some_and(|r| r.case == self.session.case) {
            return true;
        }
        match self.session.start_run() {
            Ok(()) => {
                self.office.clear();
                self.show();
                true
            }
            Err(msg) => {
                self.status = Some((msg.clone(), true));
                self.office.oops(&friendly(&msg));
                false
            }
        }
    }

    fn step_forward(&mut self, cx: &mut Ctx<'_>) {
        if !self.ensure_run() {
            return;
        }
        let Some(r) = &mut self.session.run else { return };
        if r.pos + 1 < r.frames.len() {
            r.pos += 1;
            self.show();
            cx.sfx("step");
        } else {
            self.finish_run(cx);
        }
    }

    fn finish_run(&mut self, cx: &mut Ctx<'_>) {
        self.playing = false;
        let Some(r) = &self.session.run else { return };
        match &r.result {
            Err(e) => {
                let msg = e.msg.clone();
                self.status = Some((msg.clone(), true));
                self.office.oops(&msg);
                cx.sfx("wrong");
            }
            Ok(out) => {
                let ok = r.expected.is_empty() || *out == r.expected;
                self.status = Some((if ok { format!("Returned {out}. It matches the expected answer.") } else { format!("Returned {out}, but {} was expected.", r.expected) }, !ok));
                if ok {
                    cx.sfx("correct");
                } else {
                    cx.sfx("wrong");
                    self.office.oops("That is not what the memo asked for.");
                }
            }
        }
    }

    fn save_draft(&self, cx: &mut Ctx<'_>) {
        match &self.session.level {
            Some(l) => {
                let id = l.id.clone();
                let rec = cx.progress.record_mut(&id);
                rec.draft = self.session.text();
                rec.hints_used = self.shown_hints as u8;
            }
            None => cx.progress.sandbox = self.session.text(),
        }
        cx.touch_progress();
    }

    /// Handle one tick of input.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let vp = cx.vp;
        let r = PlayRects::new(&vp);
        self.office.reduce_motion = cx.progress.settings.reduce_motion;
        let speed = Self::speed_level(cx);
        self.office.speed = [0.5, 0.8, 1.2, 2.0, 6.0][speed];
        self.office.tick();
        // overlays first
        if let Some(m) = &mut self.modal {
            match m.update(cx) {
                Answer::Yes => {
                    let purpose = m.purpose;
                    self.modal = None;
                    if purpose == Purpose::Reference {
                        if let Some(l) = &self.session.level {
                            let t = l.reference.clone();
                            self.session.set_text(&t);
                            self.save_draft(cx);
                        }
                    }
                }
                Answer::No => self.modal = None,
                Answer::Open => {}
            }
            return None;
        }
        if let Some(memo) = &mut self.memo {
            let nh = self.session.level.as_ref().map_or(0, |l| l.hints.len());
            match memo.update(cx, nh, self.shown_hints) {
                MemoOut::Close => {
                    self.memo = None;
                    self.memo_starting = false;
                    cx.sfx("button");
                }
                MemoOut::Hint => {
                    self.shown_hints += 1;
                    cx.sfx("pick");
                    self.save_draft(cx);
                }
                MemoOut::None => {}
            }
            return None;
        }
        // top bar
        let back = Rect::new(0, 0, 40.max(vp.hit_min()), 18);
        let brief = Rect::new(vp.w - 50, 0, 50, 18);
        if cx.ptr.tapped_in(&back) || (cx.input.key(Key::Escape) && self.panel.card.is_none() && self.panel.palette.is_none()) {
            self.save_draft(cx);
            return Some(if self.session.level.is_some() { Nav::Select } else { Nav::Title });
        }
        if cx.ptr.tapped_in(&brief) && self.session.level.is_some() {
            self.memo = Some(Memo::default());
            self.memo_starting = false;
        }
        // editor
        let out = self.panel.update(cx, &mut self.session, r.editor);
        if out.changed {
            self.playing = false;
            self.office.clear();
            self.status = None;
            self.save_draft(cx);
        } else if self.panel.tab == Tab::Text {
            // typing flows through update_text, which already synced
        }
        if self.session.sync() {
            self.playing = false;
            self.office.clear();
            self.save_draft(cx);
        }
        // controls
        let c = controls(&vp, r.controls, self.session.case_count());
        let typing = self.panel.tab == Tab::Text || self.panel.card.is_some();
        if cx.ptr.tapped_in(&c.reset) || (!typing && cx.input.ch('r')) {
            self.rewind();
            if self.ensure_run() {
                self.rewind();
                self.show();
            }
            cx.sfx("button");
        }
        if cx.ptr.tapped_in(&c.back) || (!typing && cx.input.key(Key::Left)) {
            if self.session.run.is_some() {
                self.playing = false;
                if let Some(run) = &mut self.session.run {
                    run.pos = run.pos.saturating_sub(1);
                }
                self.office.settle();
                self.show();
                cx.sfx("step");
            }
        }
        if cx.ptr.tapped_in(&c.step) || (!typing && cx.input.key(Key::Right)) {
            self.playing = false;
            self.step_forward(cx);
        }
        if cx.ptr.tapped_in(&c.play) || (!typing && cx.input.key(Key::Space)) {
            if self.ensure_run() {
                self.playing = !self.playing;
                if let Some(run) = &mut self.session.run {
                    if self.playing && run.pos + 1 >= run.frames.len() {
                        run.pos = 0;
                        self.office.clear();
                    }
                }
                cx.sfx("button");
                if Self::speed_level(cx) == 4 && self.playing {
                    if let Some(run) = &mut self.session.run {
                        run.pos = run.frames.len() - 1;
                    }
                    self.show();
                    self.office.settle();
                    self.finish_run(cx);
                }
            }
        }
        if cx.ptr.tapped_in(&c.speed) {
            let s = (Self::speed_level(cx) + 1) % 5;
            cx.progress.settings.speed_level = s as u8;
            cx.touch_progress();
            cx.events.push(GameEvent::SettingsChanged);
            cx.sfx("button");
        }
        for (i, cr) in c.cases.iter().enumerate() {
            if cx.ptr.tapped_in(cr) && self.session.case != i {
                self.session.case = i;
                self.rewind();
                self.session.run = None;
                if self.ensure_run() {
                    self.show();
                }
                cx.sfx("pick");
            }
        }
        if cx.ptr.tapped_in(&c.check) || (cx.input.key(Key::Enter) && cx.input.ctrl) {
            return self.check(cx);
        }
        // playback
        if self.playing {
            let ticks = STEP_TICKS[speed].max(1);
            self.timer += 1;
            if self.timer >= ticks && !self.office.busy() {
                self.timer = 0;
                self.step_forward(cx);
            }
        }
        None
    }

    fn check(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        self.playing = false;
        self.save_draft(cx);
        let Some(level) = self.session.level.clone() else {
            // sandbox: just run it
            if self.ensure_run() {
                if let Some(run) = &mut self.session.run {
                    run.pos = run.frames.len() - 1;
                }
                self.show();
                self.office.settle();
                self.finish_run(cx);
            }
            return None;
        };
        let verdict = self.session.check()?.clone();
        self.report(cx, &level, &verdict)
    }

    fn report(&mut self, cx: &mut Ctx<'_>, level: &algoclerk_core::level::Level, v: &Verdict) -> Option<Nav> {
        if v.passed {
            let text = self.session.text();
            self.improved = cx.progress.record_verdict(level, v, &text);
            self.solved = true;
            cx.touch_progress();
            cx.sfx("stars");
            return Some(Nav::Victory);
        }
        cx.sfx("wrong");
        let (msg, bubble) = if let Some(d) = v.diags.first() {
            (d.msg.clone(), friendly(&d.msg))
        } else if let Some(f) = &v.failure {
            let args = f.case.args.join(", ");
            let args = clip(&args, 60);
            match f.kind {
                FailKind::Wrong => (format!("For ({args}) I expected {} but got {}.", f.case.expect, f.got.clone().unwrap_or_default()), "My answer does not match the memo!".to_string()),
                FailKind::TimeLimit => (
                    format!("The clock ran out on {} test. {}", if f.case.large { "a big" } else { "a" }, level.technique_hint()),
                    "Too slow! The big job is still unfinished.".to_string(),
                ),
                FailKind::MemoryLimit => ("Too much shelf space used.".to_string(), "The shelves are full!".to_string()),
                FailKind::Error => {
                    let m = f.error.as_ref().map(|e| e.msg.clone()).unwrap_or_default();
                    (format!("For ({args}): {m}"), m)
                }
            }
        } else {
            ("Something is not right.".to_string(), "Hmm.".to_string())
        };
        self.status = Some((msg, true));
        self.office.oops(&bubble);
        None
    }

    /// Draw the screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let r = PlayRects::new(&vp);
        g.rect(vp.screen(), 'k');
        // top bar
        g.rect(r.top, 'K');
        let title = self.session.level.as_ref().map_or("Sandbox".to_string(), |l| l.title.clone());
        let back = Rect::new(0, 0, 40.max(vp.hit_min()), 18);
        g.icon("back", 4, 4, 'W');
        g.text(16, 5, "Back", 'W');
        let _ = back;
        g.text_c(vp.w / 2, 5, &clip(&title, ((vp.w - 110) / 6) as usize), 'y', 1);
        if self.session.level.is_some() {
            g.icon("memo", vp.w - 46, 4, 'y');
            g.text(vp.w - 34, 5, "Memo", 'W');
        }
        self.office.draw(g, (r.scene.x, r.scene.y));
        g.frame(r.scene.grow(1), 'K');
        // controls
        let c = controls(&vp, r.controls, self.session.case_count());
        let st = |rect: &Rect| widget_state(v.ptr, rect, false, false);
        let has_run = self.session.run.is_some();
        g.icon_button(c.reset, "reset", Kind::Normal, st(&c.reset));
        g.icon_button(c.back, "left", Kind::Normal, State { disabled: !has_run, ..st(&c.back) });
        g.icon_button(c.play, if self.playing { "pause" } else { "play" }, Kind::Primary, st(&c.play));
        g.icon_button(c.step, "step", Kind::Normal, st(&c.step));
        let sp = usize::from(v.progress.settings.speed_level.min(4));
        g.button(c.speed, ["x1", "x2", "x4", "x8", "max"][sp], Kind::Normal, st(&c.speed));
        let can_check = self.session.compiled.is_ok() || self.session.level.is_none();
        g.button(c.check, if self.session.level.is_some() { "Check" } else { "Run" }, Kind::Good, State { disabled: !can_check, ..st(&c.check) });
        for (i, cr) in c.cases.iter().enumerate() {
            g.button(*cr, &format!("Ex{}", i + 1), if self.session.case == i { Kind::On } else { Kind::Normal }, st(cr));
        }
        // status
        let sr = c.status;
        let (text, bad) = match (&self.status, self.session.problem()) {
            (Some((t, b)), _) => (t.clone(), *b),
            (None, Some(d)) => (d.msg, true),
            (None, None) => (self.hint_line(), false),
        };
        let lines = Font::wrap(&text, (sr.w / 6).max(8) as usize);
        let max_lines = (sr.h / 9).max(1) as usize;
        for (i, l) in lines.iter().take(max_lines).enumerate() {
            g.text(sr.x + 1, sr.y + i as i32 * 9, l, if bad { 'p' } else { 'A' });
        }
        // cost line next to the cases
        if let Some(run) = &self.session.run {
            let f = run.frames.get(run.pos);
            let label = format!("step {}/{}  cost {}", run.pos, run.frames.len().saturating_sub(1), f.map_or(0, |f| f.cost));
            g.text_r(r.controls.right() - 2, c.cases.first().map_or(sr.y - 12, |c| c.y + 4), &label, 'A', 1);
        }
        let exec = self.session.current_node();
        self.panel.draw(g, v, &self.session, r.editor, exec);
        if let Some(m) = &self.memo {
            if let Some(l) = &self.session.level {
                let rec = v.progress.record(&l.id);
                m.draw(g, v, l, &rec, self.shown_hints, self.memo_starting);
            }
        }
        if let Some(m) = &self.modal {
            m.draw(g, v);
        }
    }

    fn hint_line(&self) -> String {
        match &self.session.run {
            None => "Press play to watch the clerk run your program, or Check to judge it.".to_string(),
            Some(r) => match r.frames.get(r.pos) {
                Some(f) if f.node != 0 => "The highlighted card is the step the clerk just finished.".to_string(),
                _ => "The arguments are on the desk. Step through to follow the clerk.".to_string(),
            },
        }
    }
}

/// A short, friendly bubble for a problem message.
pub fn friendly(msg: &str) -> String {
    let first = msg.split(['.', '!']).next().unwrap_or(msg);
    let t: String = first.chars().take(60).collect();
    format!("{t}!")
}

trait TechniqueHint {
    fn technique_hint(&self) -> String;
}

impl TechniqueHint for algoclerk_core::level::Level {
    fn technique_hint(&self) -> String {
        format!("The memo hints at: {}.", self.technique)
    }
}
