// SPDX-License-Identifier: Apache-2.0
//! Raw input as the platform shell delivers it each tick (mouse, touch, keyboard, gamepad mapped to keys).
//! Coordinates are in *virtual pixels* (the low-resolution screen), already converted by the shell.

/// A non-text key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Enter / A button.
    Enter,
    /// Escape / B button.
    Escape,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Tab.
    Tab,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Space.
    Space,
    /// A letter or digit key (lower case), used for shortcuts outside text fields.
    Char(char),
}

/// Input for one tick. The shell latches events between ticks, so presses are edges.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input {
    /// Pointer position (mouse, or the current/last touch), if any.
    pub pointer: Option<(i32, i32)>,
    /// Primary button / finger is held.
    pub down: bool,
    /// Primary button / finger went down since the last tick.
    pub pressed: bool,
    /// Primary button / finger went up since the last tick.
    pub released: bool,
    /// The pointer is a finger (no hover effects, bigger tolerance).
    pub touch: bool,
    /// Wheel movement in notches (positive = content moves up, i.e. scroll down).
    pub wheel: f32,
    /// Key presses (including OS key repeat for navigation keys).
    pub keys: Vec<Key>,
    /// Text typed this tick.
    pub typed: String,
    /// Ctrl (Cmd on macOS) is held.
    pub ctrl: bool,
    /// Shift is held.
    pub shift: bool,
    /// Clipboard text delivered in answer to a paste request.
    pub paste: Option<String>,
}

impl Input {
    /// Was this key pressed this tick?
    pub fn key(&self, k: Key) -> bool {
        self.keys.contains(&k)
    }
    /// Was this letter/digit shortcut pressed (without ctrl)?
    pub fn ch(&self, c: char) -> bool {
        !self.ctrl && self.keys.contains(&Key::Char(c))
    }
    /// Ctrl + letter.
    pub fn ctrl_ch(&self, c: char) -> bool {
        self.ctrl && self.keys.contains(&Key::Char(c))
    }
    /// Any key at all (used by "press any key" prompts).
    pub fn any_key(&self) -> bool {
        !self.keys.is_empty()
    }
}

/// Tracks pointer gestures across ticks: taps, drags, long presses, scrolling.
#[derive(Debug, Clone, Default)]
pub struct Pointer {
    /// Current position.
    pub pos: Option<(i32, i32)>,
    /// Is the button down?
    pub down: bool,
    /// Pressed this tick.
    pub pressed: bool,
    /// Released this tick.
    pub released: bool,
    /// Where the current press started.
    pub start: (i32, i32),
    /// Ticks the button has been down.
    pub held_ticks: u32,
    /// Moved further than the tap tolerance since pressing.
    pub moved: bool,
    /// Last tick's position (for deltas).
    prev: Option<(i32, i32)>,
    /// Movement since the previous tick.
    pub delta: (i32, i32),
    /// The pointer is a finger.
    pub touch: bool,
    /// Wheel notches this tick.
    pub wheel: f32,
}

/// Pixels the pointer may move and still count as a tap.
pub const TAP_SLOP: i32 = 4;
/// Ticks (60 per second) of holding before a long press begins a drag.
pub const LONG_PRESS_TICKS: u32 = 22;

impl Pointer {
    /// Fold one tick of input in.
    pub fn update(&mut self, i: &Input) {
        self.touch = i.touch;
        self.wheel = i.wheel;
        self.pressed = i.pressed;
        self.released = i.released;
        self.prev = self.pos;
        self.pos = i.pointer.or(if i.down { self.pos } else { i.pointer });
        if i.pressed {
            if let Some(p) = self.pos {
                self.start = p;
            }
            self.held_ticks = 0;
            self.moved = false;
        }
        self.down = i.down || i.pressed;
        if self.down && !i.pressed {
            self.held_ticks += 1;
        }
        self.delta = match (self.prev, self.pos) {
            (Some(a), Some(b)) if self.down => (b.0 - a.0, b.1 - a.1),
            _ => (0, 0),
        };
        if self.down {
            if let Some(p) = self.pos {
                if (p.0 - self.start.0).abs() > TAP_SLOP || (p.1 - self.start.1).abs() > TAP_SLOP {
                    self.moved = true;
                }
            }
        }
        if i.released && !i.down {
            self.down = false;
        }
    }

    /// A tap or click completed this tick (released without dragging) inside `r`, and it also started inside.
    pub fn tapped_in(&self, r: &algoclerk_media::Rect) -> bool {
        self.released
            && !self.moved
            && self.pos.is_some_and(|(x, y)| r.contains(x, y))
            && r.contains(self.start.0, self.start.1)
    }

    /// Pressed down inside `r` this tick.
    pub fn pressed_in(&self, r: &algoclerk_media::Rect) -> bool {
        self.pressed && self.pos.is_some_and(|(x, y)| r.contains(x, y))
    }

    /// Currently held with the press having started inside `r`.
    pub fn held_in(&self, r: &algoclerk_media::Rect) -> bool {
        self.down && r.contains(self.start.0, self.start.1)
    }

    /// Hovering (mouse only) over `r`.
    pub fn hover(&self, r: &algoclerk_media::Rect) -> bool {
        !self.touch && self.pos.is_some_and(|(x, y)| r.contains(x, y))
    }

    /// Is the pointer inside `r` right now?
    pub fn over(&self, r: &algoclerk_media::Rect) -> bool {
        self.pos.is_some_and(|(x, y)| r.contains(x, y))
    }

    /// A long press has matured (held still long enough).
    pub fn long_press(&self) -> bool {
        self.down && !self.moved && self.held_ticks == LONG_PRESS_TICKS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoclerk_media::Rect;

    fn press(x: i32, y: i32) -> Input {
        Input {
            pointer: Some((x, y)),
            down: true,
            pressed: true,
            ..Input::default()
        }
    }

    #[test]
    fn tap_and_drag() {
        let r = Rect::new(0, 0, 20, 20);
        let mut p = Pointer::default();
        p.update(&press(5, 5));
        assert!(p.pressed_in(&r));
        p.update(&Input {
            pointer: Some((6, 5)),
            released: true,
            ..Input::default()
        });
        assert!(p.tapped_in(&r));
        let mut p = Pointer::default();
        p.update(&press(5, 5));
        p.update(&Input {
            pointer: Some((15, 5)),
            down: true,
            ..Input::default()
        });
        assert!(p.moved);
        p.update(&Input {
            pointer: Some((15, 5)),
            released: true,
            ..Input::default()
        });
        assert!(!p.tapped_in(&r));
    }

    #[test]
    fn long_press_fires_once() {
        let mut p = Pointer::default();
        p.update(&press(5, 5));
        let mut fired = 0;
        for _ in 0..40 {
            p.update(&Input {
                pointer: Some((5, 5)),
                down: true,
                ..Input::default()
            });
            if p.long_press() {
                fired += 1;
            }
        }
        assert_eq!(fired, 1);
    }

    #[test]
    fn key_helpers() {
        let i = Input {
            keys: vec![Key::Char('c'), Key::Up],
            ctrl: true,
            ..Input::default()
        };
        assert!(i.ctrl_ch('c') && !i.ch('c') && i.key(Key::Up));
    }
}
