// SPDX-License-Identifier: Apache-2.0
//! The title screen: a hanging logo sign, a little animated office (clerk at a desk, crates riding the belt)
//! and a Falabichos-style menu with the marching arrow cursor.

use crate::clerk::{ClerkArt, ClerkState, Dir, Pose};
use crate::gfx::Gfx;
use crate::layout::{Hit, Viewport};
use crate::ui::{Ctx, MenuFocus, Nav, View, draw_menu_row};
use algoclerk_media::{ArtBank, Outfit, Rect};

/// Title screen state.
#[derive(Debug, Clone)]
pub struct Title {
    /// Menu focus.
    pub menu: MenuFocus,
    art: ClerkArt,
    clock: u32,
}

/// The menu entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// Start or continue the campaign.
    Start,
    /// Sandbox.
    Sandbox,
    /// Level select.
    Levels,
    /// Settings.
    Settings,
    /// About.
    About,
    /// Quit (desktop).
    Quit,
}

/// Draw the big "AlgoClerk" logo with an ink outline, a drop shadow and a two-tone fill.
pub fn logo(g: &mut Gfx<'_>, cx: i32, y: i32, scale: i32) {
    let text = "AlgoClerk";
    let w = g.text_w(text, scale);
    let x = cx - w / 2;
    let o = (scale / 2).max(1);
    for dy in [-o, 0, o, 2 * o] {
        for dx in [-o, 0, o] {
            g.text_s(x + dx, y + dy, text, 'K', scale);
        }
    }
    g.text_s(x, y + o + 1, text, 'k', scale);
    let h = 7 * scale;
    let clip_full = g.c.set_clip(Rect::new(x + g.off.0, y + g.off.1, w + 2, h * 6 / 10));
    g.text_s(x, y, text, 'y', scale);
    g.c.set_clip(Rect::new(x + g.off.0, y + g.off.1 + h * 6 / 10, w + 2, h));
    g.text_s(x, y, text, 'Y', scale);
    g.c.set_clip(clip_full);
}

impl Title {
    /// A fresh title screen.
    pub fn new(bank: &ArtBank, outfit: &Outfit) -> Title {
        Title {
            menu: MenuFocus::default(),
            art: ClerkArt::new(bank, outfit),
            clock: 0,
        }
    }

    /// The menu entries for the current progress.
    pub fn items(cx_can_quit: bool) -> Vec<Item> {
        let mut v = vec![Item::Start, Item::Sandbox, Item::Levels, Item::Settings, Item::About];
        if cx_can_quit {
            v.push(Item::Quit);
        }
        v
    }

    /// Menu panel and row rectangles.
    pub fn menu_rects(vp: &Viewport, n: usize) -> (Rect, Vec<Rect>) {
        let row_h = if vp.touch { 28 } else { 17 };
        let w = if vp.portrait { 214 } else { 168 };
        let h = n as i32 * row_h + 16;
        let x = vp.safe.x + (vp.safe.w - w) / 2;
        let y = if vp.portrait { vp.safe.y + 214 } else { vp.safe.bottom() - h - 18 };
        let panel = Rect::new(x, y, w, h);
        let rows = (0..n as i32)
            .map(|i| Rect::new(x + 6, y + 8 + i * row_h, w - 12, row_h))
            .collect();
        (panel, rows)
    }

    /// Hit rectangles for layout tests.
    pub fn hits(vp: &Viewport, can_quit: bool) -> Vec<Hit> {
        let items = Title::items(can_quit);
        let (panel, rows) = Title::menu_rects(vp, items.len());
        let mut v = vec![Hit::area("menu", panel)];
        v.extend(rows.into_iter().map(|r| Hit::tap("menu row", r)));
        v
    }

