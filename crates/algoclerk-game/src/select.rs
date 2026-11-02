// SPDX-License-Identifier: Apache-2.0
//! Level select: an office building seen in cross-section. Every pack is a floor with its own wallpaper; the
//! levels are numbered doors with star badges and a padlock while locked. Scrolls with wheel, finger or keys.

use crate::gfx::{Gfx, clip};
use crate::input::Key;
use crate::layout::{Hit, Viewport};
use crate::ui::{Ctx, Nav, View};
use algoclerk_media::Rect;

const WALLPAPERS: [(char, char); 7] = [('w', 'E'), ('s', 'B'), ('p', 'r'), ('l', 'g'), ('y', 'Y'), ('A', 'a'), ('c', 'T')];
const DOOR_W: i32 = 24;
const DOOR_H: i32 = 32;

/// State of the level select screen.
#[derive(Debug, Clone, Default)]
pub struct Select {
    /// Focused level as (pack, level).
    pub focus: (usize, usize),
    /// Scroll offset in pixels.
    pub scroll: i32,
    /// Toast text and ticks left.
    pub toast: Option<(String, u32)>,
    last_ptr: Option<(i32, i32)>,
    jump_to_focus: bool,
}

/// Geometry of one floor in content coordinates.
#[derive(Debug, Clone)]
pub struct Floor {
    /// The floor's band.
    pub band: Rect,
    /// Door rectangles by level index.
    pub doors: Vec<Rect>,
    /// Sign rectangle.
    pub sign: Rect,
}

impl Select {
    /// Open with focus on a level.
    pub fn new(cx: &Ctx<'_>, focus_id: &str) -> Select {
        let mut s = Select::default();
        for (pi, p) in cx.campaign.packs.iter().enumerate() {
            if let Some(li) = p.levels.iter().position(|l| l.id == focus_id) {
                s.focus = (pi, li);
            }
        }
        s.jump_to_focus = true;
        s
    }

    fn area(vp: &Viewport) -> Rect {
        let s = vp.safe;
        Rect::new(s.x, s.y + 20, s.w, s.h - 20)
    }

    /// Floors laid out in content coordinates (y from 0 at the roof).
    pub fn floors(vp: &Viewport, level_counts: &[usize]) -> (Vec<Floor>, i32) {
        let s = vp.safe;
        let mut y = 26; // roof
        let mut out = Vec::new();
        for &n in level_counts.iter() {
            if vp.portrait {
                let per_row = ((s.w - 16) / 50).max(1) as usize;
                let rows = n.div_ceil(per_row).max(1) as i32;
                let h = 20 + rows * 62 + 6;
                let band = Rect::new(s.x + 4, y, s.w - 8, h);
                let sign = Rect::new(band.x + 4, band.y + 3, band.w - 8, 14);
                let dw = (band.w - 8) / per_row as i32;
                let doors = (0..n)
                    .map(|i| {
                        let (c, r) = ((i % per_row) as i32, (i / per_row) as i32);
                        Rect::new(band.x + 4 + c * dw + (dw - DOOR_W) / 2, band.y + 24 + r * 62, DOOR_W, DOOR_H)
                    })
                    .collect();
                out.push(Floor { band, doors, sign });
                y += h + 4;
            } else {
                let h = 78;
                let band = Rect::new(s.x + 8, y, s.w - 16, h);
                let sign = Rect::new(band.x + 4, band.y + 6, 88, h - 12);
                let area_w = band.w - 100;
                let dw = (area_w / n.max(1) as i32).min(54);
                let doors = (0..n)
                    .map(|i| Rect::new(band.x + 98 + i as i32 * dw + (dw - DOOR_W) / 2, band.y + 22, DOOR_W, DOOR_H))
                    .collect();
                out.push(Floor { band, doors, sign });
                y += h + 4;
            }
        }
        (out, y + 24)
    }

    fn counts(cx: &Ctx<'_>) -> Vec<usize> {
        cx.campaign.packs.iter().map(|p| p.levels.len()).collect()
    }

