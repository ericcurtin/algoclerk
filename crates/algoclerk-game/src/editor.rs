// SPDX-License-Identifier: Apache-2.0
//! The program editor panel: nested paper cards (the block view), the text view with error squiggles and
//! an on-screen keyboard, a chip pad for building cards without typing, and undo/redo.

use crate::blocks::{self, CardKind, PALETTE};
use crate::cards;
use crate::complete::{self, Item};
use crate::gfx::{Gfx, Kind, State, clip};
use crate::input::Key;
use crate::layout::Hit;
use crate::session::Session;
use crate::textedit::{self, KbAction, Keyboard, TextEdit};
use crate::ui::{Ctx, View, widget_state};
use crate::words;
use algoclerk_core::cpp::ast::Ty;
use algoclerk_media::Rect;

/// Which view of the program is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Cards.
    Blocks,
    /// Text.
    Text,
}

/// Chip pad pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pad {
    /// Variables in scope.
    Names,
    /// Digits and literals.
    Numbers,
    /// Operators and punctuation.
    Ops,
    /// Member functions of the value left of the cursor.
    Members,
    /// `std::` types and functions in reach.
    Std,
    /// Keywords.
    Words,
}

/// The card being edited.
#[derive(Debug, Clone)]
pub struct CardEdit {
    /// Statement id being edited.
    pub node: u32,
    /// The line.
    pub line: TextEdit,
    /// Active chip page.
    pub pad: Pad,
    /// Type of the last variable chip tapped (filters the tool chips).
    pub last_var: Option<Ty>,
    /// Message shown under the line.
    pub msg: Option<String>,
    /// Scroll of the chip area in pixels.
    pub scroll: i32,
}

/// The autocomplete popup of the text view.
#[derive(Debug, Clone, Default)]
pub struct Popup {
    /// Suggestions.
    pub items: Vec<Item>,
    /// Selected suggestion.
    pub sel: usize,
    /// Character index where the typed prefix starts.
    pub start: usize,
    /// First visible suggestion.
    pub top: usize,
}

/// Rows the popup shows at once.
const POPUP_ROWS: usize = 5;

/// The editor panel state.
#[derive(Debug, Clone)]
pub struct EditorPanel {
    /// Current tab.
    pub tab: Tab,
    /// Selected card row.
    pub sel: Option<usize>,
    /// Scroll offset of the card list in pixels.
    pub scroll: i32,
    /// On-screen keyboard.
    pub kb: Keyboard,
    /// Keyboard visible (Text tab).
    pub kb_open: bool,
    /// Card editor overlay.
    pub card: Option<CardEdit>,
    /// Palette overlay: `(inside)`.
    pub palette: Option<bool>,
    drag: Option<(usize, i32)>,
    drop: Option<cards::Drop>,
    /// A one-line hint shown at the bottom.
    pub note: Option<String>,
    /// Autocomplete popup of the text view.
    pub ac: Option<Popup>,
    /// The popup was dismissed for this text and cursor.
    dismissed: Option<(String, usize)>,
    /// Typing an opening bracket or quote adds the closing one.
    pub auto_close: bool,
}

impl Default for EditorPanel {
    fn default() -> Self {
        EditorPanel {
            tab: Tab::Blocks,
            sel: None,
            scroll: 0,
            kb: Keyboard::default(),
            kb_open: false,
            card: None,
            palette: None,
            drag: None,
            drop: None,
            note: None,
            ac: None,
            dismissed: None,
            auto_close: false,
        }
    }
}

/// What the panel changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Out {
    /// The program text changed.
    pub changed: bool,
}

struct Parts {
    tabs: Rect,
    strip: Rect,
    body: Rect,
    /// The compiler message under the text (text tab).
    msg: Rect,
    /// Action bar (cards) or tool row (text).
    bar: Rect,
    kb: Rect,
}

/// Lines of the note shown under the code, at most six (the clerk's messages are shorter).
const MSG_MAX_LINES: usize = 6;

fn parts(
    vp: &crate::layout::Viewport,
    area: Rect,
    kb_open: bool,
    has_bar: bool,
    tab: Tab,
    has_strip: bool,
    msg_lines: usize,
) -> Parts {
    let th = vp.btn_h();
    let tabs = Rect::new(area.x, area.y, area.w, th);
    let text = tab == Tab::Text;
    let kb_h = if kb_open && text {
        if vp.touch {
            124
        } else {
            (area.h * 2 / 5).clamp(78, 104)
        }
    } else {
        0
    };
    // with the keyboard open its keyword strip holds the tools, so the tool row disappears
    let bar_h = if text {
        if kb_open { 0 } else { th + 2 }
    } else if has_bar {
        th + 2
    } else {
        0
    };
    // the text view always keeps its note line; the cards view only has one while there is something to say
    let msg_h = if text || msg_lines > 0 {
        msg_lines.clamp(1, MSG_MAX_LINES) as i32 * 9 + 3
    } else {
        0
    };
    let below = |h: i32| if h > 0 { h + 1 } else { 0 };
    let tail = below(msg_h) + below(bar_h) + below(kb_h);
    // the prelude strip only fits when the text keeps at least a few lines
    let room = area.h - th - 2 - tail;
    let strip_h = if has_strip && text && room >= 50 {
        10
    } else {
        0
    };
    let strip = Rect::new(area.x, area.y + th + 2, area.w, strip_h);
    let body = Rect::new(
        area.x,
        area.y + th + 2 + strip_h,
        area.w,
        (area.h - th - 2 - strip_h - tail).max(20),
    );
    let msg = Rect::new(area.x, body.bottom() + 1, area.w, msg_h);
    let bar = Rect::new(area.x, msg.y + below(msg_h), area.w, bar_h);
    let kb = Rect::new(area.x, bar.y + below(bar_h), area.w, kb_h.max(0));
    Parts {
        tabs,
        strip,
        body,
        msg,
        bar,
        kb,
    }
}

fn tab_rects(r: Rect, n: i32) -> Vec<Rect> {
    crate::layout::columns(r, n, 2)
}

fn fresh_name(text: &str) -> String {
    (1..)
        .map(|i| format!("v{i}"))
        .find(|n| !text.contains(n.as_str()))
        .unwrap_or_else(|| "v1".into())
}

impl EditorPanel {
    /// Hit rectangles for layout tests.
    pub fn hits(&self, vp: &crate::layout::Viewport, area: Rect) -> Vec<Hit> {
        let p = parts(vp, area, self.kb_open, true, self.tab, false, 1);
        let mut v = vec![Hit::area("editor", area), Hit::area("editor body", p.body)];
        for (i, r) in tab_rects(p.tabs, 4).into_iter().enumerate() {
            v.push(Hit::tap(["tab blocks", "tab text", "undo", "redo"][i], r));
        }
        v
    }

