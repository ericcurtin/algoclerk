// SPDX-License-Identifier: Apache-2.0
//! A small pixel-font text editor (cursor, selection, copy/cut/paste, keyboard input) and the on-screen
//! letter-grid keyboard for touch screens.

use crate::gfx::{Gfx, Kind, State};
use crate::input::{Input, Key, Pointer};
use algoclerk_media::Rect;

/// An editable multi-line text buffer.
#[derive(Debug, Clone, Default)]
pub struct TextEdit {
    chars: Vec<char>,
    /// Cursor position in characters.
    pub cursor: usize,
    /// Selection anchor (the selection runs between anchor and cursor).
    pub anchor: Option<usize>,
    /// First visible line.
    pub scroll_line: usize,
    /// First visible column.
    pub scroll_col: usize,
    /// Keys and typing go here.
    pub focused: bool,
    /// Ticks for the caret blink.
    pub blink: u32,
    /// Allow newlines (single-line fields set this to false).
    pub multiline: bool,
    drag_from: Option<usize>,
}

/// What an editing call produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditOutcome {
    /// The text changed.
    pub changed: bool,
    /// Text to put on the clipboard (copy or cut).
    pub copy: Option<String>,
    /// The user asked to paste (ctrl+V).
    pub want_paste: bool,
}

impl TextEdit {
    /// A buffer with initial text.
    pub fn new(text: &str, multiline: bool) -> TextEdit {
        let chars: Vec<char> = text.chars().collect();
        TextEdit {
            cursor: chars.len(),
            chars,
            multiline,
            ..TextEdit::default()
        }
    }

    /// The text.
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Replace the text, keeping the cursor near where it was.
    pub fn set_text(&mut self, text: &str) {
        let keep = self.cursor;
        self.chars = text.chars().collect();
        self.cursor = keep.min(self.chars.len());
        self.anchor = None;
    }

    /// Number of lines.
    pub fn line_count(&self) -> usize {
        self.chars.iter().filter(|c| **c == '\n').count() + 1
    }

    /// `(line, column)` of a character index.
    pub fn line_col(&self, idx: usize) -> (usize, usize) {
        let idx = idx.min(self.chars.len());
        let mut line = 0;
        let mut col = 0;
        for c in &self.chars[..idx] {
            if *c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn line_start(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        let mut seen = 0;
        for (i, c) in self.chars.iter().enumerate() {
            if *c == '\n' {
                seen += 1;
                if seen == line {
                    return i + 1;
                }
            }
        }
        self.chars.len()
    }

    fn line_len(&self, line: usize) -> usize {
        let s = self.line_start(line);
        self.chars[s..].iter().take_while(|c| **c != '\n').count()
    }

    /// Character index for `(line, col)`, clamped.
    pub fn index_at(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.line_count() - 1);
        self.line_start(line) + col.min(self.line_len(line))
    }

    /// The selection as an ordered range.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.anchor.filter(|a| *a != self.cursor).map(|a| (a.min(self.cursor), a.max(self.cursor)))
    }

