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

/// Keeps typing in order when several key presses land in one tick (a slow frame rate, a fast typist, a
/// scripted keyboard): editing keys (Return, Backspace, arrows) and typed text are split into segments that
/// are handed out one per tick, so `a`, Return, `b` never turns into Return, `ab`.
#[derive(Debug, Clone, Default)]
pub struct KeyQueue {
    segments: std::collections::VecDeque<(Vec<Key>, String)>,
    keys: Vec<Key>,
    typed: String,
}

impl KeyQueue {
    /// A key that changes where or whether text goes (not a letter shortcut or Space).
    fn is_edit(k: Key) -> bool {
        !matches!(k, Key::Char(_) | Key::Space)
    }

    fn seal(&mut self) {
        if !self.keys.is_empty() || !self.typed.is_empty() {
            self.segments.push_back((
                std::mem::take(&mut self.keys),
                std::mem::take(&mut self.typed),
            ));
        }
    }

    /// A key was pressed.
    pub fn push_key(&mut self, k: Key) {
        if Self::is_edit(k) && !self.typed.is_empty() {
            self.seal();
        }
        self.keys.push(k);
    }

    /// Text was typed.
    pub fn push_text(&mut self, t: &str) {
        if self.keys.iter().any(|k| Self::is_edit(*k)) {
            self.seal();
        }
        self.typed.push_str(t);
    }

    /// The keys and text for the next tick (the oldest segment); the rest wait for later ticks.
    pub fn take_tick(&mut self) -> (Vec<Key>, String) {
        self.seal();
        self.segments.pop_front().unwrap_or_default()
    }

    /// Segments still waiting.
    pub fn pending(&self) -> usize {
        self.segments.len() + usize::from(!self.keys.is_empty() || !self.typed.is_empty())
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

#[cfg(test)]
mod key_queue_tests {
    use super::*;

    #[test]
    fn typing_stays_in_order_across_return_and_backspace() {
        let mut q = KeyQueue::default();
        q.push_text("ab");
        q.push_key(Key::Enter);
        q.push_text("c");
        q.push_key(Key::Backspace);
        q.push_key(Key::Backspace);
        q.push_text("d");
        assert_eq!(q.pending(), 5);
        assert_eq!(q.take_tick(), (vec![], "ab".to_string()));
        assert_eq!(q.take_tick(), (vec![Key::Enter], String::new()));
        assert_eq!(q.take_tick(), (vec![], "c".to_string()));
        // two Backspaces in a row share a tick, then the text after them
        assert_eq!(
            q.take_tick(),
            (vec![Key::Backspace, Key::Backspace], String::new())
        );
        assert_eq!(q.take_tick(), (vec![], "d".to_string()));
        assert_eq!(q.take_tick(), (vec![], String::new()));
    }

    #[test]
    fn letter_keys_ride_with_their_text() {
        let mut q = KeyQueue::default();
        for c in ['a', 'b'] {
            q.push_key(Key::Char(c));
            q.push_text(&c.to_string());
        }
        assert_eq!(
            q.take_tick(),
            (vec![Key::Char('a'), Key::Char('b')], "ab".to_string())
        );
    }
}
