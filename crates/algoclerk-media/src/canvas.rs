// SPDX-License-Identifier: Apache-2.0
//! A small RGBA software canvas with an optional clip rectangle.
//! Adapted from the sibling project Falabichos (Apache-2.0, same owner).

use crate::sprite::{Palette, Sprite};

/// One RGBA8 pixel.
pub type Rgba = [u8; 4];

/// An integer rectangle (x, y, width, height).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

impl Rect {
    /// A rectangle.
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    /// Right edge (exclusive).
    pub const fn right(&self) -> i32 {
        self.x + self.w
    }
    /// Bottom edge (exclusive).
    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }
    /// Is the point inside?
    pub const fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    /// Shrink by `n` on every side.
    pub const fn inset(&self, n: i32) -> Rect {
        Rect::new(self.x + n, self.y + n, self.w - 2 * n, self.h - 2 * n)
    }
    /// Grow by `n` on every side.
    pub const fn grow(&self, n: i32) -> Rect {
        self.inset(-n)
    }
    /// Intersection (may be empty, with non-positive size).
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        Rect::new(x, y, r - x, b - y)
    }
    /// True when the rectangle lies completely inside `outer`.
    pub fn inside(&self, outer: &Rect) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.right() <= outer.right()
            && self.bottom() <= outer.bottom()
    }
    /// Centre point.
    pub const fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
    /// Translate.
    pub const fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

/// An RGBA8 image that the software renderer draws into.
#[derive(Debug, Clone)]
pub struct Canvas {
    /// Width in pixels.
    pub w: usize,
    /// Height in pixels.
    pub h: usize,
    /// RGBA8, row-major.
    pub px: Vec<u8>,
    clip: Rect,
}

