// SPDX-License-Identifier: Apache-2.0
//! The program editor: instructions drawn as paper slips (icon, name, operand chip) in a scrolling list with a
//! highlighted executing line, labels as signposts with coloured tabs and jump arrows in the margin, an
//! instruction palette drawer, an operand inspector and drag and drop (mouse drag, or touch long-press).

use crate::gfx::{Gfx, Kind, State};
use crate::input::{Input, Key, LONG_PRESS_TICKS, Pointer};
use crate::layout::{Hit, Viewport};
use crate::session::Session;
use crate::ui::{Ctx, View, widget_state};
use algoclerk_core::asm::{Line, LineKind};
use algoclerk_core::isa::{Addr, Instr, Opcode, OperandKind};
use algoclerk_media::Rect;

/// Colours for labels and their jump arrows.
pub const LABEL_COLORS: [char; 6] = ['R', 'B', 'g', 'Y', 'u', 'o'];

/// Height of a comment row.
const NOTE_H: i32 = 14;

/// Something that can be added from the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// An instruction.
    Op(Opcode),
    /// A label (signpost).
    Label,
}

#[derive(Debug, Clone)]
enum PressTarget {
    Line(usize),
    Card(Item),
    Background,
}

#[derive(Debug, Clone)]
struct Press {
    target: PressTarget,
    started: bool,
}

#[derive(Debug, Clone)]
enum DragSrc {
    Line(usize),
    Card(Item),
}

#[derive(Debug, Clone)]
struct Drag {
    src: DragSrc,
    /// Insertion gap (index into the listing) under the pointer.
    gap: Option<usize>,
    trash: bool,
    pos: (i32, i32),
}

/// Editor state.
#[derive(Debug, Clone, Default)]
pub struct Editor {
    /// Selected listing line.
    pub sel: Option<usize>,
    /// Scroll offset in pixels.
    pub scroll: i32,
    /// Touch: the palette sheet is open.
    pub palette_open: bool,
    /// Keyboard focus is in the palette.
    pub pane_palette: bool,
    /// Focused/hovered palette card.
    pub pal_focus: usize,
    press: Option<Press>,
    drag: Option<Drag>,
    last_exec: Option<usize>,
}

/// Rectangles of the editor panel.
#[derive(Debug, Clone)]
pub struct EditorRects {
    /// The whole panel.
    pub panel: Rect,
    /// Mode toggle: program list.
    pub mode_list: Rect,
    /// Mode toggle: text.
    pub mode_text: Rect,
    /// Move up.
    pub up: Rect,
    /// Move down.
    pub down: Rect,
    /// Delete.
    pub trash: Rect,
    /// Touch: open the palette sheet.
    pub plus: Rect,
    /// Operand inspector.
    pub inspector: Rect,
    /// The scrolling list.
    pub list: Rect,
    /// Palette drawer or sheet (empty when closed on touch).
    pub palette: Rect,
}

impl EditorRects {
    /// Lay out the program mode of the panel.
    pub fn new(panel: Rect, vp: &Viewport, palette_open: bool, items: usize) -> EditorRects {
        let bh = vp.btn_h();
        let pad = 4;
        let tool_y = panel.y + pad;
        let mut x = panel.x + pad;
        let mode_list = Rect::new(x, tool_y, bh + 6, bh);
        x += bh + 6;
        let mode_text = Rect::new(x, tool_y, bh + 6, bh);
        let mut rx = panel.right() - pad;
        let trash = Rect::new(rx - bh, tool_y, bh, bh);
        rx -= bh + 2;
        let down = Rect::new(rx - bh, tool_y, bh, bh);
        rx -= bh + 2;
        let up = Rect::new(rx - bh, tool_y, bh, bh);
        rx -= bh + 2;
        let plus = if vp.touch { Rect::new(rx - bh, tool_y, bh, bh) } else { Rect::new(0, 0, 0, 0) };
        let ins_h = if vp.touch { 36 } else { 24 };
        let body_y = tool_y + bh + 3;
        if vp.touch && palette_open {
            // touch: the palette takes the place of the inspector and the list until a card is chosen
            let body = Rect::new(panel.x + pad, body_y, panel.w - 2 * pad, panel.bottom() - pad - body_y);
            let none = Rect::new(body.x, body.y, body.w, 0);
            return EditorRects { panel, mode_list, mode_text, up, down, trash, plus, inspector: none, list: none, palette: body };
        }
        let inspector = Rect::new(panel.x + pad, body_y, panel.w - 2 * pad, ins_h);
        let top = inspector.bottom() + 3;
        let pal_h = if vp.touch { 0 } else { 2 + 18 + (items.max(1).div_ceil(8) as i32) * 22 + 4 };
        let palette = Rect::new(panel.x + pad, panel.bottom() - pad - pal_h, panel.w - 2 * pad, pal_h);
        let list_bottom = if pal_h > 0 { palette.y - 3 } else { panel.bottom() - pad };
        let list = Rect::new(panel.x + pad, top, panel.w - 2 * pad, (list_bottom - top).max(20));
        EditorRects { panel, mode_list, mode_text, up, down, trash, plus, inspector, list, palette }
    }

    /// Hit rectangles for layout tests.
    pub fn hits(&self, touch: bool) -> Vec<Hit> {
        let mut v = vec![
            Hit::tap("mode list", self.mode_list),
            Hit::tap("mode text", self.mode_text),
            Hit::tap("up", self.up),
            Hit::tap("down", self.down),
            Hit::tap("trash", self.trash),
            Hit::area("inspector", self.inspector),
            Hit::area("list", self.list),
        ];
        if touch {
            v.push(Hit::tap("plus", self.plus));
        }
        if self.palette.h > 0 {
            v.push(Hit::area("palette", self.palette));
        }
        v
    }
}