    fn rows_rect(&self, body: Rect) -> Rect {
        body
    }

    /// The note under the code: its text and whether it is an error (red) or a friendly nudge (yellow).
    /// The text view also shows the error of a recorded run; the cards only show compile problems.
    pub fn note_of(&self, s: &Session) -> Option<(String, bool)> {
        let m = match self.tab {
            Tab::Text => s.problem_message(),
            Tab::Blocks => s.problem().map(|d| d.msg),
        };
        match m {
            Some(m) => Some((m, true)),
            None if s.nudge() => Some((crate::words::NUDGE.to_string(), false)),
            None => None,
        }
    }

    /// The error text the note under the code shows right now (so the status line need not repeat it).
    pub fn shown_error(&self, s: &Session) -> Option<String> {
        self.note_of(s).filter(|n| n.1).map(|n| n.0)
    }

    /// Lines the note under the code needs at width `w` (zero for none in the cards view).
    fn note_lines(&self, s: &Session, w: i32) -> usize {
        let n = self.note_of(s).map_or(0, |(m, _)| {
            algoclerk_media::Font::wrap(&m, ((w - 6) / 6).max(8) as usize).len()
        });
        if self.tab == Tab::Text { n.max(1) } else { n }
    }

    /// The rectangle of the note under the code for a panel placed at `area`, and how many text lines the note
    /// needs there (the rectangle is always tall enough for them, up to the cap).
    pub fn note_layout(
        &self,
        vp: &crate::layout::Viewport,
        area: Rect,
        s: &Session,
    ) -> (Rect, usize) {
        let inner = area.inset(2);
        let n = self.note_lines(s, inner.w);
        let p = parts(
            vp,
            inner,
            self.kb_open,
            true,
            self.tab,
            !s.prelude().trim().is_empty(),
            n,
        );
        (p.msg, n)
    }

    /// Draw the note under the code (all its lines).
    fn draw_note(&self, g: &mut Gfx<'_>, v: &View<'_>, s: &Session, r: Rect) {
        if r.h == 0 {
            return;
        }
        match self.note_of(s) {
            Some((m, bad)) => {
                g.rect(r, if bad { 'p' } else { 'y' });
                for (i, l) in algoclerk_media::Font::wrap(&m, ((r.w - 6) / 6).max(8) as usize)
                    .iter()
                    .take(MSG_MAX_LINES)
                    .enumerate()
                {
                    g.text(r.x + 3, r.y + 2 + i as i32 * 9, l, 'K');
                }
            }
            None => {
                g.text(r.x + 3, r.y + 2, &v.tr.t("editor.looks_good"), 'G');
            }
        }
    }

    /// Handle input. Returns whether the program changed.
    pub fn update(&mut self, cx: &mut Ctx<'_>, s: &mut Session, area: Rect) -> Out {
        let mut out = Out::default();
        let vp = cx.vp;
        // a selection can outlive the card it pointed at (a text edit, an undo)
        if self.sel.is_some() {
            let n = s.rows().len();
            self.sel = self.sel.filter(|i| *i < n);
        }
        let p = parts(
            &vp,
            area,
            self.kb_open,
            true,
            self.tab,
            !s.prelude().trim().is_empty(),
            self.note_lines(s, area.inset(2).w),
        );
        let tr = tab_rects(p.tabs, 4);
        if cx.ptr.pressed {
            self.note = None;
        }
        if self.card.is_some() {
            return self.update_card(cx, s, area);
        }
        if cx.ptr.tapped_in(&tr[0]) {
            self.tab = Tab::Blocks;
            self.kb_open = false;
        }
        if cx.ptr.tapped_in(&tr[1]) {
            self.tab = Tab::Text;
            s.edit.focused = true;
        }
        if cx.ptr.tapped_in(&tr[2]) && s.undo() {
            out.changed = true;
            cx.sfx("button");
        }
        if cx.ptr.tapped_in(&tr[3]) && s.redo() {
            out.changed = true;
            cx.sfx("button");
        }
        let ctrl_z = cx.input.ctrl_ch('z');
        if ctrl_z && !cx.input.shift && s.undo() {
            out.changed = true;
        }
        if ((ctrl_z && cx.input.shift) || cx.input.ctrl_ch('y')) && s.redo() {
            out.changed = true;
        }
        match self.tab {
            Tab::Text => out.changed |= self.update_text(cx, s, &p),
            Tab::Blocks => out.changed |= self.update_blocks(cx, s, &p),
        }
        if self.sel.is_some() {
            let n = s.rows().len();
            self.sel = self.sel.filter(|i| *i < n);
        }
        out
    }

