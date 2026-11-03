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
    /// Code behaviour: Enter keeps the indent (one level deeper after `{`) and `}` dedents.
    pub smart: bool,
    /// Typing an opening bracket or quote also types the closing one.
    pub auto_close: bool,
    /// Width in pixels of the line number gutter left of the text (set by the view for hit testing).
    pub gutter: i32,
    drag_from: Option<usize>,
    /// Pixels dragged and not yet turned into whole columns/rows (touch scrolling).
    drag_acc: (i32, i32),
    /// Cursor and length at the last [`TextEdit::follow_cursor`] call.
    followed: (usize, usize),
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
        let n = self.chars.len();
        self.anchor
            .map(|a| a.min(n))
            .filter(|a| *a != self.cursor.min(n))
            .map(|a| (a.min(self.cursor.min(n)), a.max(self.cursor.min(n))))
    }

    /// The selected text.
    pub fn selected_text(&self) -> Option<String> {
        self.selection()
            .map(|(a, b)| self.chars[a..b].iter().collect())
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
                    let idx = if line == 0 {
                        0
                    } else {
                        self.index_at(line - 1, col)
                    };
                    self.move_to(idx, input.shift);
                }
                Key::Down => {
                    let idx = if line + 1 >= self.line_count() {
                        self.chars.len()
                    } else {
                        self.index_at(line + 1, col)
                    };
                    self.move_to(idx, input.shift);
                }
                Key::Home => self.move_to(self.index_at(line, 0), input.shift),
                Key::End => self.move_to(self.index_at(line, usize::MAX / 2), input.shift),
                Key::PageUp => {
                    self.move_to(self.index_at(line.saturating_sub(6), col), input.shift)
                }
                Key::PageDown => self.move_to(self.index_at(line + 6, col), input.shift),
                Key::Backspace => out.changed |= self.backspace_smart(),
                Key::Delete => out.changed |= self.delete(),
                Key::Enter if self.multiline => {
                    if self.smart {
                        self.smart_newline();
                    } else {
                        self.insert("\n");
                    }
                    out.changed = true;
                }
                _ => {}
            }
        }
        if !input.typed.is_empty() && !input.ctrl {
            let before = self.chars.len();
            let typed: String = input.typed.chars().filter(|c| !c.is_control()).collect();
            if self.smart && typed.chars().count() <= 2 {
                for c in typed.chars() {
                    self.type_char(c);
                }
            } else {
                self.insert(&typed);
            }
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

    /// Scroll to the cursor, but only when the cursor or the text moved since the last call, so a drag or
    /// the wheel can look around without the view snapping back.
    pub fn follow_cursor(&mut self, rows: usize, cols: usize) {
        let now = (self.cursor, self.chars.len());
        if now != self.followed {
            self.followed = now;
            self.scroll_to_cursor(rows, cols);
        }
    }

    /// The longest line in characters.
    pub fn longest_line(&self) -> usize {
        self.chars
            .split(|c| *c == '\n')
            .map(<[char]>::len)
            .max()
            .unwrap_or(0)
    }

    /// Scroll with the wheel or a drag (lines).
    pub fn scroll_by(&mut self, lines: i32, rows: usize) {
        self.scroll_view(0, lines, rows, usize::MAX / 4);
    }

    /// Scroll the view by whole columns and rows, clamped to the text.
    pub fn scroll_view(&mut self, dcols: i32, drows: i32, rows: usize, cols: usize) {
        let max_row = self.line_count().saturating_sub(rows.max(1)) as i32;
        self.scroll_line = (self.scroll_line as i32 + drows).clamp(0, max_row.max(0)) as usize;
        let max_col = (self.longest_line() + 1).saturating_sub(cols.max(1)) as i32;
        self.scroll_col = (self.scroll_col as i32 + dcols).clamp(0, max_col.max(0)) as usize;
    }

    /// Character index under a pixel inside the text area `r` (for clicking and dragging).
    pub fn index_at_pixel(&self, r: Rect, scale: i32, line_h: i32, x: i32, y: i32) -> usize {
        let line = (((y - r.y - 2) / line_h.max(1)).max(0) as usize) + self.scroll_line;
        let col = (((x - r.x - self.gutter) / (6 * scale)).max(0) as usize) + self.scroll_col;
        self.index_at(line, col)
    }

    /// Mouse/touch interaction. Mouse: click to place the caret, drag to select. Touch: tap to place the
    /// caret, drag to scroll in both directions. Returns whether the view scrolled by dragging.
    pub fn pointer(&mut self, ptr: &Pointer, r: Rect, scale: i32, line_h: i32) -> bool {
        let rows = ((r.h - 4) / line_h.max(1)).max(1) as usize;
        let cols = ((r.w - self.gutter - 6) / (6 * scale)).max(1) as usize;
        let mut scrolled = false;
        if ptr.touch {
            if ptr.pressed && ptr.over(&r) {
                self.focused = true;
                self.drag_acc = (0, 0);
            }
            if ptr.down && ptr.moved && r.contains(ptr.start.0, ptr.start.1) {
                self.drag_acc.0 += ptr.delta.0;
                self.drag_acc.1 += ptr.delta.1;
                let dc = self.drag_acc.0 / (6 * scale);
                let dr = self.drag_acc.1 / line_h.max(1);
                self.drag_acc.0 -= dc * 6 * scale;
                self.drag_acc.1 -= dr * line_h.max(1);
                if dc != 0 || dr != 0 {
                    self.scroll_view(-dc, -dr, rows, cols);
                    scrolled = true;
                }
            }
            if ptr.tapped_in(&r) {
                self.focused = true;
                if let Some((x, y)) = ptr.pos {
                    self.anchor = None;
                    self.cursor = self.index_at_pixel(r, scale, line_h, x, y);
                    self.blink = 0;
                }
            }
            return scrolled;
        }
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
        false
    }

    // ---- code editing helpers ----

    fn line_range(&self, line: usize) -> (usize, usize) {
        let s = self.line_start(line);
        (s, s + self.line_len(line))
    }

    fn indent_of(&self, line: usize) -> usize {
        let (s, e) = self.line_range(line);
        self.chars[s..e].iter().take_while(|c| **c == ' ').count()
    }

    /// For each character: is it inside a comment, string or character literal?
    fn literal_mask(&self) -> Vec<bool> {
        let text = self.text();
        let mut mask = vec![false; self.chars.len()];
        let mut byte_to_char = vec![0usize; text.len() + 1];
        for (ci, (bi, c)) in text.char_indices().enumerate() {
            for k in 0..c.len_utf8() {
                byte_to_char[bi + k] = ci;
            }
        }
        byte_to_char[text.len()] = self.chars.len();
        for (sp, kind) in algoclerk_core::cpp::lexer::highlight(&text) {
            use algoclerk_core::cpp::lexer::HlKind;
            if matches!(kind, HlKind::Comment | HlKind::Str | HlKind::Char) {
                let a = byte_to_char[(sp.start as usize).min(text.len())];
                let b = byte_to_char[(sp.end as usize).min(text.len())];
                for m in mask.iter_mut().take(b).skip(a) {
                    *m = true;
                }
            }
        }
        mask
    }

    /// The bracket pair next to the cursor (the one before it first), as two character indices.
    pub fn match_bracket(&self) -> Option<(usize, usize)> {
        let at = |i: usize| self.chars.get(i).copied();
        let mask = self.literal_mask();
        let cands = [self.cursor.checked_sub(1), Some(self.cursor)];
        for i in cands.into_iter().flatten() {
            let Some(c) = at(i) else { continue };
            if mask.get(i).copied().unwrap_or(false) {
                continue;
            }
            let (open, close, forward) = match c {
                '(' => ('(', ')', true),
                '[' => ('[', ']', true),
                '{' => ('{', '}', true),
                ')' => ('(', ')', false),
                ']' => ('[', ']', false),
                '}' => ('{', '}', false),
                _ => continue,
            };
            let mut depth = 0i32;
            if forward {
                for j in i..self.chars.len() {
                    if mask[j] {
                        continue;
                    }
                    if self.chars[j] == open {
                        depth += 1;
                    } else if self.chars[j] == close {
                        depth -= 1;
                        if depth == 0 {
                            return Some((i, j));
                        }
                    }
                }
            } else {
                for j in (0..=i).rev() {
                    if mask[j] {
                        continue;
                    }
                    if self.chars[j] == close {
                        depth += 1;
                    } else if self.chars[j] == open {
                        depth -= 1;
                        if depth == 0 {
                            return Some((j, i));
                        }
                    }
                }
            }
        }
        None
    }

    /// The `{` that is still open before character index `at`.
    fn open_brace_before(&self, at: usize) -> Option<usize> {
        let mask = self.literal_mask();
        let mut depth = 0i32;
        for j in (0..at.min(self.chars.len())).rev() {
            if mask[j] {
                continue;
            }
            match self.chars[j] {
                '}' => depth += 1,
                '{' => {
                    if depth == 0 {
                        return Some(j);
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        None
    }

    /// Enter with auto-indent: keep the indentation of the line, one level deeper after `{`, and open a
    /// closing line when the cursor sits right before `}`.
    pub fn smart_newline(&mut self) {
        self.delete_selection();
        let (line, col) = self.line_col(self.cursor);
        let base = self.indent_of(line).min(col);
        let (s, _) = self.line_range(line);
        let before: String = self.chars[s..self.cursor].iter().collect();
        let deeper = before.trim_end().ends_with('{');
        let extra = if deeper { 4 } else { 0 };
        let between = deeper && self.chars.get(self.cursor) == Some(&'}');
        let mut ins = format!("\n{}", " ".repeat(base + extra));
        if between {
            ins.push('\n');
            ins.push_str(&" ".repeat(base));
        }
        let keep = self.cursor + 1 + base + extra;
        self.insert(&ins);
        if between {
            self.cursor = keep;
        }
    }

    /// Typing `}` on an otherwise empty line, when the braces of the text already balance and the next thing
    /// after the blank space is a `}` (the one auto-close added when `{` was typed): reuse that brace, dedented
    /// to the line of its opening brace, instead of adding a second one.
    fn type_over_closing_brace(&mut self) -> bool {
        if !self.auto_close {
            return false;
        }
        let (line, _) = self.line_col(self.cursor);
        let (s, _) = self.line_range(line);
        if !self.chars[s..self.cursor].iter().all(|c| *c == ' ') {
            return false;
        }
        let mask = self.literal_mask();
        let count = |b: char| {
            (0..self.chars.len())
                .filter(|i| !mask[*i] && self.chars[*i] == b)
                .count()
        };
        let (opens, closes) = (count('{'), count('}'));
        if opens > closes {
            return false;
        }
        let Some(j) = (self.cursor..self.chars.len()).find(|i| !self.chars[*i].is_whitespace())
        else {
            return false;
        };
        if self.chars[j] != '}' {
            return false;
        }
        let target = self
            .open_brace_before(self.cursor)
            .map(|b| self.indent_of(self.line_col(b).0))
            .unwrap_or(0);
        self.chars.drain(s..j);
        self.cursor = s;
        self.insert(&" ".repeat(target));
        self.move_to(self.cursor + 1, false);
        true
    }

    /// Type one character with the code behaviour (dedent on `}`, auto-close, type over the closer).
    pub fn type_char(&mut self, c: char) {
        let next = self.chars.get(self.cursor).copied();
        let prev = self
            .cursor
            .checked_sub(1)
            .and_then(|i| self.chars.get(i).copied());
        let closer = matches!(c, ')' | ']' | '}' | '"' | '\'');
        if self.auto_close && closer && next == Some(c) && self.selection().is_none() {
            self.move_to(self.cursor + 1, false);
            return;
        }
        if c == '}' && self.smart && self.selection().is_none() && self.type_over_closing_brace() {
            return;
        }
        if c == '}' && self.smart && self.selection().is_none() {
            let (line, col) = self.line_col(self.cursor);
            let (s, e) = self.line_range(line);
            if col > 0 && self.chars[s..self.cursor].iter().all(|c| *c == ' ') {
                let target = self
                    .open_brace_before(self.cursor)
                    .map(|b| self.indent_of(self.line_col(b).0))
                    .unwrap_or_else(|| col.saturating_sub(4));
                let _ = e;
                self.chars.drain(s..self.cursor);
                self.cursor = s;
                self.insert(&" ".repeat(target));
            }
            self.insert("}");
            return;
        }
        let pair = match c {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '"' => Some('"'),
            '\'' => Some('\''),
            _ => None,
        };
        if let (true, Some(p), None) = (self.auto_close, pair, self.selection()) {
            let quote = c == '"' || c == '\'';
            let room =
                next.is_none_or(|n| n.is_whitespace() || matches!(n, ')' | ']' | '}' | ';' | ','));
            let word_before = prev.is_some_and(|p| p.is_alphanumeric() || p == '_');
            if room && !(quote && word_before) {
                self.insert(&format!("{c}{p}"));
                self.cursor -= 1;
                return;
            }
        }
        self.insert(&c.to_string());
    }

    /// Backspace that removes an empty auto-closed pair as a whole.
    pub fn backspace_smart(&mut self) -> bool {
        if self.auto_close && self.selection().is_none() && self.cursor > 0 {
            let before = self.chars.get(self.cursor - 1).copied();
            let after = self.chars.get(self.cursor).copied();
            if matches!(
                (before, after),
                (Some('('), Some(')'))
                    | (Some('['), Some(']'))
                    | (Some('{'), Some('}'))
                    | (Some('"'), Some('"'))
                    | (Some('\''), Some('\''))
            ) {
                self.chars.drain(self.cursor - 1..=self.cursor);
                self.cursor -= 1;
                return true;
            }
        }
        self.backspace()
    }

    /// Comment or uncomment the selected lines (or the cursor line) with `//`. Returns whether the text
    /// changed.
    pub fn toggle_comment(&mut self) -> bool {
        let (first, last) = match self.selection() {
            Some((a, b)) => {
                let (la, _) = self.line_col(a);
                let (lb, cb) = self.line_col(b);
                (la, if cb == 0 && lb > la { lb - 1 } else { lb })
            }
            None => {
                let l = self.line_col(self.cursor).0;
                (l, l)
            }
        };
        let text = self.text();
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let last = last.min(lines.len().saturating_sub(1));
        let body: Vec<usize> = (first..=last)
            .filter(|i| !lines[*i].trim().is_empty())
            .collect();
        if body.is_empty() {
            return false;
        }
        let all = body
            .iter()
            .all(|i| lines[*i].trim_start().starts_with("//"));
        let (cl, cc) = self.line_col(self.cursor);
        let mut cursor_col = cc as i32;
        if all {
            for &i in &body {
                let ind = lines[i].len() - lines[i].trim_start().len();
                let mut cut = 2;
                if lines[i][ind + 2..].starts_with(' ') {
                    cut = 3;
                }
                lines[i].replace_range(ind..ind + cut, "");
                if i == cl {
                    cursor_col = (cc as i32 - cut as i32).max(ind as i32);
                }
            }
        } else {
            let at = body
                .iter()
                .map(|i| lines[*i].len() - lines[*i].trim_start().len())
                .min()
                .unwrap_or(0);
            for &i in &body {
                lines[i].insert_str(at, "// ");
                if i == cl && cc >= at {
                    cursor_col = cc as i32 + 3;
                }
            }
        }
        let anchor_line = self.anchor.map(|a| self.line_col(a).0);
        let new = lines.join("\n");
        self.chars = new.chars().collect();
        self.cursor = self.index_at(cl, cursor_col.max(0) as usize);
        if let Some(al) = anchor_line {
            // keep a whole-line selection
            self.anchor = Some(self.index_at(al, 0));
        }
        true
    }
}

/// What [`draw_editor`] shows besides the text.
#[derive(Debug, Clone, Default)]
pub struct EditorLook {
    /// Draw line numbers in a gutter.
    pub line_numbers: bool,
    /// One-based lines with problems (marked in red, with a dot in the gutter).
    pub error_lines: Vec<usize>,
    /// One-based line being executed.
    pub exec_line: Option<usize>,
    /// Squiggles: one-based line, first and last column.
    pub squiggles: Vec<(usize, usize, usize)>,
    /// Highlight the bracket pair next to the cursor.
    pub brackets: bool,
    /// Caret blink phase.
    pub tick: u64,
}

/// Width of the gutter in pixels for a text of `lines` lines (0 without line numbers).
pub fn gutter_width(lines: usize, scale: i32) -> i32 {
    (lines.max(1).to_string().len() as i32 * 6 + 5) * scale
}

/// Size of the visible window of an editor drawn into `r`: `(rows, cols)`.
pub fn view_size(ed: &TextEdit, r: Rect, scale: i32, line_h: i32) -> (usize, usize) {
    let rows = ((r.h - 4) / line_h).max(1) as usize;
    let cols = ((r.w - 6 - ed.gutter) / (6 * scale)).max(1) as usize;
    (rows, cols)
}

/// Draw a text editor into `r`.
pub fn draw_editor(
    g: &mut Gfx<'_>,
    ed: &TextEdit,
    r: Rect,
    scale: i32,
    line_h: i32,
    look: &EditorLook,
) {
    g.rect(r, 'W');
    g.frame(r, 'K');
    let inner = r.inset(2);
    let gutter = if look.line_numbers {
        gutter_width(ed.line_count(), scale)
    } else {
        0
    };
    let saved = g.c.set_clip(inner.offset(g.off.0, g.off.1));
    let rows = (inner.h / line_h).max(1) as usize;
    let text_x = inner.x + 1 + gutter;
    let cols = ((inner.w - 2 - gutter) / (6 * scale)).max(1) as usize;
    let text = ed.text();
    let colored = crate::syntax::lines(&text, crate::syntax::light, 'K');
    let sel = ed.selection();
    let (cur_line, _) = ed.line_col(ed.cursor);
    let pair = if look.brackets {
        ed.match_bracket()
    } else {
        None
    };
    let col_x = |c: usize| text_x + (c as i32 - ed.scroll_col as i32) * 6 * scale;
    let mut idx = 0usize;
    for (ln, line) in text.split('\n').enumerate() {
        let len = line.chars().count();
        if ln >= ed.scroll_line && ln < ed.scroll_line + rows {
            let y = inner.y + (ln - ed.scroll_line) as i32 * line_h;
            let text_area = Rect::new(text_x - 1, y - 1, inner.right() - text_x + 1, line_h);
            if ed.focused && ln == cur_line && look.exec_line != Some(ln + 1) {
                g.rect(text_area, 'X');
            }
            if look.error_lines.contains(&(ln + 1)) {
                g.rect(text_area, 'p');
            }
            if look.exec_line == Some(ln + 1) {
                g.rect(text_area, 'y');
            }
            // selection background
            if let Some((a, b)) = sel {
                let (s0, s1) = (a.max(idx), b.min(idx + len + 1));
                if s0 < s1 {
                    let x0 = col_x(s0 - idx);
                    let x1 = col_x(s1 - idx);
                    g.rect(Rect::new(x0, y - 1, (x1 - x0).max(2), line_h), 'c');
                }
            }
            // matching brackets
            if let Some((p, q)) = pair {
                for at in [p, q] {
                    if at >= idx && at <= idx + len {
                        g.rect(Rect::new(col_x(at - idx), y - 1, 6 * scale, line_h), 'y');
                        g.frame(Rect::new(col_x(at - idx), y - 1, 6 * scale, line_h), 'Y');
                    }
                }
            }
            let mut x = col_x(0);
            for (s, col) in colored.get(ln).into_iter().flatten() {
                g.text_s(x, y, s, *col, scale);
                x += s.chars().count() as i32 * 6 * scale;
            }
            for (sl, c0, c1) in &look.squiggles {
                if *sl == ln + 1 {
                    let sx = col_x(*c0);
                    let ex = col_x(*c1);
                    for px in sx..ex.max(sx + 3) {
                        g.px(
                            px,
                            y + 8 * scale + if (px / 2) % 2 == 0 { 0 } else { 1 },
                            'R',
                        );
                    }
                }
            }
            if look.line_numbers {
                let gr = Rect::new(inner.x, y - 1, gutter, line_h);
                g.rect(gr, 'A');
                let n = (ln + 1).to_string();
                let nx = inner.x + gutter - 3 * scale - n.len() as i32 * 6 * scale;
                let here = ln == cur_line && ed.focused;
                g.text_s(nx, y, &n, if here { 'K' } else { 'a' }, scale);
                if look.error_lines.contains(&(ln + 1)) {
                    g.rect(Rect::new(inner.x, y - 1, 2, line_h), 'R');
                }
            }
        }
        idx += len + 1;
    }
    if look.line_numbers {
        // the gutter also fills the empty space below the last line
        let used = (ed.line_count().saturating_sub(ed.scroll_line)).min(rows) as i32 * line_h;
        if used < inner.h {
            g.rect(
                Rect::new(inner.x, inner.y + used - 1, gutter, inner.h - used + 1),
                'A',
            );
        }
        g.vline(inner.x + gutter - 1, inner.y, inner.h, 'a');
    }
    if ed.focused && (look.tick / 30) % 2 == 0 {
        let (line, col) = ed.line_col(ed.cursor);
        if line >= ed.scroll_line
            && line < ed.scroll_line + rows
            && col >= ed.scroll_col
            && col <= ed.scroll_col + cols
        {
            let x = col_x(col);
            let y = inner.y + (line - ed.scroll_line) as i32 * line_h;
            g.rect(Rect::new(x, y - 1, 1, 8 * scale + 1), 'R');
        }
    }
    g.c.set_clip(saved);
    let content = ed.line_count() as i32 * line_h;
    g.scrollbar(
        Rect::new(r.right() - 5, r.y + 2, 3, r.h - 4 - 4),
        ed.scroll_line as i32 * line_h,
        content,
        rows as i32 * line_h,
    );
    // horizontal scroll indicator
    let longest = ed.longest_line().max(1) as i32;
    if longest > cols as i32 {
        let track = Rect::new(inner.x + gutter, r.bottom() - 4, inner.w - gutter - 6, 2);
        g.rect(track, 'A');
        let thumb = (track.w * cols as i32 / longest).max(8);
        let tx =
            track.x + (track.w - thumb) * ed.scroll_col as i32 / (longest - cols as i32 + 1).max(1);
        g.rect(Rect::new(tx, track.y, thumb, 2), 'a');
    }
}

// ---------------------------------------------------------------------------------------------- keyboard

/// What a keyboard tap asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KbAction {
    /// Type this character.
    Char(char),
    /// Type this text (a symbol such as `->` or a keyword).
    Text(String),
    /// Backspace.
    Backspace,
    /// Enter / newline.
    Enter,
    /// Comment or uncomment the line.
    Comment,
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
    /// Digits and the symbols code needs.
    Sym,
}

/// A key on the grid.
#[derive(Debug, Clone)]
pub struct KeySpec {
    /// Text on the cap.
    pub label: String,
    /// What it does.
    pub action: Option<KbAction>,
    /// Layer toggle key (shift).
    pub shift: bool,
    /// Layer toggle key (symbols).
    pub sym: bool,
    /// Width in columns.
    pub span: i32,
}

/// The on-screen letter grid with a strip of one-tap keywords above it.
#[derive(Debug, Clone, Default)]
pub struct Keyboard {
    /// Current layer.
    pub layer: Layer,
    /// The key being pressed (for the pressed look).
    pressed: Option<usize>,
    /// Horizontal scroll of the keyword strip in pixels.
    pub chip_scroll: i32,
}

const ROWS: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl_", "zxcvbnm,.:"];

/// The keyword and type chips: label and the text one tap inserts.
pub const CHIPS: &[(&str, &str)] = &[
    ("Tab", "    "),
    ("for", "for ("),
    ("while", "while ("),
    ("if", "if ("),
    ("else", "else "),
    ("return", "return "),
    ("auto", "auto "),
    ("const", "const "),
    ("int", "int "),
    ("long long", "long long "),
    ("bool", "bool "),
    ("char", "char "),
    ("std::", "std::"),
    ("vector", "vector<"),
    ("string", "string "),
    ("size_t", "size_t "),
    ("nullptr", "nullptr"),
    ("true", "true"),
    ("false", "false"),
    ("break", "break;"),
    ("continue", "continue;"),
];

impl Keyboard {
    /// Key specs row by row (the five rows under the keyword strip).
    pub fn keys(&self) -> Vec<Vec<KeySpec>> {
        let up = self.layer == Layer::Upper;
        let sym = self.layer == Layer::Sym;
        let mk =
            |label: &str, action: Option<KbAction>, shift: bool, symk: bool, span: i32| KeySpec {
                label: label.to_string(),
                action,
                shift,
                sym: symk,
                span,
            };
        let single = |c: char| mk(&c.to_string(), Some(KbAction::Char(c)), false, false, 1);
        let mut rows: Vec<Vec<KeySpec>> = Vec::new();
        if sym {
            for r in ["(){}[]<>;:", "+-*/%=&|^~"] {
                rows.push(r.chars().map(single).collect());
            }
            let mut r3: Vec<KeySpec> = "!'\"#,.?\\".chars().map(single).collect();
            r3.push(mk("->", Some(KbAction::Text("->".into())), false, false, 2));
            rows.push(r3);
            rows.push("1234567890".chars().map(single).collect());
        } else {
            for r in ROWS {
                rows.push(
                    r.chars()
                        .map(|c| {
                            single(if up && c.is_ascii_lowercase() {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            })
                        })
                        .collect(),
                );
            }
        }
        rows.push(if sym {
            vec![
                mk("abc", None, true, false, 2),
                mk("::", Some(KbAction::Text("::".into())), false, false, 2),
                mk("space", Some(KbAction::Char(' ')), false, false, 2),
                mk("<-", Some(KbAction::Backspace), false, false, 2),
                mk("Enter", Some(KbAction::Enter), false, false, 2),
            ]
        } else {
            vec![
                mk("Aa", None, true, false, 2),
                mk("#+=", None, false, true, 2),
                mk("space", Some(KbAction::Char(' ')), false, false, 3),
                mk("<-", Some(KbAction::Backspace), false, false, 1),
                mk("Enter", Some(KbAction::Enter), false, false, 2),
            ]
        });
        rows
    }

    /// Split the keyboard area into the keyword strip and the key grid.
    pub fn split(&self, area: Rect) -> (Rect, Rect) {
        let ch = (area.h / 6).clamp(14, 22);
        (
            Rect::new(area.x, area.y, area.w, ch),
            Rect::new(area.x, area.y + ch + 1, area.w, area.h - ch - 1),
        )
    }

    /// Key rectangles in `area` (rows share the height; columns are 10 per row).
    pub fn rects(&self, area: Rect) -> Vec<(Rect, KeySpec)> {
        let (_, grid) = self.split(area);
        let keys = self.keys();
        let rh = grid.h / keys.len() as i32;
        let cw = grid.w / 10;
        let mut out = Vec::new();
        for (ri, row) in keys.into_iter().enumerate() {
            let mut x = grid.x;
            for k in row {
                let w = cw * k.span;
                out.push((Rect::new(x, grid.y + ri as i32 * rh, w, rh), k));
                x += w;
            }
        }
        out
    }

    /// The pinned buttons (hide, comment) and the scrolling chips of the strip, with their actions.
    pub fn chip_rects(&self, area: Rect) -> (Vec<(Rect, String, KbAction)>, Rect) {
        let (strip, _) = self.split(area);
        let mut pinned = Vec::new();
        let mut x = strip.x;
        for (label, action) in [("Hide", KbAction::Hide), ("//", KbAction::Comment)] {
            let w = label.chars().count() as i32 * 6 + 10;
            pinned.push((Rect::new(x, strip.y, w, strip.h), label.to_string(), action));
            x += w + 1;
        }
        let rest = Rect::new(x + 1, strip.y, strip.right() - x - 1, strip.h);
        let mut chips = Vec::new();
        let mut cx = rest.x - self.chip_scroll;
        for (label, ins) in CHIPS {
            let w = label.chars().count() as i32 * 6 + 8;
            chips.push((
                Rect::new(cx, strip.y, w, strip.h),
                (*label).to_string(),
                KbAction::Text((*ins).to_string()),
            ));
            cx += w + 1;
        }
        pinned.extend(chips);
        (pinned, rest)
    }

    fn chips_width(&self) -> i32 {
        CHIPS
            .iter()
            .map(|(l, _)| l.chars().count() as i32 * 6 + 9)
            .sum()
    }

    /// Handle taps; returns the actions.
    pub fn update(&mut self, ptr: &Pointer, area: Rect) -> Vec<KbAction> {
        let mut out = Vec::new();
        let rects = self.rects(area);
        self.pressed = rects
            .iter()
            .position(|(r, _)| ptr.held_in(r) && ptr.over(r));
        // the keyword strip scrolls sideways
        let (chips, rest) = self.chip_rects(area);
        let max_scroll = (self.chips_width() - rest.w).max(0);
        if ptr.down && ptr.moved && rest.contains(ptr.start.0, ptr.start.1) {
            self.chip_scroll = (self.chip_scroll - ptr.delta.0).clamp(0, max_scroll);
        }
        if ptr.wheel != 0.0 && ptr.pos.is_some_and(|p| rest.contains(p.0, p.1)) {
            self.chip_scroll = (self.chip_scroll + (ptr.wheel * 24.0) as i32).clamp(0, max_scroll);
        }
        for (i, (r, _, a)) in chips.iter().enumerate() {
            // scrolled chips only count inside the visible strip
            let visible = i < 2 || (r.x >= rest.x - 3 && r.right() <= rest.right() + 3);
            if visible && ptr.tapped_in(r) {
                out.push(a.clone());
            }
        }
        for (r, k) in &rects {
            if k.span == 0 {
                continue;
            }
            if ptr.tapped_in(r) {
                if k.sym {
                    self.layer = Layer::Sym;
                } else if k.shift && self.layer == Layer::Sym {
                    self.layer = Layer::Lower;
                } else if k.shift {
                    self.layer = if self.layer == Layer::Lower {
                        Layer::Upper
                    } else {
                        Layer::Lower
                    };
                } else if let Some(a) = &k.action {
                    out.push(a.clone());
                    if self.layer == Layer::Upper
                        && matches!(a, KbAction::Char(c) if c.is_ascii_alphabetic())
                    {
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
        let (chips, rest) = self.chip_rects(area);
        let saved = g.c.set_clip(rest.offset(g.off.0, g.off.1));
        for (i, (r, label, _)) in chips.iter().enumerate() {
            if i < 2 {
                continue;
            }
            if r.right() < rest.x || r.x > rest.right() {
                continue;
            }
            g.chip(*r, label, if ptr.hover(r) { 'y' } else { 'c' }, 'K');
        }
        g.c.set_clip(saved);
        for (i, (r, label, a)) in chips.iter().enumerate().take(2) {
            let kind = if matches!(a, KbAction::Hide) {
                Kind::Danger
            } else {
                Kind::Normal
            };
            g.button(
                *r,
                label,
                kind,
                State {
                    hover: ptr.hover(r),
                    down: false,
                    ..State::default()
                },
            );
            let _ = i;
        }
        // scroll cue
        if self.chip_scroll + rest.w < self.chips_width() {
            g.text(rest.right() - 6, rest.y + (rest.h - 7) / 2, ">", 'W');
        }
        for (i, (r, k)) in self.rects(area).iter().enumerate() {
            if k.span == 0 {
                continue;
            }
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

    fn code(text: &str) -> TextEdit {
        let mut e = TextEdit::new(text, true);
        e.smart = true;
        e
    }

    #[test]
    fn comment_toggle_on_a_line_and_a_selection() {
        let mut e = code("int a;\n    int b;\n\nint c;");
        e.cursor = 2;
        assert!(e.toggle_comment());
        assert_eq!(e.text(), "// int a;\n    int b;\n\nint c;");
        assert!(e.toggle_comment());
        assert_eq!(e.text(), "int a;\n    int b;\n\nint c;");
        e.anchor = Some(0);
        e.cursor = e.text().len();
        assert!(e.toggle_comment());
        assert_eq!(e.text(), "// int a;\n//     int b;\n\n// int c;");
        assert!(e.toggle_comment());
        assert_eq!(e.text(), "int a;\n    int b;\n\nint c;");
        assert!(!code("\n\n").toggle_comment());
    }

    #[test]
    fn enter_keeps_the_indent_and_goes_deeper_after_a_brace() {
        let mut e = code("    int f() {");
        e.smart_newline();
        assert_eq!(e.text(), "    int f() {\n        ");
        e.insert("x;");
        e.smart_newline();
        assert_eq!(e.text(), "    int f() {\n        x;\n        ");
        // between braces a closing line is opened
        let mut e = code("if (a) {}");
        e.cursor = 8;
        e.smart_newline();
        assert_eq!(e.text(), "if (a) {\n    \n}");
        assert_eq!(e.line_col(e.cursor), (1, 4));
    }

    #[test]
    fn closing_brace_dedents_to_its_opener() {
        let mut e = code("void f() {\n    if (a) {\n        x;\n        ");
        e.type_char('}');
        assert!(e.text().ends_with("x;\n    }"), "{:?}", e.text());
        e.smart_newline();
        e.type_char('}');
        assert!(e.text().ends_with("    }\n}"), "{:?}", e.text());
        // a brace after code on the line is just a brace
        let mut e = code("x = 1; ");
        e.type_char('}');
        assert_eq!(e.text(), "x = 1; }");
    }

    #[test]
    fn brackets_match_and_ignore_strings_and_comments() {
        let mut e = code("f(a[\")\"], b);");
        e.cursor = 2; // after the first (
        assert_eq!(e.match_bracket(), Some((1, 11)));
        e.cursor = 12; // after the last )
        assert_eq!(e.match_bracket(), Some((1, 11)));
        let e2 = code("{ // }\n}");
        let mut e2 = e2;
        e2.cursor = 1;
        assert_eq!(e2.match_bracket(), Some((0, 7)));
        let mut none = code("(((");
        none.cursor = 3;
        assert_eq!(none.match_bracket(), None);
    }

    #[test]
    fn auto_close_pairs_types_over_and_deletes_together() {
        let mut e = code("");
        e.auto_close = true;
        e.type_char('(');
        assert_eq!(e.text(), "()");
        e.type_char('a');
        e.type_char(')');
        assert_eq!(e.text(), "(a)");
        assert_eq!(e.cursor, 3);
        let mut e = code("");
        e.auto_close = true;
        e.type_char('"');
        e.backspace_smart();
        assert_eq!(e.text(), "");
        // off by default: a plain bracket
        let mut e = code("");
        e.type_char('(');
        assert_eq!(e.text(), "(");
        // an apostrophe after a letter is not a pair
        let mut e = code("don");
        e.auto_close = true;
        e.type_char('\'');
        assert_eq!(e.text(), "don'");
    }

    #[test]
    fn long_lines_scroll_sideways_with_the_cursor_and_by_drag() {
        let long = "x".repeat(100);
        let mut e = code(&long);
        e.cursor = 0;
        e.follow_cursor(5, 20);
        e.cursor = 60;
        e.follow_cursor(5, 20);
        assert_eq!(e.scroll_col, 41);
        // looking around does not snap back until the cursor moves
        e.scroll_view(-10, 0, 5, 20);
        e.follow_cursor(5, 20);
        assert_eq!(e.scroll_col, 31);
        e.scroll_view(500, 0, 5, 20);
        assert_eq!(e.scroll_col, 81);
        e.scroll_view(-500, 0, 5, 20);
        assert_eq!(e.scroll_col, 0);
        // a finger drag scrolls in both directions and does not move the caret
        let mut e = code(&format!("{long}\n{long}\n{long}"));
        e.cursor = 0;
        let r = Rect::new(0, 0, 100, 40);
        let mut p = Pointer::default();
        let start = (50, 20);
        p.update(&Input {
            pointer: Some(start),
            down: true,
            pressed: true,
            touch: true,
            ..Input::default()
        });
        p.update(&Input {
            pointer: Some((20, 20)),
            down: true,
            touch: true,
            ..Input::default()
        });
        let scrolled = e.pointer(&p, r, 1, 9);
        assert!(
            scrolled && e.scroll_col == 5 && e.cursor == 0,
            "{}",
            e.scroll_col
        );
    }
}
