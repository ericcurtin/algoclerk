// SPDX-License-Identifier: Apache-2.0
//! The program editor panel: nested paper cards (the block view), the text view with error squiggles and
//! an on-screen keyboard, a chip pad for building cards without typing, and undo/redo.

use crate::blocks::{self, CardKind, PALETTE, Row};
use crate::gfx::{Gfx, Kind, State, clip};
use crate::input::{Input, Key};
use crate::layout::Hit;
use crate::session::Session;
use crate::textedit::{self, KbAction, Keyboard, TextEdit};
use crate::ui::{Ctx, View, widget_state};
use algoclerk_core::lang::ast::Ty;
use algoclerk_core::lang::check::{ALL_BUILTINS, FREE_BUILTINS, method_sigs};
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
    /// Built-in tools.
    Tools,
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
}

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
    /// A one-line hint shown at the bottom.
    pub note: Option<String>,
}

impl Default for EditorPanel {
    fn default() -> Self {
        EditorPanel { tab: Tab::Blocks, sel: None, scroll: 0, kb: Keyboard::default(), kb_open: false, card: None, palette: None, drag: None, note: None }
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
    body: Rect,
    bar: Rect,
}

fn parts(vp: &crate::layout::Viewport, area: Rect, kb_open: bool, has_bar: bool, tab: Tab) -> Parts {
    let th = vp.btn_h().min(24);
    let tabs = Rect::new(area.x, area.y, area.w, th);
    let bar_h = if has_bar || tab == Tab::Text { th + 2 } else { 0 };
    let kb_h = if kb_open && tab == Tab::Text { (area.h * 2 / 5).clamp(70, 110) } else { 0 };
    let body = Rect::new(area.x, area.y + th + 2, area.w, area.h - th - 2 - bar_h - kb_h);
    let bar = Rect::new(area.x, body.bottom() + 1, area.w, bar_h);
    Parts { tabs, body, bar }
}

fn tab_rects(r: Rect, n: i32) -> Vec<Rect> {
    crate::layout::columns(r, n, 2)
}

fn fresh_name(text: &str) -> String {
    (1..).map(|i| format!("v{i}")).find(|n| !text.contains(n.as_str())).unwrap_or_else(|| "v1".into())
}

impl EditorPanel {
    /// Hit rectangles for layout tests.
    pub fn hits(&self, vp: &crate::layout::Viewport, area: Rect) -> Vec<Hit> {
        let p = parts(vp, area, self.kb_open, self.sel.is_some(), self.tab);
        let mut v = vec![Hit::area("editor", area), Hit::area("editor body", p.body)];
        for (i, r) in tab_rects(p.tabs, 4).into_iter().enumerate() {
            v.push(Hit::tap(["tab blocks", "tab text", "undo", "redo"][i], r));
        }
        v
    }

    fn rows_rect(&self, body: Rect) -> Rect {
        body
    }

    /// Handle input. Returns whether the program changed.
    pub fn update(&mut self, cx: &mut Ctx<'_>, s: &mut Session, area: Rect) -> Out {
        let mut out = Out::default();
        let vp = cx.vp;
        let p = parts(&vp, area, self.kb_open, self.sel.is_some(), self.tab);
        let tr = tab_rects(p.tabs, 4);
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
        if cx.input.ctrl_ch('z') && s.undo() {
            out.changed = true;
        }
        if (cx.input.ctrl_ch('y') || (cx.input.ctrl_ch('Z'))) && s.redo() {
            out.changed = true;
        }
        match self.tab {
            Tab::Text => out.changed |= self.update_text(cx, s, &p),
            Tab::Blocks => out.changed |= self.update_blocks(cx, s, &p),
        }
        out
    }