    fn update_text(&mut self, cx: &mut Ctx<'_>, s: &mut Session, p: &Parts) -> bool {
        let scale = 1;
        let line_h = 9;
        let mut changed = false;
        let [keys_btn, cmt_btn, close_btn] = tool_rects(p.bar);
        let ed_rect = p.body;
        s.edit.focused = true;
        s.edit.smart = true;
        s.edit.auto_close = self.auto_close;
        s.edit.gutter = textedit::gutter_width(s.edit.line_count(), scale);
        // the popup takes the navigation keys and taps on its rows
        let mut inp = cx.input.clone();
        let mut accept: Option<usize> = None;
        let mut dismiss = false;
        if let Some(pop) = &mut self.ac {
            let last = pop.items.len().saturating_sub(1);
            let mut keep = Vec::new();
            for k in inp.keys.drain(..) {
                match k {
                    Key::Up => pop.sel = pop.sel.saturating_sub(1),
                    Key::Down => pop.sel = (pop.sel + 1).min(last),
                    Key::Enter | Key::Tab => accept = Some(pop.sel),
                    Key::Escape => dismiss = true,
                    other => keep.push(other),
                }
            }
            inp.keys = keep;
            if pop.sel < pop.top {
                pop.top = pop.sel;
            }
            if pop.sel >= pop.top + POPUP_ROWS {
                pop.top = pop.sel + 1 - POPUP_ROWS;
            }
        }
        let mut tapped_popup = false;
        if let Some(pop) = &self.ac {
            for (i, r) in self.popup_rects(s, ed_rect, &cx.vp).iter().enumerate() {
                if cx.ptr.tapped_in(r) {
                    accept = Some(pop.top + i);
                    tapped_popup = true;
                }
            }
        }
        if let Some(i) = accept {
            if let Some(pop) = self.ac.take()
                && let Some(item) = pop.items.get(i)
            {
                s.edit.anchor = Some(pop.start);
                s.edit.insert(&item.insert);
                changed = true;
                cx.sfx("pick");
            }
        } else if dismiss {
            self.dismissed = Some((s.edit.text(), s.edit.cursor));
            self.ac = None;
        }
        let before = s.edit.text();
        let o = s.edit.handle(&inp);
        if let Some(t) = o.copy {
            cx.events.push(crate::ui::GameEvent::Copy(t));
        }
        if o.want_paste {
            cx.events.push(crate::ui::GameEvent::PasteRequest);
        }
        if inp.key(Key::Tab) && !o.changed {
            s.edit.insert("    ");
        }
        if inp.ctrl_ch('/') {
            s.edit.toggle_comment();
        }
        let (rows, cols) = textedit::view_size(&s.edit, ed_rect, scale, line_h);
        if !tapped_popup {
            s.edit.pointer(cx.ptr, ed_rect, scale, line_h);
        }
        if cx.ptr.wheel != 0.0 && cx.ptr.pos.is_some_and(|p| ed_rect.contains(p.0, p.1)) {
            let n = -(cx.ptr.wheel.signum() as i32);
            if inp.shift {
                s.edit.scroll_view(n * 4, 0, rows, cols);
            } else {
                s.edit.scroll_view(0, n, rows, cols);
            }
        }
        s.edit.follow_cursor(rows, cols);
        // tool row: keyboard, comment, auto-close
        if cx.ptr.tapped_in(&keys_btn)
            || (cx.config.touch && cx.ptr.tapped_in(&ed_rect) && !self.kb_open)
        {
            self.kb_open = !self.kb_open || !cx.ptr.tapped_in(&ed_rect);
        }
        if p.bar.h > 0 && cx.ptr.tapped_in(&cmt_btn) {
            s.edit.toggle_comment();
        }
        if p.bar.h > 0 && cx.ptr.tapped_in(&close_btn) {
            self.auto_close = !self.auto_close;
            cx.sfx("button");
        }
        if self.kb_open {
            for a in self.kb.update(cx.ptr, p.kb) {
                match a {
                    KbAction::Char(c) => s.edit.type_char(c),
                    KbAction::Text(t) => s.edit.insert(&t),
                    KbAction::Backspace => {
                        s.edit.backspace_smart();
                    }
                    KbAction::Enter => s.edit.smart_newline(),
                    KbAction::Comment => {
                        s.edit.toggle_comment();
                    }
                    KbAction::Hide => self.kb_open = false,
                }
                changed = true;
            }
        }
        if s.sync() {
            changed = true;
        }
        let typed_now = s.edit.text() != before || changed;
        self.refresh_popup(s, typed_now);
        changed
    }

    /// Open the popup for the cursor right now (screenshots and tests).
    pub fn suggest_now(&mut self, s: &Session) {
        self.refresh_popup(s, true);
    }

    /// Recompute the suggestions for the cursor (only when the text just changed or the popup is open).
    fn refresh_popup(&mut self, s: &Session, typed_now: bool) {
        let old = self.ac.take();
        if (!typed_now && old.is_none()) || s.edit.anchor.is_some() {
            return;
        }
        let text = s.edit.text();
        if self.dismissed.as_ref() == Some(&(text.clone(), s.edit.cursor)) {
            return;
        }
        let cursor_b = text
            .char_indices()
            .nth(s.edit.cursor)
            .map_or(text.len(), |(i, _)| i);
        let (mut items, start_b) = complete::suggest(s, &text, cursor_b);
        items.truncate(60);
        let typed: String = text[start_b..cursor_b].to_string();
        if items.is_empty() || (items.len() == 1 && items[0].label == typed) {
            return;
        }
        let start = text[..start_b].chars().count();
        let mut pop = Popup {
            items,
            sel: 0,
            start,
            top: 0,
        };
        if let Some(o) = old
            && o.start == pop.start
            && o.items == pop.items
        {
            pop.sel = o.sel;
            pop.top = o.top;
        }
        self.ac = Some(pop);
    }

    /// Rectangles of the visible popup rows, placed under (or above) the cursor line.
    fn popup_rects(&self, s: &Session, body: Rect, vp: &crate::layout::Viewport) -> Vec<Rect> {
        let Some(pop) = &self.ac else {
            return Vec::new();
        };
        let rh = if vp.touch { 14 } else { 9 };
        let n = pop.items.len().min(POPUP_ROWS) as i32;
        let (line, col) = s.edit.line_col(s.edit.cursor);
        let w = pop
            .items
            .iter()
            .map(|i| {
                (i.label.chars().count() + 2 + i.detail.chars().count().min(26)) as i32 * 6 + 8
            })
            .max()
            .unwrap_or(60)
            .clamp(70, (body.w - 8).max(70));
        let cy = body.y + 2 + (line as i32 - s.edit.scroll_line as i32) * 9;
        let cx = body.x
            + 3
            + textedit::gutter_width(s.edit.line_count(), 1)
            + (col as i32 - s.edit.scroll_col as i32) * 6;
        let x = cx.clamp(body.x, (body.right() - w).max(body.x));
        let below = cy + 10;
        let y = if below + n * rh + 2 > body.bottom() {
            (cy - n * rh - 2).max(body.y)
        } else {
            below
        };
        (0..n).map(|i| Rect::new(x, y + i * rh, w, rh)).collect()
    }

    fn row_h(vp: &crate::layout::Viewport) -> i32 {
        if vp.touch { 24 } else { 14 }
    }