    /// The selected text.
    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|(a, b)| self.chars[a..b].iter().collect())
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((a, b)) = self.selection() {
            self.chars.drain(a..b);
            self.cursor = a;
            self.anchor = None;
            true
        } else {
            self.anchor = None;
            false
        }
    }

    /// Insert text at the cursor (replacing the selection).
    pub fn insert(&mut self, s: &str) {
        self.delete_selection();
        for c in s.chars() {
            if c == '\r' {
                continue;
            }
            if c == '\n' && !self.multiline {
                continue;
            }
            if c != '\n' && c != '\t' && !(' '..='~').contains(&c) {
                continue;
            }
            let c = if c == '\t' { ' ' } else { c };
            self.chars.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Backspace.
    pub fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
            true
        } else {
            false
        }
    }

    /// Delete.
    pub fn delete(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
            true
        } else {
            false
        }
    }

    fn move_to(&mut self, idx: usize, extend: bool) {
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        self.cursor = idx.min(self.chars.len());
        self.blink = 0;
    }

    /// Handle one tick of keyboard input. Only call when focused.
    pub fn handle(&mut self, input: &Input) -> EditOutcome {
        let mut out = EditOutcome::default();
        self.blink = self.blink.wrapping_add(1);
        if input.ctrl_ch('a') {
            self.anchor = Some(0);
            self.cursor = self.chars.len();
        }
        if input.ctrl_ch('c') || input.ctrl_ch('x') {
            if let Some(t) = self.selected_text() {
                out.copy = Some(t);
                if input.ctrl_ch('x') {
                    out.changed |= self.delete_selection();
                }
            }
        }
        if input.ctrl_ch('v') {
            out.want_paste = true;
        }
        if let Some(p) = &input.paste {
            self.insert(p);
            out.changed = true;
        }
        for k in &input.keys {
            let (line, col) = self.line_col(self.cursor);
            match k {
                Key::Left => self.move_to(self.cursor.saturating_sub(1), input.shift),
                Key::Right => self.move_to(self.cursor + 1, input.shift),
                Key::Up => {
                    let idx = if line == 0 { 0 } else { self.index_at(line - 1, col) };
                    self.move_to(idx, input.shift);
                }
                Key::Down => {
                    let idx = if line + 1 >= self.line_count() { self.chars.len() } else { self.index_at(line + 1, col) };
                    self.move_to(idx, input.shift);
                }
                Key::Home => self.move_to(self.index_at(line, 0), input.shift),
                Key::End => self.move_to(self.index_at(line, usize::MAX / 2), input.shift),
                Key::PageUp => self.move_to(self.index_at(line.saturating_sub(6), col), input.shift),
                Key::PageDown => self.move_to(self.index_at(line + 6, col), input.shift),
                Key::Backspace => out.changed |= self.backspace(),
                Key::Delete => out.changed |= self.delete(),
                Key::Enter if self.multiline => {
                    self.insert("\n");
                    out.changed = true;
                }
                _ => {}
            }
        }
        if !input.typed.is_empty() && !input.ctrl {
            let before = self.chars.len();
            let typed: String = input.typed.chars().filter(|c| !c.is_control()).collect();
            self.insert(&typed);
            out.changed |= self.chars.len() != before || !typed.is_empty();
        }
        out
    }

    /// Keep the cursor inside the visible window of `rows` x `cols`.
    pub fn scroll_to_cursor(&mut self, rows: usize, cols: usize) {
        let (line, col) = self.line_col(self.cursor);
        if line < self.scroll_line {
            self.scroll_line = line;
        }
        if line >= self.scroll_line + rows.max(1) {
            self.scroll_line = line + 1 - rows.max(1);
        }
        if col < self.scroll_col {
            self.scroll_col = col;
        }
        if col >= self.scroll_col + cols.max(1) {
            self.scroll_col = col + 1 - cols.max(1);
        }
        let max_line = self.line_count().saturating_sub(1);
        self.scroll_line = self.scroll_line.min(max_line);
    }

    /// Scroll with the wheel or a drag (lines).
    pub fn scroll_by(&mut self, lines: i32, rows: usize) {
        let max = self.line_count().saturating_sub(rows.max(1)) as i32;
        self.scroll_line = (self.scroll_line as i32 + lines).clamp(0, max.max(0)) as usize;
    }

    /// Character index under a pixel inside the text area `r` (for clicking and dragging).
    pub fn index_at_pixel(&self, r: Rect, scale: i32, line_h: i32, x: i32, y: i32) -> usize {
        let line = (((y - r.y) / line_h.max(1)).max(0) as usize) + self.scroll_line;
        let col = (((x - r.x + 3 * scale) / (6 * scale)).max(0) as usize) + self.scroll_col;
        self.index_at(line, col)
    }

    /// Mouse/touch interaction: click to place the caret, drag to select, tap to focus.
    pub fn pointer(&mut self, ptr: &Pointer, r: Rect, scale: i32, line_h: i32) {
        if ptr.pressed && ptr.over(&r) {
            self.focused = true;
            if let Some((x, y)) = ptr.pos {
                let i = self.index_at_pixel(r, scale, line_h, x, y);
                self.drag_from = Some(i);
                self.anchor = None;
                self.cursor = i;
                self.blink = 0;
            }
        } else if ptr.down && ptr.moved && self.drag_from.is_some() {
            if let Some((x, y)) = ptr.pos {
                let i = self.index_at_pixel(r, scale, line_h, x, y);
                self.anchor = self.drag_from;
                self.cursor = i;
            }
        }
        if ptr.released {
            self.drag_from = None;
        }
    }
}

