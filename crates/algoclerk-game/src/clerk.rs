// SPDX-License-Identifier: Apache-2.0
//! The clerk character: frame indices in the recolourable `art/clerk/_body_*.px` templates, facing,
//! poses and the walk/idle animation state.

use algoclerk_media::{ArtBank, Outfit, Sprite};

/// Frame indices in the clerk templates (see the comment at the top of `_body_a.px`).
pub mod f {
    /// Standing, facing the player.
    pub const IDLE: usize = 0;
    /// Eyes closed.
    pub const BLINK: usize = 1;
    /// Walking down, two frames.
    pub const WALK_D: [usize; 2] = [2, 3];
    /// Back view standing.
    pub const UP: usize = 4;
    /// Walking up.
    pub const WALK_U: [usize; 2] = [5, 6];
    /// Side view standing (left).
    pub const LEFT: usize = 7;
    /// Walking left.
    pub const WALK_L: [usize; 2] = [8, 9];
    /// Carrying a crate overhead, facing down.
    pub const CARRY_D: [usize; 2] = [10, 11];
    /// Carrying, back view.
    pub const CARRY_U: [usize; 2] = [12, 13];
    /// Carrying, side view.
    pub const CARRY_L: [usize; 2] = [14, 15];
    /// Reaching down to grab or put down.
    pub const GRAB: usize = 16;
    /// Thinking, two frames.
    pub const THINK: [usize; 2] = [17, 18];
    /// Cheering, two frames.
    pub const CHEER: [usize; 2] = [19, 20];
    /// Drooping, two frames.
    pub const SAD: [usize; 2] = [21, 22];
    /// Arms forward (working on a tile).
    pub const WORK: usize = 23;
    /// Shrugging.
    pub const SHRUG: usize = 24;
    /// Number of frames in a template.
    pub const COUNT: usize = 25;
}

/// Which way the clerk faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// Towards the viewer.
    Down,
    /// Away from the viewer.
    Up,
    /// Left.
    Left,
    /// Right (the left frames, mirrored).
    Right,
}

/// What the clerk is doing besides walking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pose {
    /// Standing around.
    Idle,
    /// Reaching for or putting down a crate.
    Grab,
    /// Operating on a tile (add, subtract ...).
    Work,
    /// Scratching his head.
    Think,
    /// Shrugging.
    Shrug,
    /// Hooray.
    Cheer,
    /// Something went wrong.
    Sad,
}

/// The clerk's state in scene coordinates (`x` is the horizontal centre, `y` the feet).
#[derive(Debug, Clone)]
pub struct ClerkState {
    /// Horizontal centre.
    pub x: f32,
    /// Feet.
    pub y: f32,
    /// Facing.
    pub dir: Dir,
    /// Current pose.
    pub pose: Pose,
    /// Holding a crate overhead.
    pub carrying: bool,
    /// Moving this tick.
    pub walking: bool,
    /// Animation clock in ticks.
    pub clock: u32,
}

impl ClerkState {
    /// A clerk standing at `(x, y)`.
    pub fn new(x: f32, y: f32) -> ClerkState {
        ClerkState {
            x,
            y,
            dir: Dir::Down,
            pose: Pose::Idle,
            carrying: false,
            walking: false,
            clock: 0,
        }
    }

    /// The sprite frame and mirroring for the current state. `blink_ok` allows idle blinking.
    pub fn frame(&self, blink_ok: bool) -> (usize, bool) {
        let step = ((self.clock / 7) % 2) as usize;
        let flip = self.dir == Dir::Right;
        if self.walking {
            let (a, b) = match (self.dir, self.carrying) {
                (Dir::Down, false) => (f::WALK_D, 0),
                (Dir::Up, false) => (f::WALK_U, 0),
                (Dir::Left | Dir::Right, false) => (f::WALK_L, 0),
                (Dir::Down, true) => (f::CARRY_D, 0),
                (Dir::Up, true) => (f::CARRY_U, 0),
                (Dir::Left | Dir::Right, true) => (f::CARRY_L, 0),
            };
            let _ = b;
            return (a[step], flip);
        }
        match self.pose {
            Pose::Grab => (f::GRAB, false),
            Pose::Work => (f::WORK, false),
            Pose::Think => (f::THINK[((self.clock / 18) % 2) as usize], false),
            Pose::Shrug => (f::SHRUG, false),
            Pose::Cheer => (f::CHEER[((self.clock / 10) % 2) as usize], false),
            Pose::Sad => (f::SAD[((self.clock / 16) % 2) as usize], false),
            Pose::Idle => {
                if self.carrying {
                    let a = match self.dir {
                        Dir::Down => f::CARRY_D,
                        Dir::Up => f::CARRY_U,
                        Dir::Left | Dir::Right => f::CARRY_L,
                    };
                    return (a[1], flip);
                }
                let blink = blink_ok && self.clock % 200 < 7;
                match self.dir {
                    Dir::Down => (if blink { f::BLINK } else { f::IDLE }, false),
                    Dir::Up => (f::UP, false),
                    Dir::Left | Dir::Right => (f::LEFT, flip),
                }
            }
        }
    }

    /// Vertical hop offset (cheering jumps on the second frame).
    pub fn hop(&self) -> i32 {
        if self.pose == Pose::Cheer && !self.walking && (self.clock / 10) % 2 == 1 {
            -2
        } else {
            0
        }
    }
}

/// Renders and caches the clerk sprite for the current outfit.
#[derive(Debug, Clone)]
pub struct ClerkArt {
    outfit: Outfit,
    sprite: Sprite,
}

impl ClerkArt {
    /// Render the clerk for `outfit` (falls back to the default outfit when the template is missing).
    pub fn new(bank: &ArtBank, outfit: &Outfit) -> ClerkArt {
        let sprite = bank
            .clerk(outfit)
            .or_else(|_| bank.clerk(&Outfit::default()))
            .unwrap_or_else(|_| Sprite::flat(16, 20, b'.'));
        ClerkArt {
            outfit: outfit.clone(),
            sprite,
        }
    }

    /// The outfit this art was made for.
    pub fn outfit(&self) -> &Outfit {
        &self.outfit
    }

    /// The sprite.
    pub fn sprite(&self) -> &Sprite {
        &self.sprite
    }

    /// Re-render when the outfit changed. Returns true if it did.
    pub fn set_outfit(&mut self, bank: &ArtBank, outfit: &Outfit) -> bool {
        if self.outfit == *outfit {
            return false;
        }
        *self = ClerkArt::new(bank, outfit);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_alternates_frames_and_mirrors_right() {
        let mut c = ClerkState::new(0.0, 0.0);
        c.walking = true;
        c.dir = Dir::Right;
        let (a, flip) = c.frame(true);
        assert!(flip && f::WALK_L.contains(&a));
        c.clock = 7;
        let (b, _) = c.frame(true);
        assert_ne!(a, b);
    }

    #[test]
    fn carrying_uses_arms_up_frames() {
        let mut c = ClerkState::new(0.0, 0.0);
        c.carrying = true;
        assert!(f::CARRY_D.contains(&c.frame(true).0));
        c.dir = Dir::Up;
        assert!(f::CARRY_U.contains(&c.frame(true).0));
    }

    #[test]
    fn frame_indices_fit_the_template() {
        assert_eq!(f::COUNT, 25);
        assert!(f::SHRUG < f::COUNT);
    }
}