    fn update_text(&mut self, cx: &mut Ctx<'_>, s: &mut Session, p: &Parts) -> bool {
        let scale = 1;
        let line_h = 9;
        let mut changed = false;
        let kb_btn = Rect::new(p.bar.x, p.bar.y, (p.bar.w / 2).min(70), p.bar.h);
        let ed_rect = p.body;
        s.edit.focused = true;
        let inp = cx.input;
        let before = s.edit.text();
        let o = s.edit.handle(inp);
        if let Some(t) = o.copy {
            cx.events.push(crate::ui::GameEvent::Copy(t));
        }
        if o.want_paste {
            cx.events.push(crate::ui::GameEvent::PasteRequest);
        }
        s.edit.pointer(cx.ptr, ed_rect, scale, line_h);
        let rows = ((ed_rect.h - 4) / line_h).max(1) as usize;
        let cols = ((ed_rect.w - 8) / 6).max(1) as usize;
        if cx.ptr.wheel != 0.0 {
            s.edit.scroll_by(-(cx.ptr.wheel.signum() as i32), rows);
        }
        // auto-indent after Enter
        if inp.key(Key::Enter) && s.edit.text() != before {
            let t = s.edit.text();
            let (line, _) = s.edit.line_col(s.edit.cursor);
            if line > 0 {
                let prev = t.split('\n').nth(line - 1).unwrap_or("");
                let indent = prev.len() - prev.trim_start().len();
                let extra = if prev.trim_end().ends_with('{') { 4 } else { 0 };
                s.edit.insert(&" ".repeat(indent + extra));
            }
        }
        s.edit.scroll_to_cursor(rows, cols);
        if cx.ptr.tapped_in(&kb_btn) || (cx.config.touch && cx.ptr.tapped_in(&ed_rect) && !self.kb_open) {
            self.kb_open = !self.kb_open || !cx.ptr.tapped_in(&ed_rect);
        }
        if self.kb_open {
            let kb_h = (p.body.h * 0 + 100).min(110);
            let area = Rect::new(p.bar.x, p.bar.bottom() + 1, p.bar.w, kb_h);
            let area = Rect::new(area.x, area.y.min(cx.vp.h - kb_h - 2), area.w, kb_h);
            for a in self.kb.update(cx.ptr, area) {
                match a {
                    KbAction::Char(c) => s.edit.insert(&c.to_string()),
                    KbAction::Backspace => {
                        s.edit.backspace();
                    }
                    KbAction::Enter => {
                        s.edit.insert("\n");
                        let t = s.edit.text();
                        let (line, _) = s.edit.line_col(s.edit.cursor);
                        let prev = t.split('\n').nth(line.saturating_sub(1)).unwrap_or("").to_string();
                        let indent = prev.len() - prev.trim_start().len() + if prev.trim_end().ends_with('{') { 4 } else { 0 };
                        s.edit.insert(&" ".repeat(indent));
                    }
                    KbAction::Hide => self.kb_open = false,
                }
                changed = true;
            }
        }
        if s.sync() {
            changed = true;
        }
        changed
    }

    fn row_h(vp: &crate::layout::Viewport) -> i32 {
        if vp.touch { 24 } else { 14 }
    }

