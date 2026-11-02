// SPDX-License-Identifier: Apache-2.0
//! Shared plumbing for screens: the per-tick context, navigation targets, events to the shell and menu helpers.

use crate::gfx::{Gfx, State};
use crate::input::{Input, Key, Pointer};
use crate::layout::Viewport;
use algoclerk_core::i18n::Strings;
use algoclerk_core::progress::Progress;
use algoclerk_levels::Campaign;
use algoclerk_media::{ArtBank, Rect};

/// Things the shell must react to.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    /// Play a sound effect (`assets/music/<name>.song`).
    Sfx(&'static str),
    /// Switch music (`None` = silence).
    Music(Option<&'static str>),
    /// Put text on the system clipboard.
    Copy(String),
    /// Ask the shell for the clipboard text; it arrives in `Input::paste` on a later tick.
    PasteRequest,
    /// Volumes or mute changed.
    SettingsChanged,
    /// Fullscreen on/off.
    Fullscreen(bool),
    /// Progress was saved.
    Saved,
    /// Quit the application.
    Quit,
}

/// Where a screen wants to go next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nav {
    /// Title screen.
    Title,
    /// Level select.
    Select,
    /// The brief (memo) of a level, then play.
    Brief(String),
    /// Straight into play.
    Play(String),
    /// The sandbox.
    Sandbox,
    /// Settings.
    Settings,
    /// About and credits.
    About,
    /// The code reveal of the level just solved.
    Code,
    /// The victory screen of the level just solved.
    Victory,
    /// Back to the program (from victory or the code reveal).
    BackToPlay,
    /// Leave the application.
    Quit,
}

/// Static configuration chosen by the shell.
#[derive(Debug, Clone)]
pub struct GameConfig {
    /// Show a Quit entry (desktop).
    pub can_quit: bool,
    /// Touch-first sizing and hints.
    pub touch: bool,
    /// The shell provides the system clipboard (otherwise the game keeps its own).
    pub system_clipboard: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            can_quit: true,
            touch: false,
            system_clipboard: false,
        }
    }
}

/// Everything a screen may read or change during one tick.
pub struct Ctx<'a> {
    /// Raw input of this tick.
    pub input: &'a Input,
    /// Pointer gestures.
    pub ptr: &'a Pointer,
    /// Screen geometry.
    pub vp: Viewport,
    /// Text.
    pub tr: &'a Strings,
    /// Saved progress and settings.
    pub progress: &'a mut Progress,
    /// All levels.
    pub campaign: &'a Campaign,
    /// Art.
    pub bank: &'a ArtBank,
    /// Outgoing events.
    pub events: &'a mut Vec<GameEvent>,
    /// Frame counter.
    pub tick: u64,
    /// Shell configuration.
    pub config: &'a GameConfig,
    /// Set when progress changed and should be saved soon.
    pub dirty: &'a mut bool,
}

impl Ctx<'_> {
    /// Emit a sound effect.
    pub fn sfx(&mut self, name: &'static str) {
        self.events.push(GameEvent::Sfx(name));
    }

    /// Text scale for reading text (1 or 2).
    pub fn text_scale(&self) -> i32 {
        i32::from(self.progress.settings.text_size.clamp(1, 2))
    }

    /// Mark progress as changed.
    pub fn touch_progress(&mut self) {
        *self.dirty = true;
    }
}

/// Read-only state for drawing.
pub struct View<'a> {
    /// Screen geometry.
    pub vp: Viewport,
    /// Text.
    pub tr: &'a Strings,
    /// Progress and settings.
    pub progress: &'a Progress,
    /// All levels.
    pub campaign: &'a Campaign,
    /// Pointer (for hover effects).
    pub ptr: &'a Pointer,
    /// Frame counter.
    pub tick: u64,
    /// Shell configuration.
    pub config: &'a GameConfig,
}

impl View<'_> {
    /// Reading text scale.
    pub fn text_scale(&self) -> i32 {
        i32::from(self.progress.settings.text_size.clamp(1, 2))
    }
}

/// Widget state from the pointer: hover and pressed highlights.
pub fn widget_state(ptr: &Pointer, r: &Rect, focus: bool, disabled: bool) -> State {
    State {
        hover: ptr.hover(r) && !disabled,
        down: ptr.held_in(r) && ptr.over(r) && !disabled,
        disabled,
        focus,
    }
}

/// Up/Down (and Tab) move a focus index with wrap-around. Returns true if it moved.
pub fn menu_nav(input: &Input, focus: &mut usize, n: usize) -> bool {
    if n == 0 {
        return false;
    }
    let before = *focus;
    if input.key(Key::Down) || (input.key(Key::Tab) && !input.shift) {
        *focus = (*focus + 1) % n;
    }
    if input.key(Key::Up) || (input.key(Key::Tab) && input.shift) {
        *focus = (*focus + n - 1) % n;
    }
    *focus != before
}

/// Was a confirm key pressed (Enter or Space)?
pub fn confirm(input: &Input) -> bool {
    input.key(Key::Enter) || input.key(Key::Space)
}

/// Was a back key pressed (Escape or Backspace)?
pub fn back(input: &Input) -> bool {
    input.key(Key::Escape)
}

/// A vertical list menu: rows, a focus index, hover tracking. Used by the title and settings screens.
#[derive(Debug, Clone, Default)]
pub struct MenuFocus {
    /// The focused row.
    pub focus: usize,
    /// Last pointer position seen (to move focus only when the mouse actually moves).
    last_ptr: Option<(i32, i32)>,
}

impl MenuFocus {
    /// Update focus from keys and mouse hover over `rows`. Returns the row that was activated, if any.
    pub fn update(&mut self, cx: &Ctx<'_>, rows: &[Rect]) -> Option<usize> {
        let moved = menu_nav(cx.input, &mut self.focus, rows.len());
        if moved {
            self.last_ptr = cx.ptr.pos;
        }
        if cx.ptr.pos != self.last_ptr && !cx.ptr.touch {
            if let Some(i) = rows.iter().position(|r| cx.ptr.over(r)) {
                self.focus = i;
            }
            self.last_ptr = cx.ptr.pos;
        }
        self.focus = self.focus.min(rows.len().saturating_sub(1));
        for (i, r) in rows.iter().enumerate() {
            if cx.ptr.tapped_in(r) {
                self.focus = i;
                return Some(i);
            }
        }
        confirm(cx.input).then_some(self.focus)
    }
}

/// Draw one menu row with a selection arrow when focused.
pub fn draw_menu_row(g: &mut Gfx<'_>, r: Rect, label: &str, focused: bool, enabled: bool) {
    if focused {
        g.rect(r, 'y');
        g.cursor(r.x + 4, r.y + (r.h - 9) / 2);
    }
    let ink = if enabled { 'K' } else { 'a' };
    g.text(r.x + 18, r.y + (r.h - 7) / 2, label, ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_nav_wraps() {
        let mut f = 0;
        let down = Input {
            keys: vec![Key::Down],
            ..Input::default()
        };
        let up = Input {
            keys: vec![Key::Up],
            ..Input::default()
        };
        assert!(menu_nav(&up, &mut f, 3));
        assert_eq!(f, 2);
        assert!(menu_nav(&down, &mut f, 3));
        assert_eq!(f, 0);
        assert!(!menu_nav(&Input::default(), &mut f, 3));
        assert!(!menu_nav(&down, &mut f, 0));
    }

    #[test]
    fn confirm_and_back() {
        let i = Input {
            keys: vec![Key::Enter],
            ..Input::default()
        };
        assert!(confirm(&i) && !back(&i));
    }
}