/// Assembly colouring of one line: `(text, colour letter)` spans.
pub fn asm_spans(line: &str) -> Vec<(String, char)> {
    let (code, comment) = match line.find("--") {
        Some(i) => (&line[..i], Some(&line[i..])),
        None => (line, None),
    };
    let mut spans = Vec::new();
    let trimmed = code.trim_end();
    let indent = code.len() - code.trim_start().len();
    if indent > 0 {
        spans.push((" ".repeat(indent), 'K'));
    }
    let body = trimmed.trim_start();
    if body.ends_with(':') {
        spans.push((body.to_string(), 'u'));
    } else if let Some((op, rest)) = body.split_once(' ') {
        spans.push((op.to_string(), 'b'));
        spans.push((format!(" {rest}"), 'K'));
    } else if !body.is_empty() {
        spans.push((body.to_string(), 'b'));
    }
    let used: usize = spans.iter().map(|(s, _)| s.chars().count()).sum();
    let total = code.chars().count();
    if total > used {
        spans.push((" ".repeat(total - used), 'K'));
    }
    if let Some(c) = comment {
        spans.push((c.to_string(), 'a'));
    }
    spans
}

/// Draw a text editor into `r`. `error_lines` are one-based lines with problems (marked in red).
#[allow(clippy::too_many_arguments)]
pub fn draw_editor(
    g: &mut Gfx<'_>,
    ed: &TextEdit,
    r: Rect,
    scale: i32,
    line_h: i32,
    error_lines: &[usize],
    highlight_line: Option<usize>,
    tick: u64,
) {
    g.rect(r, 'W');
    g.frame(r, 'K');
    let inner = r.inset(2);
    let saved = g.c.set_clip(inner.offset(g.off.0, g.off.1));
    let rows = (inner.h / line_h).max(1) as usize;
    let cols = ((inner.w - 2) / (6 * scale)).max(1) as usize;
    let text = ed.text();
    let sel = ed.selection();
    let mut idx = 0usize;
    for (ln, line) in text.split('\n').enumerate() {
        let len = line.chars().count();
        if ln >= ed.scroll_line && ln < ed.scroll_line + rows {
            let y = inner.y + (ln - ed.scroll_line) as i32 * line_h;
            if error_lines.contains(&(ln + 1)) {
                g.rect(Rect::new(inner.x, y - 1, inner.w, line_h), 'p');
            }
            if highlight_line == Some(ln + 1) {
                g.rect(Rect::new(inner.x, y - 1, inner.w, line_h), 'y');
            }
            // selection background
            if let Some((a, b)) = sel {
                let (s0, s1) = (a.max(idx), b.min(idx + len + 1));
                if s0 < s1 {
                    let c0 = s0 - idx;
                    let c1 = s1 - idx;
                    let x0 = inner.x + 1 + (c0 as i32 - ed.scroll_col as i32) * 6 * scale;
                    let x1 = inner.x + 1 + (c1 as i32 - ed.scroll_col as i32) * 6 * scale;
                    g.rect(Rect::new(x0, y - 1, (x1 - x0).max(2), line_h), 'c');
                }
            }
            let mut x = inner.x + 1 - ed.scroll_col as i32 * 6 * scale;
            for (s, col) in asm_spans(line) {
                g.text_s(x, y, &s, col, scale);
                x += s.chars().count() as i32 * 6 * scale;
            }
        }
        idx += len + 1;
    }
    if ed.focused && (tick / 30) % 2 == 0 {
        let (line, col) = ed.line_col(ed.cursor);
        if line >= ed.scroll_line && line < ed.scroll_line + rows && col >= ed.scroll_col && col <= ed.scroll_col + cols {
            let x = inner.x + 1 + (col - ed.scroll_col) as i32 * 6 * scale;
            let y = inner.y + (line - ed.scroll_line) as i32 * line_h;
            g.rect(Rect::new(x, y - 1, 1, 8 * scale + 1), 'R');
        }
    }
    g.c.set_clip(saved);
    let content = ed.line_count() as i32 * line_h;
    g.scrollbar(Rect::new(r.right() - 5, r.y + 2, 3, r.h - 4), ed.scroll_line as i32 * line_h, content, rows as i32 * line_h);
}