    /// Hit rectangles (door tap areas) for layout tests, at scroll 0, clipped to the screen.
    pub fn hits(vp: &Viewport, counts: &[usize]) -> Vec<Hit> {
        let (floors, _) = Select::floors(vp, counts);
        let area = Select::area(vp);
        let mut v = vec![Hit::area("list", area)];
        for f in floors.iter().take(2) {
            for d in &f.doors {
                let r = d.offset(0, area.y);
                v.push(Hit::tap("door", Rect::new(r.x, r.y, r.w, r.h + 14)));
            }
        }
        v
    }

    /// Handle input.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let vp = cx.vp;
        let area = Select::area(&vp);
        let counts = Select::counts(cx);
        let (floors, total) = Select::floors(&vp, &counts);
        let ptr = cx.ptr;
        if let Some((_, t)) = &mut self.toast {
            *t = t.saturating_sub(1);
            if *t == 0 {
                self.toast = None;
            }
        }
        // back
        let home = Rect::new(vp.safe.x + 3, vp.safe.y + 2, 26, 16);
        let min = vp.hit_min();
        if ptr.tapped_in(&crate::play::tap_rect(home, min, vp.screen())) || cx.input.key(Key::Escape) {
            cx.sfx("button");
            return Some(Nav::Title);
        }
        // scrolling
        if ptr.over(&area) {
            self.scroll -= (ptr.wheel * 30.0) as i32;
        }
        if ptr.down && ptr.moved && ptr.held_in(&area) {
            self.scroll -= ptr.delta.1;
        }
        // pointer hover moves focus; taps open doors
        let mut open = None;
        if ptr.pos != self.last_ptr && !ptr.touch {
            if let Some((pi, li)) = Select::door_at(&floors, self.scroll, &area, ptr.pos) {
                self.focus = (pi, li);
            }
            self.last_ptr = ptr.pos;
        }
        if ptr.released && !ptr.moved {
            if let Some((pi, li)) = Select::door_at(&floors, self.scroll, &area, ptr.pos) {
                if Select::door_at(&floors, self.scroll, &area, Some(ptr.start)) == Some((pi, li)) {
                    self.focus = (pi, li);
                    open = Some((pi, li));
                }
            }
        }
        // keyboard
        let (mut pi, mut li) = self.focus;
        let np = floors.len();
        let mut moved = false;
        if np > 0 {
            if cx.input.key(Key::Right) {
                if li + 1 < counts[pi] {
                    li += 1;
                } else if pi + 1 < np {
                    pi += 1;
                    li = 0;
                }
                moved = true;
            }
            if cx.input.key(Key::Left) {
                if li > 0 {
                    li -= 1;
                } else if pi > 0 {
                    pi -= 1;
                    li = counts[pi].saturating_sub(1);
                }
                moved = true;
            }
            if cx.input.key(Key::Down) && pi + 1 < np {
                pi += 1;
                li = li.min(counts[pi].saturating_sub(1));
                moved = true;
            }
            if cx.input.key(Key::Up) && pi > 0 {
                pi -= 1;
                li = li.min(counts[pi].saturating_sub(1));
                moved = true;
            }
            if cx.input.key(Key::Enter) || cx.input.key(Key::Space) {
                open = Some((pi, li));
            }
        }
        if moved {
            self.focus = (pi, li);
            self.jump_to_focus = true;
            cx.sfx("step");
        }
        if self.jump_to_focus {
            if let Some(f) = floors.get(self.focus.0) {
                let band = f.band;
                if band.y < self.scroll + 4 {
                    self.scroll = band.y - 4;
                } else if band.bottom() > self.scroll + area.h {
                    self.scroll = band.bottom() - area.h + 4;
                }
                if self.jump_to_focus && !moved {
                    self.scroll = (band.y - 30).max(0);
                }
            }
            self.jump_to_focus = false;
        }
        self.scroll = self.scroll.clamp(0, (total - area.h).max(0));
        if let Some((pi, li)) = open {
            let unlocked = cx.campaign.level_unlocked(cx.progress, pi, li);
            if unlocked {
                if let Some(l) = cx.campaign.packs.get(pi).and_then(|p| p.levels.get(li)) {
                    cx.sfx("button");
                    return Some(Nav::Brief(l.id.clone()));
                }
            } else {
                cx.sfx("wrong");
                self.toast = Some((cx.tr.t("packs.locked"), 150));
            }
        }
        None
    }

    fn door_at(floors: &[Floor], scroll: i32, area: &Rect, pos: Option<(i32, i32)>) -> Option<(usize, usize)> {
        let (x, y) = pos?;
        if !area.contains(x, y) {
            return None;
        }
        for (pi, f) in floors.iter().enumerate() {
            for (li, d) in f.doors.iter().enumerate() {
                let r = Rect::new(d.x - 3, d.y - 14 - scroll + area.y, d.w + 6, d.h + 14 + 14 + 4);
                if r.contains(x, y) {
                    return Some((pi, li));
                }
            }
        }
        None
    }

    /// Draw the screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let area = Select::area(&vp);
        let counts: Vec<usize> = v.campaign.packs.iter().map(|p| p.levels.len()).collect();
        let (floors, total) = Select::floors(&vp, &counts);
        // sky and the street
        g.rect(vp.screen(), 's');
        for i in 0..6 {
            let x = (i * 97 + (if v.progress.settings.reduce_motion { 0 } else { (g.tick / 6) as i32 })) % (vp.w + 40) - 30;
            g.rect(Rect::new(x, vp.safe.y + 26 + (i * 37) % 120, 22, 4), 'X');
            g.rect(Rect::new(x + 4, vp.safe.y + 23 + (i * 37) % 120, 12, 3), 'X');
        }
        let saved = g.c.set_clip(area.offset(g.off.0, g.off.1));
        let oy = area.y - self.scroll;
        let sx = vp.safe.x;
        // roof and sign
        let roof = Rect::new(sx + 2, oy + 2, vp.safe.w - 4, 22);
        g.rect(roof, 'r');
        g.frame(roof, 'K');
        g.hline(roof.x + 1, roof.y + 1, roof.w - 2, 'R');
        g.text_c(roof.x + roof.w / 2, roof.y + 7, &v.tr.t("packs.title"), 'W', 1);
        let progress = v.progress;
        for (pi, f) in floors.iter().enumerate() {
            let Some(pack) = v.campaign.packs.get(pi) else { continue };
            let band = f.band.offset(0, oy);
            if band.bottom() < area.y || band.y > area.bottom() {
                continue;
            }
            let (wa, wb) = WALLPAPERS[pi % WALLPAPERS.len()];
            let unlocked = v.campaign.pack_unlocked(progress, pi);
            // wallpaper tiles inside the band
            let sv = g.c.set_clip(band.offset(g.off.0, g.off.1).intersect(&area.offset(g.off.0, g.off.1)));
            for ty in 0..=(band.h / 16) {
                for tx in 0..=(band.w / 16) {
                    g.sprite_map(if (tx + ty) % 3 == 1 { "tiles/wall_b" } else { "tiles/wall_a" }, 0, band.x + tx * 16, band.y + ty * 16, &[('w', wa), ('E', wb)]);
                }
            }
            g.c.set_clip(sv);
            g.rect(Rect::new(band.x, band.bottom() - 8, band.w, 8), 'n');
            g.hline(band.x, band.bottom() - 8, band.w, 'd');
            g.frame(band, 'K');
            // floor sign
            let sign = f.sign.offset(0, oy);
            g.panel_with(sign, 'W', 'K', false);
            let solved = pack.levels.iter().filter(|l| progress.record(&l.id).solved).count();
            let cols = ((sign.w - 8) / 6).max(4) as usize;
            if vp.portrait {
                let t = format!("{}F {}  {}/{}", pi + 1, pack.info.title, solved, pack.levels.len());
                g.text(sign.x + 4, sign.y + 4, &clip(&t, cols), 'K');
            } else {
                g.text(sign.x + 4, sign.y + 4, &format!("{}F", pi + 1), 'R');
                for (i, l) in algoclerk_media::Font::wrap(&pack.info.title, cols).iter().take(3).enumerate() {
                    g.text(sign.x + 4, sign.y + 16 + i as i32 * 10, l, 'K');
                }
                g.text(sign.x + 4, sign.bottom() - 11, &format!("{}/{}", solved, pack.levels.len()), 'k');
                g.sprite("ui/star_small_on", 0, sign.right() - 15, sign.bottom() - 14);
                if !unlocked {
                    g.sprite("levels/padlock", 0, sign.right() - 14, sign.y + 4);
                }
            }
            for (li, level) in pack.levels.iter().enumerate() {
                let d = f.doors[li].offset(0, oy);
                if d.bottom() + 16 < area.y || d.y > area.bottom() {
                    continue;
                }
                let rec = progress.record(&level.id);
                let open = v.campaign.level_unlocked(progress, pi, li);
                let key = if !open { "levels/door_locked" } else if rec.solved { "levels/door_done" } else { "levels/door_open" };
                g.sprite(key, 0, d.x, d.y);
                // number plaque on the door
                let num = format!("{}", v.campaign.all_levels().position(|l| l.id == level.id).map_or(0, |i| i + 1));
                g.tiny_c(d.x + 12, d.y + 7, &num, if open { 'K' } else { 'W' });
                if !open {
                    g.sprite("levels/padlock", 0, d.x + 7, d.y + 17);
                }
                // stars under the door
                for s in 0..3u8 {
                    g.star(d.x - 6 + i32::from(s) * 12, d.bottom() + 2, rec.stars > s, false);
                }
                if rec.solved {
                    g.sprite("icons/stamp_ok", 0, d.right() - 8, d.y - 3);
                }
                if self.focus == (pi, li) {
                    g.cursor(d.x - 10, d.y + 12);
                    g.frame(d.grow(2), 'y');
                    g.frame(d.grow(3), 'K');
                    let name = clip(&level.title, 22);
                    let w = g.text_w(&name, 1) + 8;
                    let tx = (d.x + 12 - w / 2).clamp(vp.safe.x + 2, vp.safe.right() - w - 2);
                    g.round_rect(Rect::new(tx, d.y - 15, w, 11), 'K', 'K');
                    g.text(tx + 4, d.y - 13, &name, 'W');
                }
            }
        }
        g.c.set_clip(saved);
        g.scrollbar(Rect::new(vp.safe.right() - 4, area.y + 2, 3, area.h - 4), self.scroll, total, area.h);
        // top bar
        let top = Rect::new(vp.safe.x, vp.safe.y, vp.safe.w, 20);
        g.rect(top, 'K');
        g.hline(top.x, top.bottom() - 1, top.w, 'm');
        let home = Rect::new(vp.safe.x + 3, vp.safe.y + 2, 26, 16);
        g.icon_button(home, "home", crate::gfx::Kind::Normal, crate::ui::widget_state(v.ptr, &crate::play::tap_rect(home, vp.hit_min(), vp.screen()), false, false));
        g.text(home.right() + 6, top.y + 6, &v.tr.t("title.levels"), 'W');
        let stars = format!("{}", progress.total_stars());
        g.text_r(top.right() - 18, top.y + 6, &stars, 'y', 1);
        g.star(top.right() - 14, top.y + 4, true, false);
        if let Some((t, _)) = &self.toast {
            let w = g.text_w(t, 1) + 16;
            let r = Rect::new(vp.safe.x + (vp.safe.w - w.min(vp.safe.w - 8)) / 2, vp.safe.bottom() - 26, w.min(vp.safe.w - 8), 18);
            g.panel_with(r, 'y', 'K', true);
            g.text_c(r.x + r.w / 2, r.y + 5, &clip(t, ((r.w - 8) / 6) as usize), 'K', 1);
        }
    }
}
