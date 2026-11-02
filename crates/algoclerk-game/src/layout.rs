// SPDX-License-Identifier: Apache-2.0
//! The virtual low-resolution screen: orientation, integer scale and safe area.
//!
//! Landscape is 480x270 and portrait is 270x480 (16:9 either way, so 1080p is exactly 4x and 720p is a
//! crisp 2x with a thin border). The window picks whichever orientation gives the larger integer scale.
//! Everything (panels, text, buttons, sprites) is drawn into this space and scaled with nearest neighbour.
//! Adapted from the integer-scaled virtual screen of the sibling project Falabichos.

use algoclerk_media::Rect;

/// Landscape width.
pub const LAND_W: i32 = 480;
/// Landscape height.
pub const LAND_H: i32 = 270;
/// Portrait width.
pub const PORT_W: i32 = 270;
/// Portrait height.
pub const PORT_H: i32 = 480;

/// Smallest comfortable touch target in virtual pixels: 44 CSS pixels at the usual phone density
/// (one virtual pixel is about 1.33 CSS pixels when the 270 px portrait width fills a 360 px phone).
pub const TOUCH_MIN: i32 = 33;
/// Smallest clickable size with a mouse.
pub const MOUSE_MIN: i32 = 14;

/// Screen geometry handed to every screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// Virtual width.
    pub w: i32,
    /// Virtual height.
    pub h: i32,
    /// Portrait layout.
    pub portrait: bool,
    /// Area that is safe to put UI in (notches, rounded corners, home indicator excluded).
    pub safe: Rect,
    /// Touch-first sizing.
    pub touch: bool,
}

impl Viewport {
    /// A viewport with no unsafe margins.
    pub fn new(portrait: bool, touch: bool) -> Viewport {
        let (w, h) = if portrait {
            (PORT_W, PORT_H)
        } else {
            (LAND_W, LAND_H)
        };
        Viewport {
            w,
            h,
            portrait,
            safe: Rect::new(0, 0, w, h),
            touch,
        }
    }

    /// The whole virtual screen.
    pub fn screen(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }

    /// Minimum size of anything tappable.
    pub fn hit_min(&self) -> i32 {
        if self.touch { TOUCH_MIN } else { MOUSE_MIN }
    }

    /// Height of a normal button.
    pub fn btn_h(&self) -> i32 {
        if self.touch { TOUCH_MIN } else { 20 }
    }

    /// Height of a list row (program slips, menu rows).
    pub fn row_h(&self) -> i32 {
        if self.touch { 30 } else { 22 }
    }
}

/// Window geometry resolved into a viewport and an integer scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// The virtual screen.
    pub vp: Viewport,
    /// Integer scale in physical pixels per virtual pixel (fractional only when the window is tiny).
    pub scale: f32,
    /// Where the scaled screen sits in the window (physical pixels, top-left origin).
    pub origin: (f32, f32),
}

/// Integer scale that fits `vw`x`vh` into the available space (never below 1 unless the window is smaller).
pub fn fit_scale(avail_w: f32, avail_h: f32, vw: i32, vh: i32) -> f32 {
    let fit = (avail_w / vw as f32).min(avail_h / vh as f32);
    if fit >= 1.0 {
        fit.floor()
    } else {
        fit.max(0.1)
    }
}

/// Pick orientation and scale for a window of `win_w` x `win_h` physical pixels with safe-area insets
/// `(left, top, right, bottom)` in physical pixels.
pub fn choose(win_w: f32, win_h: f32, insets: (f32, f32, f32, f32), touch: bool) -> Display {
    // The scale is chosen from the whole window, so phones use big pixels; the notch and home bar usually
    // fall into the unused border, and whatever still overlaps shrinks `Viewport::safe`.
    let avail_w = win_w.max(1.0);
    let avail_h = win_h.max(1.0);
    let land = fit_scale(avail_w, avail_h, LAND_W, LAND_H);
    let port = fit_scale(avail_w, avail_h, PORT_W, PORT_H);
    let portrait = port > land || (port == land && avail_h > avail_w);
    let mut vp = Viewport::new(portrait, touch);
    let scale = if portrait { port } else { land };
    // The scaled screen is centred in the window; the safe rectangle is the part of the virtual screen
    // that lies inside the insets.
    let sw = vp.w as f32 * scale;
    let sh = vp.h as f32 * scale;
    let ox = (avail_w - sw) / 2.0;
    let oy = (avail_h - sh) / 2.0;
    let l = ((insets.0 - ox).max(0.0) / scale).ceil() as i32;
    let t = ((insets.1 - oy).max(0.0) / scale).ceil() as i32;
    let r = ((ox + sw - (win_w - insets.2)).max(0.0) / scale).ceil() as i32;
    let b = ((oy + sh - (win_h - insets.3)).max(0.0) / scale).ceil() as i32;
    vp.safe = Rect::new(l, t, vp.w - l - r, vp.h - t - b);
    Display {
        vp,
        scale,
        origin: (ox, oy),
    }
}