// ---------------------------------------------------------------------------------------------- keyboard

/// What a keyboard tap asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KbAction {
    /// Type this character.
    Char(char),
    /// Backspace.
    Backspace,
    /// Enter / newline.
    Enter,
    /// Hide the keyboard.
    Hide,
}

/// Which key layer is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layer {
    /// Lower-case letters.
    #[default]
    Lower,
    /// Upper-case letters.
    Upper,
}

/// A key on the grid.
#[derive(Debug, Clone)]
pub struct KeySpec {
    /// Text on the cap.
    pub label: String,
    /// What it does.
    pub action: Option<KbAction>,
    /// Layer toggle key.
    pub shift: bool,
    /// Width in columns.
    pub span: i32,
}

/// The on-screen letter grid.
#[derive(Debug, Clone, Default)]
pub struct Keyboard {
    /// Current layer.
    pub layer: Layer,
    /// The key being pressed (for the pressed look).
    pressed: Option<usize>,
}

const ROWS: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl:", "zxcvbnm_,-"];

impl Keyboard {
    /// Key specs row by row.
    pub fn keys(&self) -> Vec<Vec<KeySpec>> {
        let up = self.layer == Layer::Upper;
        let mut rows: Vec<Vec<KeySpec>> = Vec::new();
        for (ri, r) in ROWS.iter().enumerate() {
            let mut row = Vec::new();
            for c in r.chars() {
                let ch = if up && c.is_ascii_lowercase() { c.to_ascii_uppercase() } else { c };
                row.push(KeySpec {
                    label: ch.to_string(),
                    action: Some(KbAction::Char(ch)),
                    shift: false,
                    span: 1,
                });
            }
            let _ = ri;
            rows.push(row);
        }
        let mk = |label: &str, action: Option<KbAction>, shift: bool, span: i32| KeySpec {
            label: label.to_string(),
            action,
            shift,
            span,
        };
        rows.push(vec![
            mk("Aa", None, true, 2),
            mk("[", Some(KbAction::Char('[')), false, 1),
            mk("]", Some(KbAction::Char(']')), false, 1),
            mk("space", Some(KbAction::Char(' ')), false, 3),
            mk("<-", Some(KbAction::Backspace), false, 1),
            mk("Enter", Some(KbAction::Enter), false, 2),
        ]);
        rows
    }

    /// Key rectangles in `area` (rows share the height; columns are 10 per row).
    pub fn rects(&self, area: Rect) -> Vec<(Rect, KeySpec)> {
        let keys = self.keys();
        let rh = area.h / keys.len() as i32;
        let cw = area.w / 10;
        let mut out = Vec::new();
        for (ri, row) in keys.into_iter().enumerate() {
            let mut x = area.x;
            for k in row {
                let w = cw * k.span;
                out.push((Rect::new(x, area.y + ri as i32 * rh, w, rh), k));
                x += w;
            }
        }
        out
    }

    /// Handle taps; returns the actions.
    pub fn update(&mut self, ptr: &Pointer, area: Rect) -> Vec<KbAction> {
        let mut out = Vec::new();
        let rects = self.rects(area);
        self.pressed = rects.iter().position(|(r, _)| ptr.held_in(r) && ptr.over(r));
        for (r, k) in &rects {
            if ptr.tapped_in(r) {
                if k.shift {
                    self.layer = if self.layer == Layer::Lower { Layer::Upper } else { Layer::Lower };
                } else if let Some(a) = &k.action {
                    out.push(a.clone());
                    if self.layer == Layer::Upper && matches!(a, KbAction::Char(c) if c.is_ascii_alphabetic()) {
                        self.layer = Layer::Lower;
                    }
                }
            }
        }
        out
    }

