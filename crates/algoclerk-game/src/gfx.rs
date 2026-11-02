// SPDX-License-Identifier: Apache-2.0
//! Drawing helpers on the virtual low-resolution screen: panels, buttons, text, icons, bubbles.
//! Colours are palette letters, so every screen follows the active palette (warm, colour-blind, high contrast).
//! The panel and text helpers follow the look of the sibling project Falabichos (cream panels, ink borders).

use algoclerk_media::{ArtBank, Canvas, Palette, Rect, Rgba};

/// Glyph height of the main font (caps and digits).
pub const TEXT_H: i32 = 7;

/// How a button looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Cream paper button.
    Normal,
    /// Amber call to action.
    Primary,
    /// Green confirm.
    Good,
    /// Red destructive.
    Danger,
    /// Aqua, "switched on".
    On,
    /// Dark navy (toolbars on dark panels).
    Dark,
}

/// Interaction state of a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    /// The mouse is over it.
    pub hover: bool,
    /// It is being pressed.
    pub down: bool,
    /// It cannot be used.
    pub disabled: bool,
    /// Keyboard focus is here.
    pub focus: bool,
}

/// A drawing context.
#[derive(Debug)]
pub struct Gfx<'a> {
    /// The target canvas.
    pub c: &'a mut Canvas,
    /// Art and fonts.
    pub bank: &'a ArtBank,
    /// The active palette.
    pub pal: &'a Palette,
    /// Frame counter (60 per second).
    pub tick: u64,
    /// Skip decorative motion.
    pub reduce_motion: bool,
    /// Drawing origin offset (a "viewport": scene-local coordinates).
    pub off: (i32, i32),
}

/// Ease-out cubic, `t` in 0..=1.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Ease-in-out quadratic.
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
}

