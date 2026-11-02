// SPDX-License-Identifier: Apache-2.0
//! The play screen: the office scene, run controls, status line, the program editor (slips, palette, text
//! mode with an on-screen keyboard) and the memo overlay. Works in landscape and portrait.

use crate::brief::{Memo, MemoAction};
use crate::editor::{Editor, EditorRects};
use crate::gfx::{Gfx, Kind, State, clip};
use crate::input::Key;
use crate::layout::{Hit, Viewport};
use crate::modal::{Answer, Modal, Purpose};
use crate::office::{SCENE_H, SCENE_W};
use crate::session::Session;
use crate::textedit::{Keyboard, TextEdit, asm_spans, draw_editor};
use crate::ui::{Ctx, GameEvent, Nav, View, widget_state};
use algoclerk_core::asm;
use algoclerk_core::judge::Outcome;
use algoclerk_media::{ArtBank, Outfit, Rect};

/// Which editing mode the side panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Slips and palette.
    Program,
    /// Plain assembly text.
    Text,
}

/// All rectangles of the play screen.
#[derive(Debug, Clone)]
pub struct PlayRects {
    /// Top bar.
    pub top: Rect,
    /// Home button.
    pub home: Rect,
    /// Memo button.
    pub memo: Rect,
    /// Code peek button.
    pub code: Rect,
    /// Title text area.
    pub title: Rect,
    /// Office scene.
    pub scene: Rect,
    /// Back.
    pub back: Rect,
    /// Run / pause.
    pub run: Rect,
    /// Step.
    pub step: Rect,
    /// Reset.
    pub reset: Rect,
    /// Speed.
    pub speed: Rect,
    /// Check.
    pub check: Rect,
    /// Status panel.
    pub status: Rect,
    /// Previous test.
    pub t_prev: Rect,
    /// Next test.
    pub t_next: Rect,
    /// Side panel (editor).
    pub side: Rect,
    /// Text editor area.
    pub text: Rect,
    /// On-screen keyboard.
    pub kbd: Option<Rect>,
    /// Sandbox inbox field.
    pub sb_inbox: Rect,
    /// Sandbox floor minus.
    pub sb_minus: Rect,
    /// Sandbox floor plus.
    pub sb_plus: Rect,
    /// Layout is the full-screen text-with-keyboard variant.
    pub typing: bool,
}

/// Expand a rectangle around its centre to at least `min` in both directions (touch targets), within `bounds`.
pub fn tap_rect(r: Rect, min: i32, bounds: Rect) -> Rect {
    let w = r.w.max(min);
    let h = r.h.max(min);
    let x = (r.x - (w - r.w) / 2).clamp(bounds.x, (bounds.right() - w).max(bounds.x));
    let y = (r.y - (h - r.h) / 2).clamp(bounds.y, (bounds.bottom() - h).max(bounds.y));
    Rect::new(x, y, w, h)
}

impl PlayRects {
    /// Lay out the screen.
    pub fn new(vp: &Viewport, mode: Mode, kbd_open: bool) -> PlayRects {
        let s = vp.safe;
        let bh = vp.btn_h();
        let top_h = if vp.portrait { 20 } else { 18 };
        let top = Rect::new(s.x, s.y, s.w, top_h);
        let home = Rect::new(s.x + 3, s.y + 2, 26, top_h - 4);
        let memo = Rect::new(s.right() - 29, s.y + 2, 26, top_h - 4);
        let code = Rect::new(memo.x - 28, s.y + 2, 26, top_h - 4);
        let title = Rect::new(home.right() + 4, s.y, code.x - home.right() - 8, top_h);
        let typing = vp.touch && mode == Mode::Text && kbd_open;
        let scene_x = s.x + (if vp.portrait { (s.w - SCENE_W) / 2 } else { 1 });
        let scene = Rect::new(scene_x, s.y + top_h, SCENE_W, SCENE_H);
        let ctl_y = scene.bottom() + 3;
        let left_w = if vp.portrait { s.w } else { 272 };
        let widths: [i32; 6] = if vp.touch { [34, 42, 34, 34, 54, 52] } else { [28, 34, 28, 28, 42, 48] };
        let gap = 3;
        let total: i32 = widths.iter().sum::<i32>() + gap * 5;
        let mut x = s.x + (left_w - total.min(left_w)) / 2;
        let mut next = |w: i32| {
            let r = Rect::new(x, ctl_y, w, bh);
            x += w + gap;
            r
        };
        let back = next(widths[0]);
        let run = next(widths[1]);
        let step = next(widths[2]);
        let reset = next(widths[3]);
        let speed = next(widths[4]);
        let check = next(widths[5]);
        let status_y = ctl_y + bh + 3;
        let status = Rect::new(s.x + 2, status_y, left_w - 4, (s.bottom() - status_y - 2).min(if vp.portrait { 40 } else { 200 }));
        let tb = vp.hit_min().min(status.h).max(18);
        let t_next = Rect::new(status.right() - 4 - tb, status.y + (status.h - tb) / 2, tb, tb);
        let t_prev = Rect::new(t_next.x - tb - 2, t_next.y, tb, tb);
        let side = if vp.portrait {
            let y = status.bottom() + 2;
            Rect::new(s.x, y, s.w, s.bottom() - y)
        } else {
            Rect::new(s.x + 272, s.y + top_h, s.w - 272, s.h - top_h)
        };
        let kbd_h = if vp.portrait { 5 * 33 } else { 5 * 30 };
        let (text, kbd) = if typing {
            let kb = Rect::new(s.x, s.bottom() - kbd_h, s.w, kbd_h);
            (Rect::new(s.x + 2, s.y + top_h + bh + 4, s.w - 4, kb.y - (s.y + top_h + bh + 4) - 2), Some(kb))
        } else {
            let y = side.y + 4 + bh + 4;
            (Rect::new(side.x + 4, y, side.w - 8, side.bottom() - y - 4), None)
        };
        let sb_inbox = Rect::new(status.x + 4, status.y + 2, (status.w - 8) / 2, bh.min(20));
        let sb_minus = Rect::new(sb_inbox.right() + 10, status.y + 2, bh.min(20).max(vp.hit_min().min(20)), bh.min(20));
        let sb_plus = Rect::new(sb_minus.right() + 30, status.y + 2, sb_minus.w, sb_minus.h);
        PlayRects {
            top, home, memo, code, title, scene, back, run, step, reset, speed, check, status, t_prev, t_next, side, text, kbd,
            sb_inbox, sb_minus, sb_plus, typing,
        }
    }