    /// Draw the keyboard.
    pub fn draw(&self, g: &mut Gfx<'_>, area: Rect, ptr: &Pointer) {
        g.rect(area.grow(2), 'k');
        for (i, (r, k)) in self.rects(area).iter().enumerate() {
            let r = r.inset(1);
            let kind = if k.shift && self.layer == Layer::Upper {
                Kind::On
            } else if matches!(k.action, Some(KbAction::Enter)) {
                Kind::Good
            } else if matches!(k.action, Some(KbAction::Backspace)) {
                Kind::Danger
            } else {
                Kind::Normal
            };
            let st = State {
                hover: ptr.hover(&r),
                down: self.pressed == Some(i),
                ..State::default()
            };
            g.button(r, &k.label, kind, st);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(s: &str) -> Input {
        Input {
            typed: s.to_string(),
            ..Input::default()
        }
    }

    #[test]
    fn typing_moving_and_deleting() {
        let mut e = TextEdit::new("", true);
        e.handle(&typed("ab"));
        e.handle(&Input {
            keys: vec![Key::Enter],
            ..Input::default()
        });
        e.handle(&typed("cd"));
        assert_eq!(e.text(), "ab\ncd");
        assert_eq!(e.line_col(e.cursor), (1, 2));
        e.handle(&Input {
            keys: vec![Key::Up],
            ..Input::default()
        });
        assert_eq!(e.line_col(e.cursor), (0, 2));
        e.handle(&Input {
            keys: vec![Key::Backspace],
            ..Input::default()
        });
        assert_eq!(e.text(), "a\ncd");
    }

    #[test]
    fn selection_copy_cut_paste() {
        let mut e = TextEdit::new("hello", true);
        let sel = Input {
            keys: vec![Key::Char('a')],
            ctrl: true,
            ..Input::default()
        };
        e.handle(&sel);
        assert_eq!(e.selected_text().as_deref(), Some("hello"));
        let out = e.handle(&Input {
            keys: vec![Key::Char('x')],
            ctrl: true,
            ..Input::default()
        });
        assert_eq!(out.copy.as_deref(), Some("hello"));
        assert_eq!(e.text(), "");
        e.handle(&Input {
            paste: Some("a\r\nb".into()),
            ..Input::default()
        });
        assert_eq!(e.text(), "a\nb");
        let out = e.handle(&Input {
            keys: vec![Key::Char('v')],
            ctrl: true,
            ..Input::default()
        });
        assert!(out.want_paste);
    }

    #[test]
    fn shift_arrows_select() {
        let mut e = TextEdit::new("abc", true);
        e.handle(&Input {
            keys: vec![Key::Left, Key::Left],
            shift: true,
            ..Input::default()
        });
        assert_eq!(e.selected_text().as_deref(), Some("bc"));
        e.handle(&typed("X"));
        assert_eq!(e.text(), "aX");
    }

    #[test]
    fn single_line_ignores_newlines_and_junk() {
        let mut e = TextEdit::new("", false);
        e.insert("a\nb\u{1}c");
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn pixel_to_index() {
        let e = TextEdit::new("abc\ndef", true);
        let r = Rect::new(0, 0, 100, 40);
        assert_eq!(e.index_at_pixel(r, 1, 10, 7, 12), 4 + 1);
    }

    #[test]
    fn asm_colouring() {
        let s = asm_spans("    COPYTO 3  -- note");
        assert_eq!(s[1].1, 'b');
        assert_eq!(s.last().unwrap().1, 'a');
        assert_eq!(asm_spans("loop:")[0].1, 'u');
    }

    #[test]
    fn keyboard_layers_and_actions() {
        let mut k = Keyboard::default();
        let area = Rect::new(0, 0, 100, 50);
        let rects = k.rects(area);
        assert!(rects.iter().all(|(r, _)| r.inside(&area)));
        let (shift_r, _) = rects.iter().find(|(_, k)| k.shift).unwrap().clone();
        let mut p = Pointer::default();
        let c = shift_r.center();
        p.update(&Input {
            pointer: Some(c),
            down: true,
            pressed: true,
            ..Input::default()
        });
        p.update(&Input {
            pointer: Some(c),
            released: true,
            ..Input::default()
        });
        k.update(&p, area);
        assert_eq!(k.layer, Layer::Upper);
        assert!(k.keys()[1][0].label == "Q");
    }
}
