// SPDX-License-Identifier: Apache-2.0
//! Screen transitions: a classic iris wipe (a circle closes on a point, the new screen opens the same way),
//! quantised to a few steps so it stays pixel-crisp. With "reduce motion" the screens simply cut.

use algoclerk_media::Canvas;

/// Ticks for the iris to close (and again to open).
pub const IRIS_TICKS: u32 = 16;

/// Which half of the wipe we are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The old screen is shrinking away.
    Closing,
    /// The new screen is opening.
    Opening,
}

/// An iris wipe in progress.
#[derive(Debug, Clone)]
pub struct Iris {
    /// Phase.
    pub phase: Phase,
    /// Ticks into the phase.
    pub t: u32,
    /// Focus point.
    pub center: (i32, i32),
}

impl Iris {
    /// Start closing around `center`.
    pub fn new(center: (i32, i32)) -> Iris {
        Iris {
            phase: Phase::Closing,
            t: 0,
            center,
        }
    }

    /// Advance one tick. Returns true at the moment the screen must be swapped (iris fully closed).
    pub fn tick(&mut self) -> bool {
        self.t += 1;
        match self.phase {
            Phase::Closing if self.t >= IRIS_TICKS => {
                self.phase = Phase::Opening;
                self.t = 0;
                true
            }
            _ => false,
        }
    }

    /// True once the opening half is complete.
    pub fn done(&self) -> bool {
        self.phase == Phase::Opening && self.t >= IRIS_TICKS
    }

    /// Radius of the visible circle for a canvas of `w` x `h`, quantised to 8 pixel steps.
    pub fn radius(&self, w: i32, h: i32) -> i32 {
        let max = ((w * w + h * h) as f32).sqrt() as i32 + 8;
        let t = self.t.min(IRIS_TICKS) as f32 / IRIS_TICKS as f32;
        let k = match self.phase {
            Phase::Closing => 1.0 - t * t,
            Phase::Opening => t * (2.0 - t),
        };
        (((k * max as f32) as i32) / 8) * 8
    }

    /// Paint the black surround.
    pub fn draw(&self, c: &mut Canvas, ink: [u8; 4]) {
        let r = self.radius(c.w as i32, c.h as i32);
        let (cx, cy) = self.center;
        let r2 = i64::from(r) * i64::from(r);
        for y in 0..c.h as i32 {
            let dy = i64::from(y - cy);
            let row_dy2 = dy * dy;
            if row_dy2 >= r2 {
                c.rect_fill(0, y, c.w as i32, 1, ink);
                continue;
            }
            let half = ((r2 - row_dy2) as f64).sqrt() as i32;
            c.rect_fill(0, y, cx - half, 1, ink);
            c.rect_fill(cx + half, y, c.w as i32 - (cx + half), 1, ink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iris_closes_then_opens() {
        let mut i = Iris::new((50, 50));
        assert!(i.radius(100, 100) > 100);
        let mut swapped = 0;
        for _ in 0..IRIS_TICKS {
            if i.tick() {
                swapped += 1;
            }
        }
        assert_eq!(swapped, 1);
        assert_eq!(i.phase, Phase::Opening);
        assert_eq!(
            i.radius(100, 100),
            (((100 * 100 + 100 * 100) as f32).sqrt() as i32 + 8) * 0
        );
        for _ in 0..IRIS_TICKS {
            i.tick();
        }
        assert!(i.done());
    }

    #[test]
    fn iris_paints_the_corners_first() {
        let mut c = Canvas::new(64, 64);
        c.clear([255, 255, 255, 255]);
        let mut i = Iris::new((32, 32));
        for _ in 0..IRIS_TICKS - 2 {
            i.tick();
        }
        i.draw(&mut c, [0, 0, 0, 255]);
        assert_eq!(c.get(0, 0), [0, 0, 0, 255]);
        assert_eq!(c.get(32, 32), [255, 255, 255, 255]);
    }
}