    fn update_blocks(&mut self, cx: &mut Ctx<'_>, s: &mut Session, p: &Parts) -> bool {
        let rows = s.rows();
        let list = self.rows_rect(p.body);
        let geo = cards::geometry(&rows, list, Self::row_h(&cx.vp));
        let max_scroll = (geo.content - list.h).max(0);
        if cx.ptr.wheel != 0.0 {
            self.scroll = (self.scroll - (cx.ptr.wheel * 14.0) as i32).clamp(0, max_scroll);
        }
        if cx.ptr.touch
            && cx.ptr.down
            && cx.ptr.moved
            && self.drag.is_none()
            && list.contains(cx.ptr.start.0, cx.ptr.start.1)
            && cx.ptr.held_ticks < crate::input::LONG_PRESS_TICKS
        {
            self.scroll = (self.scroll - cx.ptr.delta.1).clamp(0, max_scroll);
        }
        let mut changed = false;
        // palette overlay first
        if let Some(inside) = self.palette {
            let cells = self.palette_rects(list);
            for (i, r) in cells.iter().enumerate() {
                if cx.ptr.tapped_in(r) {
                    if let Some(mut prog) = s.good.clone() {
                        let entry = s.entry_name();
                        let fill = blocks::Fill::for_function(
                            fresh_name(&s.edit.text()),
                            prog.find(&entry),
                        );
                        let text = fill.apply(PALETTE[i].1);
                        let anchor =
                            self.sel
                                .and_then(|i| rows.get(i))
                                .map(|r| r.node)
                                .filter(|n| {
                                    rows.iter().any(|r| {
                                        r.node == *n
                                            && !matches!(r.kind, CardKind::Header | CardKind::Item)
                                    })
                                });
                        let res = blocks::insert_stmt(
                            &mut prog,
                            &entry,
                            anchor,
                            inside && anchor.is_some(),
                            &text,
                        );
                        match res {
                            Ok(t) => {
                                s.set_text(&t);
                                changed = true;
                                cx.sfx("pick");
                                self.note = Some(cx.tr.t("editor.new_card"));
                            }
                            Err(e) => self.note = Some(e),
                        }
                    }
                    self.palette = None;
                    return changed;
                }
            }
            if (cx.ptr.released && !cx.ptr.moved && !cells.iter().any(|r| cx.ptr.tapped_in(r)))
                || cx.input.key(Key::Escape)
            {
                self.palette = None;
            }
            return changed;
        }
        // an empty body: the big button is the first thing to tap
        if let Some(r) = cards::empty_slot(&rows, &geo, list, self.scroll, Self::row_h(&cx.vp) + 8)
            && cx.ptr.tapped_in(&r)
        {
            self.sel = None;
            self.palette = Some(false);
            cx.sfx("pick");
            return changed;
        }
        // card taps, and drag and drop (mouse drag, or a touch long press) into any block
        for (i, row) in rows.iter().enumerate() {
            let Some(r) = cards::card_rect(&geo, i, list, self.scroll) else {
                continue;
            };
            if r.y + r.h < list.y || r.y > list.bottom() {
                continue;
            }
            if cx.ptr.tapped_in(&r) && cx.ptr.pos.is_some_and(|p| list.contains(p.0, p.1)) {
                let again = self.sel == Some(i) && row.editable();
                self.sel = Some(i);
                cx.sfx("pick");
                if again {
                    // a second tap on the selected card edits it
                    self.open_card(s, row.node);
                    return changed;
                }
                if matches!(row.kind, CardKind::Header | CardKind::Item) {
                    self.sel = None;
                }
            }
            let started = (cx.ptr.long_press() && r.contains(cx.ptr.start.0, cx.ptr.start.1))
                || (!cx.ptr.touch
                    && cx.ptr.down
                    && cx.ptr.moved
                    && r.contains(cx.ptr.start.0, cx.ptr.start.1));
            if started && row.movable() && self.drag.is_none() {
                self.drag = Some((i, 0));
                self.sel = Some(i);
            }
        }
        if let Some((from, _)) = self.drag {
            let (px, py) = cx.ptr.pos.unwrap_or((0, 0));
            if py < list.y + 12 {
                self.scroll = (self.scroll - 3).clamp(0, max_scroll);
            } else if py > list.bottom() - 12 {
                self.scroll = (self.scroll + 3).clamp(0, max_scroll);
            }
            self.drop = cards::drop_at(&rows, &geo, px, py - list.y + self.scroll);
            if cx.ptr.released {
                self.drag = None;
                let target = self.drop.take();
                let node = rows.get(from).map(|r| r.node);
                if let (Some(d), Some(node), Some(mut prog)) = (target, node, s.good.clone()) {
                    if s.apply(blocks::move_to(&mut prog, node, d.slot, d.index)) {
                        changed = true;
                        self.select_node(s, node);
                        cx.sfx("put");
                    } else if d.row != from {
                        self.note = Some(crate::words::CANT_DROP.to_string());
                    }
                }
            }
            return changed;
        }
        // action bar
        let bar = p.bar;
        if let Some(i) = self.sel.filter(|i| *i < rows.len()) {
            let row = &rows[i];
            let b = crate::layout::columns(bar, 6, 2);
            let node = row.node;
            let mut prog = s.good.clone();
            if cx.ptr.tapped_in(&b[0]) && row.movable() {
                if let Some(p) = prog.as_mut() {
                    if s.apply(blocks::step_stmt(p, node, true)) {
                        changed = true;
                        self.select_node(s, node);
                    }
                }
            } else if cx.ptr.tapped_in(&b[1]) && row.movable() {
                if let Some(p) = prog.as_mut() {
                    if s.apply(blocks::step_stmt(p, node, false)) {
                        changed = true;
                        self.select_node(s, node);
                    }
                }
            } else if cx.ptr.tapped_in(&b[2]) && row.editable() {
                self.open_card(s, node);
            } else if cx.ptr.tapped_in(&b[3]) && row.movable() {
                if let Some(p) = prog.as_mut() {
                    if s.apply(blocks::delete_stmt(p, node)) {
                        changed = true;
                        self.sel = None;
                        cx.sfx("put");
                    }
                }
            } else if cx.ptr.tapped_in(&b[4]) {
                self.palette = Some(false);
            } else if cx.ptr.tapped_in(&b[5]) && row.kind == CardKind::Compound {
                self.palette = Some(true);
            }
            if cx.input.key(Key::Delete) && row.movable() {
                if let Some(p) = prog.as_mut() {
                    changed |= s.apply(blocks::delete_stmt(p, node));
                    self.sel = None;
                }
            }
        } else if cx.ptr.tapped_in(&bar) && bar.h > 0 {
            self.palette = Some(false);
        }
        // keyboard navigation
        if cx.input.key(Key::Down) {
            self.sel = Some(
                self.sel
                    .map_or(1, |i| (i + 1).min(rows.len().saturating_sub(1))),
            );
        }
        if cx.input.key(Key::Up) {
            self.sel = Some(self.sel.map_or(0, |i| i.saturating_sub(1)));
        }
        if cx.input.key(Key::Enter) && self.card.is_none() {
            if let Some(i) = self.sel {
                if let Some(row) = rows.get(i) {
                    if row.editable() {
                        self.open_card(s, row.node);
                    }
                }
            }
        }
        self.scroll = self.scroll.clamp(0, max_scroll);
        changed
    }

    /// Select the card of a statement (after an edit moved it).
    fn select_node(&mut self, s: &Session, node: u32) {
        self.sel = s
            .rows()
            .iter()
            .position(|r| r.node == node && !matches!(r.kind, CardKind::Else | CardKind::Foot));
    }

    /// Open the card editor on a statement card.
    pub fn open_card(&mut self, s: &Session, node: u32) {
        if let Some(t) = s.good.as_ref().and_then(|p| blocks::card_text(p, node)) {
            let mut line = TextEdit::new(&t, false);
            line.focused = true;
            self.card = Some(CardEdit {
                node,
                line,
                pad: Pad::Names,
                last_var: None,
                msg: None,
                scroll: 0,
            });
        }
    }

    fn palette_rects(&self, list: Rect) -> Vec<Rect> {
        let cols = 2;
        let cw = (list.w - 8) / cols;
        let ch = 18;
        PALETTE
            .iter()
            .enumerate()
            .map(|(i, _)| {
                Rect::new(
                    list.x + 4 + (i as i32 % cols) * cw,
                    list.y + 14 + (i as i32 / cols) * (ch + 2),
                    cw - 2,
                    ch,
                )
            })
            .collect()
    }

