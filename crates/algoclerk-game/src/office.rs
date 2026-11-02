// SPDX-License-Identifier: Apache-2.0
//! The office scene: a tile-built room where the clerk tends the values of the running program.
//! Every local variable sits in a labelled tray, drawn as the matching pixel widget.

use crate::clerk::{ClerkArt, ClerkState, Dir, Pose};
use crate::gfx::Gfx;
use crate::session::Frame;
use crate::value_view::{self, W};
use algoclerk_core::cpp::ast::Ty;
use algoclerk_core::cpp::value::{Heap, parse_value, render};
use algoclerk_media::{ArtBank, Outfit, Rect};

/// Scene width.
pub const SCENE_W: i32 = 270;
/// Scene height.
pub const SCENE_H: i32 = 176;
const FLOOR_TOP: i32 = 52;

#[derive(Debug, Clone)]
struct Tray {
    name: String,
    rect: Rect,
    w: W,
    changed: bool,
    param: bool,
}

#[derive(Debug, Clone)]
struct Spark {
    at: (f32, f32),
    ttl: u32,
}

/// The scene state.
#[derive(Debug, Clone)]
pub struct Office {
    /// The clerk.
    pub clerk: ClerkState,
    art: ClerkArt,
    trays: Vec<Tray>,
    hidden: usize,
    prev: Vec<(String, String)>,
    target: (f32, f32),
    out_value: Option<(W, bool)>,
    expected: Option<(W, String)>,
    out_ok: Option<bool>,
    /// Speech bubble text.
    pub bubble: Option<String>,
    sparks: Vec<Spark>,
    cost: u64,
    clock: u64,
    /// Skip decoration.
    pub reduce_motion: bool,
    /// Speed multiplier for walking.
    pub speed: f32,
    /// Arguments of the run (labelled as inbox items).
    nparams: usize,
}

impl Office {
    /// A fresh office.
    pub fn new(bank: &ArtBank, outfit: &Outfit) -> Office {
        Office {
            clerk: ClerkState::new(60.0, 150.0),
            art: ClerkArt::new(bank, outfit),
            trays: Vec::new(),
            hidden: 0,
            prev: Vec::new(),
            target: (60.0, 150.0),
            out_value: None,
            expected: None,
            out_ok: None,
            bubble: None,
            sparks: Vec::new(),
            cost: 0,
            clock: 0,
            reduce_motion: false,
            speed: 1.0,
            nparams: 0,
        }
    }

    /// Change the clerk's outfit.
    pub fn set_outfit(&mut self, bank: &ArtBank, outfit: &Outfit) {
        self.art.set_outfit(bank, outfit);
    }

    /// Forget everything shown.
    pub fn clear(&mut self) {
        self.trays.clear();
        self.prev.clear();
        self.out_value = None;
        self.expected = None;
        self.out_ok = None;
        self.bubble = None;
        self.cost = 0;
        self.hidden = 0;
        self.clerk.carrying = false;
        self.clerk.pose = Pose::Idle;
        self.target = (60.0, 150.0);
    }