    /// Hit rectangles for layout tests.
    pub fn hits(&self, vp: &Viewport, mode: Mode, palette_open: bool) -> Vec<Hit> {
        let b = vp.screen();
        let m = vp.hit_min();
        let mut v = vec![Hit::area("scene", self.scene), Hit::area("status", self.status), Hit::area("side", self.side)];
        if self.typing {
            v = vec![Hit::area("text", self.text)];
            if let Some(k) = self.kbd {
                v.push(Hit::area("keyboard", k));
            }
        }
        for (n, r) in [("home", self.home), ("memo", self.memo), ("code", self.code)] {
            v.push(Hit::tap(n, tap_rect(r, m, b)));
        }
        if !self.typing {
            for (n, r) in [("back", self.back), ("run", self.run), ("step", self.step), ("reset", self.reset), ("speed", self.speed), ("check", self.check)] {
                v.push(Hit::tap(n, r));
            }
            v.push(Hit::tap("test prev", self.t_prev));
            v.push(Hit::tap("test next", self.t_next));
            if mode == Mode::Program {
                v.extend(EditorRects::new(self.side, vp, palette_open, 15).hits(vp.touch));
            } else {
                v.push(Hit::area("text", self.text));
            }
        }
        v
    }
}

/// The play screen state.
#[derive(Debug)]
pub struct Play {
    /// The session (program, machine, office).
    pub st: Session,
    /// Program editor state.
    pub ed: Editor,
    /// Text editor.
    pub text: TextEdit,
    /// Sandbox inbox field.
    pub inbox_field: TextEdit,
    /// Keyboard.
    pub kbd: Keyboard,
    /// Keyboard is open (touch).
    pub kbd_open: bool,
    /// Mode of the side panel.
    pub mode: Mode,
    /// Memo overlay.
    pub memo: Option<Memo>,
    /// Confirmation dialog.
    pub modal: Option<Modal>,
    /// Toast text and ticks left.
    pub toast: Option<(String, u32)>,
    /// Level order number (for the memo title).
    pub order: usize,
    /// Pending navigation requested by a dialog.
    pending: Option<Nav>,
    /// The memo was opened as the level brief (primary button says "Start shift").
    pub intro: bool,
    /// The last Check improved the stored result.
    pub improved: bool,
}

impl Play {
    /// Start a level.
    ///
    /// # Errors
    /// A message when the level cannot start.
    pub fn level(cx: &mut Ctx<'_>, id: &str, show_memo: bool) -> Result<Play, String> {
        let level = cx.campaign.find(id).ok_or_else(|| format!("no level {id}"))?.clone();
        let rec = cx.progress.record(id);
        let outfit = outfit_of(cx.progress, cx.bank);
        let st = Session::for_level(&level, &rec.draft, rec.hints_used, cx.bank, &outfit, cx.progress.settings.speed_level)?;
        cx.progress.last_level = id.to_string();
        let order = cx.campaign.all_levels().position(|l| l.id == id).map_or(1, |i| i + 1);
        let mut p = Play::from_session(st, order);
        p.memo = show_memo.then(Memo::default);
        p.intro = show_memo;
        p.st.office.reduce_motion = cx.progress.settings.reduce_motion;
        Ok(p)
    }

    /// Start the sandbox.
    pub fn sandbox(cx: &mut Ctx<'_>) -> Play {
        let outfit = outfit_of(cx.progress, cx.bank);
        let st = Session::sandbox(&cx.progress.sandbox, cx.bank, &outfit, cx.progress.settings.speed_level);
        let mut p = Play::from_session(st, 0);
        p.mode = Mode::Text;
        p.text.set_text(&p.st.text);
        p.st.office.reduce_motion = cx.progress.settings.reduce_motion;
        p
    }

    fn from_session(st: Session, order: usize) -> Play {
        let text = TextEdit::new(&st.text, true);
        let inbox = TextEdit::new(&st.sandbox_input, false);
        Play {
            st,
            ed: Editor::default(),
            text,
            inbox_field: inbox,
            kbd: Keyboard::default(),
            kbd_open: false,
            mode: Mode::Program,
            memo: None,
            modal: None,
            toast: None,
            order,
            pending: None,
            intro: false,
            improved: false,
        }
    }