    // ---- card editor ----

    /// The variables (name, type) a card at `node` can use.
    fn card_names(&self, s: &Session, node: u32) -> Vec<(String, Ty)> {
        let line = s.node_line(node).unwrap_or(usize::MAX);
        let mut names = complete::names_at(s, line, None);
        if names.is_empty()
            && let Some(p) = &s.good
        {
            for f in p.fns().filter(|f| f.body.is_some()) {
                for pa in &f.params {
                    names.push((pa.name.clone(), pa.spec.ty.clone()));
                }
            }
        }
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for (n, t) in names.into_iter().rev() {
            if !seen.contains(&n) {
                seen.push(n.clone());
                out.push((n, t));
            }
        }
        out.reverse();
        out
    }

    /// The value the member chips apply to: the type of the expression left of the cursor, whether to reach
    /// it with `->`, and whether the `.` or `->` is already typed. Falls back to the last variable tapped.
    fn receiver_here(&self, s: &Session, card: &CardEdit) -> Option<(Ty, bool, bool)> {
        let text = card.line.text();
        let before: String = text.chars().take(card.line.cursor).collect();
        let (expr, typed) = receiver_before(&before);
        let line = s.node_line(card.node).unwrap_or(usize::MAX);
        let arrow_of = |t: &Ty| matches!(t, Ty::Ptr(_) | Ty::Iter { .. });
        if let Some(e) = expr
            && let Some(t) = complete::receiver_type(s, &e, line, None)
        {
            let arrow = typed.unwrap_or_else(|| arrow_of(&t));
            return Some((t, arrow, typed.is_some()));
        }
        let t = card.last_var.clone()?;
        let arrow = typed.unwrap_or_else(|| arrow_of(&t));
        Some((t, arrow, typed.is_some()))
    }

