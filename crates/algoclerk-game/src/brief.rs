// SPDX-License-Identifier: Apache-2.0
//! The level memo: story, statement, typed examples drawn as widgets, unlocked and locked tools,
//! targets and the hint ladder as sticky notes.

use crate::gfx::{Gfx, Kind, State, clip};
use crate::layout::{Hit, Viewport};
use crate::ui::{Ctx, View, widget_state};
use crate::value_view;
use algoclerk_core::lang::value::{NodeData, parse_value};
use algoclerk_core::level::Level;
use algoclerk_core::progress::LevelRecord;
use algoclerk_media::{Font, Rect};

/// Number of memo pages.
pub const PAGES: usize = 3;

/// The memo overlay state.
#[derive(Debug, Clone, Default)]
pub struct Memo {
    /// Current page.
    pub page: usize,
    /// Scroll of the page in pixels.
    pub scroll: i32,
}

/// Rectangles of the memo controls.
#[derive(Debug, Clone, Copy)]
pub struct MemoRects {
    /// Previous page.
    pub prev: Rect,
    /// Next page.
    pub next: Rect,
    /// Close / start.
    pub primary: Rect,
    /// Reveal the next hint.
    pub hint: Rect,
    /// Reveal the reference solution.
    pub reveal: Rect,
    /// Page body.
    pub body: Rect,
}

/// What the memo asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoOut {
    /// Nothing.
    None,
    /// Close the memo.
    Close,
    /// Show another hint.
    Hint,
}

impl Memo {
    /// The memo panel rectangle.
    pub fn panel(vp: &Viewport) -> Rect {
        if vp.portrait {
            Rect::new(6, 18, vp.w - 12, vp.h - 24)
        } else {
            Rect::new(40, 12, vp.w - 80, vp.h - 20)
        }
    }

    /// Control rectangles inside a panel.
    pub fn rects(panel: Rect, bh: i32, _portrait: bool) -> MemoRects {
        let bar_y = panel.bottom() - bh - 4;
        let w3 = (panel.w - 16) / 4;
        MemoRects {
            prev: Rect::new(panel.x + 6, bar_y, w3.min(48), bh),
            next: Rect::new(panel.x + 6 + w3.min(48) + 4, bar_y, w3.min(48), bh),
            primary: Rect::new(panel.right() - 6 - 90, bar_y, 90, bh),
            hint: Rect::new(panel.x + 6, panel.bottom() - 2 * bh - 8, 110, bh),
            reveal: Rect::new(panel.x + 6 + 114, panel.bottom() - 2 * bh - 8, 110, bh),
            body: Rect::new(
                panel.x + 6,
                panel.y + 16,
                panel.w - 12,
                panel.h - 16 - 2 * bh - 14,
            ),
        }
    }

    /// Hit rectangles for layout tests.
    pub fn hits(vp: &Viewport) -> Vec<Hit> {
        let p = Self::panel(vp);
        let m = Self::rects(p, vp.btn_h(), vp.portrait);
        vec![
            Hit::area("memo", p),
            Hit::tap("memo prev", m.prev),
            Hit::tap("memo next", m.next),
            Hit::tap("memo primary", m.primary),
            Hit::tap("memo hint", m.hint),
        ]
    }

    /// Handle input.
    pub fn update(&mut self, cx: &mut Ctx<'_>, level_hints: usize, shown: usize) -> MemoOut {
        let p = Self::panel(&cx.vp);
        let m = Self::rects(p, cx.vp.btn_h(), cx.vp.portrait);
        if cx.ptr.tapped_in(&m.prev) && self.page > 0 {
            self.page -= 1;
            self.scroll = 0;
            cx.sfx("button");
        }
        if cx.ptr.tapped_in(&m.next) && self.page + 1 < PAGES {
            self.page += 1;
            self.scroll = 0;
            cx.sfx("button");
        }
        if cx.input.key(crate::input::Key::Right) && self.page + 1 < PAGES {
            self.page += 1;
            self.scroll = 0;
        }
        if cx.input.key(crate::input::Key::Left) && self.page > 0 {
            self.page -= 1;
            self.scroll = 0;
        }
        if cx.ptr.wheel != 0.0 {
            self.scroll = (self.scroll - (cx.ptr.wheel * 12.0) as i32).max(0);
        }
        if cx.ptr.down && cx.ptr.moved && inr(&m.body, cx.ptr.start) {
            self.scroll = (self.scroll - cx.ptr.delta.1).max(0);
        }
        if cx.ptr.tapped_in(&m.hint) && shown < level_hints {
            return MemoOut::Hint;
        }
        if cx.ptr.tapped_in(&m.primary) || cx.input.key(crate::input::Key::Escape) {
            return MemoOut::Close;
        }
        MemoOut::None
    }