impl Gfx<'_> {
    /// Colour of a palette letter.
    #[inline]
    pub fn col(&self, ch: char) -> Rgba {
        self.pal.col(ch)
    }

    /// Filled rectangle.
    pub fn rect(&mut self, r: Rect, ch: char) {
        let c = self.col(ch);
        self.c.rect_fill(r.x + self.off.0, r.y + self.off.1, r.w, r.h, c);
    }

    /// One-pixel outline.
    pub fn frame(&mut self, r: Rect, ch: char) {
        let c = self.col(ch);
        self.c.rect_outline(r.x + self.off.0, r.y + self.off.1, r.w, r.h, c);
    }

    /// Horizontal line.
    pub fn hline(&mut self, x: i32, y: i32, w: i32, ch: char) {
        let c = self.col(ch);
        self.c.hline(x + self.off.0, y + self.off.1, w, c);
    }

    /// Vertical line.
    pub fn vline(&mut self, x: i32, y: i32, h: i32, ch: char) {
        let c = self.col(ch);
        self.c.vline(x + self.off.0, y + self.off.1, h, c);
    }

    /// Single pixel.
    pub fn px(&mut self, x: i32, y: i32, ch: char) {
        let c = self.col(ch);
        self.c.set(x + self.off.0, y + self.off.1, c);
    }

    /// Line.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, ch: char) {
        let c = self.col(ch);
        let (ox, oy) = self.off;
        self.c.line(x0 + ox, y0 + oy, x1 + ox, y1 + oy, c);
    }

    /// A rectangle filled with a checkerboard of two letters (`b` may be `'.'` for transparent).
    pub fn checker(&mut self, r: Rect, a: char, b: char, phase: i32) {
        let (ca, cb) = (self.col(a), self.col(b));
        self.c.checker_fill(r.offset(self.off.0, self.off.1), ca, cb, phase);
    }

    /// Text at 1:1 (top-left at `x`,`y`). Returns the x after the text.
    pub fn text(&mut self, x: i32, y: i32, s: &str, ch: char) -> i32 {
        let c = self.col(ch);
        self.bank.font.draw(self.c, s, x + self.off.0, y + self.off.1, c, 1)
    }

    /// Text at an integer scale.
    pub fn text_s(&mut self, x: i32, y: i32, s: &str, ch: char, scale: i32) -> i32 {
        let c = self.col(ch);
        self.bank.font.draw(self.c, s, x + self.off.0, y + self.off.1, c, scale)
    }

    /// Text with a one-pixel (scaled) drop shadow.
    pub fn text_shadow(&mut self, x: i32, y: i32, s: &str, ch: char, shadow: char, scale: i32) -> i32 {
        self.text_s(x + scale, y + scale, s, shadow, scale);
        self.text_s(x, y, s, ch, scale)
    }

    /// Centre text horizontally on `cx`.
    pub fn text_c(&mut self, cx: i32, y: i32, s: &str, ch: char, scale: i32) {
        let w = self.bank.font.width(s, scale);
        self.text_s(cx - w / 2, y, s, ch, scale);
    }

    /// Right-align text so it ends at `rx`.
    pub fn text_r(&mut self, rx: i32, y: i32, s: &str, ch: char, scale: i32) {
        let w = self.bank.font.width(s, scale);
        self.text_s(rx - w, y, s, ch, scale);
    }

    /// Text centred in a rectangle both ways.
    pub fn text_in(&mut self, r: Rect, s: &str, ch: char, scale: i32) {
        let w = self.bank.font.width(s, scale);
        let h = TEXT_H * scale;
        self.text_s(r.x + (r.w - w) / 2, r.y + (r.h - h) / 2, s, ch, scale);
    }

    /// Pixel width of `s` in the main font at `scale`.
    pub fn text_w(&self, s: &str, scale: i32) -> i32 {
        self.bank.font.width(s, scale)
    }

    /// Text in the tiny 3x5 font (digits, capitals).
    pub fn tiny(&mut self, x: i32, y: i32, s: &str, ch: char) -> i32 {
        let c = self.col(ch);
        let up: String = s.chars().map(|c| c.to_ascii_uppercase()).collect();
        self.bank.tiny.draw(self.c, &up, x + self.off.0, y + self.off.1, c, 1)
    }

    /// Tiny text centred on `cx`.
    pub fn tiny_c(&mut self, cx: i32, y: i32, s: &str, ch: char) {
        let w = s.chars().count() as i32 * 4 - 1;
        self.tiny(cx - w / 2, y, s, ch);
    }

    /// Draw a sprite frame.
    pub fn sprite(&mut self, key: &str, frame: usize, x: i32, y: i32) {
        let s = self.bank.sp(key);
        self.c.blit(s, frame, x + self.off.0, y + self.off.1, false, self.pal);
    }

    /// Draw a sprite frame mirrored.
    pub fn sprite_flip(&mut self, key: &str, frame: usize, x: i32, y: i32, flip: bool) {
        let s = self.bank.sp(key);
        self.c.blit(s, frame, x + self.off.0, y + self.off.1, flip, self.pal);
    }

    /// Draw a sprite frame with letters remapped.
    pub fn sprite_map(&mut self, key: &str, frame: usize, x: i32, y: i32, map: &[(char, char)]) {
        let s = self.bank.sp(key);
        self.c.blit_map(s, frame, (x + self.off.0, y + self.off.1), false, self.pal, map);
    }

    /// Draw an ink icon (`icons/<name>`) recoloured to `ch`.
    pub fn icon(&mut self, name: &str, x: i32, y: i32, ch: char) {
        let key = format!("icons/{name}");
        let s = self.bank.sp(&key);
        self.c.blit_map(s, 0, (x + self.off.0, y + self.off.1), false, self.pal, &[('K', ch)]);
    }

    /// Draw an icon centred in a rectangle.
    pub fn icon_in(&mut self, name: &str, r: Rect, ch: char) {
        let key = format!("icons/{name}");
        let (w, h) = {
            let s = self.bank.sp(&key);
            (s.w as i32, s.h as i32)
        };
        self.icon(name, r.x + (r.w - w) / 2, r.y + (r.h - h) / 2, ch);
    }

    /// A rounded cream panel with an ink border and a drop shadow.
    pub fn panel(&mut self, r: Rect, fill: char) {
        self.panel_with(r, fill, 'K', true);
    }

    /// A rounded panel with chosen fill, border colour and optional shadow.
    pub fn panel_with(&mut self, r: Rect, fill: char, border: char, shadow: bool) {
        if r.w < 5 || r.h < 5 {
            return;
        }
        if shadow {
            self.round_rect(r.offset(2, 2), 'k', 'k');
        }
        self.round_rect(r, fill, border);
        // inner bevel
        let hi = if fill == 'W' { 'X' } else { fill };
        self.hline(r.x + 2, r.y + 1, r.w - 4, hi);
        let sh = match fill {
            'W' => 'w',
            _ => fill,
        };
        self.hline(r.x + 2, r.bottom() - 2, r.w - 4, sh);
    }

    /// A rectangle with knocked-out corners, border and fill.
    pub fn round_rect(&mut self, r: Rect, fill: char, border: char) {
        let (cf, cb) = (self.col(fill), self.col(border));
        let r = r.offset(self.off.0, self.off.1);
        self.c.rect_fill(r.x + 1, r.y, r.w - 2, r.h, cf);
        self.c.rect_fill(r.x, r.y + 1, r.w, r.h - 2, cf);
        self.c.hline(r.x + 1, r.y, r.w - 2, cb);
        self.c.hline(r.x + 1, r.bottom() - 1, r.w - 2, cb);
        self.c.vline(r.x, r.y + 1, r.h - 2, cb);
        self.c.vline(r.right() - 1, r.y + 1, r.h - 2, cb);
    }

    /// A dark ribbon header with centred or left text.
    pub fn header(&mut self, r: Rect, title: &str, centered: bool) {
        self.round_rect(r, 'k', 'K');
        self.hline(r.x + 2, r.y + 1, r.w - 4, 'm');
        let ty = r.y + (r.h - TEXT_H) / 2;
        if centered {
            self.text_c(r.x + r.w / 2, ty, title, 'W', 1);
        } else {
            self.text(r.x + 6, ty, title, 'W');
        }
    }

    /// The face colours `(face, shade, text)` of a button kind.
    pub fn kind_colors(kind: Kind) -> (char, char, char) {
        match kind {
            Kind::Normal => ('W', 'w', 'K'),
            Kind::Primary => ('Y', 'o', 'K'),
            Kind::Good => ('g', 'G', 'K'),
            Kind::Danger => ('R', 'r', 'W'),
            Kind::On => ('c', 'T', 'K'),
            Kind::Dark => ('k', 'K', 'W'),
        }
    }

    /// Draw the box of a button; returns the content rectangle (shifted when pressed).
    pub fn button_box(&mut self, r: Rect, kind: Kind, st: State) -> Rect {
        let (mut face, mut shade, _) = Gfx::kind_colors(kind);
        if st.disabled {
            face = 'A';
            shade = 'a';
        } else if st.hover && !st.down {
            face = match kind {
                Kind::Normal => 'X',
                Kind::Primary => 'y',
                Kind::Good => 'l',
                Kind::Danger => 'o',
                Kind::On => 'c',
                Kind::Dark => 'm',
            };
        }
        let down = st.down && !st.disabled;
        // shadow / base
        self.round_rect(r, shade, 'K');
        let face_rect = if down {
            Rect::new(r.x + 1, r.y + 2, r.w - 2, r.h - 3)
        } else {
            Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 3)
        };
        self.rect(face_rect, face);
        if !down && !st.disabled {
            self.hline(r.x + 2, r.y + 1, r.w - 4, 'X');
        }
        if st.focus {
            self.focus_ring(r);
        }
        face_rect
    }

    /// Marching-dashes focus ring around a rectangle.
    pub fn focus_ring(&mut self, r: Rect) {
        let g = r.grow(2).offset(self.off.0, self.off.1);
        let phase = if self.reduce_motion { 0 } else { (self.tick / 6) as i32 };
        let (a, b) = (self.col('y'), self.col('K'));
        for x in g.x..g.right() {
            let c = if (x + phase) % 4 < 2 { a } else { b };
            self.c.set(x, g.y, c);
            self.c.set(x, g.bottom() - 1, c);
        }
        for y in g.y..g.bottom() {
            let c = if (y + phase) % 4 < 2 { a } else { b };
            self.c.set(g.x, y, c);
            self.c.set(g.right() - 1, y, c);
        }
    }

    /// A text button.
    pub fn button(&mut self, r: Rect, label: &str, kind: Kind, st: State) {
        let face = self.button_box(r, kind, st);
        let (_, _, mut tc) = Gfx::kind_colors(kind);
        if st.disabled {
            tc = 'a';
        }
        self.text_in(face, label, tc, 1);
    }

    /// An icon button.
    pub fn icon_button(&mut self, r: Rect, icon: &str, kind: Kind, st: State) {
        let face = self.button_box(r, kind, st);
        let (_, _, mut tc) = Gfx::kind_colors(kind);
        if st.disabled {
            tc = 'a';
        }
        self.icon_in(icon, face, tc);
    }

    /// A vertical scrollbar track + thumb inside `r`.
    pub fn scrollbar(&mut self, r: Rect, offset: i32, content: i32, view: i32) {
        if content <= view || r.h < 8 {
            return;
        }
        self.rect(r, 'w');
        self.frame(r, 'a');
        let th = ((r.h as f32 * view as f32 / content as f32) as i32).clamp(8, r.h);
        let travel = r.h - th;
        let max_off = (content - view).max(1);
        let ty = r.y + (travel as f32 * offset.clamp(0, max_off) as f32 / max_off as f32) as i32;
        self.rect(Rect::new(r.x, ty, r.w, th), 'k');
        self.hline(r.x, ty, r.w, 'm');
    }

    /// The marching selection arrow, pointing right, with its tip near `(x, y)`.
    pub fn cursor(&mut self, x: i32, y: i32) {
        let bob = if self.reduce_motion { 0 } else { ((self.tick / 20) % 2) as i32 };
        self.sprite("ui/cursor", 0, x + bob, y);
    }

    /// A star (`big` = 17x17, otherwise 11x11).
    pub fn star(&mut self, x: i32, y: i32, on: bool, big: bool) {
        let key = format!("ui/star_{}_{}", if big { "big" } else { "small" }, if on { "on" } else { "off" });
        self.sprite(&key, 0, x, y);
    }

    /// A speech bubble whose tail points down at `(tip_x, ..)`. `r` is the bubble body.
    pub fn bubble(&mut self, r: Rect, tip_x: i32, fill: char, border: char) {
        self.round_rect(r, fill, border);
        let tx = tip_x.clamp(r.x + 4, r.right() - 11);
        self.rect(Rect::new(tx, r.bottom() - 1, 7, 1), fill);
        self.sprite_map("ui/bubble_tail", 0, tx, r.bottom() - 1, &[('K', border), ('W', fill)]);
    }

    /// A small coloured pill with text (labels, counters).
    pub fn chip(&mut self, r: Rect, text: &str, fill: char, ink: char) {
        self.round_rect(r, fill, 'K');
        self.text_in(r, text, ink, 1);
    }

    /// Dim a rectangle with a sparse dither of ink (modal shade), so the screen behind stays readable.
    pub fn shade(&mut self, r: Rect) {
        let c = self.col('K');
        let r = r.offset(self.off.0, self.off.1);
        for y in (r.y..r.bottom()).step_by(2) {
            for x in (r.x..r.right()).step_by(2) {
                self.c.set(x, y, c);
            }
        }
    }

    /// Wrap text at `cols` and draw; returns the number of lines drawn (limited by `max_lines`).
    #[allow(clippy::too_many_arguments)]
    pub fn paragraph(
        &mut self,
        x: i32,
        y: i32,
        cols: usize,
        s: &str,
        ch: char,
        scale: i32,
        line_h: i32,
        max_lines: usize,
    ) -> usize {
        let lines = algoclerk_media::Font::wrap(s, cols);
        let n = lines.len().min(max_lines);
        for (i, l) in lines.iter().take(n).enumerate() {
            self.text_s(x, y + i as i32 * line_h, l, ch, scale);
        }
        n
    }
}

/// Number of text columns that fit in `px` pixels at `scale`.
pub fn cols_for(px: i32, scale: i32) -> usize {
    (px / (6 * scale)).max(1) as usize
}

/// Truncate to `n` characters with a trailing `..` (display only).
pub fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(2)).collect();
        t.push_str("..");
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_and_cols() {
        assert_eq!(clip("hello", 10), "hello");
        assert_eq!(clip("hello world", 7), "hello..");
        assert_eq!(cols_for(60, 1), 10);
        assert_eq!(cols_for(60, 2), 5);
    }

    #[test]
    fn easing_is_bounded() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert!(ease_in_out(0.25) < 0.25);
        assert_eq!(ease_out(5.0), 1.0);
    }
}
