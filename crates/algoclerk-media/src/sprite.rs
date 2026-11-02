// SPDX-License-Identifier: Apache-2.0
//! Palettes and the `.px` text pixel-art format.
//!
//! A palette file has one colour per line: `K = #1d1b33  ink`. `. = transparent` is implicit.
//! The master palette and its alternatives (colour-blind safe, high contrast) use the *same* letters, so a
//! sprite is stored as letters and resolved against whichever palette is active when it is drawn.
//!
//! ```text
//! # comment
//! size: 16x16        # frame size (required)
//! outline: K         # optional: auto-outline colour (palette letter)
//! repeat: 4x4        # optional: the grid is a WxH pattern tiled over `size`
//! bob: yes           # optional: add a second frame = first shifted down 1px (idle animation)
//! ................   # one row per line; '.' is transparent; letters come from the palette
//! ...
//! ---                # starts the next animation frame
//! ```
//!
//! Adapted from the sibling project Falabichos (Apache-2.0, same owner); sprites keep palette letters
//! instead of resolved colours so the palette can be swapped at runtime.

use crate::canvas::{Rgba, hex};
use std::collections::HashMap;

/// Colour used for letters that are missing from a palette (loud on purpose).
pub const MISSING: Rgba = [255, 0, 255, 255];

/// A set of colours addressed by single characters.
#[derive(Debug, Clone)]
pub struct Palette {
    table: [Rgba; 128],
    order: Vec<char>,
    names: HashMap<char, String>,
}

impl Default for Palette {
    fn default() -> Palette {
        let mut table = [MISSING; 128];
        table[b'.' as usize] = [0, 0, 0, 0];
        Palette {
            table,
            order: Vec::new(),
            names: HashMap::new(),
        }
    }
}

impl Palette {
    /// Parse a palette file.
    ///
    /// # Errors
    /// A readable message naming the offending line.
    pub fn parse(text: &str) -> Result<Palette, String> {
        let mut p = Palette::default();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (k, rest) = line
                .split_once('=')
                .ok_or(format!("palette line {}: expected `c = #rrggbb`", n + 1))?;
            let mut it = k.trim().chars();
            let ch = it.next().ok_or("empty key")?;
            if it.next().is_some() || !ch.is_ascii_graphic() {
                return Err(format!(
                    "palette line {}: key must be one printable ASCII character",
                    n + 1
                ));
            }
            let mut words = rest.split_whitespace();
            let val = words.next().unwrap_or("");
            let name: Vec<&str> = words.collect();
            let color = if val == "transparent" {
                [0, 0, 0, 0]
            } else {
                hex(val).ok_or(format!("palette line {}: bad colour `{val}`", n + 1))?
            };
            if p.order.contains(&ch) {
                return Err(format!("palette line {}: duplicate key `{ch}`", n + 1));
            }
            p.table[ch as usize] = color;
            p.order.push(ch);
            p.names.insert(ch, name.join(" "));
        }
        Ok(p)
    }

    /// Colour for a letter (magenta if the letter is not in the palette; never panics).
    #[inline]
    pub fn col(&self, ch: char) -> Rgba {
        self.table.get(ch as usize).copied().unwrap_or(MISSING)
    }

    /// Colour for a letter, `None` when the palette does not define it.
    pub fn get(&self, ch: char) -> Option<Rgba> {
        self.order.contains(&ch).then(|| self.col(ch))
    }

    /// Does the palette define this letter?
    pub fn has(&self, ch: char) -> bool {
        ch == '.' || self.order.contains(&ch)
    }

    /// The letters in file order.
    pub fn chars(&self) -> &[char] {
        &self.order
    }

    /// Human name of a colour (second column of the file).
    pub fn name(&self, ch: char) -> &str {
        self.names.get(&ch).map_or("", String::as_str)
    }

    /// Number of opaque colours (the style guide says: at most 32).
    pub fn opaque_count(&self) -> usize {
        self.order.iter().filter(|c| self.col(**c)[3] != 0).count()
    }
}

/// A decoded sprite: palette letters, one byte per pixel (`.` is transparent), one or more frames.
#[derive(Debug, Clone)]
pub struct Sprite {
    /// Frame width.
    pub w: usize,
    /// Frame height.
    pub h: usize,
    /// `w * h` letters per frame.
    pub frames: Vec<Vec<u8>>,
}

impl Sprite {
    /// A frame, wrapping the index.
    pub fn frame(&self, i: usize) -> &[u8] {
        &self.frames[i % self.frames.len()]
    }