/// The palette items offered by a session.
pub fn palette_items(st: &Session) -> Vec<Item> {
    let mut v: Vec<Item> = st.allowed.opcodes().into_iter().map(Item::Op).collect();
    v.push(Item::Label);
    v
}

/// Names of the labels in listing order.
pub fn label_names(st: &Session) -> Vec<String> {
    st.listing
        .lines
        .iter()
        .filter_map(|l| if let LineKind::Label(n) = &l.kind { Some(n.clone()) } else { None })
        .collect()
}

fn label_color(st: &Session, name: &str) -> char {
    let idx = label_names(st).iter().position(|n| n == name).unwrap_or(0);
    LABEL_COLORS[idx % LABEL_COLORS.len()]
}

fn fresh_label(st: &Session) -> String {
    let used = label_names(st);
    for base in ["loop", "again", "skip", "done", "next", "back"] {
        if !used.iter().any(|u| u == base) {
            return base.to_string();
        }
    }
    (1..).map(|i| format!("l{i}")).find(|n| !used.contains(n)).unwrap_or_default()
}

/// A new line for a palette item.
pub fn new_line(st: &Session, item: &Item) -> (Line, bool) {
    match item {
        Item::Label => (Line::label(fresh_label(st)), false),
        Item::Op(op) => {
            let target = label_names(st).first().cloned();
            let need_label = op.operand_kind() == OperandKind::Label && target.is_none();
            let name = target.unwrap_or_else(|| "loop".to_string());
            (Line::instr(Instr::with_addr(*op, Addr::Direct(0), name)), need_label)
        }
    }
}

impl Editor {
    /// Row heights and positions in content coordinates.
    fn rows(&self, st: &Session, vp: &Viewport) -> Vec<(i32, i32)> {
        let rh = vp.row_h();
        let mut y = 0;
        let mut out = Vec::with_capacity(st.listing.lines.len());
        for l in &st.listing.lines {
            let h = if matches!(l.kind, LineKind::Blank) { NOTE_H } else { rh };
            out.push((y, h));
            y += h;
        }
        out
    }

    fn content_h(&self, st: &Session, vp: &Viewport) -> i32 {
        self.rows(st, vp).last().map_or(0, |(y, h)| y + h) + 6
    }

    fn clamp_scroll(&mut self, st: &Session, vp: &Viewport, list: &Rect) {
        let max = (self.content_h(st, vp) - list.h).max(0);
        self.scroll = self.scroll.clamp(0, max);
    }

    /// Make sure listing line `idx` is visible.
    pub fn reveal(&mut self, st: &Session, vp: &Viewport, list: &Rect, idx: usize) {
        let rows = self.rows(st, vp);
        if let Some((y, h)) = rows.get(idx) {
            if *y < self.scroll {
                self.scroll = *y;
            } else if y + h > self.scroll + list.h {
                self.scroll = y + h - list.h + 2;
            }
        }
        self.clamp_scroll(st, vp, list);
    }

    fn gap_at(&self, st: &Session, vp: &Viewport, list: &Rect, py: i32) -> usize {
        let rows = self.rows(st, vp);
        let cy = py - list.y + self.scroll;
        for (i, (y, h)) in rows.iter().enumerate() {
            if cy < y + h / 2 {
                return i;
            }
        }
        rows.len()
    }

    fn row_at(&self, st: &Session, vp: &Viewport, list: &Rect, px: i32, py: i32) -> Option<usize> {
        if !list.contains(px, py) {
            return None;
        }
        let cy = py - list.y + self.scroll;
        self.rows(st, vp).iter().position(|(y, h)| cy >= *y && cy < y + h)
    }

    // ------------------------------------------------------------------ editing operations

    /// Insert a palette item after the selection (or at the end). Returns the new line index.
    pub fn insert_item(&mut self, st: &mut Session, item: &Item, at: Option<usize>) -> usize {
        let (line, need_label) = new_line(st, item);
        let mut pos = at.unwrap_or_else(|| self.sel.map_or(st.listing.lines.len(), |s| s + 1)).min(st.listing.lines.len());
        if need_label {
            st.listing.lines.insert(0, Line::label("loop"));
            pos += 1;
        }
        st.listing.lines.insert(pos, line);
        st.after_edit(true);
        self.sel = Some(pos);
        pos
    }

    /// Delete a line.
    pub fn delete_line(&mut self, st: &mut Session, idx: usize) {
        if idx < st.listing.lines.len() {
            st.listing.lines.remove(idx);
            st.after_edit(true);
            let n = st.listing.lines.len();
            self.sel = if n == 0 { None } else { Some(idx.min(n - 1)) };
        }
    }

    /// Move line `from` so it ends up before gap `gap` (an index into the current list).
    pub fn move_line(&mut self, st: &mut Session, from: usize, gap: usize) {
        if from >= st.listing.lines.len() {
            return;
        }
        let line = st.listing.lines.remove(from);
        let to = if gap > from { gap - 1 } else { gap }.min(st.listing.lines.len());
        st.listing.lines.insert(to, line);
        st.after_edit(true);
        self.sel = Some(to);
    }

