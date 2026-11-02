// SPDX-License-Identifier: Apache-2.0
//! A small yes/no dialog drawn over the game (spoiler warnings and similar).

use crate::gfx::{Gfx, Kind};
use crate::input::Key;
use crate::layout::{Hit, Viewport, centered};
use crate::ui::{Ctx, View, widget_state};
use algoclerk_media::{Font, Rect};

/// What a dialog is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Reveal the reference solution.
    Reference,
    /// Peek at the equivalent code before solving.
    PeekCode,
    /// Delete all progress.
    Reset,
    /// A one-time notice (either button dismisses it).
    Notice,
}

/// A yes/no dialog.
#[derive(Debug, Clone)]
pub struct Modal {
    /// What it is for.
    pub purpose: Purpose,
    /// Title.
    pub title: String,
    /// Body text.
    pub body: String,
    /// Label of the yes button.
    pub yes: String,
    /// Label of the no button.
    pub no: String,
    /// Keyboard focus: true = yes.
    pub focus_yes: bool,
}

/// The user's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Still open.
    Open,
    /// Yes.
    Yes,
    /// No or dismissed.
    No,
}

impl Modal {
    /// Rectangles: `(panel, yes, no)`.
    pub fn rects(vp: &Viewport) -> (Rect, Rect, Rect) {
        let w = if vp.portrait { vp.safe.w - 20 } else { 300 };
        let h = if vp.portrait {
            150 + vp.btn_h()
        } else {
            110 + vp.btn_h()
        };
        let panel = centered(vp.safe, w.min(vp.safe.w), h.min(vp.safe.h));
        let bh = vp.btn_h();
        let bw = (panel.w - 24) / 2;
        let yes = Rect::new(panel.x + 8, panel.bottom() - bh - 8, bw, bh);
        let no = Rect::new(panel.right() - 8 - bw, panel.bottom() - bh - 8, bw, bh);
        (panel, yes, no)
    }

    /// Hit rectangles for layout tests.
    pub fn hits(vp: &Viewport) -> Vec<Hit> {
        let (p, y, n) = Modal::rects(vp);
        vec![Hit::area("modal", p), Hit::tap("yes", y), Hit::tap("no", n)]
    }

    /// Handle input.
    pub fn update(&mut self, cx: &Ctx<'_>) -> Answer {
        let (_, yes, no) = Modal::rects(&cx.vp);
        if cx.ptr.tapped_in(&yes) {
            return Answer::Yes;
        }
        if cx.ptr.tapped_in(&no) || cx.input.key(Key::Escape) {
            return Answer::No;
        }
        if cx.input.key(Key::Left) || cx.input.key(Key::Right) || cx.input.key(Key::Tab) {
            self.focus_yes = !self.focus_yes;
        }
        if cx.input.key(Key::Enter) || cx.input.key(Key::Space) {
            return if self.focus_yes {
                Answer::Yes
            } else {
                Answer::No
            };
        }
        Answer::Open
    }

    /// Draw the dialog over a dimmed screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        g.shade(v.vp.screen());
        let (panel, yes, no) = Modal::rects(&v.vp);
        g.panel(panel, 'W');
        let head = Rect::new(panel.x + 4, panel.y + 4, panel.w - 8, 16);
        g.header(head, &self.title, true);
        let cols = ((panel.w - 24) / 6) as usize;
        let n = Font::wrap(&self.body, cols).len();
        g.paragraph(
            panel.x + 12,
            head.bottom() + 8,
            cols,
            &self.body,
            'K',
            1,
            11,
            n.min(8),
        );
        g.button(
            yes,
            &self.yes,
            Kind::Good,
            widget_state(v.ptr, &yes, self.focus_yes, false),
        );
        g.button(
            no,
            &self.no,
            Kind::Normal,
            widget_state(v.ptr, &no, !self.focus_yes, false),
        );
    }
}