    /// Draw the memo.
    pub fn draw(
        &self,
        g: &mut Gfx<'_>,
        v: &View<'_>,
        level: &Level,
        rec: &LevelRecord,
        shown_hints: usize,
        starting: bool,
    ) {
        let vp = v.vp;
        g.shade(vp.screen());
        let p = Self::panel(&vp);
        let m = Self::rects(p, vp.btn_h(), vp.portrait);
        g.panel(p, 'W');
        let title = clip(&level.title, ((p.w - 20) / 6) as usize);
        g.header(Rect::new(p.x, p.y, p.w, 14), &title, true);
        let saved = g.c.set_clip(m.body.offset(g.off.0, g.off.1));
        let mut y = m.body.y - self.scroll;
        let x = m.body.x;
        let cols = ((m.body.w - 4) / 6) as usize;
        let h = |g: &mut Gfx<'_>, y: &mut i32, text: &str| {
            g.text(x, *y, text, 'B');
            g.hline(x, *y + 9, m.body.w, 'E');
            *y += 13;
        };
        let para = |g: &mut Gfx<'_>, y: &mut i32, text: &str, ink: char| {
            for l in Font::wrap(text, cols) {
                g.text(x, *y, &l, ink);
                *y += 9;
            }
            *y += 3;
        };
        match self.page {
            0 => {
                h(g, &mut y, &v.tr.t("brief.job"));
                para(g, &mut y, &level.story, 'k');
                h(g, &mut y, &v.tr.t("brief.task"));
                para(g, &mut y, &level.statement, 'K');
                g.round_rect(Rect::new(x, y, m.body.w - 4, 24), 'K', 'k');
                let sig = level.signature.text();
                for (i, l) in Font::wrap(&sig, cols.saturating_sub(2))
                    .iter()
                    .take(2)
                    .enumerate()
                {
                    g.text(x + 4, y + 4 + i as i32 * 9, l, 'y');
                }
                y += 28;
                h(g, &mut y, &v.tr.t("brief.constraints"));
                for c in &level.constraints {
                    for (i, l) in Font::wrap(c, cols.saturating_sub(2)).iter().enumerate() {
                        g.text(x, y, if i == 0 { "-" } else { " " }, 'k');
                        g.text(x + 8, y, l, 'K');
                        y += 9;
                    }
                }
            }
            1 => {
                h(g, &mut y, &v.tr.t("brief.examples"));
                let mut nodes = vec![NodeData::default()];
                for (n, ex) in level.examples.iter().take(4).enumerate() {
                    g.text(x, y, &format!("#{}", n + 1), 'k');
                    y += 9;
                    let mut ws = Vec::new();
                    for (a, lit) in level.signature.params.iter().zip(ex.args.iter()) {
                        if let Ok(val) = parse_value(lit, &a.ty, &mut nodes) {
                            ws.push(value_view::build(&val, &a.ty, &nodes, 110));
                        }
                    }
                    let mut cx = x + 2;
                    let mut row_h = 14;
                    for (w, p) in ws.iter().zip(level.signature.params.iter()) {
                        g.tiny(cx, y - 1, &p.name.to_uppercase(), 'a');
                        w.draw(g, cx, y + 5, false);
                        cx += w.w + 6;
                        row_h = row_h.max(w.h + 6);
                    }
                    if let Ok(val) = parse_value(&ex.expect, &level.signature.ret, &mut nodes) {
                        let w = value_view::build(&val, &level.signature.ret, &nodes, 100);
                        g.icon("right", cx, y + 6, 'k');
                        w.draw(g, cx + 12, y + 5, false);
                        row_h = row_h.max(w.h + 6);
                    }
                    y += row_h + 4;
                    if !ex.note.is_empty() {
                        para(g, &mut y, &ex.note, 'k');
                    }
                }
            }
            _ => {
                h(g, &mut y, &v.tr.t("brief.unlocked"));
                let mut cx = x;
                let chip = |g: &mut Gfx<'_>,
                            cx: &mut i32,
                            y: &mut i32,
                            label: &str,
                            fill: char,
                            lock: bool| {
                    let w = label.chars().count() as i32 * 6 + 8 + if lock { 9 } else { 0 };
                    if *cx + w > x + m.body.w {
                        *cx = x;
                        *y += 14;
                    }
                    g.round_rect(Rect::new(*cx, *y, w, 12), fill, 'K');
                    if lock {
                        g.icon("lock", *cx + 3, *y + 1, 'K');
                    }
                    g.text(*cx + 4 + if lock { 9 } else { 0 }, *y + 2, label, 'K');
                    *cx += w + 3;
                };
                for t in level.allowed_types() {
                    chip(g, &mut cx, &mut y, &t, 'c', false);
                }
                for b in &level.builtins {
                    chip(g, &mut cx, &mut y, b, 'g', false);
                }
                y += 16;
                if level.helpers || level.recursion {
                    let yn = |b: bool| {
                        if b {
                            v.tr.t("brief.yes")
                        } else {
                            v.tr.t("brief.no")
                        }
                    };
                    para(
                        g,
                        &mut y,
                        &v.tr.format(
                            "brief.helpers",
                            &[
                                ("helpers", yn(level.helpers)),
                                ("recursion", yn(level.recursion)),
                            ],
                        ),
                        'k',
                    );
                }
                if !level.locked.is_empty() {
                    h(g, &mut y, &v.tr.t("brief.locked"));
                    cx = x;
                    for k in &level.locked {
                        chip(g, &mut cx, &mut y, &k.name, 'p', true);
                    }
                    y += 16;
                    for k in &level.locked {
                        para(g, &mut y, &format!("{}: {}", k.name, k.why), 'k');
                    }
                }
                h(g, &mut y, &v.tr.t("brief.technique"));
                para(
                    g,
                    &mut y,
                    &format!("{}  ({})", level.technique, level.big_o),
                    'K',
                );
                h(g, &mut y, &v.tr.t("brief.targets"));
                for (name, sprite, size, speed) in [
                    (
                        v.tr.t("result.medal_bronze"),
                        "ui/medal_clip",
                        level.medals.size[0],
                        level.medals.speed[0],
                    ),
                    (
                        v.tr.t("result.medal_silver"),
                        "ui/medal_staple",
                        level.medals.size[1],
                        level.medals.speed[1],
                    ),
                    (
                        v.tr.t("result.medal_gold"),
                        "ui/medal_seal",
                        level.medals.size[2],
                        level.medals.speed[2],
                    ),
                ] {
                    g.sprite(sprite, 0, x, y - 5);
                    g.text(x + 20, y, &name, 'K');
                    g.text(
                        x + 98,
                        y,
                        &v.tr.format(
                            "brief.target_line",
                            &[("size", size.to_string()), ("cost", speed.to_string())],
                        ),
                        'k',
                    );
                    y += 18;
                }
                if rec.stars > 0 {
                    para(
                        g,
                        &mut y,
                        &v.tr.format(
                            "brief.best",
                            &[
                                ("size", rec.best_size.unwrap_or(0).to_string()),
                                ("cost", format!("{:.0}", rec.best_speed.unwrap_or(0.0))),
                            ],
                        ),
                        'G',
                    );
                }
                h(g, &mut y, &v.tr.t("brief.hints"));
                if shown_hints == 0 {
                    para(g, &mut y, &v.tr.t("brief.no_hints"), 'k');
                }
                for (i, hint) in level.hints.iter().take(shown_hints).enumerate() {
                    let lines = Font::wrap(hint, cols.saturating_sub(4));
                    let hh = lines.len() as i32 * 9 + 8;
                    g.sprite("ui/sticky", 0, x, y);
                    g.rect(Rect::new(x + 2, y + 2, m.body.w - 10, hh), 'y');
                    g.frame(Rect::new(x + 2, y + 2, m.body.w - 10, hh), 'Y');
                    g.text(x + 5, y + 5, &format!("{}.", i + 1), 'r');
                    for (j, l) in lines.iter().enumerate() {
                        g.text(x + 18, y + 5 + j as i32 * 9, l, 'K');
                    }
                    y += hh + 4;
                }
            }
        }
        g.c.set_clip(saved);
        // page dots
        for i in 0..PAGES {
            let cx = m.next.right() + 10 + i as i32 * 11;
            g.round_rect(
                Rect::new(cx, m.next.y + m.next.h / 2 - 3, 7, 7),
                if i == self.page { 'y' } else { 'A' },
                'K',
            );
        }
        let st = |r: &Rect| widget_state(v.ptr, r, false, false);
        g.icon_button(
            m.prev,
            "left",
            Kind::Normal,
            State {
                disabled: self.page == 0,
                ..st(&m.prev)
            },
        );
        g.icon_button(
            m.next,
            "right",
            Kind::Normal,
            State {
                disabled: self.page + 1 >= PAGES,
                ..st(&m.next)
            },
        );
        g.button(
            m.hint,
            &if shown_hints < level.hints.len() {
                v.tr.t("brief.hint")
            } else {
                v.tr.t("brief.no_more")
            },
            Kind::Normal,
            State {
                disabled: shown_hints >= level.hints.len(),
                ..st(&m.hint)
            },
        );
        g.button(
            m.primary,
            &if starting {
                v.tr.t("brief.start")
            } else {
                v.tr.t("brief.close")
            },
            Kind::Primary,
            st(&m.primary),
        );
    }
}

fn inr(r: &Rect, p: (i32, i32)) -> bool {
    p.0 >= r.x && p.0 < r.right() && p.1 >= r.y && p.1 < r.bottom()
}