    /// A solid placeholder sprite (used when art is missing).
    pub fn flat(w: usize, h: usize, letter: u8) -> Sprite {
        Sprite {
            w,
            h,
            frames: vec![vec![letter; w * h]],
        }
    }

    /// Letters used by the sprite (without `.`).
    pub fn letters(&self) -> Vec<char> {
        let mut v: Vec<char> = self
            .frames
            .iter()
            .flatten()
            .filter(|b| **b != b'.')
            .map(|b| *b as char)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Mirror every frame left to right.
    pub fn mirrored(&self) -> Sprite {
        let frames = self
            .frames
            .iter()
            .map(|f| {
                let mut out = f.clone();
                for y in 0..self.h {
                    out[y * self.w..(y + 1) * self.w].reverse();
                }
                out
            })
            .collect();
        Sprite {
            w: self.w,
            h: self.h,
            frames,
        }
    }
}

/// A parsed `.px` grid before roles are resolved (character templates use digits as role codes).
#[derive(Debug, Clone)]
pub struct Grid {
    /// Frame width.
    pub w: usize,
    /// Frame height.
    pub h: usize,
    /// Auto outline letter.
    pub outline: Option<char>,
    /// Add a bobbing second frame.
    pub bob: bool,
    /// Rows of letters per frame.
    pub frames: Vec<Vec<Vec<char>>>,
}

/// Parse a `.px` file.
///
/// # Errors
/// A readable message naming the offending line or frame.
pub fn parse_grid(text: &str) -> Result<Grid, String> {
    let (mut w, mut h) = (0usize, 0usize);
    let mut outline = None;
    let mut repeat: Option<(usize, usize)> = None;
    let mut bob = false;
    let mut frames: Vec<Vec<Vec<char>>> = vec![];
    let mut cur: Vec<Vec<char>> = vec![];
    let parse_dim = |v: &str| -> Result<(usize, usize), String> {
        let (a, b) = v.split_once('x').ok_or(format!("bad size `{v}` (want WxH)"))?;
        Ok((
            a.trim().parse().map_err(|_| format!("bad size `{v}`"))?,
            b.trim().parse().map_err(|_| format!("bad size `{v}`"))?,
        ))
    };
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "---" {
            frames.push(std::mem::take(&mut cur));
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let (k, v) = (k.trim(), v.trim());
            match k {
                "size" => (w, h) = parse_dim(v)?,
                "outline" => outline = v.chars().next(),
                "repeat" => repeat = Some(parse_dim(v)?),
                "bob" => bob = matches!(v, "yes" | "true"),
                "name" => {}
                other => return Err(format!("line {}: unknown header `{other}`", n + 1)),
            }
            continue;
        }
        cur.push(line.chars().collect());
    }
    if !cur.is_empty() {
        frames.push(cur);
    }
    if w == 0 || h == 0 {
        return Err("missing `size: WxH`".into());
    }
    if frames.is_empty() {
        return Err("no pixel rows".into());
    }
    let (pw, ph) = repeat.unwrap_or((w, h));
    let mut out_frames = vec![];
    for (fi, f) in frames.iter().enumerate() {
        if f.len() != ph || f.iter().any(|r| r.len() != pw) {
            return Err(format!(
                "frame {fi}: expected {pw}x{ph} rows, got {}x{}",
                f.first().map_or(0, Vec::len),
                f.len()
            ));
        }
        if repeat.is_some() {
            let mut full = vec![vec!['.'; w]; h];
            for (y, row) in full.iter_mut().enumerate() {
                for (x, px) in row.iter_mut().enumerate() {
                    *px = f[y % ph][x % pw];
                }
            }
            out_frames.push(full);
        } else {
            out_frames.push(f.clone());
        }
    }
    Ok(Grid {
        w,
        h,
        outline,
        bob,
        frames: out_frames,
    })
}

