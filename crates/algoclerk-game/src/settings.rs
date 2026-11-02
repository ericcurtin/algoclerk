// SPDX-License-Identifier: Apache-2.0
//! Settings: palette (warm, colour-blind, high contrast), clerk outfit picker, text size, reduce motion,
//! animation speed, volumes, language, unlock-all and fullscreen. One scrolling column that works for mouse,
//! touch and keyboard.

use crate::clerk::{ClerkArt, ClerkState, Dir, Pose};
use crate::gfx::{Gfx, Kind, State, clip};
use crate::input::Key;
use crate::layout::{Hit, Viewport};
use crate::modal::{Answer, Modal, Purpose};
use crate::play::{outfit_of, tap_rect};
use crate::result::blit_scaled;
use crate::ui::{Ctx, GameEvent, Nav, View, widget_state};
use algoclerk_core::i18n;
use algoclerk_media::bank::PALETTES;
use algoclerk_media::{ArtBank, Outfit, Rect};

/// Settings rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// Palette.
    Palette,
    /// Reading text size.
    TextSize,
    /// Reduce motion.
    Reduce,
    /// Animation speed.
    Speed,
    /// Music volume.
    Music,
    /// Effects volume.
    Sfx,
    /// Language.
    Lang,
    /// Fullscreen (desktop).
    Fullscreen,
    /// Unlock all levels.
    Unlock,
    /// Delete all progress (asks first).
    Reset,
    /// Outfit preset.
    Preset,
    /// Body style.
    Body,
    /// One colour role (0 hair .. 5 extra).
    Role(usize),
    /// Back.
    Back,
}

const HAIR: &[char] = &['K', 'N', 'n', 'Y', 'y', 'R', 'A', 'u', 'p'];
const SKIN: &[char] = &['H', 'S', 'd', 'n', 'N'];
const SHIRT: &[char] = &['B', 'g', 'R', 'Y', 'u', 'W', 'k', 'o', 'p', 'T', 'b'];
const PANTS: &[char] = &['k', 'K', 'n', 'N', 'b', 'm', 'G', 'a', 'r'];
const SHOES: &[char] = &['K', 'N', 'n', 'W', 'R', 'A', 'y'];
const EXTRA: &[char] = &['R', 'Y', 'W', 'g', 'B', 'u', 'o', 'y', 'K'];

fn role_options(i: usize) -> &'static [char] {
    [HAIR, SKIN, SHIRT, PANTS, SHOES, EXTRA][i.min(5)]
}

fn role_get(o: &Outfit, i: usize) -> char {
    [o.hair, o.skin, o.shirt, o.pants, o.shoes, o.extra][i.min(5)]
}

fn role_set(o: &mut Outfit, i: usize, c: char) {
    match i {
        0 => o.hair = c,
        1 => o.skin = c,
        2 => o.shirt = c,
        3 => o.pants = c,
        4 => o.shoes = c,
        _ => o.extra = c,
    }
}

/// State of the settings screen.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Focused row index.
    pub focus: usize,
    /// Scroll offset in pixels.
    pub scroll: i32,
    art: ClerkArt,
    preview: ClerkState,
    modal: Option<Modal>,
}

/// A laid-out row.
#[derive(Debug, Clone)]
pub struct Placed {
    /// What it is.
    pub row: Row,
    /// Rectangle in content coordinates.
    pub rect: Rect,
}

impl Settings {
    /// Open the screen.
    pub fn new(cx: &Ctx<'_>) -> Settings {
        let outfit = outfit_of(cx.progress, cx.bank);
        Settings {
            focus: 0,
            scroll: 0,
            art: ClerkArt::new(cx.bank, &outfit),
            preview: ClerkState::new(0.0, 0.0),
            modal: None,
        }
    }

    /// The rows shown (fullscreen only on desktop).
    pub fn rows(can_quit: bool) -> Vec<Row> {
        let mut v = vec![
            Row::Palette,
            Row::TextSize,
            Row::Reduce,
            Row::Speed,
            Row::Music,
            Row::Sfx,
            Row::Lang,
        ];
        if can_quit {
            v.push(Row::Fullscreen);
        }
        v.push(Row::Unlock);
        v.push(Row::Reset);
        v.push(Row::Back);
        v.extend([Row::Preset, Row::Body]);
        v.extend((0..6).map(Row::Role));
        v
    }