impl Canvas {
    /// A transparent canvas.
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            px: vec![0; w * h * 4],
            clip: Rect::new(0, 0, w as i32, h as i32),
        }
    }

    /// Bounds of the canvas.
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.w as i32, self.h as i32)
    }

    /// Restrict drawing to `r` (intersected with the canvas). Returns the previous clip.
    pub fn set_clip(&mut self, r: Rect) -> Rect {
        let old = self.clip;
        self.clip = r.intersect(&self.bounds());
        old
    }

    /// Remove the clip rectangle.
    pub fn reset_clip(&mut self) {
        self.clip = self.bounds();
    }

    /// Fill the whole canvas (ignores the clip).
    pub fn clear(&mut self, c: Rgba) {
        for p in self.px.chunks_exact_mut(4) {
            p.copy_from_slice(&c);
        }
    }

    /// Set one pixel (alpha-blended if `c[3] < 255`, skipped if 0). Clipped.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if c[3] == 0 || !self.clip.contains(x, y) {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        if c[3] == 255 {
            self.px[i..i + 4].copy_from_slice(&c);
        } else {
            let a = u32::from(c[3]);
            for (dst, src) in self.px[i..i + 3].iter_mut().zip(c.iter()) {
                *dst = ((u32::from(*src) * a + u32::from(*dst) * (255 - a)) / 255) as u8;
            }
            self.px[i + 3] = 255;
        }
    }

    /// Read one pixel (must be in bounds).
    pub fn get(&self, x: usize, y: usize) -> Rgba {
        let i = (y * self.w + x) * 4;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    /// Filled rectangle.
    pub fn rect_fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        let r = Rect::new(x, y, w, h).intersect(&self.clip);
        if r.w <= 0 || r.h <= 0 || c[3] == 0 {
            return;
        }
        if c[3] == 255 {
            for yy in r.y..r.bottom() {
                let start = (yy as usize * self.w + r.x as usize) * 4;
                for p in self.px[start..start + r.w as usize * 4].chunks_exact_mut(4) {
                    p.copy_from_slice(&c);
                }
            }
        } else {
            for yy in r.y..r.bottom() {
                for xx in r.x..r.right() {
                    self.set(xx, yy, c);
                }
            }
        }
    }

    /// One-pixel rectangle outline.
    pub fn rect_outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        self.rect_fill(x, y, w, 1, c);
        self.rect_fill(x, y + h - 1, w, 1, c);
        self.rect_fill(x, y, 1, h, c);
        self.rect_fill(x + w - 1, y, 1, h, c);
    }

    /// Horizontal line.
    pub fn hline(&mut self, x: i32, y: i32, w: i32, c: Rgba) {
        self.rect_fill(x, y, w, 1, c);
    }

    /// Vertical line.
    pub fn vline(&mut self, x: i32, y: i32, h: i32, c: Rgba) {
        self.rect_fill(x, y, 1, h, c);
    }

    /// Bresenham line.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Filled circle (pixel-centre test).
    pub fn disc(&mut self, cx: i32, cy: i32, r: i32, c: Rgba) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy <= r * r {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// Fill a rectangle with a 2x2 checkerboard of two colours (`b` may be transparent). `phase` 0 or 1.
    pub fn checker_fill(&mut self, r: Rect, a: Rgba, b: Rgba, phase: i32) {
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if (x + y + phase) & 1 == 0 {
                    self.set(x, y, a);
                } else {
                    self.set(x, y, b);
                }
            }
        }
    }

    /// Draw one frame of a sprite at 1:1. `flip` mirrors horizontally. Colours come from `pal`.
    pub fn blit(&mut self, s: &Sprite, frame: usize, x: i32, y: i32, flip: bool, pal: &Palette) {
        let f = s.frame(frame);
        for sy in 0..s.h {
            let row = &f[sy * s.w..(sy + 1) * s.w];
            for (sx, &ch) in row.iter().enumerate() {
                if ch == b'.' {
                    continue;
                }
                let dx = if flip { s.w - 1 - sx } else { sx } as i32;
                self.set(x + dx, y + sy as i32, pal.col(ch as char));
            }
        }
    }

    /// Like [`Canvas::blit`] but letters found in `map` (`(from, to)`) are swapped first (recolouring icons).
    pub fn blit_map(
        &mut self,
        s: &Sprite,
        frame: usize,
        pos: (i32, i32),
        flip: bool,
        pal: &Palette,
        map: &[(char, char)],
    ) {
        let f = s.frame(frame);
        for sy in 0..s.h {
            for sx in 0..s.w {
                let mut ch = f[sy * s.w + sx] as char;
                if ch == '.' {
                    continue;
                }
                if let Some((_, to)) = map.iter().find(|(from, _)| *from == ch) {
                    ch = *to;
                }
                let dx = if flip { s.w - 1 - sx } else { sx } as i32;
                self.set(pos.0 + dx, pos.1 + sy as i32, pal.col(ch));
            }
        }
    }

    /// Like [`Canvas::blit`] but every opaque pixel is drawn in `color` (silhouettes, flashes).
    pub fn blit_tint(&mut self, s: &Sprite, frame: usize, x: i32, y: i32, flip: bool, color: Rgba) {
        let f = s.frame(frame);
        for sy in 0..s.h {
            for sx in 0..s.w {
                if f[sy * s.w + sx] == b'.' {
                    continue;
                }
                let dx = if flip { s.w - 1 - sx } else { sx } as i32;
                self.set(x + dx, y + sy as i32, color);
            }
        }
    }

    /// Draw only the sub-rectangle `(rx, ry, rw, rh)` of a frame.
    pub fn blit_region(
        &mut self,
        s: &Sprite,
        frame: usize,
        region: (usize, usize, usize, usize),
        pos: (i32, i32),
        pal: &Palette,
    ) {
        let f = s.frame(frame);
        let (rx, ry, rw, rh) = region;
        for sy in ry..(ry + rh).min(s.h) {
            for sx in rx..(rx + rw).min(s.w) {
                let ch = f[sy * s.w + sx];
                if ch != b'.' {
                    self.set(
                        pos.0 + (sx - rx) as i32,
                        pos.1 + (sy - ry) as i32,
                        pal.col(ch as char),
                    );
                }
            }
        }
    }

    /// Copy another canvas onto this one at `(x, y)`.
    pub fn draw_canvas(&mut self, src: &Canvas, x: i32, y: i32) {
        for sy in 0..src.h {
            for sx in 0..src.w {
                self.set(x + sx as i32, y + sy as i32, src.get(sx, sy));
            }
        }
    }

    /// Tint the whole canvas toward a colour (`amount` 0..=255), used for fades.
    pub fn fade_to(&mut self, c: Rgba, amount: u8) {
        let a = u32::from(amount);
        for p in self.px.chunks_exact_mut(4) {
            for (dst, src) in p[..3].iter_mut().zip(c.iter()) {
                *dst = ((u32::from(*src) * a + u32::from(*dst) * (255 - a)) / 255) as u8;
            }
        }
    }

    /// Encode as PNG.
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = vec![];
        {
            let mut enc = png::Encoder::new(&mut out, self.w as u32, self.h as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            if let Ok(mut w) = enc.write_header() {
                let _ = w.write_image_data(&self.px);
            }
        }
        out
    }

    /// Nearest-neighbour upscale.
    pub fn upscaled(&self, k: usize) -> Canvas {
        let mut out = Canvas::new(self.w * k, self.h * k);
        for y in 0..out.h {
            for x in 0..out.w {
                let c = self.get(x / k, y / k);
                let i = (y * out.w + x) * 4;
                out.px[i..i + 4].copy_from_slice(&c);
            }
        }
        out
    }

    /// A stable 64-bit FNV-1a hash of the pixels (golden-image smoke tests).
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in &self.px {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    /// Mean absolute difference per channel against another canvas of the same size (0..=255).
    pub fn mean_diff(&self, other: &Canvas) -> f64 {
        if self.w != other.w || self.h != other.h {
            return 255.0;
        }
        let total: u64 = self
            .px
            .iter()
            .zip(&other.px)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        total as f64 / self.px.len().max(1) as f64
    }
}

