// SPDX-License-Identifier: Apache-2.0
//! The About and credits screen.

use crate::gfx::Gfx;
use crate::input::Key;
use crate::layout::Hit;
use crate::title::logo;
use crate::ui::{Ctx, Nav, View};
use algoclerk_media::{Font, Rect};

/// About screen state.
#[derive(Debug, Clone, Default)]
pub struct About {
    /// Scroll offset in pixels.
    pub scroll: i32,
}

const LINES: [&str; 14] = [
    "about.line_game",
    "about.line_licence",
    "about.line_art",
    "about.line_music",
    "about.line_font",
    "about.line_levels",
    "about.line_engine",
    "about.line_falabichos",
    "about.line_privacy",
    "about.line_source",
    "about.line_howto",
    "about.line_thanks",
    "about.line_keys",
    "about.line_touch",
];

impl About {
    fn body(vp: &crate::layout::Viewport) -> Rect {
        let s = vp.safe;
        Rect::new(s.x + 8, s.y + 74, s.w - 16, s.h - 74 - 32 - 6)
    }

    fn back_rect(vp: &crate::layout::Viewport) -> Rect {
        let s = vp.safe;
        let bh = vp.btn_h();
        Rect::new(s.x + (s.w - 110) / 2, s.bottom() - bh - 6, 110, bh)
    }

    /// Hit rectangles for layout tests.
    pub fn hits(vp: &crate::layout::Viewport) -> Vec<Hit> {
        vec![
            Hit::area("body", About::body(vp)),
            Hit::tap("back", About::back_rect(vp)),
        ]
    }

    /// Handle input.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let body = About::body(&cx.vp);
        if cx.ptr.over(&body) {
            self.scroll -= (cx.ptr.wheel * 24.0) as i32;
        }
        if cx.ptr.down && cx.ptr.moved && cx.ptr.held_in(&body) {
            self.scroll -= cx.ptr.delta.1;
        }
        for k in &cx.input.keys {
            match k {
                Key::Down => self.scroll += 12,
                Key::Up => self.scroll -= 12,
                Key::PageDown => self.scroll += body.h - 12,
                Key::PageUp => self.scroll -= body.h - 12,
                _ => {}
            }
        }
        self.scroll = self.scroll.max(0);
        if cx.ptr.tapped_in(&About::back_rect(&cx.vp))
            || cx.input.key(Key::Escape)
            || cx.input.key(Key::Enter)
        {
            cx.sfx("button");
            return Some(Nav::Title);
        }
        None
    }

    fn lines(v: &View<'_>, cols: usize) -> Vec<(String, char)> {
        let mut out = Vec::new();
        for k in LINES {
            for l in Font::wrap(&v.tr.t(k), cols) {
                out.push((l, 'K'));
            }
            out.push((String::new(), 'K'));
        }
        out
    }

    /// Draw the screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let s = vp.safe;
        g.rect(vp.screen(), 'T');
        for ty in 0..=(vp.h / 16) {
            for tx in 0..=(vp.w / 16) {
                g.sprite(
                    if (tx + ty) % 2 == 0 {
                        "tiles/carpet_a"
                    } else {
                        "tiles/carpet_b"
                    },
                    0,
                    tx * 16,
                    ty * 16,
                );
            }
        }
        logo(g, s.x + s.w / 2, s.y + 10, if vp.portrait { 3 } else { 4 });
        g.text_c(
            s.x + s.w / 2 + 1,
            s.y + 52,
            &format!("v{}", env!("CARGO_PKG_VERSION")),
            'K',
            1,
        );
        g.text_c(
            s.x + s.w / 2,
            s.y + 51,
            &format!("v{}", env!("CARGO_PKG_VERSION")),
            'W',
            1,
        );
        let body = About::body(&vp);
        g.panel(body.grow(4), 'W');
        let cols = ((body.w - 12) / 6) as usize;
        let lines = About::lines(v, cols);
        let content = lines.len() as i32 * 10;
        let max = (content - body.h).max(0);
        let scroll = self.scroll.min(max);
        let saved = g.c.set_clip(body.offset(g.off.0, g.off.1));
        for (i, (l, c)) in lines.iter().enumerate() {
            g.text(body.x + 2, body.y + i as i32 * 10 - scroll, l, *c);
        }
        g.c.set_clip(saved);
        g.scrollbar(
            Rect::new(body.right() - 2, body.y, 3, body.h),
            scroll,
            content,
            body.h,
        );
        let back = About::back_rect(&vp);
        g.button(
            back,
            &v.tr.t("common.back"),
            crate::gfx::Kind::Primary,
            crate::ui::widget_state(v.ptr, &back, true, false),
        );
    }
}