    /// Handle input.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        self.clock = self.clock.wrapping_add(1);
        let items = Title::items(cx.config.can_quit);
        let (_, rows) = Title::menu_rects(&cx.vp, items.len());
        let before = self.menu.focus;
        let chosen = self.menu.update(cx, &rows);
        if self.menu.focus != before {
            cx.sfx("step");
        }
        let item = items[chosen?];
        cx.sfx("button");
        Some(match item {
            Item::Start => {
                let id = next_level_id(cx);
                Nav::Brief(id)
            }
            Item::Sandbox => Nav::Sandbox,
            Item::Levels => Nav::Select,
            Item::Settings => Nav::Settings,
            Item::About => Nav::About,
            Item::Quit => Nav::Quit,
        })
    }

    /// Draw the screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let (w, h) = (vp.w, vp.h);
        let portrait = vp.portrait;
        let wall_h = if portrait { 160 } else { 112 };
        // floor and wall
        for ty in 0..=(h / 16) {
            for tx in 0..=(w / 16) {
                let k = if (tx + ty) % 2 == 0 { "tiles/carpet_a" } else { "tiles/carpet_b" };
                g.sprite(k, 0, tx * 16, ty * 16);
            }
        }
        for tx in 0..=(w / 16) {
            g.sprite("tiles/wall_top", 0, tx * 16, 0);
            for ty in 1..(wall_h / 16 - 1) {
                g.sprite(if (tx + ty) % 4 == 1 { "tiles/wall_b" } else { "tiles/wall_a" }, 0, tx * 16, ty * 16);
            }
            g.sprite("tiles/dado", 0, tx * 16, wall_h - 16);
        }
        let (tick, rm) = (g.tick, v.progress.settings.reduce_motion);
        let anim = move |p: u64, n: usize| if rm { 0 } else { ((tick / p) % n as u64) as usize };
        // wall decor (the sign hangs in the middle, windows and posters flank it)
        let win = anim(90, 2);
        if portrait {
            g.sprite("props/window", win, 22, 112);
            g.sprite("props/window", win, 216, 112);
            g.sprite("props/poster_a", 0, 66, 116);
            g.sprite("props/poster_b", 0, 188, 116);
            g.sprite("props/clock", 0, 127, 112);
        } else {
            g.sprite("props/window", win, 18, 34);
            g.sprite("props/window", win, 54, 34);
            g.sprite("props/poster_a", 0, 30, 66);
            g.sprite("props/window", win, 394, 34);
            g.sprite("props/window", win, 430, 34);
            g.sprite("props/poster_b", 0, 434, 66);
            g.sprite("props/clock", 0, 56, 8);
            g.sprite("props/shelf", 0, 402, 8);
        }
        // hanging sign with the logo and the tagline
        let scale = if portrait { 4 } else { 5 };
        let sign_w = 9 * 6 * scale + 28;
        let tag = v.tr.t("title.tagline");
        let tag_cols = ((sign_w - 20) / 6) as usize;
        let tag_lines = algoclerk_media::Font::wrap(&tag, tag_cols);
        let sign_h = 7 * scale + 22 + tag_lines.len() as i32 * 10 + 6;
        let sign = Rect::new((w - sign_w) / 2, if portrait { 18 } else { 8 }, sign_w, sign_h);
        let bob = if v.progress.settings.reduce_motion { 0 } else { [0, 0, 1, 1][((g.tick / 40) % 4) as usize] };
        for cxp in [sign.x + 16, sign.right() - 17] {
            g.vline(cxp, 0, sign.y + bob, 'a');
            g.vline(cxp + 1, 0, sign.y + bob, 'A');
        }
        let sign = sign.offset(0, bob);
        g.panel_with(sign, 'k', 'K', true);
        g.frame(sign.inset(3), 'm');
        logo(g, w / 2, sign.y + 10, scale);
        let ty = sign.y + 10 + 7 * scale + 8;
        for (i, l) in tag_lines.iter().enumerate() {
            g.text_c(w / 2, ty + i as i32 * 10, l, 'A', 1);
        }
        // the belt with crates
        let belt_y = if portrait { 172 } else { 126 };
        let phase = if v.progress.settings.reduce_motion { 0 } else { (g.tick / 2) as i32 };
        let frame = ((g.tick / 3) % 4) as usize * usize::from(!v.progress.settings.reduce_motion);
        for tx in 0..=(w / 16) {
            g.sprite("tiles/belt_h", frame, tx * 16, belt_y);
        }
        for tx in 0..=(w / 16) {
            if tx % 3 == 0 {
                g.sprite("tiles/belt_feet", 0, tx * 16, belt_y + 12);
            }
        }
        let n_crates = w / 56 + 1;
        for i in 0..n_crates {
            let x = ((phase * 3 / 4 + i * 56) % (n_crates * 56)) - 16;
            let val = (i * 7 + 3) % 40;
            g.sprite(["tiles/crate_a", "tiles/crate_b", "tiles/crate_c", "tiles/crate_d"][(val % 4) as usize], 0, x, belt_y - 12);
            g.tiny_c(x + 8, belt_y - 12 + 6, &val.to_string(), 'K');
        }
        // desk and clerk
        let (dx, dy) = if portrait { (24, 200 + 130) } else { (26, 168) };
        let (dx, dy) = if portrait { (dx, dy) } else { (dx, dy) };
        let mut c = ClerkState::new((dx + 44) as f32, (dy + 14) as f32);
        c.dir = Dir::Left;
        c.pose = Pose::Work;
        c.clock = g.tick as u32;
        let (frame, flip) = c.frame(!v.progress.settings.reduce_motion);
        g.rect(Rect::new(dx + 36, dy + 12, 16, 3), 't');
        g.c.blit(self.art.sprite(), frame, dx + 36 + g.off.0, dy - 6 + g.off.1, flip, g.pal);
        g.sprite("props/desk", anim(30, 2), dx, dy - 10);
        g.sprite("props/mug", anim(14, 3), dx + 17, dy - 7);
        if !portrait {
            g.sprite("props/cabinet_a", 0, 386, 138);
            g.sprite("props/cabinet_b", 0, 402, 138);
            g.sprite("props/plant", anim(50, 2), 436, 150);
            g.sprite("props/cooler", anim(45, 2), 330, 148);
            g.sprite("props/bin", 0, 82, 190);
        } else {
            g.sprite("props/cabinet_a", 0, 180, 372);
            g.sprite("props/cabinet_b", 0, 196, 372);
            g.sprite("props/plant", anim(50, 2), 232, 380);
            g.sprite("props/cooler", anim(45, 2), 150, 378);
        }
        // the menu
        let items = Title::items(v.config.can_quit);
        let (panel, rows) = Title::menu_rects(&vp, items.len());
        g.panel(panel, 'W');
        for (i, (it, r)) in items.iter().zip(&rows).enumerate() {
            let label = match it {
                Item::Start => {
                    if v.progress.solved_count() > 0 { v.tr.t("title.continue") } else { v.tr.t("title.play") }
                }
                Item::Sandbox => v.tr.t("title.sandbox"),
                Item::Levels => v.tr.t("title.levels"),
                Item::Settings => v.tr.t("title.settings"),
                Item::About => v.tr.t("title.about"),
                Item::Quit => v.tr.t("title.quit"),
            };
            if vp.touch {
                let r2 = r.inset(1);
                g.round_rect(r2, if i == self.menu.focus { 'y' } else { 'w' }, 'K');
                g.text_c(r2.x + r2.w / 2, r2.y + (r2.h - 7) / 2, &label, 'K', 1);
            } else {
                draw_menu_row(g, *r, &label, i == self.menu.focus, true);
            }
        }
        // footer
        let fy = vp.safe.bottom() - 11;
        g.text_shadow(vp.safe.x + 6, fy, &format!("v{}", env!("CARGO_PKG_VERSION")), 'W', 'K', 1);
        let stars = format!("{} {}", v.tr.t("title.stars"), v.progress.total_stars());
        g.text_r(vp.safe.right() - 6, fy, &stars, 'W', 1);
        g.c.reset_clip();
    }
}

/// The level the "Start / Continue" entry opens: the first level that is not solved yet, else the last played.
pub fn next_level_id(cx: &Ctx<'_>) -> String {
    for p in &cx.campaign.packs {
        for l in &p.levels {
            if !cx.progress.record(&l.id).solved {
                return l.id.clone();
            }
        }
    }
    if !cx.progress.last_level.is_empty() && cx.campaign.find(&cx.progress.last_level).is_some() {
        return cx.progress.last_level.clone();
    }
    cx.campaign
        .all_levels()
        .next()
        .map(|l| l.id.clone())
        .unwrap_or_default()
}