    fn pad_chips(&self, s: &Session, card: &CardEdit) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = Vec::new();
        match card.pad {
            Pad::Names => {
                for (n, _) in self.card_names(s, card.node) {
                    v.push((n.clone(), n));
                }
            }
            Pad::Numbers => {
                for d in [
                    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "true", "false", "nullptr",
                    "\"\"", "''", "{}", "-", "'a'",
                ] {
                    v.push((d.to_string(), d.to_string()));
                }
            }
            Pad::Ops => {
                for d in [
                    "+", "-", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "&&", "||", "!",
                    "(", ")", "[", "]", "{", "}", ",", "=", "+=", "-=", "++", "--", ".", "->",
                    "::", "&", "|", "^", "<<", ">>", "?", ":",
                ] {
                    let tight = matches!(
                        d,
                        "!" | "("
                            | ")"
                            | "["
                            | "]"
                            | "{"
                            | "}"
                            | ","
                            | "++"
                            | "--"
                            | "."
                            | "->"
                            | "::"
                    );
                    let ins = if tight || d == "," {
                        d.to_string()
                    } else {
                        format!(" {d} ")
                    };
                    v.push((d.to_string(), ins));
                }
            }
            Pad::Members => {
                if let Some((t, arrow, typed)) = self.receiver_here(s, card) {
                    let sep = if typed {
                        ""
                    } else if arrow {
                        "->"
                    } else {
                        "."
                    };
                    let shown = if arrow { "->" } else { "." };
                    for it in complete::card_member_items(s, &t, arrow, "") {
                        v.push((
                            format!("{shown}{}", it.label),
                            format!("{sep}{}", it.insert),
                        ));
                    }
                }
            }
            Pad::Std => {
                for it in complete::card_std_items(s, "") {
                    v.push((format!("std::{}", it.label), format!("std::{}", it.insert)));
                }
            }
            Pad::Words => {
                for d in [
                    "if",
                    "else",
                    "for",
                    "while",
                    "do",
                    "switch",
                    "case",
                    "default:",
                    "return",
                    "break;",
                    "continue;",
                    "auto",
                    "const",
                    "int",
                    "long long",
                    "double",
                    "bool",
                    "char",
                    "size_t",
                    "unsigned",
                    "static_cast<int>(",
                    "{",
                    "}",
                    ";",
                ] {
                    let ins = if d.ends_with('(') || d.ends_with(';') || d == "{" || d == "}" {
                        d.to_string()
                    } else {
                        format!("{d} ")
                    };
                    v.push((d.to_string(), ins));
                }
            }
        }
        v
    }

    fn card_layout(
        &self,
        vp: &crate::layout::Viewport,
        area: Rect,
    ) -> (Rect, Rect, Rect, Vec<Rect>, Rect, Rect, Rect) {
        let th = vp.btn_h().min(26);
        let line = Rect::new(area.x + 2, area.y + 14, area.w - 4, 17);
        let msg = Rect::new(area.x + 2, line.bottom() + 2, area.w - 4, 18);
        let tabs_r = Rect::new(area.x + 2, msg.bottom() + 1, area.w - 4, th - 4);
        let tabs = weighted_tabs(tabs_r, &[6, 4, 4, 4, 6, 6]);
        let ok = Rect::new(area.right() - 62, area.bottom() - th - 2, 60, th);
        let cancel = Rect::new(area.x + 2, area.bottom() - th - 2, 60, th);
        let chips = Rect::new(
            area.x + 2,
            tabs_r.bottom() + 2,
            area.w - 4,
            ok.y - tabs_r.bottom() - 4,
        );
        (line, msg, tabs_r, tabs, chips, ok, cancel)
    }

    fn chip_rects(
        &self,
        chips: Rect,
        labels: &[(String, String)],
        vp: &crate::layout::Viewport,
        scroll: i32,
    ) -> Vec<Rect> {
        let h = if vp.touch { 22 } else { 13 };
        let (mut x, mut y) = (chips.x, chips.y - scroll);
        let mut out = Vec::new();
        for (l, _) in labels {
            let w = (l.chars().count() as i32 * 6 + 8).max(if vp.touch { 28 } else { 18 });
            if x + w > chips.right() {
                x = chips.x;
                y += h + 2;
            }
            out.push(Rect::new(x, y, w, h));
            x += w + 2;
        }
        out
    }

    fn update_card(&mut self, cx: &mut Ctx<'_>, s: &mut Session, area: Rect) -> Out {
        let mut out = Out::default();
        let Some(mut card) = self.card.take() else {
            return out;
        };
        let (line_r, _msg, _tabs_r, tabs, chips_r, ok_r, cancel_r) = self.card_layout(&cx.vp, area);
        let o = card.line.handle(cx.input);
        if let Some(t) = o.copy {
            cx.events.push(crate::ui::GameEvent::Copy(t));
        }
        card.line.pointer(cx.ptr, line_r, 1, 11);
        let (vrows, vcols) = textedit::view_size(&card.line, line_r, 1, 11);
        card.line.follow_cursor(vrows, vcols);
        let pads = [
            Pad::Names,
            Pad::Numbers,
            Pad::Ops,
            Pad::Members,
            Pad::Std,
            Pad::Words,
        ];
        for (i, r) in tabs.iter().enumerate() {
            if cx.ptr.tapped_in(r) {
                card.pad = pads[i];
            }
        }
        let labels = self.pad_chips(s, &card);
        let rects = self.chip_rects(chips_r, &labels, &cx.vp, card.scroll);
        let content = rects
            .last()
            .map_or(0, |r| r.bottom() - chips_r.y + card.scroll);
        let max_scroll = (content - chips_r.h).max(0);
        if cx.ptr.wheel != 0.0
            && chips_r.contains(
                cx.ptr.pos.map_or(-1, |p| p.0),
                cx.ptr.pos.map_or(-1, |p| p.1),
            )
        {
            card.scroll = (card.scroll + (cx.ptr.wheel * 10.0) as i32).clamp(0, max_scroll);
        }
        if cx.ptr.down && cx.ptr.moved && chips_r.contains(cx.ptr.start.0, cx.ptr.start.1) {
            card.scroll = (card.scroll - cx.ptr.delta.1).clamp(0, max_scroll);
        }
        for (i, r) in rects.iter().enumerate() {
            if r.y >= chips_r.y && r.bottom() <= chips_r.bottom() && cx.ptr.tapped_in(r) {
                let ins = labels[i].1.clone();
                if card.pad == Pad::Names {
                    let name = labels[i].0.clone();
                    card.last_var = self
                        .card_names(s, card.node)
                        .into_iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, t)| t);
                    card.pad = Pad::Members;
                    card.scroll = 0;
                }
                card.line.insert(&ins);
                if matches!(ins.as_str(), "\"\"" | "''" | "{}" | "()") {
                    card.line.cursor = card.line.cursor.saturating_sub(1);
                }
                cx.sfx("pick");
            }
        }
        // live check
        let text = card.line.text();
        card.msg = match s.good.clone() {
            Some(mut prog) => match blocks::parse_stmt(&mut prog, &text) {
                Ok(_) => Some("ok".to_string()),
                Err(e) => Some(e),
            },
            None => None,
        };
        let confirm = cx.ptr.tapped_in(&ok_r) || cx.input.key(Key::Enter);
        let cancel = cx.ptr.tapped_in(&cancel_r) || cx.input.key(Key::Escape);
        if confirm {
            if let Some(mut prog) = s.good.clone() {
                match blocks::edit_stmt(&mut prog, card.node, &text) {
                    Ok(t) => {
                        s.set_text(&t);
                        out.changed = true;
                        cx.sfx("put");
                        return out;
                    }
                    Err(e) => card.msg = Some(e),
                }
            }
        }
        if cancel {
            return out;
        }
        self.card = Some(card);
        out
    }

    // ---- drawing ----

    /// Draw the panel.
    pub fn draw(
        &self,
        g: &mut Gfx<'_>,
        v: &View<'_>,
        s: &Session,
        area: Rect,
        executing: Option<u32>,
    ) {
        let vp = v.vp;
        g.panel(area, 'W');
        let p = parts(
            &vp,
            area.inset(2),
            self.kb_open,
            true,
            self.tab,
            !s.prelude().trim().is_empty(),
            self.note_lines(s, area.inset(2).w),
        );
        let tr = tab_rects(p.tabs, 4);
        let st = |r: &Rect| widget_state(v.ptr, r, false, false);
        g.button(
            tr[0],
            &v.tr.t("editor.cards"),
            if self.tab == Tab::Blocks {
                Kind::On
            } else {
                Kind::Normal
            },
            st(&tr[0]),
        );
        g.button(
            tr[1],
            &v.tr.t("editor.text"),
            if self.tab == Tab::Text {
                Kind::On
            } else {
                Kind::Normal
            },
            st(&tr[1]),
        );
        g.icon_button(
            tr[2],
            "left",
            Kind::Normal,
            State {
                disabled: !s.can_undo(),
                ..st(&tr[2])
            },
        );
        g.icon_button(
            tr[3],
            "right",
            Kind::Normal,
            State {
                disabled: !s.can_redo(),
                ..st(&tr[3])
            },
        );
        match self.tab {
            Tab::Blocks => self.draw_blocks(g, v, s, &p, executing),
            Tab::Text => self.draw_text(g, v, s, &p, executing),
        }
        if let Some(card) = &self.card {
            self.draw_card(g, v, s, area.inset(2), card);
        }
    }

    fn draw_blocks(
        &self,
        g: &mut Gfx<'_>,
        v: &View<'_>,
        s: &Session,
        p: &Parts,
        executing: Option<u32>,
    ) {
        let vp = v.vp;
        let list = self.rows_rect(p.body);
        g.rect(list, 'A');
        g.frame(list, 'a');
        let saved = g.c.set_clip(list.offset(g.off.0, g.off.1));
        let rows = s.rows();
        let geo = cards::geometry(&rows, list, Self::row_h(&vp));
        cards::draw(
            g,
            &rows,
            &geo,
            &cards::Look {
                list,
                scroll: self.scroll,
                sel: self.sel,
                executing,
                broken: s.problem().is_some(),
                dragging: self.drag.map(|d| d.0),
                drop: self.drop,
            },
        );
        g.c.set_clip(saved);
        g.scrollbar(
            Rect::new(list.right() - 3, list.y + 1, 2, list.h - 2),
            self.scroll,
            geo.content,
            list.h,
        );
        if let Some(r) = cards::empty_slot(&rows, &geo, list, self.scroll, Self::row_h(&vp) + 8) {
            let st = widget_state(v.ptr, &r, false, false);
            g.button(r, crate::words::ADD_STATEMENT, Kind::Primary, st);
            let cols = ((list.w - 24) / 6).max(8) as usize;
            for (i, l) in algoclerk_media::Font::wrap(crate::words::FIRST_CARD_HINT, cols)
                .iter()
                .enumerate()
            {
                g.text(list.x + 12, r.bottom() + 5 + i as i32 * 9, l, 'a');
            }
        }
        self.draw_note(g, v, s, p.msg);
        // drag ghost
        if let (Some((from, _)), Some(pos)) = (self.drag, v.ptr.pos) {
            if let Some(row) = rows.get(from) {
                let w = list.w - 40;
                let r = Rect::new(
                    list.x + 20,
                    (pos.1 - 6).clamp(list.y, list.bottom() - 14),
                    w,
                    14,
                );
                g.round_rect(r, 'y', 'R');
                let cols = ((w - 8) / 6).max(4) as usize;
                let first = algoclerk_media::Font::wrap(&row.text, cols)
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                g.text(r.x + 4, r.y + 3, &first, 'K');
            }
        }
        // action bar
        if self.sel.is_some() {
            let b = crate::layout::columns(p.bar, 6, 2);
            let t = |k: &str| v.tr.t(k);
            let labels = [
                ("up", t("editor.up")),
                ("down", t("editor.down")),
                ("pointer", t("editor.edit")),
                ("trash", t("editor.del")),
                ("plus", t("editor.add")),
                ("plus", t("editor.inside")),
            ];
            let cur = self.sel.and_then(|i| rows.get(i));
            let compound = cur.is_some_and(|r| r.kind == CardKind::Compound);
            let movable = cur.is_some_and(blocks::Row::movable);
            let editable = cur.is_some_and(blocks::Row::editable);
            for (i, r) in b.iter().enumerate() {
                let disabled = match i {
                    0 | 1 | 3 => !movable,
                    2 => !editable,
                    5 => !compound,
                    _ => false,
                };
                let st = State {
                    disabled,
                    ..widget_state(v.ptr, r, false, disabled)
                };
                if labels[i].1.len() <= 2 || i == 2 {
                    g.button(
                        *r,
                        &labels[i].1,
                        if i == 3 { Kind::Danger } else { Kind::Normal },
                        st,
                    );
                } else {
                    g.icon_button(
                        *r,
                        labels[i].0,
                        if i == 3 { Kind::Danger } else { Kind::Normal },
                        st,
                    );
                }
            }
        } else if p.bar.h > 0 {
            g.button(
                p.bar,
                &v.tr.t("editor.add_card"),
                Kind::Primary,
                widget_state(v.ptr, &p.bar, false, false),
            );
        }
        if let Some(inside) = self.palette {
            let r = list.inset(1);
            g.panel(r, 'W');
            g.header(
                Rect::new(r.x, r.y, r.w, 12),
                &if inside {
                    v.tr.t("editor.add_inside")
                } else {
                    v.tr.t("editor.add_after")
                },
                true,
            );
            for (i, c) in self.palette_rects(list).iter().enumerate() {
                g.button(
                    *c,
                    PALETTE[i].0,
                    Kind::Normal,
                    widget_state(v.ptr, c, false, false),
                );
            }
        }
        if let Some(n) = &self.note {
            let lines = algoclerk_media::Font::wrap(n, ((list.w - 12) / 6).max(8) as usize);
            let h = lines.len() as i32 * 9 + 5;
            let r = Rect::new(list.x + 2, list.bottom() - h - 2, list.w - 4, h);
            g.round_rect(r, 'y', 'K');
            for (i, l) in lines.iter().enumerate() {
                g.text(r.x + 4, r.y + 3 + i as i32 * 9, l, 'K');
            }
        }
    }

    fn draw_text(
        &self,
        g: &mut Gfx<'_>,
        v: &View<'_>,
        s: &Session,
        p: &Parts,
        executing: Option<u32>,
    ) {
        let span = s.problem_span().or_else(|| s.run_error_span());
        let exec_line = executing.and_then(|n| s.node_line(n));
        if p.strip.h > 0 {
            self.draw_strip(g, s, p.strip);
        }
        let look = textedit::EditorLook {
            line_numbers: true,
            error_lines: span.map(|x| x.0).into_iter().collect(),
            exec_line,
            squiggles: span.into_iter().collect(),
            brackets: true,
            tick: v.tick,
        };
        let mut ed = s.edit.clone();
        ed.gutter = textedit::gutter_width(ed.line_count(), 1);
        textedit::draw_editor(g, &ed, p.body, 1, 9, &look);
        // the clerk's message under the text, wrapped, never cut off
        self.draw_note(g, v, s, p.msg);
        if p.bar.h > 0 {
            let [keys_btn, cmt_btn, close_btn] = tool_rects(p.bar);
            let st = |r: &Rect| widget_state(v.ptr, r, false, false);
            g.button(
                keys_btn,
                &v.tr.t("editor.keys"),
                Kind::Normal,
                st(&keys_btn),
            );
            g.button(cmt_btn, "//", Kind::Normal, st(&cmt_btn));
            g.button(
                close_btn,
                "()",
                if self.auto_close {
                    Kind::On
                } else {
                    Kind::Normal
                },
                st(&close_btn),
            );
        }
        if self.kb_open {
            self.kb.draw(g, p.kb, v.ptr);
        }
        self.draw_popup(g, v, s, p.body);
    }

    /// The one-line summary of the read-only prelude above the text.
    fn draw_strip(&self, g: &mut Gfx<'_>, s: &Session, r: Rect) {
        g.rect(r, 'A');
        let mut parts: Vec<String> = Vec::new();
        for l in s.prelude().lines() {
            if let Some(h) = l.trim().strip_prefix("#include") {
                parts.push(h.trim().to_string());
            }
        }
        if let Some(p) = &s.good {
            for it in p.items.iter().take(p.prelude_items) {
                match &it.kind {
                    algoclerk_core::cpp::ast::ItemKind::Struct(st) => parts.push(st.name.clone()),
                    algoclerk_core::cpp::ast::ItemKind::Alias { name, .. } => {
                        parts.push(name.clone());
                    }
                    _ => {}
                }
            }
        }
        let line = format!("{}: {}", words::PRELUDE_BAR, parts.join(" "));
        g.text(
            r.x + 3,
            r.y + 1,
            &clip(&line, ((r.w - 6) / 6).max(4) as usize),
            'm',
        );
    }

    fn draw_popup(&self, g: &mut Gfx<'_>, v: &View<'_>, s: &Session, body: Rect) {
        let Some(pop) = &self.ac else { return };
        let rects = self.popup_rects(s, body, &v.vp);
        let (Some(first), Some(last)) = (rects.first(), rects.last()) else {
            return;
        };
        let all = Rect::new(first.x, first.y, first.w, last.bottom() - first.y);
        g.rect(all.grow(1), 'K');
        for (i, r) in rects.iter().enumerate() {
            let idx = pop.top + i;
            let Some(item) = pop.items.get(idx) else {
                break;
            };
            g.rect(*r, if idx == pop.sel { 'y' } else { 'W' });
            let ty = r.y + (r.h - 7) / 2;
            g.text(r.x + 3, ty, &item.label, 'K');
            let used = item.label.chars().count() as i32 * 6 + 12;
            let room = ((r.w - used - 4) / 6).max(0) as usize;
            if room > 3 {
                g.text(r.x + used, ty, &clip(&item.detail, room), 'a');
            }
        }
        if pop.items.len() > POPUP_ROWS {
            let count = format!("{} OF {}", pop.sel + 1, pop.items.len());
            let w = count.chars().count() as i32 * 4 - 1;
            g.tiny(all.right() - 3 - w, all.bottom() + 2, &count, 'k');
        }
    }

    fn draw_card(&self, g: &mut Gfx<'_>, v: &View<'_>, s: &Session, area: Rect, card: &CardEdit) {
        g.panel(area, 'W');
        g.header(
            Rect::new(area.x, area.y, area.w, 12),
            &v.tr.t("editor.edit_card"),
            true,
        );
        let (line_r, msg_r, _tabs_r, tabs, chips_r, ok_r, cancel_r) = self.card_layout(&v.vp, area);
        let mut ed = card.line.clone();
        ed.focused = true;
        textedit::draw_editor(
            g,
            &ed,
            line_r,
            1,
            11,
            &textedit::EditorLook {
                brackets: true,
                tick: v.tick,
                ..textedit::EditorLook::default()
            },
        );
        let (text, colour) = match &card.msg {
            Some(m) if m == "ok" => (v.tr.t("editor.card_fine"), 'G'),
            Some(m) => (m.clone(), 'R'),
            None => (String::new(), 'K'),
        };
        for (i, l) in algoclerk_media::Font::wrap(&text, (msg_r.w / 6) as usize)
            .iter()
            .take(2)
            .enumerate()
        {
            g.text(msg_r.x, msg_r.y + i as i32 * 8, l, colour);
        }
        let names = [
            v.tr.t("editor.names"),
            v.tr.t("editor.numbers"),
            v.tr.t("editor.ops"),
            crate::words::MEMBERS.to_string(),
            "std::".to_string(),
            v.tr.t("editor.words"),
        ];
        let pads = [
            Pad::Names,
            Pad::Numbers,
            Pad::Ops,
            Pad::Members,
            Pad::Std,
            Pad::Words,
        ];
        for (i, r) in tabs.iter().enumerate() {
            g.button(
                *r,
                &names[i],
                if card.pad == pads[i] {
                    Kind::On
                } else {
                    Kind::Normal
                },
                widget_state(v.ptr, r, false, false),
            );
        }
        let labels = self.pad_chips(s, card);
        let saved = g.c.set_clip(chips_r.offset(g.off.0, g.off.1));
        for (i, r) in self
            .chip_rects(chips_r, &labels, &v.vp, card.scroll)
            .iter()
            .enumerate()
        {
            if r.bottom() < chips_r.y || r.y > chips_r.bottom() {
                continue;
            }
            g.button(
                *r,
                &labels[i].0,
                Kind::Normal,
                widget_state(v.ptr, r, false, false),
            );
        }
        g.c.set_clip(saved);
        if card.pad == Pad::Members && labels.is_empty() {
            g.text(
                chips_r.x,
                chips_r.bottom() - 9,
                &v.tr.t("editor.pick_name"),
                'k',
            );
        }
        g.button(
            ok_r,
            &v.tr.t("common.ok"),
            Kind::Good,
            widget_state(v.ptr, &ok_r, false, false),
        );
        g.button(
            cancel_r,
            &v.tr.t("common.cancel"),
            Kind::Normal,
            widget_state(v.ptr, &cancel_r, false, false),
        );
    }
}