    fn selected_instr<'a>(&self, st: &'a Session) -> Option<&'a Instr<String>> {
        match &st.listing.lines.get(self.sel?)?.kind {
            LineKind::Instr(i) => Some(i),
            _ => None,
        }
    }

    fn set_instr(&mut self, st: &mut Session, f: impl FnOnce(&Instr<String>) -> Instr<String>) {
        let Some(i) = self.sel else { return };
        if let Some(Line { kind: LineKind::Instr(ins), .. }) = st.listing.lines.get_mut(i) {
            *ins = f(ins);
            st.after_edit(true);
        }
    }

    /// Change the tile operand by `delta` (wraps inside the floor).
    pub fn bump_slot(&mut self, st: &mut Session, delta: i32) {
        let n = st.floor_len.max(1) as i32;
        self.set_instr(st, |ins| {
            let Some(a) = ins.addr() else { return ins.clone() };
            let s = (a.slot() as i32 + delta).rem_euclid(n) as usize;
            let a2 = if a.is_indirect() { Addr::Indirect(s) } else { Addr::Direct(s) };
            Instr::with_addr(ins.opcode(), a2, String::new())
        });
    }

    /// Set the tile operand directly (clicking a tile in the scene).
    pub fn set_slot(&mut self, st: &mut Session, slot: usize) {
        self.set_instr(st, |ins| {
            let Some(a) = ins.addr() else { return ins.clone() };
            let a2 = if a.is_indirect() { Addr::Indirect(slot) } else { Addr::Direct(slot) };
            Instr::with_addr(ins.opcode(), a2, String::new())
        });
    }

    /// Toggle between `n` and `[n]`.
    pub fn toggle_pointer(&mut self, st: &mut Session) {
        if !st.allowed.allows_indirect() {
            return;
        }
        self.set_instr(st, |ins| {
            let Some(a) = ins.addr() else { return ins.clone() };
            let a2 = if a.is_indirect() { Addr::Direct(a.slot()) } else { Addr::Indirect(a.slot()) };
            Instr::with_addr(ins.opcode(), a2, String::new())
        });
    }

    /// Choose the previous/next label as the jump target.
    pub fn cycle_label(&mut self, st: &mut Session, delta: i32) {
        let names = label_names(st);
        if names.is_empty() {
            return;
        }
        self.set_instr(st, |ins| {
            let Some(cur) = ins.target() else { return ins.clone() };
            let idx = names.iter().position(|n| n == cur).map_or(0, |i| i as i32);
            let next = (idx + delta).rem_euclid(names.len() as i32) as usize;
            Instr::with_addr(ins.opcode(), Addr::Direct(0), names[next].clone())
        });
    }

    fn operand_kind(&self, st: &Session) -> Option<OperandKind> {
        self.selected_instr(st).map(|i| i.opcode().operand_kind())
    }

    /// Is the selected instruction one that takes a tile (so scene taps set the operand)?
    pub fn wants_tile(&self, st: &Session) -> bool {
        self.operand_kind(st) == Some(OperandKind::Address)
    }

    // ------------------------------------------------------------------ update

    /// Handle input for the program list. Returns true when the program changed.
    pub fn update(&mut self, cx: &mut Ctx<'_>, st: &mut Session, rects: &EditorRects) -> bool {
        let vp = cx.vp;
        let before = st.listing.clone();
        let ptr = cx.ptr;
        let items = palette_items(st);
        let touch = vp.touch;
        let palette_visible = rects.palette.h > 0;

        // toolbar buttons
        if let Some(sel) = self.sel {
            if ptr.tapped_in(&rects.trash) {
                self.delete_line(st, sel);
                cx.sfx("sfx_putdown");
            } else if ptr.tapped_in(&rects.up) && sel > 0 {
                self.move_line(st, sel, sel - 1);
                cx.sfx("sfx_step");
            } else if ptr.tapped_in(&rects.down) && sel + 1 < st.listing.lines.len() {
                self.move_line(st, sel, sel + 2);
                cx.sfx("sfx_step");
            }
        }
        if touch && ptr.tapped_in(&rects.plus) {
            self.palette_open = !self.palette_open;
            cx.sfx("sfx_button");
        }
        self.inspector_update(cx, st, rects);

        // wheel scroll
        if ptr.over(&rects.list) {
            self.scroll -= (ptr.wheel * 28.0) as i32;
        }

        // press tracking
        if ptr.pressed {
            self.press = None;
            if let Some((x, y)) = ptr.pos {
                if let Some(i) = self.row_at(st, &vp, &rects.list, x, y) {
                    self.press = Some(Press { target: PressTarget::Line(i), started: false });
                } else if rects.list.contains(x, y) {
                    self.press = Some(Press { target: PressTarget::Background, started: false });
                } else if palette_visible && rects.palette.contains(x, y) {
                    if let Some(idx) = self.card_at(&rects.palette, &vp, &items, x, y) {
                        self.press = Some(Press { target: PressTarget::Card(items[idx].clone()), started: false });
                        self.pal_focus = idx;
                    }
                }
            }
        }
        if let (Some(p), true) = (self.press.clone(), ptr.down) {
            let start_drag = if ptr.touch { ptr.long_press() } else { ptr.moved };
            if self.drag.is_none() && !p.started {
                match &p.target {
                    PressTarget::Line(i) if start_drag => {
                        self.drag = Some(Drag { src: DragSrc::Line(*i), gap: None, trash: false, pos: ptr.pos.unwrap_or((0, 0)) });
                        self.sel = Some(*i);
                    }
                    PressTarget::Card(it) if start_drag => {
                        self.drag = Some(Drag { src: DragSrc::Card(it.clone()), gap: None, trash: false, pos: ptr.pos.unwrap_or((0, 0)) });
                    }
                    _ => {}
                }
            }
            // finger or mouse dragging on a row/background scrolls the list when no item drag started
            if self.drag.is_none() && ptr.moved && matches!(p.target, PressTarget::Background | PressTarget::Line(_)) && ptr.touch {
                self.scroll -= ptr.delta.1;
            }
            if self.drag.is_none() && ptr.moved && matches!(p.target, PressTarget::Background) && !ptr.touch {
                self.scroll -= ptr.delta.1;
            }
        }
        if let Some(d) = &mut self.drag {
            if let Some((x, y)) = ptr.pos {
                d.pos = (x, y);
                d.gap = rects.list.contains(x, y).then(|| 0);
                d.trash = rects.palette.h > 0 && rects.palette.contains(x, y) && matches!(d.src, DragSrc::Line(_));
            }
        }
        if let Some(d) = self.drag.clone() {
            if let Some((_, y)) = ptr.pos {
                if d.gap.is_some() {
                    let gap = self.gap_at(st, &vp, &rects.list, y);
                    if let Some(g) = self.drag.as_mut() {
                        g.gap = Some(gap);
                    }
                }
            }
            // autoscroll near the edges
            if let Some((_, y)) = ptr.pos {
                if y < rects.list.y + 12 {
                    self.scroll -= 3;
                } else if y > rects.list.bottom() - 12 {
                    self.scroll += 3;
                }
            }
        }
        if ptr.released {
            if let Some(d) = self.drag.take() {
                match d.src {
                    DragSrc::Line(i) => {
                        if d.trash {
                            self.delete_line(st, i);
                            cx.sfx("sfx_putdown");
                        } else if let Some(g) = d.gap {
                            self.move_line(st, i, g);
                            cx.sfx("sfx_putdown");
                        }
                    }
                    DragSrc::Card(item) => {
                        if let Some(g) = d.gap {
                            self.insert_item(st, &item, Some(g));
                            cx.sfx("sfx_putdown");
                        }
                        if touch {
                            self.palette_open = false;
                        }
                    }
                }
            } else if let Some(p) = self.press.take() {
                if !ptr.moved {
                    match p.target {
                        PressTarget::Line(i) => {
                            self.sel = Some(i);
                            self.pane_palette = false;
                            cx.sfx("sfx_step");
                        }
                        PressTarget::Card(item) => {
                            self.insert_item(st, &item, None);
                            cx.sfx("sfx_putdown");
                            if touch {
                                self.palette_open = false;
                            }
                        }
                        PressTarget::Background => {
                            self.sel = None;
                        }
                    }
                }
            }
            self.press = None;
        }
        // hover in the palette moves the focus (mouse only)
        if !ptr.touch && palette_visible {
            if let Some((x, y)) = ptr.pos {
                if let Some(i) = self.card_at(&rects.palette, &vp, &items, x, y) {
                    self.pal_focus = i;
                }
            }
        }
        self.keys(cx, st, rects, &items);
        // keep the executing line in view
        let exec = st.executing_line();
        if exec != self.last_exec {
            if let Some(i) = exec {
                self.reveal(st, &vp, &rects.list, i);
            }
            self.last_exec = exec;
        }
        self.clamp_scroll(st, &vp, &rects.list);
        before != st.listing
    }

    fn keys(&mut self, cx: &mut Ctx<'_>, st: &mut Session, rects: &EditorRects, items: &[Item]) {
        let input: &Input = cx.input;
        if input.key(Key::Tab) {
            self.pane_palette = !self.pane_palette;
            if self.pane_palette {
                self.palette_open = true;
            }
        }
        let n = st.listing.lines.len();
        if self.pane_palette {
            let per_row = self.cards_per_row(&cx.vp, &rects.palette).max(1);
            if input.key(Key::Left) {
                self.pal_focus = (self.pal_focus + items.len() - 1) % items.len();
            }
            if input.key(Key::Right) {
                self.pal_focus = (self.pal_focus + 1) % items.len();
            }
            if input.key(Key::Up) && self.pal_focus >= per_row {
                self.pal_focus -= per_row;
            }
            if input.key(Key::Down) && self.pal_focus + per_row < items.len() {
                self.pal_focus += per_row;
            }
            if input.key(Key::Enter) || input.key(Key::Space) {
                let it = items[self.pal_focus.min(items.len() - 1)].clone();
                self.insert_item(st, &it, None);
                cx.sfx("sfx_putdown");
                if cx.vp.touch {
                    self.palette_open = false;
                    self.pane_palette = false;
                }
            }
            return;
        }
        let shift = input.shift;
        if input.key(Key::Down) && n > 0 {
            match self.sel {
                None => self.sel = Some(0),
                Some(s) if shift && s + 1 < n => self.move_line(st, s, s + 2),
                Some(s) if s + 1 < n => self.sel = Some(s + 1),
                _ => {}
            }
        }
        if input.key(Key::Up) && n > 0 {
            match self.sel {
                None => self.sel = Some(n - 1),
                Some(s) if shift && s > 0 => self.move_line(st, s, s - 1),
                Some(s) if s > 0 => self.sel = Some(s - 1),
                _ => {}
            }
        }
        if (input.key(Key::Delete) || input.key(Key::Backspace)) && self.sel.is_some() {
            if let Some(s) = self.sel {
                self.delete_line(st, s);
            }
        }
        match self.operand_kind(st) {
            Some(OperandKind::Address) => {
                if input.key(Key::Left) {
                    self.bump_slot(st, -1);
                }
                if input.key(Key::Right) {
                    self.bump_slot(st, 1);
                }
                if input.ch('p') {
                    self.toggle_pointer(st);
                }
            }
            Some(OperandKind::Label) => {
                if input.key(Key::Left) {
                    self.cycle_label(st, -1);
                }
                if input.key(Key::Right) {
                    self.cycle_label(st, 1);
                }
            }
            _ => {}
        }
        if let Some(s) = self.sel {
            if s >= n {
                self.sel = if n == 0 { None } else { Some(n - 1) };
            }
        }
        if let Some(s) = self.sel {
            self.reveal(st, &cx.vp, &rects.list, s);
        }
    }

    // ------------------------------------------------------------------ palette geometry

    fn card_size(&self, vp: &Viewport) -> (i32, i32) {
        if vp.touch { (vp.hit_min(), vp.hit_min()) } else { (24, 22) }
    }

    fn cards_per_row(&self, vp: &Viewport, pal: &Rect) -> usize {
        let (w, _) = self.card_size(vp);
        ((pal.w - 4) / w).max(1) as usize
    }

    fn card_rect(&self, vp: &Viewport, pal: &Rect, i: usize) -> Rect {
        let (w, h) = self.card_size(vp);
        let per = self.cards_per_row(vp, pal);
        let cols_w = per as i32 * w;
        let x0 = pal.x + (pal.w - cols_w) / 2;
        let y0 = pal.y + 20;
        Rect::new(x0 + (i % per) as i32 * w, y0 + (i / per) as i32 * h, w, h)
    }

    fn card_at(&self, pal: &Rect, vp: &Viewport, items: &[Item], x: i32, y: i32) -> Option<usize> {
        (0..items.len()).find(|i| self.card_rect(vp, pal, *i).contains(x, y))
    }

    // ------------------------------------------------------------------ inspector

    /// Rectangles of the inspector controls: (minus, value, plus, pointer toggle).
    pub fn inspector_rects(ins: &Rect, vp: &Viewport) -> [Rect; 4] {
        let bh = ins.h - 4;
        let bw = if vp.touch { vp.hit_min() } else { 18 };
        let x0 = ins.x + 4 + 6 * 5 + 6;
        let minus = Rect::new(x0, ins.y + 2, bw, bh);
        let val = Rect::new(minus.right() + 2, ins.y + 2, 34, bh);
        let plus = Rect::new(val.right() + 2, ins.y + 2, bw, bh);
        let ptr = Rect::new(plus.right() + 6, ins.y + 2, bw + 4, bh);
        [minus, val, plus, ptr]
    }

    fn inspector_update(&mut self, cx: &mut Ctx<'_>, st: &mut Session, rects: &EditorRects) {
        let [minus, _, plus, ptrb] = Editor::inspector_rects(&rects.inspector, &cx.vp);
        let ptr = cx.ptr;
        match self.operand_kind(st) {
            Some(OperandKind::Address) => {
                if ptr.tapped_in(&minus) {
                    self.bump_slot(st, -1);
                    cx.sfx("sfx_step");
                }
                if ptr.tapped_in(&plus) {
                    self.bump_slot(st, 1);
                    cx.sfx("sfx_step");
                }
                if ptr.tapped_in(&ptrb) {
                    self.toggle_pointer(st);
                    cx.sfx("sfx_button");
                }
            }
            Some(OperandKind::Label) => {
                if ptr.tapped_in(&minus) {
                    self.cycle_label(st, -1);
                    cx.sfx("sfx_step");
                }
                if ptr.tapped_in(&plus) {
                    self.cycle_label(st, 1);
                    cx.sfx("sfx_step");
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ drawing

    /// Draw the toolbar, inspector, list and palette.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>, st: &Session, rects: &EditorRects, text_mode: bool) {
        let ptr = v.ptr;
        g.panel(rects.panel, 'w');
        // toolbar
        g.icon_button(rects.mode_list, "list", if text_mode { Kind::Normal } else { Kind::On }, widget_state(ptr, &rects.mode_list, false, false));
        g.icon_button(rects.mode_text, "text", if text_mode { Kind::On } else { Kind::Normal }, widget_state(ptr, &rects.mode_text, false, false));
        let has_sel = self.sel.is_some();
        let n = st.listing.lines.len();
        let sel = self.sel.unwrap_or(0);
        g.icon_button(rects.up, "up", Kind::Normal, widget_state(ptr, &rects.up, false, !has_sel || sel == 0));
        g.icon_button(rects.down, "down", Kind::Normal, widget_state(ptr, &rects.down, false, !has_sel || sel + 1 >= n));
        g.icon_button(rects.trash, "trash", Kind::Danger, widget_state(ptr, &rects.trash, false, !has_sel));
        if v.vp.touch {
            g.icon_button(rects.plus, "plus", if self.palette_open { Kind::On } else { Kind::Primary }, widget_state(ptr, &rects.plus, false, false));
        } else if v.vp.portrait {
            let size = format!("{} {}", v.tr.t("editor.size"), st.listing.instruction_count());
            let sw = g.text_w(&size, 1);
            let x = rects.mode_text.right() + 8;
            if x + sw < rects.up.x - 4 {
                g.text(x, rects.mode_list.y + (rects.mode_list.h - 7) / 2, &size, 'k');
            }
        }
        if !(v.vp.touch && self.palette_open) {
            self.draw_inspector(g, v, st, rects);
            self.draw_list(g, v, st, rects);
        }
        if rects.palette.h > 0 {
            self.draw_palette(g, v, st, rects);
        }
        if let Some(d) = &self.drag {
            self.draw_drag(g, v, st, d);
        }
    }

    fn draw_inspector(&self, g: &mut Gfx<'_>, v: &View<'_>, st: &Session, rects: &EditorRects) {
        let ins = rects.inspector;
        g.round_rect(ins, 'W', 'K');
        let ptr = v.ptr;
        let [minus, val, plus, ptrb] = Editor::inspector_rects(&ins, &v.vp);
        let ty = ins.y + (ins.h - 7) / 2;
        match self.selected_instr(st).map(|i| (i.opcode().operand_kind(), i.clone())) {
            Some((OperandKind::Address, ins_i)) => {
                let a = ins_i.addr().unwrap_or(Addr::Direct(0));
                g.text(ins.x + 5, ty, "TILE", 'k');
                g.icon_button(minus, "minus", Kind::Normal, widget_state(ptr, &minus, false, false));
                g.round_rect(val, 'c', 'K');
                g.text_in(val, &a.to_string(), 'K', 1);
                g.icon_button(plus, "plus", Kind::Normal, widget_state(ptr, &plus, false, false));
                g.icon_button(ptrb, "pointer", if a.is_indirect() { Kind::On } else { Kind::Normal }, widget_state(ptr, &ptrb, false, !st.allowed.allows_indirect()));
                if ptrb.right() + 6 < ins.right() {
                    if let Some(name) = st.level.as_ref().and_then(|l| l.slot_name(a.slot())) {
                        g.text(ptrb.right() + 6, ty, &crate::gfx::clip(name, ((ins.right() - ptrb.right() - 8) / 6).max(1) as usize), 'b');
                    }
                }
            }
            Some((OperandKind::Label, ins_i)) => {
                let name = ins_i.target().cloned().unwrap_or_default();
                let col = label_color(st, &name);
                g.text(ins.x + 5, ty, "GOTO", 'k');
                g.icon_button(minus, "left", Kind::Normal, widget_state(ptr, &minus, false, false));
                let chip = Rect::new(val.x, val.y, val.w + 20, val.h);
                g.round_rect(chip, col, 'K');
                g.text_in(chip, &crate::gfx::clip(&name, 8), if col == 'Y' { 'K' } else { 'W' }, 1);
                let plus2 = Rect::new(chip.right() + 2, plus.y, plus.w, plus.h);
                g.icon_button(plus2, "right", Kind::Normal, widget_state(ptr, &plus2, false, false));
            }
            Some((OperandKind::None, _)) => {
                g.text(ins.x + 5, ty, &v.tr.t("editor.no_operand"), 'a');
            }
            None => {
                let msg = if st.listing.lines.is_empty() { v.tr.t("editor.empty_short") } else { v.tr.t("editor.hint_select") };
                g.text(ins.x + 5, ty, &crate::gfx::clip(&msg, ((ins.w - 10) / 6) as usize), 'a');
            }
        }
    }

    fn draw_list(&self, g: &mut Gfx<'_>, v: &View<'_>, st: &Session, rects: &EditorRects) {
        let list = rects.list;
        g.rect(list, 'A');
        g.frame(list, 'a');
        let saved = g.c.set_clip(list.inset(1).offset(g.off.0, g.off.1));
        let rows = self.rows(st, &v.vp);
        let exec = st.executing_line();
        let margin = 16;
        // jump arrows in the margin
        for (i, l) in st.listing.lines.iter().enumerate() {
            let LineKind::Instr(ins) = &l.kind else { continue };
            let Some(t) = ins.target() else { continue };
            let Some(li) = st.listing.lines.iter().position(|x| matches!(&x.kind, LineKind::Label(n) if n == t)) else { continue };
            let col = label_color(st, t);
            let lane = label_names(st).iter().position(|n| n == t).unwrap_or(0) % 5;
            let x = list.x + 3 + lane as i32 * 2;
            let (jy, jh) = rows[i];
            let (ly, lh) = rows[li];
            let y1 = list.y - self.scroll + jy + jh / 2;
            let y2 = list.y - self.scroll + ly + lh / 2;
            g.hline(x, y1, margin - lane as i32 * 2 - 2, col);
            g.vline(x, y1.min(y2), (y1 - y2).abs() + 1, col);
            g.hline(x, y2, margin - lane as i32 * 2 - 2, col);
            // arrow head at the label end
            g.px(x + margin - lane as i32 * 2 - 3, y2 - 1, col);
            g.px(x + margin - lane as i32 * 2 - 3, y2 + 1, col);
            g.px(x + margin - lane as i32 * 2 - 4, y2 - 2, col);
            g.px(x + margin - lane as i32 * 2 - 4, y2 + 2, col);
        }
        for (i, l) in st.listing.lines.iter().enumerate() {
            let (cy, h) = rows[i];
            let y = list.y - self.scroll + cy;
            if y + h < list.y || y > list.bottom() {
                continue;
            }
            let slip = Rect::new(list.x + margin, y + 1, list.w - margin - 7, h - 2);
            let selected = self.sel == Some(i);
            let executing = exec == Some(i);
            match &l.kind {
                LineKind::Instr(ins) => self.draw_slip(g, v, st, slip, ins, selected, executing, l.comment.as_deref()),
                LineKind::Label(name) => self.draw_label_row(g, slip, name, st, selected),
                LineKind::Blank => {
                    let t = l.comment.clone().unwrap_or_default();
                    g.rect(slip, 'E');
                    g.text(slip.x + 3, slip.y + (slip.h - 7) / 2, &crate::gfx::clip(&format!("-- {t}"), ((slip.w - 6) / 6) as usize), 'k');
                    if selected {
                        g.frame(slip, 'B');
                    }
                }
            }
            if executing {
                let bob = if v.progress.settings.reduce_motion { 0 } else { ((g.tick / 14) % 2) as i32 };
                g.sprite("ui/cursor", 0, slip.x - 9 + bob, slip.y + (slip.h - 9) / 2);
            }
        }
        if st.listing.lines.is_empty() {
            let msg = v.tr.t("editor.empty");
            g.paragraph(list.x + 8, list.y + 10, ((list.w - 16) / 6) as usize, &msg, 'k', 1, 10, 6);
        }
        // drop indicator
        if let Some(Drag { gap: Some(gap), .. }) = &self.drag {
            let y = if let Some((ry, _)) = rows.get(*gap) {
                list.y - self.scroll + ry
            } else {
                list.y - self.scroll + rows.last().map_or(0, |(y, h)| y + h)
            };
            g.rect(Rect::new(list.x + margin, y - 1, list.w - margin - 7, 3), 'R');
            g.rect(Rect::new(list.x + margin - 4, y - 3, 4, 7), 'R');
        }
        g.c.set_clip(saved);
        g.scrollbar(Rect::new(list.right() - 6, list.y + 2, 4, list.h - 4), self.scroll, self.content_h(st, &v.vp), list.h);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_slip(&self, g: &mut Gfx<'_>, v: &View<'_>, st: &Session, r: Rect, ins: &Instr<String>, selected: bool, executing: bool, comment: Option<&str>) {
        let (fill, border) = if executing { ('y', 'K') } else { ('W', 'K') };
        g.round_rect(r.offset(1, 1), 'a', 'a');
        g.round_rect(r, fill, border);
        if executing {
            g.frame(r.inset(1), 'Y');
        }
        if selected {
            g.frame(r.inset(1), 'B');
            g.frame(r.inset(2), 'B');
        }
        // punch holes
        for dy in [r.h / 3, 2 * r.h / 3] {
            g.px(r.x + 4, r.y + dy, 'a');
        }
        let key = ins.opcode().key();
        g.sprite(&format!("icons/op_{key}"), 0, r.x + 8, r.y + (r.h - 12) / 2);
        g.text(r.x + 24, r.y + (r.h - 7) / 2, ins.opcode().mnemonic(), 'K');
        // operand chip
        if let Some(a) = ins.addr() {
            let text = a.to_string();
            let w = text.chars().count() as i32 * 6 + 10;
            let chip = Rect::new(r.right() - w - 5, r.y + 3, w, r.h - 6);
            g.round_rect(chip, 'c', 'K');
            g.text_in(chip, &text, 'K', 1);
            if a.is_indirect() {
                g.sprite("ui/pointer", 0, chip.x - 9, chip.y + (chip.h - 10) / 2);
            }
            if let Some(name) = st.level.as_ref().and_then(|l| l.slot_name(a.slot())) {
                let max = ((chip.x - (r.x + 24 + 8 * 6) - 12) / 6).max(0) as usize;
                if max >= 2 {
                    g.text(chip.x - 6 - (max.min(name.chars().count()) as i32) * 6, r.y + (r.h - 7) / 2, &crate::gfx::clip(name, max), 'b');
                }
            }
        } else if let Some(t) = ins.target() {
            let col = label_color(st, t);
            let name = crate::gfx::clip(t, 8);
            let w = name.chars().count() as i32 * 6 + 10;
            let chip = Rect::new(r.right() - w - 5, r.y + 3, w, r.h - 6);
            g.round_rect(chip, col, 'K');
            g.text_in(chip, &name, if col == 'Y' { 'K' } else { 'W' }, 1);
        }
        let _ = (v, comment);
    }

    fn draw_label_row(&self, g: &mut Gfx<'_>, r: Rect, name: &str, st: &Session, selected: bool) {
        let col = label_color(st, name);
        g.rect(r, 'w');
        g.rect(Rect::new(r.x, r.y, 6, r.h), col);
        g.frame(r, 'K');
        g.sprite("icons/signpost", 0, r.x + 10, r.y + (r.h - 12) / 2);
        g.text(r.x + 26, r.y + (r.h - 7) / 2, &format!("{name}:"), 'K');
        if selected {
            g.frame(r.inset(1), 'B');
            g.frame(r.inset(2), 'B');
        }
    }

    fn draw_palette(&self, g: &mut Gfx<'_>, v: &View<'_>, st: &Session, rects: &EditorRects) {
        let pal = rects.palette;
        g.round_rect(pal, 'k', 'K');
        let items = palette_items(st);
        let focus = self.pal_focus.min(items.len() - 1);
        // description of the focused card
        let (name, desc) = match &items[focus] {
            Item::Op(op) => (op.mnemonic().to_string(), v.tr.t(&format!("op.{}", op.key()))),
            Item::Label => (v.tr.t("editor.label_name"), v.tr.t("op.label")),
        };
        g.text(pal.x + 4, pal.y + 3, &name, 'y');
        let cols = ((pal.w - 8) / 6) as usize;
        let first = algoclerk_media::Font::wrap(&desc, cols.saturating_sub(name.chars().count() + 1).max(8));
        if let Some(l) = first.first() {
            g.text(pal.x + 4 + (name.chars().count() as i32 + 1) * 6, pal.y + 3, &crate::gfx::clip(l, cols.saturating_sub(name.chars().count() + 1)), 'A');
        }
        for (i, item) in items.iter().enumerate() {
            let r = self.card_rect(&v.vp, &pal, i);
            let r = r.inset(1);
            let focused = (self.pane_palette || !v.ptr.touch) && i == focus;
            let kind = if matches!(item, Item::Label) { Kind::Primary } else { Kind::Normal };
            let st_w = State { hover: v.ptr.hover(&r), down: v.ptr.held_in(&r) && v.ptr.over(&r), disabled: false, focus: self.pane_palette && focused };
            let face = g.button_box(r, kind, st_w);
            match item {
                Item::Op(op) => {
                    let key = format!("icons/op_{}", op.key());
                    let (w, h) = { let s = g.bank.sp(&key); (s.w as i32, s.h as i32) };
                    g.sprite(&key, 0, face.x + (face.w - w) / 2, face.y + (face.h - h) / 2);
                }
                Item::Label => g.icon_in("label", face, 'K'),
            }
        }
    }

    fn draw_drag(&self, g: &mut Gfx<'_>, v: &View<'_>, st: &Session, d: &Drag) {
        let (x, y) = d.pos;
        let w = 112;
        let r = Rect::new(x - 20, y - 10, w, 20);
        match &d.src {
            DragSrc::Line(i) => {
                if let Some(Line { kind: LineKind::Instr(ins), .. }) = st.listing.lines.get(*i) {
                    self.draw_slip(g, v, st, r, ins, false, false, None);
                } else if let Some(Line { kind: LineKind::Label(n), .. }) = st.listing.lines.get(*i) {
                    self.draw_label_row(g, r, n, st, false);
                }
            }
            DragSrc::Card(Item::Op(op)) => {
                g.round_rect(r, 'W', 'K');
                g.sprite(&format!("icons/op_{}", op.key()), 0, r.x + 6, r.y + 4);
                g.text(r.x + 24, r.y + 7, op.mnemonic(), 'K');
            }
            DragSrc::Card(Item::Label) => {
                g.round_rect(r, 'w', 'K');
                g.sprite("icons/signpost", 0, r.x + 6, r.y + 4);
                g.text(r.x + 24, r.y + 7, &v.tr.t("editor.label_name"), 'K');
            }
        }
        if d.trash {
            g.icon("trash", x + 18, y - 18, 'R');
        }
    }
}

/// Re-exported for the play screen.
pub fn long_press_ticks() -> u32 {
    LONG_PRESS_TICKS
}

/// Unused pointer helper kept for symmetry with other widgets.
pub fn pointer_over(ptr: &Pointer, r: &Rect) -> bool {
    ptr.over(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoclerk_media::{ArtBank, Assets, Outfit};

    fn session() -> (Session, ArtBank) {
        let bank = ArtBank::load(&Assets::embedded()).unwrap();
        let s = Session::sandbox("", &bank, &Outfit::default(), 1);
        (s, bank)
    }

    #[test]
    fn insert_move_delete_roundtrip_to_text() {
        let (mut st, _b) = session();
        let mut ed = Editor::default();
        ed.insert_item(&mut st, &Item::Op(Opcode::Inbox), None);
        ed.insert_item(&mut st, &Item::Op(Opcode::Outbox), None);
        ed.insert_item(&mut st, &Item::Op(Opcode::Jump), None);
        assert!(st.text.contains("INBOX") && st.text.contains("OUTBOX") && st.text.contains("loop:"));
        assert!(st.problems.is_empty(), "{:?}", st.problems);
        ed.move_line(&mut st, 3, 1);
        assert!(st.listing.lines[1].kind != st.listing.lines[3].kind);
        ed.delete_line(&mut st, 1);
        assert_eq!(st.listing.lines.len(), 3);
    }

    #[test]
    fn operands_change_through_the_inspector_helpers() {
        let (mut st, _b) = session();
        let mut ed = Editor::default();
        ed.insert_item(&mut st, &Item::Op(Opcode::CopyTo), None);
        ed.bump_slot(&mut st, 1);
        assert!(st.text.contains("COPYTO 1"));
        ed.bump_slot(&mut st, -2);
        assert!(st.text.contains(&format!("COPYTO {}", st.floor_len - 1)));
        ed.toggle_pointer(&mut st);
        assert!(st.text.contains('['));
        ed.set_slot(&mut st, 4);
        assert!(st.text.contains("COPYTO [4]"));
    }

    #[test]
    fn label_cycling_picks_other_labels() {
        let (mut st, _b) = session();
        let mut ed = Editor::default();
        ed.insert_item(&mut st, &Item::Label, None);
        ed.insert_item(&mut st, &Item::Label, None);
        ed.insert_item(&mut st, &Item::Op(Opcode::Jump), None);
        let first = st.text.clone();
        ed.cycle_label(&mut st, 1);
        assert_ne!(first, st.text);
    }

    #[test]
    fn rects_stay_inside_the_panel() {
        for touch in [false, true] {
            for portrait in [false, true] {
                let vp = Viewport::new(portrait, touch);
                let panel = if portrait { Rect::new(0, 250, 270, 230) } else { Rect::new(272, 18, 208, 252) };
                for open in [false, true] {
                    let r = EditorRects::new(panel, &vp, open, 15);
                    for h in r.hits(touch) {
                        assert!(h.rect.inside(&panel), "{} {:?} outside {:?} (touch {touch}, portrait {portrait})", h.name, h.rect, panel);
                    }
                }
            }
        }
    }
}