    fn rects(&self, vp: &Viewport) -> PlayRects {
        PlayRects::new(vp, self.mode, self.kbd_open)
    }

    /// Hit rectangles for layout tests.
    pub fn hits(&self, vp: &Viewport) -> Vec<Hit> {
        self.rects(vp).hits(vp, self.mode, self.ed.palette_open)
    }

    fn remember_draft(&self, cx: &mut Ctx<'_>) {
        if self.st.is_sandbox() {
            cx.progress.sandbox = self.st.text.clone();
        } else {
            cx.progress.record_mut(&self.st.id()).draft = self.st.text.clone();
        }
        cx.touch_progress();
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Ctx<'_>) {
        if self.mode == mode {
            return;
        }
        if mode == Mode::Text {
            self.text.set_text(&self.st.text);
        } else {
            self.kbd_open = false;
            self.text.focused = false;
            self.inbox_field.focused = false;
        }
        self.mode = mode;
        cx.sfx("sfx_button");
    }

    fn toast(&mut self, s: String) {
        self.toast = Some((s, 120));
    }

    /// Handle one tick.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let vp = cx.vp;
        let r = self.rects(&vp);
        self.st.office.reduce_motion = cx.progress.settings.reduce_motion;
        self.st.tick(cx.tr);
        if let Some((_, t)) = &mut self.toast {
            *t = t.saturating_sub(1);
            if *t == 0 {
                self.toast = None;
            }
        }
        if let Some(a) = self.pending.take() {
            return Some(a);
        }
        // dialogs first
        if let Some(m) = &mut self.modal {
            match m.update(cx) {
                Answer::Open => {}
                Answer::No => self.modal = None,
                Answer::Yes => {
                    let purpose = m.purpose;
                    self.modal = None;
                    return self.confirmed(cx, purpose);
                }
            }
            return None;
        }
        if self.memo.is_some() {
            return self.update_memo(cx);
        }
        let b = vp.screen();
        let min = vp.hit_min();
        let ptr = cx.ptr;
        // top bar
        if ptr.tapped_in(&tap_rect(r.home, min, b)) || (cx.input.key(Key::Escape) && !self.text.focused && !self.inbox_field.focused && !self.kbd_open) {
            cx.sfx("sfx_button");
            self.remember_draft(cx);
            return Some(if self.st.is_sandbox() { Nav::Title } else { Nav::Select });
        }
        if cx.input.key(Key::Escape) {
            self.text.focused = false;
            self.inbox_field.focused = false;
            self.kbd_open = false;
        }
        if ptr.tapped_in(&tap_rect(r.memo, min, b)) && !self.st.is_sandbox() || (cx.input.ch('b') && !self.typing_focus()) && !self.st.is_sandbox() {
            self.memo = Some(Memo::default());
            cx.sfx("sfx_button");
            return None;
        }
        if ptr.tapped_in(&tap_rect(r.code, min, b)) {
            cx.sfx("sfx_button");
            return self.peek_code(cx);
        }
        if r.typing {
            self.update_typing(cx, &r);
            return None;
        }
        // controls
        self.update_controls(cx, &r);
        // scene: tap a tile to set the operand of the selected instruction
        if ptr.tapped_in(&r.scene) && self.mode == Mode::Program && self.ed.wants_tile(&self.st) {
            if let Some((x, y)) = ptr.pos {
                if let Some(slot) = self.st.office.geom.slot_at(x - r.scene.x, y - r.scene.y) {
                    self.ed.set_slot(&mut self.st, slot);
                    cx.sfx("sfx_step");
                    self.after_program_change(cx);
                }
            }
        }
        self.st.office.pickable = self.mode == Mode::Program && self.ed.wants_tile(&self.st);
        self.st.office.operand_focus = None;
        // status: test selector / sandbox fields
        if self.st.is_sandbox() {
            self.update_sandbox(cx, &r);
        } else if self.st.tests.len() > 1 {
            if ptr.tapped_in(&r.t_prev) {
                self.st.cycle_test(-1);
            }
            if ptr.tapped_in(&r.t_next) {
                self.st.cycle_test(1);
            }
        }
        // the side panel
        match self.mode {
            Mode::Program => {
                let er = EditorRects::new(r.side, &vp, self.ed.palette_open, crate::editor::palette_items(&self.st).len());
                if ptr.tapped_in(&er.mode_text) {
                    self.set_mode(Mode::Text, cx);
                } else if self.ed.update(cx, &mut self.st, &er) {
                    self.after_program_change(cx);
                }
            }
            Mode::Text => self.update_text(cx, &r),
        }
        // shortcuts
        self.shortcuts(cx);
        None
    }

    fn typing_focus(&self) -> bool {
        self.text.focused || self.inbox_field.focused
    }

    fn after_program_change(&mut self, cx: &mut Ctx<'_>) {
        self.remember_draft(cx);
        if self.mode == Mode::Text {
            self.text.set_text(&self.st.text);
        }
    }

    fn shortcuts(&mut self, cx: &mut Ctx<'_>) {
        if self.typing_focus() || self.ed.pane_palette {
            return;
        }
        let i = cx.input;
        if i.key(Key::Space) || i.ch('r') && false {
            self.toggle_run(cx);
        }
        if i.ch('.') || i.ch('s') {
            self.do_step(cx);
        }
        if i.ch(',') || i.ch('z') {
            self.st.step_back();
            cx.sfx("sfx_step");
        }
        if i.ch('r') {
            self.st.reset_machine();
            cx.sfx("sfx_button");
        }
        for (k, lvl) in [('1', 0u8), ('2', 1), ('3', 2), ('4', 3), ('5', 4)] {
            if i.ch(k) {
                self.set_speed(cx, lvl);
            }
        }
        if i.ch('t') {
            let m = if self.mode == Mode::Program { Mode::Text } else { Mode::Program };
            self.set_mode(m, cx);
        }
    }

    fn set_speed(&mut self, cx: &mut Ctx<'_>, lvl: u8) {
        self.st.set_speed(lvl);
        cx.progress.settings.speed_level = lvl;
        cx.touch_progress();
    }

    fn toggle_run(&mut self, cx: &mut Ctx<'_>) {
        if !self.st.runnable() {
            let msg = self.problem_text(cx);
            self.toast(msg);
            cx.sfx("sfx_wrong");
            return;
        }
        self.st.toggle_run();
        cx.sfx("sfx_button");
    }

    fn do_step(&mut self, cx: &mut Ctx<'_>) {
        if !self.st.runnable() {
            let msg = self.problem_text(cx);
            self.toast(msg);
            cx.sfx("sfx_wrong");
            return;
        }
        self.st.running = false;
        self.st.step(cx.tr);
        cx.sfx("sfx_step");
    }

    fn problem_text(&self, cx: &Ctx<'_>) -> String {
        if let Some(p) = self.st.problems_text(cx.tr).first() {
            p.clone()
        } else if self.st.listing.instruction_count() == 0 {
            cx.tr.t("play.empty_program")
        } else {
            cx.tr.t("play.cannot_run")
        }
    }

    fn update_controls(&mut self, cx: &mut Ctx<'_>, r: &PlayRects) {
        let ptr = cx.ptr;
        if ptr.tapped_in(&r.run) {
            self.toggle_run(cx);
        }
        if ptr.tapped_in(&r.step) {
            self.do_step(cx);
        }
        if ptr.tapped_in(&r.back) {
            self.st.step_back();
            cx.sfx("sfx_step");
        }
        if ptr.tapped_in(&r.reset) {
            self.st.reset_machine();
            cx.sfx("sfx_button");
        }
        if ptr.tapped_in(&r.speed) {
            let next = (self.st.speed_level + 1) % 5;
            self.set_speed(cx, next);
            cx.sfx("sfx_button");
        }
        if ptr.tapped_in(&r.check) {
            if !self.st.runnable() {
                let msg = self.problem_text(cx);
                self.toast(msg);
                cx.sfx("sfx_wrong");
            } else if self.st.is_sandbox() {
                let t = self.st.text.clone();
                cx.events.push(GameEvent::Copy(t));
                self.toast(cx.tr.t("play.copied_solution"));
                cx.sfx("sfx_button");
            } else {
                self.check(cx);
            }
        }
    }

    /// Judge the program; on success remember the result and ask to show the victory screen.
    fn check(&mut self, cx: &mut Ctx<'_>) {
        let passed = self.st.check();
        let Some(level) = self.st.level.clone() else { return };
        let Some(verdict) = self.st.verdict.clone() else { return };
        if passed {
            let text = self.st.listing.to_text();
            self.improved = cx.progress.record_verdict(&level, &verdict, &text);
            cx.touch_progress();
            self.pending = Some(Nav::Victory);
        } else {
            cx.sfx("sfx_wrong");
            let msg = verdict.headline(cx.tr);
            self.st.message = Some(msg.clone());
            // show the first failing test in the office
            if let Some(f) = verdict.first_failure() {
                self.st.test_idx = f.index.min(self.st.tests.len().saturating_sub(1));
                self.st.reset_machine();
                self.st.message = Some(msg);
            }
        }
    }

    fn update_sandbox(&mut self, cx: &mut Ctx<'_>, r: &PlayRects) {
        let ptr = cx.ptr;
        let mut apply = false;
        if ptr.tapped_in(&r.sb_minus) && self.st.sandbox_floor > 1 {
            self.st.sandbox_floor -= 1;
            apply = true;
        }
        if ptr.tapped_in(&r.sb_plus) && self.st.sandbox_floor < crate::session::SANDBOX_MAX_FLOOR {
            self.st.sandbox_floor += 1;
            apply = true;
        }
        self.inbox_field.pointer(ptr, r.sb_inbox.inset(2), 1, 9);
        if ptr.pressed && ptr.over(&r.sb_inbox) {
            self.text.focused = false;
        }
        if self.inbox_field.focused {
            let out = self.inbox_field.handle(cx.input);
            if out.changed {
                self.st.sandbox_input = self.inbox_field.text();
                apply = true;
            }
            if cx.input.key(Key::Enter) {
                self.inbox_field.focused = false;
            }
        }
        if apply {
            let outfit = outfit_of(cx.progress, cx.bank);
            self.st.apply_sandbox_settings(cx.bank, &outfit);
        }
    }

    fn update_text(&mut self, cx: &mut Ctx<'_>, r: &PlayRects) {
        let ptr = cx.ptr;
        let vp = cx.vp;
        let er = EditorRects::new(r.side, &vp, false, 15);
        if ptr.tapped_in(&er.mode_list) {
            self.set_mode(Mode::Program, cx);
            return;
        }
        let scale = cx.text_scale();
        let line_h = 9 * scale + 2;
        let area = text_area(r, &vp);
        let was = self.text.focused;
        self.text.pointer(ptr, area, scale, line_h);
        if ptr.pressed && !ptr.over(&area) && !ptr.over(&r.sb_inbox) {
            self.text.focused = false;
        }
        if self.text.focused && !was && vp.touch {
            self.kbd_open = true;
        }
        if self.text.focused {
            self.inbox_field.focused = false;
            let out = self.text.handle(cx.input);
            self.apply_text_outcome(cx, out);
            let rows = (area.h / line_h).max(1) as usize;
            let cols = ((area.w - 8) / (6 * scale)).max(1) as usize;
            self.text.scroll_to_cursor(rows, cols);
            if ptr.over(&area) {
                self.text.scroll_by(-(ptr.wheel * 3.0) as i32, rows);
            }
        } else if ptr.over(&area) {
            let rows = (area.h / line_h).max(1) as usize;
            self.text.scroll_by(-(ptr.wheel * 3.0) as i32, rows);
        }
        // toolbar buttons of the text mode: tidy, copy, paste
        let [tidy, copy, paste] = text_buttons(&er, &vp);
        if ptr.tapped_in(&tidy) {
            self.text.set_text(&self.st.listing.to_text());
            cx.sfx("sfx_button");
        }
        if ptr.tapped_in(&copy) {
            cx.events.push(GameEvent::Copy(self.text.text()));
            self.toast(cx.tr.t("play.copied_solution"));
            cx.sfx("sfx_button");
        }
        if ptr.tapped_in(&paste) {
            cx.events.push(GameEvent::PasteRequest);
            self.text.focused = true;
        }
    }

    fn apply_text_outcome(&mut self, cx: &mut Ctx<'_>, out: crate::textedit::EditOutcome) {
        if let Some(c) = out.copy {
            cx.events.push(GameEvent::Copy(c));
        }
        if out.want_paste {
            cx.events.push(GameEvent::PasteRequest);
        }
        if out.changed {
            self.st.text = self.text.text();
            self.st.set_text(cx.tr);
            self.remember_draft(cx);
            self.ed.sel = None;
        }
    }

    fn update_typing(&mut self, cx: &mut Ctx<'_>, r: &PlayRects) {
        let ptr = cx.ptr;
        let scale = cx.text_scale();
        let line_h = 9 * scale + 2;
        self.text.pointer(ptr, r.text, scale, line_h);
        self.text.focused = true;
        let out = self.text.handle(cx.input);
        self.apply_text_outcome(cx, out);
        if let Some(k) = r.kbd {
            for a in self.kbd.update(ptr, k) {
                let mut input = crate::input::Input::default();
                match a {
                    crate::textedit::KbAction::Char(c) => input.typed = c.to_string(),
                    crate::textedit::KbAction::Backspace => input.keys.push(Key::Backspace),
                    crate::textedit::KbAction::Enter => input.keys.push(Key::Enter),
                    crate::textedit::KbAction::Hide => {
                        self.kbd_open = false;
                        continue;
                    }
                }
                let out = self.text.handle(&input);
                self.apply_text_outcome(cx, out);
                cx.sfx("sfx_step");
            }
        }
        // done button (hide keyboard): the home-like button in the toolbar row
        let done = typing_done_rect(r, &cx.vp);
        if ptr.tapped_in(&done) {
            self.kbd_open = false;
            self.text.focused = false;
        }
        let rows = (r.text.h / line_h).max(1) as usize;
        let cols = ((r.text.w - 8) / (6 * scale)).max(1) as usize;
        self.text.scroll_to_cursor(rows, cols);
    }

    fn update_memo(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let vp = cx.vp;
        let rect = memo_rect(&vp);
        let hints_total = self.st.level.as_ref().map_or(0, |l| l.hints.len());
        let can_reveal = self.st.level.as_ref().is_some_and(|l| l.solution.is_some());
        let hints_left = self.st.hints_shown < hints_total;
        let mut memo = self.memo.take().unwrap_or_default();
        let act = memo.update(cx, rect, hints_left, can_reveal);
        let esc = cx.input.key(Key::Escape);
        self.memo = Some(memo);
        match act {
            MemoAction::Primary => {
                self.memo = None;
                self.intro = false;
                cx.sfx("sfx_button");
            }
            MemoAction::Hint => {
                self.st.hints_shown += 1;
                let id = self.st.id();
                cx.progress.record_mut(&id).hints_used = self.st.hints_shown.min(255) as u8;
                cx.touch_progress();
                cx.sfx("sfx_button");
            }
            MemoAction::Reveal => {
                self.modal = Some(Modal {
                    purpose: Purpose::Reference,
                    title: cx.tr.t("brief.reference"),
                    body: cx.tr.t("brief.reference_warning"),
                    yes: cx.tr.t("brief.reference_yes"),
                    no: cx.tr.t("common.cancel"),
                    focus_yes: false,
                });
            }
            MemoAction::None => {}
        }
        if esc || (cx.ptr.pressed && !cx.ptr.over(&rect)) {
            self.memo = None;
        }
        None
    }

    fn peek_code(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        if self.st.program.is_none() || !self.st.problems.is_empty() {
            let msg = cx.tr.t("result.no_code");
            self.toast(msg);
            return None;
        }
        let solved = self.st.level.as_ref().is_none_or(|l| cx.progress.record(&l.id).solved);
        if solved {
            return Some(Nav::Code);
        }
        self.modal = Some(Modal {
            purpose: Purpose::PeekCode,
            title: cx.tr.t("play.peek"),
            body: cx.tr.t("play.peek_warning"),
            yes: cx.tr.t("play.peek_yes"),
            no: cx.tr.t("common.cancel"),
            focus_yes: false,
        });
        None
    }

    fn confirmed(&mut self, cx: &mut Ctx<'_>, purpose: Purpose) -> Option<Nav> {
        match purpose {
            Purpose::Reference => {
                let text = self.st.level.as_ref().and_then(|l| l.solution.as_ref()).map(|s| s.asm.clone())?;
                if let Ok(l) = asm::parse(&text) {
                    self.st.listing = l;
                    self.st.after_edit(true);
                    let id = self.st.id();
                    cx.progress.record_mut(&id).reference_revealed = true;
                    self.remember_draft(cx);
                    self.text.set_text(&self.st.text);
                    self.ed.sel = None;
                    self.memo = None;
                }
                None
            }
            Purpose::Reset => None,
            Purpose::PeekCode => {
                let id = self.st.id();
                cx.progress.record_mut(&id).code_revealed = true;
                cx.touch_progress();
                Some(Nav::Code)
            }
        }
    }

    // ------------------------------------------------------------------ drawing

    /// Draw the screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let r = self.rects(&vp);
        g.rect(vp.screen(), 'k');
        self.draw_top(g, v, &r);
        if r.typing {
            self.draw_typing(g, v, &r);
        } else {
            self.st.office.draw(g, (r.scene.x, r.scene.y));
            g.frame(r.scene.grow(1), 'K');
            self.draw_controls(g, v, &r);
            self.draw_status(g, v, &r);
            match self.mode {
                Mode::Program => {
                    let er = EditorRects::new(r.side, &vp, self.ed.palette_open, crate::editor::palette_items(&self.st).len());
                    self.ed.draw(g, v, &self.st, &er, false);
                }
                Mode::Text => self.draw_text_panel(g, v, &r),
            }
        }
        if let Some((t, _)) = &self.toast {
            let cols = ((vp.safe.w - 40) / 6) as usize;
            let lines = algoclerk_media::Font::wrap(t, cols);
            let h = lines.len() as i32 * 10 + 8;
            let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32 * 6 + 12;
            let x = vp.safe.x + (vp.safe.w - w) / 2;
            let y = r.scene.bottom() - h - 4;
            g.panel_with(Rect::new(x, y, w, h), 'y', 'K', true);
            for (i, l) in lines.iter().enumerate() {
                g.text(x + 6, y + 4 + i as i32 * 10, l, 'K');
            }
        }
        if let Some(m) = &self.memo {
            g.shade(vp.screen());
            let mut mm = m.clone();
            let level = self.st.level.as_ref();
            if let Some(level) = level {
                let rec = v.progress.record(&level.id);
                let label = if self.intro { v.tr.t("brief.start") } else { v.tr.t("brief.close") };
                mm.draw(g, v, memo_rect(&vp), level, self.order, self.st.hints_shown, &label, &rec);
            }
        }
        if let Some(m) = &self.modal {
            m.draw(g, v);
        }
    }

    fn draw_top(&self, g: &mut Gfx<'_>, v: &View<'_>, r: &PlayRects) {
        let ptr = v.ptr;
        g.rect(r.top, 'K');
        g.hline(r.top.x, r.top.bottom() - 1, r.top.w, 'm');
        g.icon_button(r.home, "home", Kind::Normal, widget_state(ptr, &tap_rect(r.home, v.vp.hit_min(), v.vp.screen()), false, false));
        if !self.st.is_sandbox() {
            g.icon_button(r.memo, "memo", Kind::Normal, widget_state(ptr, &tap_rect(r.memo, v.vp.hit_min(), v.vp.screen()), false, false));
        }
        g.icon_button(r.code, "code", Kind::Normal, widget_state(ptr, &tap_rect(r.code, v.vp.hit_min(), v.vp.screen()), false, false));
        let title = match &self.st.level {
            Some(l) => format!("{:02} {}", self.order, l.title),
            None => v.tr.t("sandbox.title"),
        };
        let max = (r.title.w / 6).max(4) as usize;
        let ty = r.title.y + (r.title.h - 7) / 2;
        g.text(r.title.x, ty, &clip(&title, max), 'W');
        // size and steps
        if !v.vp.portrait {
            let size = self.st.listing.instruction_count();
            let steps = self.st.machine.as_ref().map_or(0, |m| m.steps);
            let s = format!("{} {}  {} {}", v.tr.t("editor.size"), size, v.tr.t("play.steps"), steps);
            let x = r.title.right() - g.text_w(&s, 1);
            if x > r.title.x + 6 * (title.chars().count() as i32 + 2) {
                g.text(x, ty, &s, 'y');
            }
        }
    }

    fn draw_controls(&self, g: &mut Gfx<'_>, v: &View<'_>, r: &PlayRects) {
        let ptr = v.ptr;
        let runnable = self.st.runnable();
        let done = self.st.machine.as_ref().is_some_and(algoclerk_core::vm::Machine::is_done);
        let can_back = self.st.machine.as_ref().is_some_and(algoclerk_core::vm::Machine::can_step_back);
        g.icon_button(r.back, "back", Kind::Normal, widget_state(ptr, &r.back, false, !can_back));
        let run_icon = if self.st.running { "pause" } else { "play" };
        g.icon_button(r.run, run_icon, if self.st.running { Kind::On } else { Kind::Good }, widget_state(ptr, &r.run, false, !runnable));
        g.icon_button(r.step, "step", Kind::Normal, widget_state(ptr, &r.step, false, !runnable || (done && false)));
        g.icon_button(r.reset, "reset", Kind::Normal, widget_state(ptr, &r.reset, false, false));
        let label = ["1x", "2x", "4x", "8x", "MAX"][usize::from(self.st.speed_level.min(4))];
        let sb = g.button_box(r.speed, Kind::Normal, widget_state(ptr, &r.speed, false, false));
        g.icon("fast", sb.x + 3, sb.y + (sb.h - 10) / 2, 'K');
        g.text(sb.x + 15, sb.y + (sb.h - 7) / 2, label, 'K');
        let (label, kind) = if self.st.is_sandbox() {
            (v.tr.t("play.share_short"), Kind::Normal)
        } else {
            (v.tr.t("play.check"), Kind::Primary)
        };
        g.button(r.check, &label, kind, widget_state(ptr, &r.check, false, !runnable));
    }

    fn draw_status(&self, g: &mut Gfx<'_>, v: &View<'_>, r: &PlayRects) {
        let st = &self.st;
        g.panel(r.status, 'W');
        let inner = Rect::new(r.status.x + 5, r.status.y + 3, r.status.w - 10, r.status.h - 6);
        if st.is_sandbox() {
            self.draw_sandbox_fields(g, v, r);
            return;
        }
        let ptr = v.ptr;
        let multi = st.tests.len() > 1;
        let text_w = inner.w - if multi { r.t_prev.w * 2 + 10 } else { 0 };
        let cols = (text_w / 6).max(8) as usize;
        let (msg, ink) = if let Some(p) = st.problems_text(v.tr).first() {
            (p.clone(), 'r')
        } else if let Some(e) = st.text_errors.first() {
            (e.clone(), 'r')
        } else if let Some(m) = &st.message {
            (m.clone(), if st.finished_ok == Some(true) { 'G' } else { 'r' })
        } else if st.machine.as_ref().is_some_and(|m| m.steps > 0) {
            (v.tr.t("play.running"), 'k')
        } else {
            (v.tr.t("play.ready"), 'k')
        };
        let lines = algoclerk_media::Font::wrap(&msg, cols);
        let max_lines = ((inner.h - 12) / 9).max(1) as usize;
        for (i, l) in lines.iter().take(max_lines).enumerate() {
            let text = if i + 1 == max_lines && lines.len() > max_lines { clip(l, cols) } else { l.clone() };
            g.text(inner.x, inner.y + i as i32 * 9, &text, ink);
        }
        // verdict chips
        let cy = inner.bottom() - 8;
        if let Some(verdict) = &st.verdict {
            for (i, t) in verdict.tests.iter().take(((text_w - 2) / 11).max(1) as usize).enumerate() {
                let ok = t.outcome == Outcome::Pass;
                g.sprite(if ok { "icons/stamp_ok" } else { "icons/stamp_bad" }, 0, inner.x + i as i32 * 11, cy - 1);
                if i == st.test_idx {
                    g.hline(inner.x + i as i32 * 11, cy + 9, 9, 'K');
                }
            }
        } else if multi {
            for i in 0..st.tests.len().min(((text_w - 2) / 11).max(1) as usize) {
                let on = i == st.test_idx;
                let c = Rect::new(inner.x + i as i32 * 11, cy, 9, 7);
                g.round_rect(c, if on { 'y' } else { 'w' }, 'K');
            }
        }
        if multi {
            g.icon_button(r.t_prev, "left", Kind::Normal, widget_state(ptr, &r.t_prev, false, false));
            g.icon_button(r.t_next, "right", Kind::Normal, widget_state(ptr, &r.t_next, false, false));
            let label = format!("{}/{}", st.test_idx + 1, st.tests.len());
            g.text_r(r.t_prev.x - 4, r.t_prev.y + (r.t_prev.h - 7) / 2, &label, 'k', 1);
        }
    }

    fn draw_sandbox_fields(&self, g: &mut Gfx<'_>, v: &View<'_>, r: &PlayRects) {
        let ptr = v.ptr;
        g.text(r.sb_inbox.x, r.sb_inbox.y - 0, &v.tr.t("sandbox.inbox_short"), 'k');
        let field = Rect::new(r.sb_inbox.x, r.sb_inbox.y + 9, r.sb_inbox.w, r.sb_inbox.h.min(16));
        let focus = self.inbox_field.focused;
        g.rect(field, 'W');
        g.frame(field, if focus { 'B' } else { 'K' });
        g.text(field.x + 3, field.y + (field.h - 7) / 2, &clip(&self.inbox_field.text(), ((field.w - 6) / 6) as usize), 'K');
        if focus && (v.tick / 30) % 2 == 0 {
            let x = field.x + 3 + (self.inbox_field.cursor.min(((field.w - 8) / 6) as usize) as i32) * 6;
            g.rect(Rect::new(x, field.y + 2, 1, field.h - 4), 'R');
        }
        g.text(r.sb_minus.x - 2, r.sb_minus.y, &v.tr.t("sandbox.tiles"), 'k');
        let (m, p) = (Rect::new(r.sb_minus.x, r.sb_minus.y + 9, r.sb_minus.w, field.h), Rect::new(r.sb_plus.x, r.sb_minus.y + 9, r.sb_plus.w, field.h));
        g.icon_button(m, "minus", Kind::Normal, widget_state(ptr, &m, false, false));
        g.icon_button(p, "plus", Kind::Normal, widget_state(ptr, &p, false, false));
        g.text_in(Rect::new(m.right(), m.y, p.x - m.right(), m.h), &self.st.sandbox_floor.to_string(), 'K', 1);
    }

    fn draw_text_panel(&self, g: &mut Gfx<'_>, v: &View<'_>, r: &PlayRects) {
        let vp = v.vp;
        let er = EditorRects::new(r.side, &vp, false, 15);
        let ptr = v.ptr;
        g.panel(r.side, 'w');
        g.icon_button(er.mode_list, "list", Kind::Normal, widget_state(ptr, &er.mode_list, false, false));
        g.icon_button(er.mode_text, "text", Kind::On, widget_state(ptr, &er.mode_text, false, false));
        let [tidy, copy, paste] = text_buttons(&er, &vp);
        g.icon_button(tidy, "list", Kind::Normal, widget_state(ptr, &tidy, false, false));
        g.icon_button(copy, "copy", Kind::Normal, widget_state(ptr, &copy, false, false));
        g.icon_button(paste, "note", Kind::Normal, widget_state(ptr, &paste, false, false));
        let scale = v.text_scale();
        let line_h = 9 * scale + 2;
        let area = text_area(r, &vp);
        let errs: Vec<usize> = self.st.text_errors.iter().filter_map(|e| error_line_of(e)).collect();
        let exec = self.st.executing_line().and_then(|i| self.st.program.as_ref().and_then(|p| p.text_line.get(self.exec_instr()).copied().filter(|_| i < usize::MAX)));
        draw_editor(g, &self.text, area, scale, line_h, &errs, exec, v.tick);
        if let Some(e) = self.st.text_errors.first() {
            let y = area.bottom() + 2;
            let cols = (r.side.w / 6 - 3) as usize;
            if y + 9 < r.side.bottom() {
                g.text(r.side.x + 6, y, &clip(e, cols), 'r');
            }
        }
        let _ = asm_spans;
    }

    fn exec_instr(&self) -> usize {
        self.st.machine.as_ref().map_or(0, |m| m.pc)
    }

    fn draw_typing(&self, g: &mut Gfx<'_>, v: &View<'_>, r: &PlayRects) {
        let vp = v.vp;
        let ptr = v.ptr;
        let scale = v.text_scale();
        let line_h = 9 * scale + 2;
        let done = typing_done_rect(r, &vp);
        g.button(done, &v.tr.t("common.done"), Kind::Good, widget_state(ptr, &done, false, false));
        let errs: Vec<usize> = self.st.text_errors.iter().filter_map(|e| error_line_of(e)).collect();
        draw_editor(g, &self.text, r.text, scale, line_h, &errs, None, v.tick);
        if let Some(e) = self.st.text_errors.first() {
            g.text(r.text.x + 4, r.text.bottom() - 10, &clip(e, ((r.text.w - 8) / 6) as usize), 'r');
        }
        if let Some(k) = r.kbd {
            self.kbd.draw(g, k, ptr);
        }
    }
}