/// The chips and keys buttons under the text.
fn tool_rects(bar: Rect) -> [Rect; 3] {
    let n = 3;
    let c = crate::layout::columns(bar, n, 2);
    [c[0], c[1], c[2]]
}

/// The receiver expression that ends `before` (`seen`, `grid[i]`, `m.find(k)`) and the separator typed after
/// it (`Some(true)` for `->`, `Some(false)` for `.`).
fn receiver_before(before: &str) -> (Option<String>, Option<bool>) {
    let mut t = before.trim_end();
    let typed = if let Some(r) = t.strip_suffix("->") {
        t = r;
        Some(true)
    } else if let Some(r) = t.strip_suffix('.') {
        t = r;
        Some(false)
    } else {
        None
    };
    let cs: Vec<char> = t.chars().collect();
    let mut j = cs.len();
    while j > 0 {
        let c = cs[j - 1];
        if c.is_alphanumeric() || c == '_' || c == '.' {
            j -= 1;
        } else if c == '>' && j >= 2 && cs[j - 2] == '-' {
            j -= 2;
        } else if c == ')' || c == ']' {
            let (open, close) = if c == ')' { ('(', ')') } else { ('[', ']') };
            let mut depth = 0;
            let mut k = j;
            while k > 0 {
                k -= 1;
                if cs[k] == close {
                    depth += 1;
                } else if cs[k] == open {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            if depth != 0 {
                return (None, typed);
            }
            j = k;
        } else {
            break;
        }
    }
    let e: String = cs[j..].iter().collect();
    let e = e.trim_start_matches(['.', '-', '>']).to_string();
    ((!e.is_empty()).then_some(e), typed)
}

/// Tab rectangles whose widths follow the weights (label lengths), filling `r` with 2 pixel gaps.
fn weighted_tabs(r: Rect, weights: &[i32]) -> Vec<Rect> {
    let total: i32 = weights.iter().sum();
    let room = r.w - 2 * (weights.len() as i32 - 1);
    let mut x = r.x;
    let mut acc = 0;
    let mut out = Vec::new();
    for w in weights {
        acc += w;
        let end = r.x + room * acc / total + 2 * (out.len() as i32);
        out.push(Rect::new(x, r.y, (end - x).max(8), r.h));
        x = end + 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_receiver_is_the_expression_left_of_the_cursor() {
        assert_eq!(receiver_before("seen"), (Some("seen".into()), None));
        assert_eq!(
            receiver_before("x = seen."),
            (Some("seen".into()), Some(false))
        );
        assert_eq!(receiver_before("it->"), (Some("it".into()), Some(true)));
        assert_eq!(
            receiver_before("return grid[r][c]"),
            (Some("grid[r][c]".into()), None)
        );
        assert_eq!(
            receiver_before("if (m.find(k)->"),
            (Some("m.find(k)".into()), Some(true))
        );
        assert_eq!(receiver_before("a + "), (None, None));
        assert_eq!(receiver_before(""), (None, None));
        // an unbalanced bracket gives up instead of guessing
        assert_eq!(receiver_before("f(a]").0, None);
    }

    #[test]
    fn weighted_tabs_fill_the_row_without_overlap() {
        let r = Rect::new(10, 5, 180, 16);
        let t = weighted_tabs(r, &[6, 4, 4, 4, 6, 6]);
        assert_eq!(t.len(), 6);
        assert!(t.windows(2).all(|w| w[0].right() < w[1].x));
        assert!(t.iter().all(|x| x.x >= r.x && x.right() <= r.right() + 1));
        assert!(t[0].w > t[1].w);
    }
}