    /// Show a recorded frame: lay out the trays and send the clerk to whatever changed.
    pub fn show(&mut self, f: &Frame, nparams: usize, expected: Option<(&str, &Ty, &str)>) {
        self.cost = f.cost;
        self.nparams = nparams;
        let width_budget = SCENE_W - 16;
        let mut items: Vec<(String, W, bool, bool)> = Vec::new();
        let texts: Vec<String> = f
            .vars
            .iter()
            .map(|(_, ty, v)| render(v, ty, &f.heap))
            .collect();
        for (i, (name, ty, v)) in f.vars.iter().enumerate() {
            let changed = self
                .prev
                .iter()
                .find(|(n, _)| n == name)
                .is_none_or(|(_, old)| *old != texts[i]);
            let budget = if f.vars.len() <= 2 {
                width_budget - 20
            } else {
                128
            };
            items.push((
                name.clone(),
                value_view::build(v, ty, &f.heap, budget),
                changed,
                i < nparams,
            ));
        }
        // pack trays row by row
        let mut trays = Vec::new();
        let (mut x, mut y, mut row_h) = (6, FLOOR_TOP + 4, 0);
        let mut hidden = 0;
        for (name, w, changed, param) in items {
            let tw = (w.w + 8)
                .max(name.chars().count() as i32 * 4 + 12)
                .min(width_budget);
            let th = w.h + 11;
            if x + tw > SCENE_W - 6 {
                x = 6;
                y += row_h + 3;
                row_h = 0;
            }
            let bottom = if x + tw > SCENE_W - 126 {
                SCENE_H - 42
            } else {
                SCENE_H - 6
            };
            if y + th > bottom {
                hidden += 1;
                continue;
            }
            trays.push(Tray {
                name,
                rect: Rect::new(x, y, tw, th),
                w,
                changed,
                param,
            });
            x += tw + 4;
            row_h = row_h.max(th);
        }
        self.hidden = hidden;
        self.prev = f
            .vars
            .iter()
            .zip(texts)
            .map(|((n, _, _), t)| (n.clone(), t))
            .collect();
        // the clerk walks to the first tray that changed
        let go = trays
            .iter()
            .find(|t| t.changed && !t.param)
            .or_else(|| trays.iter().find(|t| t.changed));
        if let Some(t) = go {
            self.target = (
                (t.rect.x - 8).max(10) as f32,
                (t.rect.bottom() + 10).min(SCENE_H - 4) as f32,
            );
            if t.rect.x < 14 {
                self.target.0 = (t.rect.x + t.rect.w / 2) as f32;
            }
            self.clerk.pose = Pose::Idle;
        }
        self.trays = trays;
        // returned value goes to the outbox
        match (&f.returned, expected) {
            (Some(v), exp) => {
                if let Some((_, ty, _)) = exp {
                    self.out_value = Some((value_view::build(v, ty, &f.heap, 110), false));
                }
                self.clerk.carrying = false;
                self.target = ((SCENE_W - 136) as f32, (SCENE_H - 12) as f32);
            }
            (None, _) => {}
        }
        if let Some((text, ty, got)) = expected {
            let mut h2 = Heap::default();
            if let Ok(ev) = parse_value(text, ty, &mut h2) {
                self.expected = Some((value_view::build(&ev, ty, &h2, 110), text.to_string()));
            }
            self.out_ok = if f.returned.is_some() {
                Some(got == text)
            } else {
                None
            };
        }
        if self.trays.iter().any(|t| t.changed) && !self.reduce_motion {
            if let Some(t) = self.trays.iter().find(|t| t.changed) {
                self.sparks.push(Spark {
                    at: ((t.rect.x + t.rect.w / 2) as f32, t.rect.y as f32),
                    ttl: 30,
                });
            }
        }
    }

    /// Show the problem as a speech bubble and a sad clerk.
    pub fn oops(&mut self, text: &str) {
        self.bubble = Some(text.to_string());
        self.clerk.pose = Pose::Sad;
        self.clerk.walking = false;
    }

    /// Celebrate.
    pub fn cheer(&mut self) {
        self.clerk.pose = Pose::Cheer;
        self.bubble = None;
    }

    /// Is the clerk still walking to a tray?
    pub fn busy(&self) -> bool {
        self.clerk.walking
    }

    /// Advance animation by one tick.
    pub fn tick(&mut self) {
        self.clock += 1;
        self.clerk.clock = self.clerk.clock.wrapping_add(1);
        let (dx, dy) = (self.target.0 - self.clerk.x, self.target.1 - self.clerk.y);
        let dist = (dx * dx + dy * dy).sqrt();
        let step = 1.6 * self.speed.max(0.5);
        if dist > step && self.clerk.pose != Pose::Sad {
            self.clerk.walking = true;
            self.clerk.x += dx / dist * step;
            self.clerk.y += dy / dist * step;
            self.clerk.dir = if dx.abs() > dy.abs() {
                if dx > 0.0 { Dir::Right } else { Dir::Left }
            } else if dy > 0.0 {
                Dir::Down
            } else {
                Dir::Up
            };
        } else {
            self.clerk.walking = false;
            self.clerk.x = self.target.0;
            self.clerk.y = self.target.1;
        }
        for s in &mut self.sparks {
            s.ttl = s.ttl.saturating_sub(1);
        }
        self.sparks.retain(|s| s.ttl > 0);
    }

    /// Snap to the target (instant playback).
    pub fn settle(&mut self) {
        self.clerk.x = self.target.0;
        self.clerk.y = self.target.1;
        self.clerk.walking = false;
    }

    fn anim(&self, period: u64, frames: usize) -> usize {
        if self.reduce_motion {
            0
        } else {
            ((self.clock / period) % frames as u64) as usize
        }
    }