    fn area(vp: &Viewport) -> Rect {
        let s = vp.safe;
        Rect::new(s.x + 4, s.y + 22, s.w - 8, s.h - 26)
    }

    /// Row height of this screen.
    fn row_h(vp: &Viewport) -> i32 {
        if vp.touch { vp.hit_min() + 2 } else { 20 }
    }

    /// Lay the rows out in content coordinates (relative to the area's top-left). In landscape the display and
    /// sound options sit on the left and the clerk's outfit on the right; in portrait everything is one column.
    pub fn layout(vp: &Viewport, can_quit: bool) -> (Vec<Placed>, i32) {
        let area = Settings::area(vp);
        let rh = Settings::row_h(vp);
        let two = !vp.portrait;
        let colw = if two { (area.w - 12) / 2 } else { area.w - 8 };
        let mut out = Vec::new();
        let (mut ly, mut ry) = (4, 4);
        for row in Settings::rows(can_quit) {
            let right = matches!(row, Row::Preset | Row::Body | Row::Role(_));
            let (x, y) = if two && right {
                (8 + colw, &mut ry)
            } else if two {
                (4, &mut ly)
            } else {
                (4, &mut ly)
            };
            if matches!(row, Row::Palette | Row::Preset | Row::Music) {
                *y += 14;
            }
            if row == Row::Preset {
                *y += 66;
            }
            if !two && right && row == Row::Preset {
                // single column: continue below the left rows
            }
            out.push(Placed {
                row,
                rect: Rect::new(x, *y, colw, rh),
            });
            *y += rh + 2;
            if !two {
                ry = ly;
            }
        }
        (out, ly.max(ry) + 4)
    }

    /// Hit rectangles for layout tests (first screenful).
    pub fn hits(vp: &Viewport, can_quit: bool) -> Vec<Hit> {
        let area = Settings::area(vp);
        let (rows, _) = Settings::layout(vp, can_quit);
        let mut v = vec![Hit::area("list", area)];
        for p in rows.iter().take(4) {
            v.push(Hit::tap("row", p.rect.offset(area.x, area.y)));
        }
        v.push(Hit::tap(
            "home",
            tap_rect(
                Rect::new(vp.safe.x + 3, vp.safe.y + 2, 26, 16),
                vp.hit_min(),
                vp.screen(),
            ),
        ));
        v
    }

    fn arrows(r: &Rect, vp: &Viewport) -> (Rect, Rect) {
        let w = if vp.touch { vp.hit_min() } else { 18 };
        let h = r.h - 2;
        let value_w = if r.w < 260 { 64 } else { 80 };
        (
            Rect::new(r.right() - 2 * w - value_w - 6, r.y + 1, w, h),
            Rect::new(r.right() - w - 2, r.y + 1, w, h),
        )
    }

