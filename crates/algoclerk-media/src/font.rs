// SPDX-License-Identifier: Apache-2.0
//! Monospaced bitmap fonts defined as text (`assets/art/font.txt`, 5x7, and `tinyfont.txt`, 3x5).
//!
//! One glyph per line: `<char or U+XXXX> <rows...>` where every row is `cols` characters of `#` (ink)
//! and `.` (empty). The big font has 8 rows (caps use rows 0..=6, x-height rows 2..=6, descenders row 7).
//! Text outside printable ASCII is *folded* (curly quotes, dashes, accented letters) so translations do not
//! show `?`; [`Font::covers`] reports whether a character can be drawn without a missing-glyph box.
//! Based on the sibling project Falabichos' font module (Apache-2.0, same owner); the glyph artwork is new.

use crate::canvas::{Canvas, Rgba};
use std::collections::HashMap;

/// Pixel advance of the 5x7 font at scale 1.
pub const ADVANCE: i32 = 6;
/// Default line height of the 5x7 font at scale 1.
pub const LINE_HEIGHT: i32 = 11;

/// A bitmap font.
#[derive(Debug, Clone)]
pub struct Font {
    cols: usize,
    rows: usize,
    glyphs: HashMap<char, Vec<u8>>,
}

/// Map a character to the closest drawable ASCII character (identity for ASCII).
pub fn fold(c: char) -> char {
    match c {
        '\u{2018}' | '\u{2019}' | '\u{2032}' => '\'',
        '\u{201C}' | '\u{201D}' | '\u{AB}' | '\u{BB}' => '"',
        '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
        '\u{2026}' => '.',
        '\u{A0}' | '\u{2009}' | '\u{202F}' => ' ',
        '\u{D7}' => 'x',
        '\u{2022}' | '\u{B7}' => '*',
        '\u{C0}'..='\u{C5}' => 'A',
        '\u{C7}' => 'C',
        '\u{C8}'..='\u{CB}' => 'E',
        '\u{CC}'..='\u{CF}' => 'I',
        '\u{D1}' => 'N',
        '\u{D2}'..='\u{D6}' => 'O',
        '\u{D9}'..='\u{DC}' => 'U',
        '\u{E0}'..='\u{E5}' => 'a',
        '\u{E7}' => 'c',
        '\u{E8}'..='\u{EB}' => 'e',
        '\u{EC}'..='\u{EF}' => 'i',
        '\u{F1}' => 'n',
        '\u{F2}'..='\u{F6}' => 'o',
        '\u{F9}'..='\u{FC}' => 'u',
        other => other,
    }
}

impl Font {
    /// Parse a font file whose glyphs are `cols` wide and `rows` tall.
    ///
    /// # Errors
    /// A readable message naming the offending line.
    pub fn parse(text: &str, cols: usize, rows: usize) -> Result<Font, String> {
        let mut glyphs = HashMap::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.split_whitespace();
            let key = it.next().ok_or("empty line")?;
            let ch = if let Some(hex) = key.strip_prefix("U+") {
                u32::from_str_radix(hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or(format!("line {}: bad code point", n + 1))?
            } else {
                let mut cs = key.chars();
                let c = cs.next().ok_or("empty key")?;
                if cs.next().is_some() {
                    return Err(format!("line {}: key `{key}` must be one character or U+XXXX", n + 1));
                }
                c
            };
            let rws: Vec<&str> = it.collect();
            if rws.len() != rows {
                return Err(format!("line {}: glyph `{key}` needs {rows} rows, has {}", n + 1, rws.len()));
            }
            let mut bits = vec![0u8; rows];
            for (r, row) in rws.iter().enumerate() {
                if row.chars().count() != cols || row.chars().any(|c| c != '#' && c != '.') {
                    return Err(format!(
                        "line {}: glyph `{key}` row {r} must be {cols} chars of # or .",
                        n + 1
                    ));
                }
                for (x, c) in row.chars().enumerate() {
                    if c == '#' {
                        bits[r] |= 1 << (cols - 1 - x);
                    }
                }
            }
            if glyphs.insert(ch, bits).is_some() {
                return Err(format!("line {}: duplicate glyph `{key}`", n + 1));
            }
        }
        Ok(Font { cols, rows, glyphs })
    }

    /// Horizontal advance at scale 1.
    pub fn advance(&self) -> i32 {
        self.cols as i32 + 1
    }

    /// Glyph height in rows.
    pub fn rows(&self) -> i32 {
        self.rows as i32
    }

    /// Number of glyphs.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Can this character be drawn (directly or after folding) without a missing-glyph box?
    pub fn covers(&self, c: char) -> bool {
        self.glyphs.contains_key(&fold(c))
    }