    /// Draw the scene with its top-left corner at `origin`.
    pub fn draw(&self, g: &mut Gfx<'_>, origin: (i32, i32), tr: &algoclerk_core::i18n::Strings) {
        let saved_off = g.off;
        let saved_clip = g.c.set_clip(Rect::new(
            origin.0 + saved_off.0,
            origin.1 + saved_off.1,
            SCENE_W,
            SCENE_H,
        ));
        g.off = (origin.0 + saved_off.0, origin.1 + saved_off.1);
        self.draw_room(g, tr);
        self.draw_trays(g, tr);
        self.draw_outbox(g, tr);
        self.draw_clerk(g);
        self.draw_fx(g);
        g.off = saved_off;
        g.c.set_clip(saved_clip);
    }

    fn draw_room(&self, g: &mut Gfx<'_>, tr: &algoclerk_core::i18n::Strings) {
        for ty in 0..9 {
            for tx in 0..17 {
                let k = if (tx + ty) % 2 == 0 {
                    "tiles/carpet_a"
                } else {
                    "tiles/carpet_b"
                };
                g.sprite(k, 0, tx * 16, 48 + ty * 16);
            }
        }
        for tx in 0..17 {
            g.sprite("tiles/wall_top", 0, tx * 16, 0);
            g.sprite(
                if tx % 3 == 1 {
                    "tiles/wall_b"
                } else {
                    "tiles/wall_a"
                },
                0,
                tx * 16,
                16,
            );
            g.sprite("tiles/dado", 0, tx * 16, 32);
        }
        let w = self.anim(80, 2);
        g.sprite("props/shelf", 0, 26, 8);
        g.sprite("props/poster_a", 0, 66, 9);
        g.sprite("props/window", w, 88, 8);
        g.sprite("props/clock", 0, 127, 5);
        g.sprite("props/window", w, 156, 8);
        g.sprite("props/poster_b", 0, 194, 9);
        g.sprite("props/poster_c", 0, 218, 11);
        self.draw_clock_hands(g);
        g.sprite("props/cabinet_a", 0, 6, 28);
        g.sprite("props/cabinet_b", 0, 24, 28);
        g.sprite("props/bin", 0, 46, 36);
        g.sprite("props/desk", self.anim(30, 2), 106, 24);
        g.sprite("props/mug", self.anim(14, 3), 124, 27);
        g.sprite("props/cooler", self.anim(45, 2), 188, 28);
        g.sprite("props/plant", self.anim(50, 2), 238, 26);
        g.sprite("props/lamp", 0, 123, 0);
        self.light_pool(g, 135, 112, 100, 56);
        // the cost counter on the wall under the clock
        let label = tr.format("office.cost", &[("n", self.cost.to_string())]);
        let w = g.text_w(&label, 1) + 8;
        g.round_rect(Rect::new(135 - w / 2, 22, w, 11), 'K', 'k');
        g.text(135 - w / 2 + 4, 24, &label, 'y');
    }

    fn draw_clock_hands(&self, g: &mut Gfx<'_>) {
        let (cx, cy) = (127 + 8, 5 + 8);
        let m = (self.cost % 60) as f32 * std::f32::consts::TAU / 60.0;
        let h = (self.cost % 720) as f32 * std::f32::consts::TAU / 720.0;
        let end = |len: f32, a: f32| {
            (
                cx + (a.sin() * len).round() as i32,
                cy - (a.cos() * len).round() as i32,
            )
        };
        let (hx, hy) = end(3.0, h);
        let (mx, my) = end(5.0, m);
        g.line(cx, cy, hx, hy, 'K');
        g.line(cx, cy, mx, my, 'R');
        g.px(cx, cy, 'K');
    }

    fn light_pool(&self, g: &mut Gfx<'_>, cx: i32, cy: i32, rx: i32, ry: i32) {
        let (t_dark, t_mid) = (g.col('t'), g.col('T'));
        let (ox, oy) = g.off;
        for y in (cy - ry).max(48)..(cy + ry).min(SCENE_H) {
            for x in (cx - rx).max(0)..(cx + rx).min(SCENE_W) {
                let (dx, dy) = ((x - cx) as f32 / rx as f32, (y - cy) as f32 / ry as f32);
                let d = dx * dx + dy * dy;
                if d >= 1.0 || (d > 0.72 && (x + y) & 1 == 1) {
                    continue;
                }
                let (px, py) = (x + ox, y + oy);
                if px < 0 || py < 0 || px as usize >= g.c.w || py as usize >= g.c.h {
                    continue;
                }
                if g.c.get(px as usize, py as usize) == t_dark {
                    g.c.set(px, py, t_mid);
                }
            }
        }
    }