impl Display {
    /// Convert a window position (physical pixels) to virtual coordinates.
    pub fn to_virtual(&self, x: f32, y: f32) -> (i32, i32) {
        (
            ((x - self.origin.0) / self.scale).floor() as i32,
            ((y - self.origin.1) / self.scale).floor() as i32,
        )
    }
}

/// An interactive or notable rectangle reported by a screen, for layout tests.
#[derive(Debug, Clone)]
pub struct Hit {
    /// What it is.
    pub name: &'static str,
    /// Where it is.
    pub rect: Rect,
    /// Must be big enough to tap.
    pub tap: bool,
}

impl Hit {
    /// A tappable rectangle.
    pub fn tap(name: &'static str, rect: Rect) -> Hit {
        Hit {
            name,
            rect,
            tap: true,
        }
    }
    /// A layout-only rectangle (panel, area).
    pub fn area(name: &'static str, rect: Rect) -> Hit {
        Hit {
            name,
            rect,
            tap: false,
        }
    }
}

/// Split `r` into `n` equal columns with `gap` pixels between.
pub fn columns(r: Rect, n: i32, gap: i32) -> Vec<Rect> {
    let n = n.max(1);
    let w = (r.w - gap * (n - 1)) / n;
    (0..n)
        .map(|i| Rect::new(r.x + i * (w + gap), r.y, w, r.h))
        .collect()
}

/// Centre a `w`x`h` box in `r`.
pub fn centered(r: Rect, w: i32, h: i32) -> Rect {
    Rect::new(r.x + (r.w - w) / 2, r.y + (r.h - h) / 2, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_hd_is_4x_landscape() {
        let d = choose(1920.0, 1080.0, (0.0, 0.0, 0.0, 0.0), false);
        assert!(!d.vp.portrait);
        assert_eq!(d.scale, 4.0);
        assert_eq!(d.origin, (0.0, 0.0));
        assert_eq!(d.to_virtual(4.0, 8.0), (1, 2));
    }

    #[test]
    fn phone_portrait_uses_the_portrait_layout() {
        let d = choose(1080.0, 2400.0, (0.0, 0.0, 0.0, 0.0), true);
        assert!(d.vp.portrait);
        assert_eq!(d.scale, 4.0);
        let l = choose(2400.0, 1080.0, (0.0, 0.0, 0.0, 0.0), true);
        assert!(!l.vp.portrait);
        assert_eq!(l.scale, 4.0);
    }

    #[test]
    fn near_square_windows_pick_the_better_scale() {
        let d = choose(1000.0, 900.0, (0.0, 0.0, 0.0, 0.0), false);
        assert_eq!(d.scale, 2.0);
        assert!(!d.vp.portrait);
    }

    #[test]
    fn tiny_windows_scale_down_fractionally() {
        let d = choose(240.0, 135.0, (0.0, 0.0, 0.0, 0.0), false);
        assert!(d.scale < 1.0 && d.scale > 0.4);
    }

    #[test]
    fn safe_area_shrinks_the_usable_rectangle() {
        // 20:9 phone landscape with a notch on the left and a home bar at the bottom.
        let d = choose(2400.0, 1080.0, (96.0, 0.0, 96.0, 40.0), true);
        assert!(d.vp.safe.x >= 0 && d.vp.safe.right() <= d.vp.w);
        assert!(d.vp.safe.inside(&d.vp.screen()));
        assert!(d.vp.safe.h <= d.vp.h);
    }

    #[test]
    fn columns_and_centering() {
        let cs = columns(Rect::new(0, 0, 100, 10), 4, 4);
        assert_eq!(cs.len(), 4);
        assert!(cs.iter().all(|c| c.inside(&Rect::new(0, 0, 100, 10))));
        assert_eq!(
            centered(Rect::new(0, 0, 100, 100), 20, 10),
            Rect::new(40, 45, 20, 10)
        );
    }
}
