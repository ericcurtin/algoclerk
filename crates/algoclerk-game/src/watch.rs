// SPDX-License-Identifier: Apache-2.0
//! The watch board on the office floor: one line per visible variable as `type name = value` with the C++
//! type, wrapped to the board's width, the variable that just changed highlighted.

use crate::gfx::Gfx;
use crate::syntax::Piece;
use algoclerk_media::Rect;

/// One watched variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Variable name.
    pub name: String,
    /// C++ type as the player writes it (`std::vector<int>`).
    pub ty: String,
    /// The value, rendered like a C++ initialiser.
    pub value: String,
    /// It changed in the last step.
    pub changed: bool,
}

/// Most lines one entry takes on the board; a longer value ends in `..` (the full value is in the tray).
pub const MAX_ENTRY_LINES: usize = 2;

fn coloured(e: &Entry) -> Vec<(char, char)> {
    let head = format!("{} {} = ", e.ty, e.name);
    let n = head.chars().count();
    let value_colour = if e.changed { 'y' } else { 'W' };
    head.chars()
        .enumerate()
        .map(|(i, c)| {
            let col = if i < e.ty.chars().count() {
                'c'
            } else if i + 3 >= n {
                'A'
            } else {
                'W'
            };
            (c, col)
        })
        .chain(e.value.chars().map(|c| (c, value_colour)))
        .collect()
}

fn pieces_of(row: &[(char, char)], indent: bool) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    if indent {
        pieces.push(("  ".into(), 'W'));
    }
    for (c, col) in row {
        match pieces.last_mut() {
            Some(l) if l.1 == *col => l.0.push(*c),
            _ => pieces.push((c.to_string(), *col)),
        }
    }
    pieces
}

/// All lines of an entry after wrapping to `cols` columns (breaks go after a comma or space when one is
/// near), nothing dropped.
pub fn entry_lines_full(e: &Entry, cols: usize) -> Vec<Vec<Piece>> {
    let cols = cols.max(8);
    let chars = coloured(e);
    let mut rows: Vec<Vec<Piece>> = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let room = if at == 0 { cols } else { cols - 2 };
        let mut end = (at + room).min(chars.len());
        if end < chars.len() {
            let window = &chars[at..end];
            if let Some(i) = window.iter().rposition(|(c, _)| *c == ' ' || *c == ',') {
                if i + 1 > room / 3 {
                    end = at + i + 1;
                }
            }
        }
        rows.push(pieces_of(&chars[at..end], at > 0));
        at = end;
    }
    rows
}

/// The lines shown for an entry: at most [`MAX_ENTRY_LINES`], the last one ending in `..` when cut.
pub fn entry_lines(e: &Entry, cols: usize) -> Vec<Vec<Piece>> {
    let mut rows = entry_lines_full(e, cols);
    if rows.len() > MAX_ENTRY_LINES {
        rows.truncate(MAX_ENTRY_LINES);
        if let Some(last) = rows.last_mut() {
            let mut text: Vec<(char, char)> = last
                .iter()
                .flat_map(|(t, c)| t.chars().map(move |ch| (ch, *c)))
                .collect();
            while text.len() + 2 > cols.max(8) {
                text.pop();
            }
            let col = text.last().map_or('W', |x| x.1);
            text.extend([('.', col), ('.', col)]);
            *last = pieces_of(&text, false);
        }
    }
    rows
}

/// How many entries fit into `max_lines` lines at `cols` columns (whole entries only).
pub fn fitting(entries: &[Entry], cols: usize, max_lines: usize) -> (usize, usize) {
    let mut used = 0;
    for (i, e) in entries.iter().enumerate() {
        let n = entry_lines(e, cols).len();
        if used + n > max_lines {
            return (i, used);
        }
        used += n;
    }
    (entries.len(), used)
}

/// Draw the board into `area`. `more` is the text shown above it when entries are left out ("+2 more").
pub fn draw(g: &mut Gfx<'_>, entries: &[Entry], area: Rect, more: &dyn Fn(usize) -> String) {
    g.round_rect(area, 'k', 'K');
    g.hline(area.x + 2, area.y + 1, area.w - 4, 'm');
    let cols = ((area.w - 8) / 6).max(8) as usize;
    let max_lines = ((area.h - 5) / 9).max(1) as usize;
    let (n, _) = fitting(entries, cols, max_lines);
    let mut y = area.y + 4;
    for e in &entries[..n] {
        for l in entry_lines(e, cols) {
            let mut x = area.x + 4;
            for (t, col) in &l {
                g.text(x, y, t, *col);
                x += t.chars().count() as i32 * 6;
            }
            y += 9;
        }
    }
    if n < entries.len() {
        let text = more(entries.len() - n);
        let w = text.chars().count() as i32 * 4 - 1;
        g.tiny(area.right() - 3 - w, area.y - 7, &text, 'W');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, ty: &str, value: &str) -> Entry {
        Entry {
            name: name.into(),
            ty: ty.into(),
            value: value.into(),
            changed: false,
        }
    }

    #[test]
    fn long_entries_wrap_and_keep_every_character() {
        let en = e("nums", "std::vector<int>", "{2, 7, 11, 15, 21, 33, 45, 51}");
        let lines = entry_lines_full(&en, 18);
        assert!(lines.len() >= 3);
        let cut = entry_lines(&en, 18);
        assert_eq!(cut.len(), MAX_ENTRY_LINES);
        let last: String = cut[1].iter().map(|p| p.0.as_str()).collect();
        assert!(last.ends_with("..") && last.chars().count() <= 18);
        let text: String = lines
            .iter()
            .map(|l| l.iter().map(|p| p.0.as_str()).collect::<String>())
            .map(|s| s.trim_start().to_string())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(
            text.replace(' ', ""),
            "std::vector<int>nums={2,7,11,15,21,33,45,51}"
        );
        assert!(lines.iter().all(|l| crate::syntax::width(l) <= 18));
    }

    #[test]
    fn only_whole_entries_are_shown() {
        let list = vec![e("a", "int", "1"), e("b", "std::string", "\"hello world\"")];
        assert_eq!(fitting(&list, 30, 5), (2, 2));
        assert_eq!(fitting(&list, 14, 2).0, 1);
    }
}