    fn adjust(&mut self, cx: &mut Ctx<'_>, row: Row, delta: i32) {
        let s = &mut cx.progress.settings;
        match row {
            Row::Palette => {
                let cur = PALETTES
                    .iter()
                    .position(|(id, _)| *id == s.palette)
                    .unwrap_or(0) as i32;
                s.palette = PALETTES[(cur + delta).rem_euclid(PALETTES.len() as i32) as usize]
                    .0
                    .to_string();
            }
            Row::TextSize => s.text_size = if s.text_size >= 2 { 1 } else { 2 },
            Row::Reduce => s.reduce_motion = !s.reduce_motion,
            Row::Speed => s.speed_level = (i32::from(s.speed_level) + delta).clamp(0, 4) as u8,
            Row::Music => {
                s.music_volume = (i32::from(s.music_volume) + delta).clamp(0, 10) as u8;
                cx.events.push(GameEvent::SettingsChanged);
            }
            Row::Sfx => {
                s.sfx_volume = (i32::from(s.sfx_volume) + delta).clamp(0, 10) as u8;
                cx.events.push(GameEvent::SettingsChanged);
            }
            Row::Lang => {
                let langs = i18n::available_languages();
                let cur = langs.iter().position(|l| *l == s.language).unwrap_or(0) as i32;
                s.language =
                    langs[(cur + delta).rem_euclid(langs.len() as i32) as usize].to_string();
            }
            Row::Fullscreen => {
                s.fullscreen = !s.fullscreen;
                let on = s.fullscreen;
                cx.events.push(GameEvent::Fullscreen(on));
            }
            Row::Unlock => s.unlock_all = !s.unlock_all,
            Row::Preset => {
                let n = cx.bank.outfits.len() as i32;
                let cur = outfit_of_settings(&s.outfit, cx.bank);
                let idx = cx
                    .bank
                    .outfits
                    .iter()
                    .position(|(_, o)| *o == cur)
                    .map_or(-1, |i| i as i32);
                let next = (idx + delta).rem_euclid(n.max(1)) as usize;
                s.outfit = cx.bank.outfits[next].1.to_code();
            }
            Row::Body => {
                let mut o = outfit_of_settings(&s.outfit, cx.bank);
                let bodies = cx.bank.bodies();
                let cur = bodies.iter().position(|b| *b == o.body).unwrap_or(0) as i32;
                o.body =
                    bodies[(cur + delta).rem_euclid(bodies.len().max(1) as i32) as usize].clone();
                s.outfit = o.to_code();
            }
            Row::Role(i) => {
                let mut o = outfit_of_settings(&s.outfit, cx.bank);
                let opts = role_options(i);
                let cur = opts.iter().position(|c| *c == role_get(&o, i)).unwrap_or(0) as i32;
                role_set(
                    &mut o,
                    i,
                    opts[(cur + delta).rem_euclid(opts.len() as i32) as usize],
                );
                s.outfit = o.to_code();
            }
            Row::Reset | Row::Back => {}
        }
        cx.touch_progress();
        let outfit = outfit_of(cx.progress, cx.bank);
        self.art.set_outfit(cx.bank, &outfit);
        cx.sfx("button");
    }