    fn draw_trays(&self, g: &mut Gfx<'_>, tr: &algoclerk_core::i18n::Strings) {
        for t in &self.trays {
            let hot = t.changed && !self.reduce_motion && (self.clock / 14) % 2 == 0;
            g.round_rect(t.rect, if hot { 'y' } else { 'n' }, 'N');
            g.rect(
                Rect::new(t.rect.x + 2, t.rect.y + 9, t.rect.w - 4, t.rect.h - 11),
                if t.param { 'w' } else { 'E' },
            );
            let label = crate::gfx::clip(&t.name, ((t.rect.w - 6) / 4).max(2) as usize);
            g.tiny(t.rect.x + 3, t.rect.y + 2, &label.to_uppercase(), 'W');
            if t.param {
                g.px(t.rect.right() - 4, t.rect.y + 3, 'y');
            }
            t.w.draw(
                g,
                t.rect.x + 4 + (t.rect.w - 8 - t.w.w).max(0) / 2,
                t.rect.y + 10,
                t.changed,
            );
        }
        if self.hidden > 0 {
            g.tiny(
                SCENE_W - 60,
                FLOOR_TOP - 2,
                &tr.format("office.more", &[("n", self.hidden.to_string())]),
                'W',
            );
        }
        if self.trays.is_empty() && self.bubble.is_none() {
            g.round_rect(Rect::new(70, 80, 130, 24), 'W', 'K');
            g.text_c(135, 88, &tr.t("office.press_play"), 'K', 1);
        }
    }

    fn draw_outbox(&self, g: &mut Gfx<'_>, tr: &algoclerk_core::i18n::Strings) {
        let r = Rect::new(SCENE_W - 122, SCENE_H - 38, 118, 34);
        g.round_rect(r, 'k', 'K');
        g.rect(Rect::new(r.x + 2, r.y + 10, r.w - 4, r.h - 12), 'm');
        g.tiny(r.x + 4, r.y + 3, &tr.t("office.outbox"), 'W');
        if let Some((w, _)) = &self.out_value {
            w.draw(g, r.x + 6, r.y + 11 + (r.h - 13 - w.h).max(0) / 2, false);
        }
        if let Some((w, _)) = &self.expected {
            let gx = r.x + r.w - w.w.min(60) - 6;
            if self.out_value.is_none() {
                g.tiny(r.x + 4, r.y + 14, &tr.t("office.expected"), 'A');
                w.draw(g, gx, r.y + 11, false);
            }
        }
        if let Some(ok) = self.out_ok {
            g.sprite(
                if ok {
                    "icons/stamp_ok"
                } else {
                    "icons/stamp_bad"
                },
                0,
                r.right() - 18,
                r.y - 12,
            );
        }
    }

    fn draw_clerk(&self, g: &mut Gfx<'_>) {
        let x = self.clerk.x.round() as i32;
        let y = self.clerk.y.round() as i32;
        g.rect(Rect::new(x - 6, y - 1, 12, 3), 't');
        g.rect(Rect::new(x - 4, y - 2, 8, 1), 't');
        let (frame, flip) = self.clerk.frame(!self.reduce_motion);
        let hop = self.clerk.hop();
        g.c.blit(
            self.art.sprite(),
            frame,
            x - 8 + g.off.0,
            y - 20 + hop + g.off.1,
            flip,
            g.pal,
        );
        if self.clerk.walking && !self.reduce_motion && (self.clerk.clock / 6) % 2 == 0 {
            let dx = match self.clerk.dir {
                Dir::Left => 6,
                Dir::Right => -10,
                _ => -4,
            };
            g.sprite(
                "fx/dust",
                ((self.clerk.clock / 6) % 3) as usize,
                x + dx,
                y - 5,
            );
        }
    }

    fn draw_fx(&self, g: &mut Gfx<'_>) {
        for s in &self.sparks {
            let frame = ((30 - s.ttl) / 5) as usize % 4;
            let (x, y) = (s.at.0 as i32, s.at.1 as i32 - ((30 - s.ttl) / 6) as i32);
            g.sprite("fx/sparkle", frame, x - 4, y - 4);
        }
        if let Some(text) = &self.bubble {
            let lines = algoclerk_media::Font::wrap(text, 24);
            let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32 * 6 + 10;
            let h = lines.len() as i32 * 10 + 8;
            let hx = self.clerk.x.round() as i32;
            let top = self.clerk.y.round() as i32 - 20 - h - 8;
            let x = (hx - w / 2).clamp(2, SCENE_W - w - 2);
            let y = top.max(2);
            g.bubble(Rect::new(x, y, w, h), hx - 3, 'W', 'K');
            for (i, l) in lines.iter().enumerate() {
                g.text(x + 5, y + 4 + i as i32 * 10, l, 'K');
            }
        }
    }
}