/// Parse `#rrggbb`.
pub fn hex(c: &str) -> Option<Rgba> {
    let c = c.strip_prefix('#')?;
    if c.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(c, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8, 255])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_clips_and_blends() {
        let mut c = Canvas::new(4, 4);
        c.set(-1, 0, [1, 2, 3, 255]);
        c.set(4, 0, [1, 2, 3, 255]);
        c.set(1, 1, [10, 20, 30, 255]);
        assert_eq!(c.get(1, 1), [10, 20, 30, 255]);
        c.set(1, 1, [110, 120, 130, 128]);
        let g = c.get(1, 1);
        assert!(g[0] > 10 && g[0] < 110);
    }

    #[test]
    fn clip_rect_limits_drawing() {
        let mut c = Canvas::new(8, 8);
        c.set_clip(Rect::new(2, 2, 3, 3));
        c.rect_fill(0, 0, 8, 8, [9, 9, 9, 255]);
        assert_eq!(c.get(1, 1)[3], 0);
        assert_eq!(c.get(2, 2), [9, 9, 9, 255]);
        assert_eq!(c.get(5, 5)[3], 0);
        c.reset_clip();
        c.set(0, 0, [1, 1, 1, 255]);
        assert_eq!(c.get(0, 0)[3], 255);
    }

    #[test]
    fn rect_helpers() {
        let a = Rect::new(0, 0, 10, 10);
        assert!(Rect::new(2, 2, 3, 3).inside(&a));
        assert!(!Rect::new(8, 2, 3, 3).inside(&a));
        assert_eq!(a.inset(2), Rect::new(2, 2, 6, 6));
        assert!(a.contains(9, 9) && !a.contains(10, 9));
    }

    #[test]
    fn png_header_and_checksum() {
        let c = Canvas::new(3, 2);
        assert_eq!(&c.to_png()[1..4], b"PNG");
        let mut d = Canvas::new(3, 2);
        assert_eq!(c.checksum(), d.checksum());
        d.set(0, 0, [1, 2, 3, 255]);
        assert_ne!(c.checksum(), d.checksum());
        assert!(c.mean_diff(&d) > 0.0);
    }

    #[test]
    fn hex_parse() {
        assert_eq!(hex("#ff0080"), Some([255, 0, 128, 255]));
        assert_eq!(hex("ff0080"), None);
    }
}