    fn update_blocks(&mut self, cx: &mut Ctx<'_>, s: &mut Session, p: &Parts) -> bool {
        let rows = s.rows();
        let rh = Self::row_h(&cx.vp);
        let list = self.rows_rect(p.body);
        let content = rows.len() as i32 * rh;
        let max_scroll = (content - list.h).max(0);
        if cx.ptr.wheel != 0.0 {
            self.scroll = (self.scroll - (cx.ptr.wheel * rh as f32) as i32).clamp(0, max_scroll);
        }
        if cx.ptr.touch && cx.ptr.down && cx.ptr.moved && self.drag.is_none() && list.contains(cx.ptr.start.0, cx.ptr.start.1) && cx.ptr.held_ticks < crate::input::LONG_PRESS_TICKS {
            self.scroll = (self.scroll - cx.ptr.delta.1).clamp(0, max_scroll);
        }
        let mut changed = false;
        // palette overlay first
        if let Some(inside) = self.palette {
            let cells = self.palette_rects(list);
            for (i, r) in cells.iter().enumerate() {
                if cx.ptr.tapped_in(r) {
                    let text = PALETTE[i].1.replace("{n}", &fresh_name(&s.edit.text()));
                    if let Some(mut prog) = s.good.clone() {
                        let anchor = self.sel.and_then(|i| rows.get(i)).map(|r| r.node).filter(|n| rows.iter().any(|r| r.node == *n && r.kind != CardKind::Header));
                        let res = blocks::insert_stmt(&mut prog, anchor, inside && anchor.is_some(), &text);
                        match res {
                            Ok(t) => {
                                s.set_text(&t);
                                changed = true;
                                cx.sfx("pick");
                                self.note = Some("New card added. Tap Edit to change it.".into());
                            }
                            Err(e) => self.note = Some(e),
                        }
                    }
                    self.palette = None;
                    return changed;
                }
            }
            if (cx.ptr.released && !cx.ptr.moved && !cells.iter().any(|r| cx.ptr.tapped_in(r))) || cx.input.key(Key::Escape) {
                self.palette = None;
            }
            return changed;
        }
        // card taps and drag to reorder
        for (i, row) in rows.iter().enumerate() {
            let r = Rect::new(list.x, list.y + i as i32 * rh - self.scroll, list.w, rh);
            if r.y + r.h < list.y || r.y > list.bottom() {
                continue;
            }
            if cx.ptr.tapped_in(&r) && cx.ptr.pos.is_some_and(|p| list.contains(p.0, p.1)) {
                self.sel = Some(i);
                cx.sfx("pick");
                if row.kind == CardKind::Header {
                    self.sel = None;
                }
            }
            if ((cx.ptr.long_press() && r.contains(cx.ptr.start.0, cx.ptr.start.1)) || (!cx.ptr.touch && cx.ptr.down && cx.ptr.moved && r.contains(cx.ptr.start.0, cx.ptr.start.1))) && row.kind != CardKind::Header && row.kind != CardKind::Else {
                self.drag = Some((i, 0));
            }
        }
        if let Some((from, _)) = self.drag {
            if cx.ptr.released {
                let y = cx.ptr.pos.map_or(0, |p| p.1);
                let to = (((y - list.y + self.scroll) / rh).max(0) as usize).min(rows.len().saturating_sub(1));
                self.drag = None;
                if to != from {
                    if let (Some(mut prog), Some(node)) = (s.good.clone(), rows.get(from).map(|r| r.node)) {
                        let up = to < from;
                        let mut last_text = None;
                        for _ in 0..60 {
                            let Some(t) = blocks::move_stmt(&mut prog, node, up) else { break };
                            last_text = Some(t);
                            let nrows = blocks::rows(&prog);
                            let idx = nrows.iter().position(|r| r.node == node && r.kind != CardKind::Else).unwrap_or(0);
                            if (up && idx <= to) || (!up && idx >= to) {
                                break;
                            }
                        }
                        if s.apply(last_text) {
                            changed = true;
                            self.sel = Some(to);
                            cx.sfx("put");
                        }
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
            if cx.ptr.tapped_in(&b[0]) {
                if let Some(p) = prog.as_mut() {
                    if s.apply(blocks::move_stmt(p, node, true)) {
                        changed = true;
                        self.sel = Some(i.saturating_sub(1));
                    }
                }
            } else if cx.ptr.tapped_in(&b[1]) {
                if let Some(p) = prog.as_mut() {
                    if s.apply(blocks::move_stmt(p, node, false)) {
                        changed = true;
                        self.sel = Some(i + 1);
                    }
                }
            } else if cx.ptr.tapped_in(&b[2]) && row.kind != CardKind::Else {
                if let Some(p) = &s.good {
                    let stmt_text = find_header(p, node).unwrap_or_default();
                    self.card = Some(CardEdit { node, line: TextEdit::new(&stmt_text, false), pad: Pad::Names, last_var: None, msg: None });
                    if let Some(c) = &mut self.card {
                        c.line.focused = true;
                    }
                }
            } else if cx.ptr.tapped_in(&b[3]) && row.kind != CardKind::Else {
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
            if cx.input.key(Key::Delete) {
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
            self.sel = Some(self.sel.map_or(1, |i| (i + 1).min(rows.len().saturating_sub(1))));
        }
        if cx.input.key(Key::Up) {
            self.sel = Some(self.sel.map_or(0, |i| i.saturating_sub(1)));
        }
        if cx.input.key(Key::Enter) && self.card.is_none() {
            if let Some(i) = self.sel {
                if let (Some(row), Some(p)) = (rows.get(i), &s.good) {
                    if row.kind != CardKind::Header && row.kind != CardKind::Else {
                        let t = find_header(p, row.node).unwrap_or_default();
                        self.card = Some(CardEdit { node: row.node, line: TextEdit::new(&t, false), pad: Pad::Names, last_var: None, msg: None });
                    }
                }
            }
        }
        self.scroll = self.scroll.clamp(0, max_scroll);
        changed
    }

    fn palette_rects(&self, list: Rect) -> Vec<Rect> {
        let cols = 2;
        let cw = (list.w - 8) / cols;
        let ch = 18;
        PALETTE.iter().enumerate().map(|(i, _)| Rect::new(list.x + 4 + (i as i32 % cols) * cw, list.y + 14 + (i as i32 / cols) * (ch + 2), cw - 2, ch)).collect()
    }

    // ---- card editor ----

    fn pad_chips(&self, s: &Session, card: &CardEdit) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = Vec::new();
        match card.pad {
            Pad::Names => {
                if let Ok(c) = &s.compiled {
                    let fidx = c.program.fns.iter().position(|f| f.name == s.entry_name()).unwrap_or(0);
                    let mut seen = Vec::new();
                    for (n, t) in &c.info.fns[fidx].locals {
                        if !seen.contains(n) {
                            seen.push(n.clone());
                            v.push((format!("{n}"), format!("{n}:{t}")));
                        }
                    }
                } else if let Some(p) = &s.good {
                    for f in &p.fns {
                        for pa in &f.params {
                            v.push((pa.name.clone(), format!("{}:{}", pa.name, pa.ty)));
                        }
                    }
                }
            }
            Pad::Numbers => {
                for d in ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "true", "false", "null", "none", "\"\"", "''", "[]", "-"] {
                    v.push((d.to_string(), d.to_string()));
                }
            }
            Pad::Ops => {
                for d in ["+", "-", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "and", "or", "not", "(", ")", "[", "]", ",", "=", "+=", "-=", ".", "..", "&", "|", "^", "<<", ">>"] {
                    v.push((d.to_string(), format!(" {d} ").replace("  ", " ")));
                }
            }
            Pad::Tools => {
                let allowed = s.level.as_ref().map(|l| l.check_opts().allowed.builtins).unwrap_or(None);
                let ok = |n: &str| allowed.as_ref().is_none_or(|a| a.contains(n));
                for n in FREE_BUILTINS {
                    if ok(n) && !matches!(*n, "enumerate" | "zip") {
                        v.push((format!("{n}()"), format!("{n}(")));
                    }
                }
                if ok("enumerate") {
                    v.push(("enumerate".into(), "enumerate(".into()));
                }
                if let Some(t) = &card.last_var {
                    for n in ALL_BUILTINS {
                        if !FREE_BUILTINS.contains(n) && ok(n) && method_sigs(t, n).is_some() {
                            v.push((format!(".{n}"), format!(".{n}(")));
                        }
                    }
                }
            }
            Pad::Words => {
                for d in ["let", "if", "else", "while", "for", "in", "range(", "return", "break;", "continue;", "{", "}", ";"] {
                    v.push((d.to_string(), format!("{d} ").replace("( ", "(")));
                }
            }
        }
        v
    }

    fn card_layout(&self, vp: &crate::layout::Viewport, area: Rect) -> (Rect, Rect, Rect, Vec<Rect>, Rect, Rect, Rect) {
        let th = vp.btn_h().min(26);
        let line = Rect::new(area.x + 2, area.y + 14, area.w - 4, th);
        let msg = Rect::new(area.x + 2, line.bottom() + 2, area.w - 4, 18);
        let tabs_r = Rect::new(area.x + 2, msg.bottom() + 1, area.w - 4, th - 4);
        let tabs = tab_rects(tabs_r, 5);
        let ok = Rect::new(area.right() - 62, area.bottom() - th - 2, 60, th);
        let cancel = Rect::new(area.x + 2, area.bottom() - th - 2, 60, th);
        let chips = Rect::new(area.x + 2, tabs_r.bottom() + 2, area.w - 4, ok.y - tabs_r.bottom() - 4);
        (line, msg, tabs_r, tabs, chips, ok, cancel)
    }

    fn chip_rects(&self, chips: Rect, labels: &[(String, String)], vp: &crate::layout::Viewport) -> Vec<Rect> {
        let h = if vp.touch { 22 } else { 13 };
        let (mut x, mut y) = (chips.x, chips.y);
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
        let Some(mut card) = self.card.take() else { return out };
        let (line_r, _msg, _tabs_r, tabs, chips_r, ok_r, cancel_r) = self.card_layout(&cx.vp, area);
        let o = card.line.handle(cx.input);
        if let Some(t) = o.copy {
            cx.events.push(crate::ui::GameEvent::Copy(t));
        }
        card.line.pointer(cx.ptr, line_r, 1, 11);
        let pads = [Pad::Names, Pad::Numbers, Pad::Ops, Pad::Tools, Pad::Words];
        for (i, r) in tabs.iter().enumerate() {
            if cx.ptr.tapped_in(r) {
                card.pad = pads[i];
            }
        }
        let labels = self.pad_chips(s, &card);
        let rects = self.chip_rects(chips_r, &labels, &cx.vp);
        for (i, r) in rects.iter().enumerate() {
            if cx.ptr.tapped_in(r) {
                let ins = labels[i].1.clone();
                if card.pad == Pad::Names {
                    card.last_var = s.compiled.as_ref().ok().and_then(|c| {
                        let fidx = c.program.fns.iter().position(|f| f.name == s.entry_name()).unwrap_or(0);
                        c.info.fns[fidx].locals.iter().find(|(n, _)| *n == labels[i].0).map(|(_, t)| t.clone())
                    });
                    if card.last_var.is_none() {
                        card.last_var = s.good.as_ref().and_then(|p| p.fns.iter().flat_map(|f| f.params.iter()).find(|pa| pa.name == labels[i].0).map(|pa| pa.ty.clone()));
                    }
                    card.pad = Pad::Tools;
                }
                card.line.insert(&ins);
                cx.sfx("pick");
            }
        }
        // live check
        let text = card.line.text();
        card.msg = match blocks::parse_stmt(&text) {
            Ok(_) => Some("ok".to_string()),
            Err(e) => Some(e),
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
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>, s: &Session, area: Rect, executing: Option<u32>) {
        let vp = v.vp;
        g.panel(area, 'W');
        let p = parts(&vp, area.inset(2), self.kb_open, self.sel.is_some(), self.tab);
        let tr = tab_rects(p.tabs, 4);
        let st = |r: &Rect| widget_state(v.ptr, r, false, false);
        g.button(tr[0], "Cards", if self.tab == Tab::Blocks { Kind::On } else { Kind::Normal }, st(&tr[0]));
        g.button(tr[1], "Text", if self.tab == Tab::Text { Kind::On } else { Kind::Normal }, st(&tr[1]));
        g.icon_button(tr[2], "left", Kind::Normal, State { disabled: !s.can_undo(), ..st(&tr[2]) });
        g.icon_button(tr[3], "right", Kind::Normal, State { disabled: !s.can_redo(), ..st(&tr[3]) });
        match self.tab {
            Tab::Blocks => self.draw_blocks(g, v, s, &p, executing),
            Tab::Text => self.draw_text(g, v, s, &p, executing),
        }
        if let Some(card) = &self.card {
            self.draw_card(g, v, s, area.inset(2), card);
        }
    }

    fn draw_blocks(&self, g: &mut Gfx<'_>, v: &View<'_>, s: &Session, p: &Parts, executing: Option<u32>) {
        let vp = v.vp;
        let list = self.rows_rect(p.body);
        let rh = Self::row_h(&vp);
        g.rect(list, 'A');
        g.frame(list, 'a');
        let saved = g.c.set_clip(list.offset(g.off.0, g.off.1));
        let rows = s.rows();
        let broken = s.problem().is_some();
        for (i, row) in rows.iter().enumerate() {
            let y = list.y + i as i32 * rh - self.scroll;
            if y + rh < list.y || y > list.bottom() {
                continue;
            }
            let indent = row.depth as i32 * 7;
            let r = Rect::new(list.x + 2 + indent, y + 1, list.w - 4 - indent - 4, rh - 2);
            let selected = self.sel == Some(i);
            let running = executing == Some(row.node) && row.kind != CardKind::Else;
            let (fill, stripe) = match row.kind {
                CardKind::Header => ('k', 'm'),
                CardKind::Compound => ('W', 'B'),
                CardKind::Else => ('w', 'B'),
                CardKind::Simple => ('W', 'g'),
            };
            let fill = if running { 'c' } else if selected { 'y' } else { fill };
            g.round_rect(r, fill, if selected { 'R' } else { 'K' });
            g.rect(Rect::new(r.x + 1, r.y + 1, 3, r.h - 2), stripe);
            let ink = if row.kind == CardKind::Header { 'W' } else { 'K' };
            let ty = r.y + (r.h - 7) / 2;
            if rh >= 20 || row.kind != CardKind::Else {
                if r.h >= 12 && row.kind != CardKind::Else {
                    g.icon(row.icon, r.x + 6, r.y + (r.h - 9) / 2, if row.kind == CardKind::Header { 'W' } else { 'k' });
                }
            }
            let tx = r.x + 17;
            let cols = ((r.w - 20) / 6).max(4) as usize;
            g.text(tx, ty, &clip(&row.text, cols), if broken && selected { 'R' } else { ink });
            if running {
                g.cursor(r.right() - 11, r.y + (r.h - 9) / 2);
            }
        }
        g.c.set_clip(saved);
        let content = rows.len() as i32 * rh;
        g.scrollbar(Rect::new(list.right() - 3, list.y + 1, 2, list.h - 2), self.scroll, content, list.h);
        if let Some(d) = s.problem() {
            let r = Rect::new(list.x + 2, list.bottom() - 32, list.w - 4, 30);
            g.round_rect(r, 'p', 'R');
            let lines = algoclerk_media::Font::wrap(&d.msg, ((r.w - 8) / 6) as usize);
            for (i, l) in lines.iter().take(3).enumerate() {
                g.text(r.x + 4, r.y + 3 + i as i32 * 9, l, 'K');
            }
        }
        // drag ghost
        if let (Some((from, _)), Some(pos)) = (self.drag, v.ptr.pos) {
            if let Some(row) = rows.get(from) {
                let r = Rect::new(list.x + 6, pos.1 - 6, list.w - 12, rh - 2);
                g.round_rect(r, 'y', 'R');
                g.text(r.x + 6, r.y + (r.h - 7) / 2, &clip(&row.text, 28), 'K');
            }
        }
        // action bar
        if self.sel.is_some() {
            let b = crate::layout::columns(p.bar, 6, 2);
            let labels = [("up", "up"), ("down", "down"), ("pointer", "Edit"), ("trash", "del"), ("plus", "add"), ("plus", "in")];
            let compound = self.sel.and_then(|i| rows.get(i)).is_some_and(|r| r.kind == CardKind::Compound);
            for (i, r) in b.iter().enumerate() {
                let disabled = i == 5 && !compound;
                let st = State { disabled, ..widget_state(v.ptr, r, false, disabled) };
                if labels[i].1.len() <= 2 || i == 2 {
                    g.button(*r, labels[i].1, if i == 3 { Kind::Danger } else { Kind::Normal }, st);
                } else {
                    g.icon_button(*r, labels[i].0, if i == 3 { Kind::Danger } else { Kind::Normal }, st);
                }
            }
        } else if p.bar.h > 0 {
            g.button(p.bar, "+ add a card", Kind::Primary, widget_state(v.ptr, &p.bar, false, false));
        }
        if let Some(inside) = self.palette {
            let r = list.inset(1);
            g.panel(r, 'W');
            g.header(Rect::new(r.x, r.y, r.w, 12), if inside { "Add inside" } else { "Add after" }, true);
            for (i, c) in self.palette_rects(list).iter().enumerate() {
                g.button(*c, PALETTE[i].0, Kind::Normal, widget_state(v.ptr, c, false, false));
            }
        }
        if let Some(n) = &self.note {
            let _ = n;
        }
    }

    fn draw_text(&self, g: &mut Gfx<'_>, v: &View<'_>, s: &Session, p: &Parts, executing: Option<u32>) {
        let err = s.problem();
        let span = s.problem_span();
        let exec_line = executing.and_then(|n| {
            let prog = algoclerk_core::lang::parse::parse(&s.edit.text()).ok()?;
            let sp = prog.span(n)?;
            Some(algoclerk_core::lang::diag::line_col(&s.edit.text(), sp.start).0)
        });
        let sq: Vec<(usize, usize, usize)> = span.into_iter().collect();
        let err_lines: Vec<usize> = span.map(|x| x.0).into_iter().collect();
        textedit::draw_editor(g, &s.edit, p.body, 1, 9, &err_lines, exec_line, &sq, v.tick);
        let kb_btn = Rect::new(p.bar.x, p.bar.y, (p.bar.w / 2).min(70), p.bar.h);
        g.button(kb_btn, if self.kb_open { "Hide keys" } else { "Keys" }, Kind::Normal, widget_state(v.ptr, &kb_btn, false, false));
        let msg_r = Rect::new(kb_btn.right() + 4, p.bar.y, p.bar.w - kb_btn.w - 4, p.bar.h);
        if let Some(d) = err {
            let lines = algoclerk_media::Font::wrap(&d.msg, (msg_r.w / 6).max(8) as usize);
            for (i, l) in lines.iter().take(2).enumerate() {
                g.text(msg_r.x, msg_r.y + 1 + i as i32 * 8, l, 'R');
            }
        } else {
            g.text(msg_r.x, msg_r.y + (msg_r.h - 7) / 2, "Looks good", 'G');
        }
        if self.kb_open {
            let kb_h = 100.min(110);
            let area = Rect::new(p.bar.x, (p.bar.bottom() + 1).min(v.vp.h - kb_h - 2), p.bar.w, kb_h);
            self.kb.draw(g, area, v.ptr);
        }
    }

    fn draw_card(&self, g: &mut Gfx<'_>, v: &View<'_>, s: &Session, area: Rect, card: &CardEdit) {
        g.panel(area, 'W');
        g.header(Rect::new(area.x, area.y, area.w, 12), "Edit card", true);
        let (line_r, msg_r, _tabs_r, tabs, chips_r, ok_r, cancel_r) = self.card_layout(&v.vp, area);
        g.rect(line_r, 'X');
        g.frame(line_r, 'K');
        let txt = card.line.text();
        let cols = ((line_r.w - 6) / 6).max(4) as usize;
        let shown: String = if txt.chars().count() > cols { txt.chars().skip(txt.chars().count() - cols).collect() } else { txt.clone() };
        g.text(line_r.x + 3, line_r.y + (line_r.h - 7) / 2, &shown, 'K');
        let caret_x = line_r.x + 3 + shown.chars().count() as i32 * 6;
        if (v.tick / 30) % 2 == 0 {
            g.rect(Rect::new(caret_x, line_r.y + 3, 1, line_r.h - 6), 'R');
        }
        let (text, colour) = match &card.msg {
            Some(m) if m == "ok" => ("This card is fine.".to_string(), 'G'),
            Some(m) => (m.clone(), 'R'),
            None => (String::new(), 'K'),
        };
        for (i, l) in algoclerk_media::Font::wrap(&text, (msg_r.w / 6) as usize).iter().take(2).enumerate() {
            g.text(msg_r.x, msg_r.y + i as i32 * 8, l, colour);
        }
        let names = ["Names", "123", "Ops", "Tools", "Words"];
        let pads = [Pad::Names, Pad::Numbers, Pad::Ops, Pad::Tools, Pad::Words];
        for (i, r) in tabs.iter().enumerate() {
            g.button(*r, names[i], if card.pad == pads[i] { Kind::On } else { Kind::Normal }, widget_state(v.ptr, r, false, false));
        }
        let labels = self.pad_chips(s, card);
        for (i, r) in self.chip_rects(chips_r, &labels, &v.vp).iter().enumerate() {
            if r.bottom() > chips_r.bottom() {
                break;
            }
            g.button(*r, &labels[i].0, Kind::Normal, widget_state(v.ptr, r, false, false));
        }
        if card.pad == Pad::Tools && card.last_var.is_none() {
            g.text(chips_r.x, chips_r.bottom() - 9, "Pick a name first for its methods", 'k');
        }
        g.button(ok_r, "OK", Kind::Good, widget_state(v.ptr, &ok_r, false, false));
        g.button(cancel_r, "Cancel", Kind::Normal, widget_state(v.ptr, &cancel_r, false, false));
    }
}

fn find_header(p: &algoclerk_core::lang::ast::Program, id: u32) -> Option<String> {
    let mut found = None;
    for f in &p.fns {
        algoclerk_core::lang::ast::walk_stmts(&f.body, &mut |s| {
            if s.id == id {
                found = Some(blocks::header_text(s));
            }
        });
    }
    found
}

#[allow(dead_code)]
fn unused(_: &Input) {}