    /// Handle input.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let vp = cx.vp;
        let area = Settings::area(&vp);
        let rows = Settings::rows(cx.config.can_quit);
        let (placed, total) = Settings::layout(&vp, cx.config.can_quit);
        let ptr = cx.ptr;
        self.preview.clock = self.preview.clock.wrapping_add(1);
        if let Some(m) = &mut self.modal {
            match m.update(cx) {
                Answer::Open => {}
                Answer::No => self.modal = None,
                Answer::Yes => {
                    self.modal = None;
                    let settings = cx.progress.settings.clone();
                    *cx.progress = algoclerk_core::progress::Progress::default();
                    cx.progress.settings = settings;
                    cx.touch_progress();
                    cx.sfx("wrong");
                }
            }
            return None;
        }
        let min = vp.hit_min();
        if ptr.tapped_in(&tap_rect(
            Rect::new(vp.safe.x + 3, vp.safe.y + 2, 26, 16),
            min,
            vp.screen(),
        )) || cx.input.key(Key::Escape)
        {
            cx.sfx("button");
            return Some(Nav::Title);
        }
        if ptr.over(&area) {
            self.scroll -= (ptr.wheel * 28.0) as i32;
        }
        if ptr.down && ptr.moved && ptr.held_in(&area) {
            self.scroll -= ptr.delta.1;
        }
        // keyboard
        let before = self.focus;
        if cx.input.key(Key::Down) || cx.input.key(Key::Tab) {
            self.focus = (self.focus + 1) % rows.len();
        }
        if cx.input.key(Key::Up) {
            self.focus = (self.focus + rows.len() - 1) % rows.len();
        }
        if self.focus != before {
            if let Some(p) = placed.get(self.focus) {
                if p.rect.y < self.scroll + 4 {
                    self.scroll = p.rect.y - 4;
                } else if p.rect.bottom() > self.scroll + area.h {
                    self.scroll = p.rect.bottom() - area.h + 4;
                }
            }
            cx.sfx("step");
        }
        let row = rows[self.focus.min(rows.len() - 1)];
        if cx.input.key(Key::Left) {
            self.adjust(cx, row, -1);
        }
        if cx.input.key(Key::Right) {
            self.adjust(cx, row, 1);
        }
        if cx.input.key(Key::Enter) || cx.input.key(Key::Space) {
            if row == Row::Back {
                cx.sfx("button");
                return Some(Nav::Title);
            }
            if matches!(
                row,
                Row::TextSize | Row::Reduce | Row::Fullscreen | Row::Unlock
            ) {
                self.adjust(cx, row, 1);
            }
            if row == Row::Reset {
                self.ask_reset(cx);
            }
        }
        // pointer
        for (i, p) in placed.iter().enumerate() {
            let r = p.rect.offset(area.x, area.y - self.scroll);
            if !area.contains(r.x + 2, r.y + r.h / 2) {
                continue;
            }
            let (l, rr) = Settings::arrows(&r, &vp);
            if ptr.tapped_in(&l) && area.contains(l.x, l.y) {
                self.focus = i;
                self.adjust(cx, p.row, -1);
            } else if ptr.tapped_in(&rr) {
                self.focus = i;
                self.adjust(cx, p.row, 1);
            } else if ptr.tapped_in(&r) {
                self.focus = i;
                if p.row == Row::Back {
                    cx.sfx("button");
                    return Some(Nav::Title);
                }
                if matches!(
                    p.row,
                    Row::TextSize | Row::Reduce | Row::Fullscreen | Row::Unlock
                ) {
                    self.adjust(cx, p.row, 1);
                }
                if p.row == Row::Reset {
                    self.ask_reset(cx);
                }
            }
        }
        self.scroll = self.scroll.clamp(0, (total - area.h).max(0));
        None
    }

    fn ask_reset(&mut self, cx: &Ctx<'_>) {
        self.modal = Some(Modal {
            purpose: Purpose::Reset,
            title: cx.tr.t("settings.reset"),
            body: cx.tr.t("settings.reset_warning"),
            yes: cx.tr.t("settings.reset_yes"),
            no: cx.tr.t("common.cancel"),
            focus_yes: false,
        });
    }

    fn label(row: Row, v: &View<'_>) -> String {
        let key = match row {
            Row::Palette => "settings.palette",
            Row::TextSize => "settings.text_size",
            Row::Reduce => "settings.reduce_motion",
            Row::Speed => "settings.speed",
            Row::Music => "settings.music",
            Row::Sfx => "settings.sfx",
            Row::Lang => "settings.language",
            Row::Fullscreen => "settings.fullscreen",
            Row::Unlock => "settings.unlock_all",
            Row::Reset => "settings.reset",
            Row::Preset => "settings.outfit",
            Row::Body => "settings.body",
            Row::Role(0) => "settings.hair",
            Row::Role(1) => "settings.skin",
            Row::Role(2) => "settings.shirt",
            Row::Role(3) => "settings.pants",
            Row::Role(4) => "settings.shoes",
            Row::Role(_) => "settings.extra",
            Row::Back => "common.back",
        };
        v.tr.t(key)
    }

    /// Draw the screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let area = Settings::area(&vp);
        let (placed, total) = Settings::layout(&vp, v.config.can_quit);
        let s = &v.progress.settings;
        g.rect(vp.screen(), 'k');
        for ty in 0..=(vp.h / 16) {
            for tx in 0..=(vp.w / 16) {
                g.sprite(
                    if (tx + ty) % 2 == 0 {
                        "tiles/wall_a"
                    } else {
                        "tiles/wall_b"
                    },
                    0,
                    tx * 16,
                    ty * 16,
                );
            }
        }
        // top bar
        let top = Rect::new(vp.safe.x, vp.safe.y, vp.safe.w, 20);
        g.rect(top, 'K');
        g.hline(top.x, top.bottom() - 1, top.w, 'm');
        let home = Rect::new(vp.safe.x + 3, vp.safe.y + 2, 26, 16);
        g.icon_button(
            home,
            "home",
            Kind::Normal,
            widget_state(
                v.ptr,
                &tap_rect(home, vp.hit_min(), vp.screen()),
                false,
                false,
            ),
        );
        g.text(home.right() + 6, top.y + 6, &v.tr.t("title.settings"), 'W');
        g.panel(area.grow(2), 'W');
        let saved = g.c.set_clip(area.offset(g.off.0, g.off.1));
        let oy = area.y - self.scroll;
        for (i, p) in placed.iter().enumerate() {
            let r = p.rect.offset(area.x, oy);
            if r.bottom() < area.y - 20 || r.y > area.bottom() + 20 {
                continue;
            }
            // section headings
            let head = match p.row {
                Row::Palette => Some("settings.section_display"),
                Row::Music => Some("settings.section_sound"),
                Row::Preset => Some("settings.section_outfit"),
                _ => None,
            };
            if let Some(h) = head {
                let off = if p.row == Row::Preset { 66 } else { 0 };
                let hr = Rect::new(r.x, r.y - 13 - off, r.w, 11);
                g.rect(hr, 'w');
                g.text(hr.x + 4, hr.y + 2, &v.tr.t(h).to_uppercase(), 'b');
            }
            if p.row == Row::Preset {
                self.draw_preview(g, v, Rect::new(r.x, r.y - 64, r.w, 62));
            }
            let focused = self.focus == i;
            if focused {
                g.rect(r, 'y');
                g.cursor(r.x + 2, r.y + (r.h - 9) / 2);
            }
            g.text(
                r.x + 14,
                r.y + (r.h - 7) / 2,
                &clip(&Settings::label(p.row, v), if r.w < 260 { 12 } else { 18 }),
                'K',
            );
            self.draw_value(g, v, p.row, r, s);
        }
        g.c.set_clip(saved);
        g.scrollbar(
            Rect::new(area.right() - 2, area.y + 1, 3, area.h - 2),
            self.scroll,
            total,
            area.h,
        );
        if let Some(m) = &self.modal {
            m.draw(g, v);
        }
    }

    fn draw_preview(&self, g: &mut Gfx<'_>, v: &View<'_>, r: Rect) {
        g.rect(r, 'T');
        g.frame(r, 'K');
        for tx in 0..(r.w / 16 + 1) {
            for ty in 0..(r.h / 16 + 1) {
                g.sprite(
                    if (tx + ty) % 2 == 0 {
                        "tiles/carpet_a"
                    } else {
                        "tiles/carpet_b"
                    },
                    0,
                    r.x + 1 + tx * 16,
                    r.y + 1 + ty * 16,
                );
            }
        }
        g.frame(r, 'K');
        let mut c = self.preview.clone();
        c.dir = Dir::Down;
        c.pose = Pose::Idle;
        if (self.preview.clock / 150) % 3 == 1 && !v.progress.settings.reduce_motion {
            c.pose = Pose::Cheer;
        }
        let (frame, _) = c.frame(!v.progress.settings.reduce_motion);
        let cx = r.x + r.w / 2;
        g.rect(Rect::new(cx - 20, r.bottom() - 10, 40, 5), 't');
        blit_scaled(
            g,
            &self.art,
            frame,
            cx - 24,
            r.bottom() - 6 - 60 + c.hop() * 3,
            3,
        );
    }

    fn draw_value(
        &self,
        g: &mut Gfx<'_>,
        v: &View<'_>,
        row: Row,
        r: Rect,
        s: &algoclerk_core::progress::Settings,
    ) {
        let ptr = v.ptr;
        let (l, rr) = Settings::arrows(&r, &v.vp);
        let vr = Rect::new(l.right(), r.y, rr.x - l.right(), r.h);
        let toggle = |g: &mut Gfx<'_>, on: bool| {
            let b = Rect::new(r.right() - 56, r.y + 1, 52, r.h - 2);
            g.button(
                b,
                &if on {
                    v.tr.t("common.on")
                } else {
                    v.tr.t("common.off")
                },
                if on { Kind::On } else { Kind::Normal },
                State::default(),
            );
            let _ = ptr;
        };
        let arrows = |g: &mut Gfx<'_>| {
            g.icon_button(l, "left", Kind::Normal, widget_state(ptr, &l, false, false));
            g.icon_button(
                rr,
                "right",
                Kind::Normal,
                widget_state(ptr, &rr, false, false),
            );
        };
        match row {
            Row::Palette => {
                arrows(g);
                let (id, _) = PALETTES
                    .iter()
                    .find(|(id, _)| *id == s.palette)
                    .copied()
                    .unwrap_or(PALETTES[0]);
                g.text_in(vr, &v.tr.t(&format!("palette.{id}")), 'K', 1);
            }
            Row::TextSize => {
                let b = Rect::new(r.right() - 56, r.y + 1, 52, r.h - 2);
                g.button(
                    b,
                    if s.text_size >= 2 { "2x" } else { "1x" },
                    Kind::Normal,
                    State::default(),
                );
            }
            Row::Reduce => toggle(g, s.reduce_motion),
            Row::Fullscreen => toggle(g, s.fullscreen),
            Row::Unlock => toggle(g, s.unlock_all),
            Row::Speed => {
                arrows(g);
                g.text_in(
                    vr,
                    ["1x", "2x", "4x", "8x", "MAX"][usize::from(s.speed_level.min(4))],
                    'K',
                    1,
                );
            }
            Row::Music | Row::Sfx => {
                arrows(g);
                let n = if row == Row::Music {
                    s.music_volume
                } else {
                    s.sfx_volume
                };
                let bw = (vr.w - 8) / 10;
                for i in 0..10 {
                    let bar = Rect::new(
                        vr.x + 4 + i * bw,
                        vr.y + r.h / 2 - 2 - i / 3,
                        bw - 1,
                        5 + i / 3,
                    );
                    g.rect(bar, if i < i32::from(n) { 'g' } else { 'A' });
                }
            }
            Row::Lang => {
                arrows(g);
                let langs = i18n::available_languages();
                let code = langs
                    .iter()
                    .find(|c| **c == s.language)
                    .copied()
                    .unwrap_or("en");
                let name = i18n::load_language(code)
                    .map_or_else(|| code.to_string(), |t| t.t("meta.language"));
                g.text_in(vr, &name, 'K', 1);
            }
            Row::Preset => {
                arrows(g);
                let cur = outfit_of_settings(&s.outfit, g.bank);
                let name = g
                    .bank
                    .outfits
                    .iter()
                    .find(|(_, o)| *o == cur)
                    .map_or_else(|| v.tr.t("settings.custom"), |(n, _)| n.clone());
                g.text_in(vr, &clip(&name, (vr.w / 6).max(3) as usize), 'K', 1);
            }
            Row::Body => {
                arrows(g);
                let o = outfit_of_settings(&s.outfit, g.bank);
                g.text_in(vr, &o.body.to_uppercase(), 'K', 1);
            }
            Row::Role(i) => {
                arrows(g);
                let o = outfit_of_settings(&s.outfit, g.bank);
                let c = role_get(&o, i);
                let sw = Rect::new(vr.x + vr.w / 2 - 14, vr.y + 3, 28, vr.h - 6);
                g.round_rect(sw, c, 'K');
            }
            Row::Reset => {
                let b = Rect::new(r.right() - 86, r.y + 1, 82, r.h - 2);
                g.button(
                    b,
                    &v.tr.t("settings.reset_short"),
                    Kind::Danger,
                    State::default(),
                );
            }
            Row::Back => {
                let b = Rect::new(r.right() - 90, r.y + 1, 86, r.h - 2);
                g.button(b, &v.tr.t("common.back"), Kind::Primary, State::default());
            }
        }
    }
}

/// The outfit stored in settings (default when empty or unknown).
pub fn outfit_of_settings(code: &str, bank: &ArtBank) -> Outfit {
    Outfit::from_code(code)
        .filter(|o| bank.has_template(&format!("clerk/body_{}", o.body)))
        .unwrap_or_default()
}