    /// Characters of `s` that cannot be drawn. Newlines and tabs are layout, not glyphs.
    pub fn missing(&self, s: &str) -> Vec<char> {
        let mut v: Vec<char> = s
            .chars()
            .filter(|c| *c != '\n' && *c != '\r' && *c != '\t' && !self.covers(*c))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Pixel width of `s` at `scale`.
    pub fn width(&self, s: &str, scale: i32) -> i32 {
        s.chars().count() as i32 * self.advance() * scale
    }

    fn glyph(&self, c: char) -> Option<&Vec<u8>> {
        self.glyphs.get(&fold(c)).or_else(|| self.glyphs.get(&'?'))
    }

    /// Draw one character at `(x, y)` (top-left of the glyph box).
    pub fn draw_char(&self, cv: &mut Canvas, c: char, x: i32, y: i32, color: Rgba, scale: i32) {
        let Some(bits) = self.glyph(c) else {
            return;
        };
        for (r, row) in bits.iter().enumerate() {
            if *row == 0 {
                continue;
            }
            for col in 0..self.cols {
                if row & (1 << (self.cols - 1 - col)) != 0 {
                    cv.rect_fill(
                        x + col as i32 * scale,
                        y + r as i32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }

    /// Draw `s` with its top-left at `(x, y)`. Returns the x position after the text.
    pub fn draw(&self, cv: &mut Canvas, s: &str, x: i32, y: i32, color: Rgba, scale: i32) -> i32 {
        let mut cx = x;
        for c in s.chars() {
            self.draw_char(cv, c, cx, y, color, scale);
            cx += self.advance() * scale;
        }
        cx
    }

    /// Word-wrap to `cols` characters per line. Honours `\n`. Very long words are split.
    pub fn wrap(s: &str, cols: usize) -> Vec<String> {
        let cols = cols.max(1);
        let mut lines = vec![];
        for para in s.split('\n') {
            let mut line = String::new();
            let mut len = 0usize;
            for word in para.split_whitespace() {
                let word: String = word.chars().map(fold).collect();
                let wl = word.chars().count();
                if len > 0 && len + 1 + wl > cols {
                    lines.push(std::mem::take(&mut line));
                    len = 0;
                }
                if wl > cols {
                    let cl: Vec<char> = word.chars().collect();
                    for chunk in cl.chunks(cols) {
                        if len > 0 {
                            lines.push(std::mem::take(&mut line));
                        }
                        line = chunk.iter().collect();
                        len = chunk.len();
                    }
                    continue;
                }
                if len > 0 {
                    line.push(' ');
                    len += 1;
                }
                line.push_str(&word);
                len += wl;
            }
            lines.push(line);
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Font {
        Font::parse(
            "A .###. #...# #...# ##### #...# #...# #...# .....\na ..... ..... .###. ....# .#### #...# .#### .....\n? .###. #...# ....# ...#. ..#.. ..... ..#.. .....\nU+0020 ..... ..... ..... ..... ..... ..... ..... .....\n",
            5,
            8,
        )
        .unwrap()
    }

    #[test]
    fn parse_errors() {
        assert!(Font::parse("A .###.", 5, 8).is_err());
        assert!(Font::parse("AB .... .... .... .... .... .... .... ....", 5, 8).is_err());
        assert!(Font::parse("A ##### ##### ##### ##### ##### ##### ##### #####\nA ##### ##### ##### ##### ##### ##### ##### #####", 5, 8).is_err());
    }

    #[test]
    fn folding_and_coverage() {
        let f = tiny();
        assert!(f.covers('A') && f.covers('\u{C1}') && f.covers('\u{E1}'));
        assert!(!f.covers('z'));
        assert_eq!(f.missing("Az\nz"), vec!['z']);
        assert_eq!(fold('\u{2019}'), '\'');
    }

    #[test]
    fn draws_a_glyph() {
        let f = tiny();
        let mut c = Canvas::new(12, 10);
        let end = f.draw(&mut c, "A", 0, 0, [255, 255, 255, 255], 1);
        assert_eq!(end, 6);
        assert_eq!(c.get(1, 0), [255, 255, 255, 255]);
        assert_eq!(c.get(0, 0)[3], 0);
        let mut c2 = Canvas::new(12, 18);
        f.draw(&mut c2, "A", 0, 0, [255, 255, 255, 255], 2);
        assert_eq!(c2.get(2, 0), [255, 255, 255, 255]);
        assert_eq!(c2.get(3, 1), [255, 255, 255, 255]);
    }

    #[test]
    fn wrapping() {
        let l = Font::wrap("aa aa aa\nA", 5);
        assert_eq!(l, vec!["aa aa", "aa", "A"]);
        let l = Font::wrap("aaaaaaa", 3);
        assert_eq!(l, vec!["aaa", "aaa", "a"]);
    }
}