fn typing_done_rect(r: &PlayRects, vp: &Viewport) -> Rect {
    let bh = vp.btn_h();
    Rect::new(r.top.right() - 80 - 4, r.top.bottom() + 2, 80, bh)
}

fn text_buttons(er: &EditorRects, vp: &Viewport) -> [Rect; 3] {
    let bh = vp.btn_h();
    let mut x = er.panel.right() - 4 - bh;
    let paste = Rect::new(x, er.mode_list.y, bh, bh);
    x -= bh + 2;
    let copy = Rect::new(x, er.mode_list.y, bh, bh);
    x -= bh + 2;
    let tidy = Rect::new(x, er.mode_list.y, bh, bh);
    [tidy, copy, paste]
}

fn text_area(r: &PlayRects, vp: &Viewport) -> Rect {
    let _ = vp;
    r.text
}

fn error_line_of(e: &str) -> Option<usize> {
    let rest = e.strip_prefix("Line ")?;
    rest.split(':').next()?.trim().parse().ok()
}

/// The memo overlay rectangle.
pub fn memo_rect(vp: &Viewport) -> Rect {
    if vp.portrait {
        Rect::new(vp.safe.x + 8, vp.safe.y + 14, vp.safe.w - 16, vp.safe.h - 28)
    } else {
        Rect::new(vp.safe.x + 40, vp.safe.y + 10, vp.safe.w - 80, vp.safe.h - 20)
    }
}

/// The outfit chosen in settings (the default one when unset or invalid).
pub fn outfit_of(p: &algoclerk_core::progress::Progress, bank: &ArtBank) -> Outfit {
    Outfit::from_code(&p.settings.outfit)
        .filter(|o| bank.has_template(&format!("clerk/body_{}", o.body)))
        .unwrap_or_default()
}

/// Unused state accessor kept for tests.
pub fn state_of(_s: State) {}