impl Grid {
    /// Resolve role digits and validate every letter against `pal`.
    ///
    /// # Errors
    /// An unknown palette letter.
    pub fn render(&self, pal: &Palette, roles: &HashMap<char, char>) -> Result<Sprite, String> {
        let mut frames: Vec<Vec<u8>> = vec![];
        for f in &self.frames {
            let mut px = vec![b'.'; self.w * self.h];
            for (y, row) in f.iter().enumerate() {
                for (x, &ch) in row.iter().enumerate() {
                    let ch = roles.get(&ch).copied().unwrap_or(ch);
                    if !pal.has(ch) {
                        return Err(format!("unknown palette letter `{ch}` at ({x},{y})"));
                    }
                    px[y * self.w + x] = ch as u8;
                }
            }
            if let Some(o) = self.outline {
                if !pal.has(o) {
                    return Err(format!("unknown outline letter `{o}`"));
                }
                let src = px.clone();
                for y in 0..self.h {
                    for x in 0..self.w {
                        if src[y * self.w + x] != b'.' {
                            continue;
                        }
                        let near = [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)].iter().any(|(dx, dy)| {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            nx >= 0
                                && ny >= 0
                                && (nx as usize) < self.w
                                && (ny as usize) < self.h
                                && src[ny as usize * self.w + nx as usize] != b'.'
                                && src[ny as usize * self.w + nx as usize] != o as u8
                        });
                        if near {
                            px[y * self.w + x] = o as u8;
                        }
                    }
                }
            }
            frames.push(px);
        }
        if self.bob && frames.len() == 1 {
            let first = frames[0].clone();
            let mut second = vec![b'.'; self.w * self.h];
            for y in 1..self.h {
                for x in 0..self.w {
                    second[y * self.w + x] = first[(y - 1) * self.w + x];
                }
            }
            frames.push(second);
        }
        Ok(Sprite {
            w: self.w,
            h: self.h,
            frames,
        })
    }
}

/// Parse a `.px` file with no roles.
///
/// # Errors
/// See [`parse_grid`] and [`Grid::render`].
pub fn parse_px(text: &str, pal: &Palette) -> Result<Sprite, String> {
    parse_grid(text)?.render(pal, &HashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> Palette {
        Palette::parse("K = #000000\nW = #ffffff\n").unwrap()
    }

    #[test]
    fn parses_frames_and_outline() {
        let t = "size: 4x3\noutline: K\n....\n.W..\n....\n---\n....\n..W.\n....\n";
        let s = parse_px(t, &pal()).unwrap();
        assert_eq!(s.frames.len(), 2);
        assert_eq!(s.frames[0][1], b'K', "pixel above the W is outlined");
        assert_eq!(s.frames[0][5], b'W');
        assert_eq!(s.frames[0][0], b'.');
    }

    #[test]
    fn repeat_tiles_pattern() {
        let s = parse_px("size: 4x4\nrepeat: 2x2\nKW\nWK\n", &pal()).unwrap();
        assert_eq!(s.frames[0][0], s.frames[0][2]);
        assert_ne!(s.frames[0][0], s.frames[0][1]);
    }

    #[test]
    fn bob_adds_second_frame() {
        let s = parse_px("size: 2x2\nbob: yes\nW.\n..\n", &pal()).unwrap();
        assert_eq!(s.frames.len(), 2);
        assert_eq!(s.frames[1][2], b'W');
    }

    #[test]
    fn roles_are_resolved() {
        let g = parse_grid("size: 2x1\n13\n").unwrap();
        let roles: HashMap<char, char> = [('1', 'K'), ('3', 'W')].into_iter().collect();
        let s = g.render(&pal(), &roles).unwrap();
        assert_eq!(s.frames[0], vec![b'K', b'W']);
        assert!(g.render(&pal(), &HashMap::new()).is_err());
    }

    #[test]
    fn mirrored_flips() {
        let s = parse_px("size: 2x1\nKW\n", &pal()).unwrap();
        assert_eq!(s.mirrored().frames[0], vec![b'W', b'K']);
    }

    #[test]
    fn errors_are_clear() {
        assert!(parse_px("size: 2x2\nWW\n", &pal()).unwrap_err().contains("expected"));
        assert!(parse_px("size: 2x2\nQW\nWW\n", &pal())
            .unwrap_err()
            .contains("unknown palette letter"));
        assert!(parse_px("WW\n", &pal()).unwrap_err().contains("size"));
        assert!(Palette::parse("KK = #000000").is_err());
        assert!(Palette::parse("K = #000000\nK = #111111").is_err());
    }

    #[test]
    fn palette_counts_and_names() {
        let p = Palette::parse("K = #000000 ink black\nW = #ffffff\n. = transparent\n").unwrap();
        assert_eq!(p.opaque_count(), 2);
        assert_eq!(p.name('K'), "ink black");
        assert_eq!(p.col('?'), MISSING);
        assert_eq!(p.col('.')[3], 0);
    }
}
