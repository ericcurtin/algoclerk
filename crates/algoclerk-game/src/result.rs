// SPDX-License-Identifier: Apache-2.0
//! The victory scene (confetti, the cheering clerk, stars filling one by one, medals) and the code reveal
//! (C, C++ and Rust tabs in the pixel monospace font with syntax colours, scrolling, copy).

use crate::clerk::{ClerkArt, ClerkState, Dir, Pose};
use crate::gfx::{Gfx, Kind, State, clip, ease_out};
use crate::input::Key;
use crate::layout::{Hit, Viewport};
use crate::play::{Play, outfit_of, tap_rect};
use crate::ui::{Ctx, GameEvent, Nav, View, widget_state};
use algoclerk_core::codegen::highlight::{TokenKind, highlight};
use algoclerk_core::codegen::{self, Generated, Lang, Options, Stats};
use algoclerk_core::judge::{Medal, Verdict};
use algoclerk_core::share;
use algoclerk_media::{ArtBank, Rect};

/// Which part of the result flow is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Stars, medals and stats.
    Victory,
    /// The code reveal.
    Code,
}

/// One syntax-coloured line.
pub type Spans = Vec<(String, char)>;

/// The generated code of one language, split into coloured lines.
#[derive(Debug, Clone)]
pub struct CodeText {
    /// Language.
    pub lang: Lang,
    /// Plain source.
    pub source: String,
    /// Coloured lines.
    pub lines: Vec<Spans>,
    /// Longest line in characters.
    pub width: usize,
    /// Algorithm note.
    pub note: Option<(String, String)>,
    /// Structured (loops) or goto form.
    pub structured: bool,
}

/// Colour letter of a token class.
pub fn token_color(k: TokenKind) -> char {
    match k {
        TokenKind::Keyword => 'b',
        TokenKind::Type => 'u',
        TokenKind::Number => 'o',
        TokenKind::Comment => 'a',
        TokenKind::Str => 'G',
        TokenKind::Preproc => 'r',
        TokenKind::Ident => 'K',
        TokenKind::Punct => 'k',
        TokenKind::Space => 'K',
    }
}

/// Split highlighted code into lines of coloured spans (tabs become two spaces).
pub fn colour_lines(lang: Lang, code: &str) -> Vec<Spans> {
    let mut lines: Vec<Spans> = vec![Vec::new()];
    for t in highlight(lang, code) {
        let col = token_color(t.kind);
        for (i, part) in t.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Vec::new());
            }
            if !part.is_empty() {
                let part = part.replace('\t', "  ");
                if let Some(last) = lines.last_mut() {
                    if let Some((s, c)) = last.last_mut() {
                        if *c == col {
                            s.push_str(&part);
                            continue;
                        }
                    }
                    last.push((part, col));
                }
            }
        }
    }
    while lines.last().is_some_and(Vec::is_empty) && lines.len() > 1 {
        lines.pop();
    }
    lines
}

/// Everything the result screens show.
#[derive(Debug)]
pub struct ResultScreen {
    /// The play screen to return to.
    pub play: Box<Play>,
    /// Current stage.
    pub stage: Stage,
    /// The verdict when solved (None for a code peek).
    pub verdict: Option<Verdict>,
    /// Stars and medals were already recorded; this is the best before this run, to detect improvements.
    pub new_best: bool,
    /// Generated code per language (C, C++, Rust).
    pub codes: Vec<CodeText>,
    /// Selected language tab.
    pub lang: usize,
    /// Code scroll (lines, columns).
    pub scroll: (i32, i32),
    /// Ticks since the screen opened.
    pub t: u32,
    /// Level order number.
    pub order: usize,
    art: ClerkArt,
    /// Stage-entry tick (for the code tab slide).
    stage_t: u32,
    /// A next level exists.
    pub has_next: bool,
    /// Sounds already played for stars.
    stars_played: u8,
}

impl ResultScreen {
    /// Build from the play screen. `solved` is true when arriving from a passed Check.
    pub fn new(cx: &mut Ctx<'_>, play: Box<Play>, solved: bool) -> ResultScreen {
        let verdict = play.st.verdict.clone().filter(|v| v.passed && solved);
        let opts = Options {
            stats: verdict.as_ref().map(|v| Stats { size: v.size, avg_steps: v.avg_steps, worst_steps: v.worst_steps }),
            floor_len: Some(play.st.floor_len),
            ..Options::default()
        };
        let mut codes = Vec::new();
        if let Some(p) = &play.st.program {
            for lang in Lang::ALL {
                let g: Generated = codegen::generate(p, lang, play.st.level.as_ref(), &opts);
                let lines = colour_lines(lang, &g.code);
                let width = lines.iter().map(|l| l.iter().map(|(s, _)| s.chars().count()).sum::<usize>()).max().unwrap_or(0);
                codes.push(CodeText {
                    lang,
                    source: g.code.clone(),
                    lines,
                    width,
                    note: g.explain.as_ref().map(|e| (e.algorithm.clone(), e.text.clone())),
                    structured: g.structured,
                });
            }
        }
        let lang = Lang::ALL.iter().position(|l| l.key() == cx.progress.settings.code_lang).unwrap_or(2);
        let order = play.order;
        let has_next = play.st.level.as_ref().is_some_and(|l| cx.campaign.next_after(&l.id).is_some());
        let outfit = outfit_of(cx.progress, cx.bank);
        ResultScreen {
            art: ClerkArt::new(cx.bank, &outfit),
            play,
            stage: if solved { Stage::Victory } else { Stage::Code },
            verdict,
            new_best: false,
            codes,
            lang,
            scroll: (0, 0),
            t: 0,
            order,
            stage_t: 0,
            has_next,
            stars_played: 0,
        }
    }

    fn stars(&self) -> u8 {
        self.verdict.as_ref().map_or(0, |v| v.medals.stars())
    }

    /// The tabs (C, C++, Rust) rectangles, in landscape and portrait.
    pub fn tab_rects(vp: &Viewport) -> [Rect; 3] {
        let s = vp.safe;
        let h = vp.btn_h().min(26);
        let w = if vp.portrait { (s.w - 12) / 3 } else { 70 };
        let y = s.y + 20;
        let x0 = s.x + 6;
        [0, 1, 2].map(|i| Rect::new(x0 + i * (w + 3), y, w, h))
    }

    /// Code area, footer and button rectangles for the code stage: `(code, footer, [copy, back, next])`.
    pub fn code_rects(vp: &Viewport) -> (Rect, Rect, [Rect; 3]) {
        let s = vp.safe;
        let bh = vp.btn_h();
        let tabs_h = vp.btn_h().min(26);
        let foot_h = if vp.portrait { bh * 2 + 70 } else { 62 };
        let code = Rect::new(s.x + 4, s.y + 20 + tabs_h + 3, s.w - 8, s.h - 20 - tabs_h - 3 - foot_h - 6);
        let footer = Rect::new(s.x + 4, code.bottom() + 3, s.w - 8, foot_h);
        let btns = if vp.portrait {
            let w = (footer.w - 8) / 3;
            let y = footer.bottom() - bh - 4;
            [0, 1, 2].map(|i| Rect::new(footer.x + 4 + i * (w + 0), y, w - 3, bh))
        } else {
            let w = 74;
            let y = footer.bottom() - bh - 4;
            [0, 1, 2].map(|i| Rect::new(footer.right() - 4 - (3 - i) * (w + 3) + 3, y, w, bh))
        };
        (code, footer, btns)
    }

    /// Victory stage button rectangles: `[code, next, levels, back]` and the stats panel.
    pub fn victory_rects(vp: &Viewport) -> ([Rect; 4], Rect, Rect) {
        let s = vp.safe;
        let bh = vp.btn_h();
        if vp.portrait {
            let panel = Rect::new(s.x + 6, s.y + 236, s.w - 12, 100);
            let w = (s.w - 12 - 4) / 2;
            let y = panel.bottom() + 6;
            let rects = [
                Rect::new(s.x + 6, y, w, bh),
                Rect::new(s.x + 10 + w, y, w, bh),
                Rect::new(s.x + 6, y + bh + 4, w, bh),
                Rect::new(s.x + 10 + w, y + bh + 4, w, bh),
            ];
            (rects, panel, Rect::new(s.x, s.y, s.w, 232))
        } else {
            let px = s.x + 296;
            let panel = Rect::new(px, s.y + 8, s.right() - px - 8, 108);
            let w = panel.w;
            let y = panel.bottom() + 8;
            let rects = [0, 1, 2, 3].map(|i| Rect::new(px, y + i * (bh + 4), w, bh));
            (rects, panel, Rect::new(s.x, s.y, 292, s.h))
        }
    }

    /// Hit rectangles for layout tests.
    pub fn hits(&self, vp: &Viewport) -> Vec<Hit> {
        let mut v = Vec::new();
        match self.stage {
            Stage::Victory => {
                let (b, panel, scene) = ResultScreen::victory_rects(vp);
                v.push(Hit::area("stats", panel));
                v.push(Hit::area("scene", scene));
                for r in b {
                    v.push(Hit::tap("button", r));
                }
            }
            Stage::Code => {
                let (code, footer, b) = ResultScreen::code_rects(vp);
                v.push(Hit::area("code", code));
                v.push(Hit::area("footer", footer));
                for r in b {
                    v.push(Hit::tap("button", r));
                }
                for r in ResultScreen::tab_rects(vp) {
                    v.push(Hit::tap("tab", r));
                }
            }
        }
        v
    }

    /// Handle input.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        self.t += 1;
        self.stage_t += 1;
        match self.stage {
            Stage::Victory => self.update_victory(cx),
            Stage::Code => self.update_code(cx),
        }
    }

    fn update_victory(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let (b, _, _) = ResultScreen::victory_rects(&cx.vp);
        let stars = self.stars();
        for i in 0..stars {
            if self.t == 36 + u32::from(i) * 30 && self.stars_played <= i {
                self.stars_played = i + 1;
                cx.sfx("sfx_star");
            }
        }
        if self.t == 4 {
            cx.sfx("sfx_correct");
        }
        let ptr = cx.ptr;
        if ptr.tapped_in(&b[0]) || cx.input.ch('c') {
            self.stage = Stage::Code;
            self.stage_t = 0;
            cx.sfx("sfx_button");
        }
        if ptr.tapped_in(&b[1]) || cx.input.key(Key::Enter) {
            cx.sfx("sfx_button");
            return Some(if self.has_next {
                self.play.st.level.as_ref().and_then(|l| cx.campaign.next_after(&l.id)).map_or(Nav::Select, |l| Nav::Brief(l.id.clone()))
            } else {
                Nav::Select
            });
        }
        if ptr.tapped_in(&b[2]) || cx.input.ch('l') {
            cx.sfx("sfx_button");
            return Some(Nav::Select);
        }
        if ptr.tapped_in(&b[3]) || cx.input.key(Key::Escape) {
            cx.sfx("sfx_button");
            return Some(Nav::BackToPlay);
        }
        None
    }

    fn update_code(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        let vp = cx.vp;
        let (code, _, b) = ResultScreen::code_rects(&vp);
        let tabs = ResultScreen::tab_rects(&vp);
        let ptr = cx.ptr;
        for (i, t) in tabs.iter().enumerate() {
            if ptr.tapped_in(t) && self.lang != i {
                self.lang = i;
                self.scroll = (0, 0);
                cx.progress.settings.code_lang = Lang::ALL[i].key().to_string();
                cx.touch_progress();
                cx.sfx("sfx_step");
            }
        }
        let n = Lang::ALL.len();
        if cx.input.key(Key::Left) {
            self.lang = (self.lang + n - 1) % n;
            self.scroll = (0, 0);
        }
        if cx.input.key(Key::Right) || cx.input.key(Key::Tab) {
            self.lang = (self.lang + 1) % n;
            self.scroll = (0, 0);
        }
        let scale = cx.text_scale();
        let line_h = 9 * scale;
        let rows = ((code.h - 6) / line_h).max(1);
        let Some(ct) = self.codes.get(self.lang) else {
            // nothing to show: go back
            return if ptr.tapped_in(&b[1]) || cx.input.key(Key::Escape) { Some(self.back_nav()) } else { None };
        };
        let total = ct.lines.len() as i32;
        let max_y = (total - rows).max(0);
        let cols = ((code.w - 8 - 18) / (6 * scale)).max(1);
        let max_x = (ct.width as i32 - cols).max(0);
        if ptr.over(&code) {
            self.scroll.0 -= (ptr.wheel * 3.0) as i32;
        }
        if ptr.down && ptr.moved && ptr.held_in(&code) {
            self.scroll.0 -= ptr.delta.1 / line_h.max(1) + i32::from(ptr.delta.1 % line_h > line_h / 2);
            self.scroll.1 -= ptr.delta.0 / (6 * scale);
        }
        for k in &cx.input.keys {
            match k {
                Key::Down => self.scroll.0 += 1,
                Key::Up => self.scroll.0 -= 1,
                Key::PageDown => self.scroll.0 += rows - 1,
                Key::PageUp => self.scroll.0 -= rows - 1,
                Key::Home => self.scroll = (0, 0),
                Key::End => self.scroll.0 = max_y,
                _ => {}
            }
        }
        self.scroll.0 = self.scroll.0.clamp(0, max_y);
        self.scroll.1 = self.scroll.1.clamp(0, max_x);
        let copy = cx.input.ctrl_ch('c') || ptr.tapped_in(&b[0]);
        if copy {
            cx.events.push(GameEvent::Copy(ct.source.clone()));
            cx.sfx("sfx_button");
        }
        if ptr.tapped_in(&b[1]) || cx.input.key(Key::Escape) {
            cx.sfx("sfx_button");
            return Some(self.back_nav());
        }
        if ptr.tapped_in(&b[2]) || cx.input.key(Key::Enter) {
            cx.sfx("sfx_button");
            if self.verdict.is_some() && self.has_next {
                return self.play.st.level.as_ref().and_then(|l| cx.campaign.next_after(&l.id)).map(|l| Nav::Brief(l.id.clone()));
            }
            return Some(Nav::Select);
        }
        None
    }

    fn back_nav(&self) -> Nav {
        if self.verdict.is_some() { Nav::Victory } else { Nav::BackToPlay }
    }

    /// Share text for the program (used by the "copy program" button).
    pub fn share_text(&self) -> String {
        let stats = self.verdict.as_ref().map(|v| share::ShareStats { size: v.size, avg_steps: v.avg_steps, worst_steps: v.worst_steps });
        share::export(&self.play.st.id(), &self.play.st.listing, stats)
    }

    // ------------------------------------------------------------------ drawing

    /// Draw the current stage.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        match self.stage {
            Stage::Victory => self.draw_victory(g, v),
            Stage::Code => self.draw_code(g, v),
        }
    }

    fn confetti(&self, g: &mut Gfx<'_>, area: Rect, reduce: bool) {
        if reduce {
            return;
        }
        let cols = ['R', 'Y', 'g', 'B', 'p', 'c', 'y', 'u'];
        let t = self.t as i32;
        for i in 0..70i32 {
            let seed = (i * 7919 + 13) as u32;
            let x = area.x + ((seed.wrapping_mul(2654435761) >> 8) % area.w as u32) as i32;
            let speed = 1 + (seed % 3) as i32;
            let start = -(((seed >> 5) % 120) as i32);
            let y = area.y + ((t + start) * speed / 2).rem_euclid(area.h + 20) - 10;
            let sway = ((t + i * 5) / 8 % 4) as i32 - 1;
            if t + start < 0 || y > area.bottom() {
                continue;
            }
            let c = cols[(i % cols.len() as i32) as usize];
            g.rect(Rect::new(x + sway, y, 3, 2), c);
            if i % 3 == 0 {
                g.px(x + sway + 1, y + 2, c);
            }
        }
    }

    fn draw_victory(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let (btns, panel, scene) = ResultScreen::victory_rects(&vp);
        let reduce = v.progress.settings.reduce_motion;
        // stage backdrop: wall, floor and a spotlight
        g.rect(vp.screen(), 'k');
        for ty in 0..=(scene.h / 16) {
            for tx in 0..=(scene.w / 16) {
                let y = scene.y + ty * 16;
                let key = if y < scene.y + scene.h / 2 {
                    if ty == 0 { "tiles/wall_top" } else if ty < scene.h / 32 - 1 { "tiles/wall_a" } else { "tiles/dado" }
                } else if (tx + ty) % 2 == 0 {
                    "tiles/carpet_a"
                } else {
                    "tiles/carpet_b"
                };
                g.sprite(key, 0, scene.x + tx * 16, y);
            }
        }
        let c = scene.center().0;
        let floor_y = scene.y + scene.h / 2 + 20;
        // title banner
        let slide = ease_out(self.t as f32 / 24.0);
        let title = v.tr.t("result.solved");
        let tw = g.text_w(&title, 2) + 24;
        let by = scene.y + 8 - ((1.0 - slide) * 40.0) as i32;
        g.panel_with(Rect::new(c - tw / 2, by, tw, 26), 'y', 'K', true);
        g.text_c(c, by + 6, &title, 'K', 2);
        if let Some(l) = &self.play.st.level {
            g.text_c(c + 1, by + 31, &clip(&l.title, 34), 'K', 1);
            g.text_c(c, by + 30, &clip(&l.title, 34), 'W', 1);
        }
        // red carpet + clerk cheering at 2x
        g.rect(Rect::new(c - 38, floor_y + 14, 76, 22), 'r');
        g.frame(Rect::new(c - 38, floor_y + 14, 76, 22), 'K');
        g.frame(Rect::new(c - 35, floor_y + 17, 70, 16), 'y');
        let mut clerk = ClerkState::new(c as f32, floor_y as f32);
        clerk.pose = Pose::Cheer;
        clerk.dir = Dir::Down;
        clerk.clock = if reduce { 0 } else { self.t };
        let (frame, _) = clerk.frame(false);
        let hop = if reduce { 0 } else { clerk.hop() * 2 };
        blit_scaled(g, &self.art, frame, c - 16, floor_y + 18 - 40 + hop, 2);
        // stars fill one by one
        let stars = self.stars();
        for i in 0..3u8 {
            let x = c - 3 * 22 / 2 + i32::from(i) * 22 + 2;
            let y = scene.y + 56 + if i == 1 { -4 } else { 0 };
            let reveal_t = 36 + u32::from(i) * 30;
            let on = i < stars && self.t >= reveal_t;
            if on {
                let pop = ((self.t - reveal_t) as f32 / 10.0).min(1.0);
                let off = ((1.0 - ease_out(pop)) * -14.0) as i32;
                g.star(x, y + off, true, true);
                if !reduce && self.t - reveal_t < 16 {
                    g.sprite("fx/sparkle", ((self.t - reveal_t) / 4) as usize % 4, x + 14, y + off - 2);
                    g.sprite("fx/sparkle", ((self.t - reveal_t) / 4 + 2) as usize % 4, x - 6, y + off + 8);
                }
            } else {
                g.star(x, y, false, true);
            }
        }
        self.confetti(g, scene, reduce);
        // stats panel
        g.panel(panel, 'W');
        if let (Some(vd), Some(level)) = (&self.verdict, &self.play.st.level) {
            let lines = [
                (v.tr.t("brief.size"), vd.size.to_string(), vd.medals.size, level.targets.size_par, level.targets.size_ace),
                (v.tr.t("brief.speed"), format!("{:.1}", vd.avg_steps), vd.medals.speed, level.targets.speed_par, level.targets.speed_ace),
            ];
            for (i, (name, value, medal, par, ace)) in lines.iter().enumerate() {
                let y = panel.y + 8 + i as i32 * 38;
                g.text(panel.x + 8, y, &clip(name, 14), 'k');
                g.text_s(panel.x + 8, y + 10, value, 'K', 2);
                let mk = match medal {
                    Some(Medal::Seal) => "ui/medal_seal",
                    Some(Medal::Staple) => "ui/medal_staple",
                    Some(Medal::Clip) => "ui/medal_clip",
                    None => "ui/medal_empty",
                };
                g.sprite(mk, 0, panel.right() - 26, y + 2);
                let tgt = format!("{} {par}  {} {ace}", v.tr.t("brief.par"), v.tr.t("brief.ace"));
                g.text(panel.x + 8 + 12 * 4 + 10, y + 14, &clip(&tgt, ((panel.w - 100) / 6).max(4) as usize), 'a');
            }
            if self.new_best {
                g.chip(Rect::new(panel.x + 8, panel.bottom() - 16, 60, 11), &v.tr.t("result.new_best"), 'y', 'K');
            }
        }
        // buttons
        let ptr = v.ptr;
        let next_label = if self.has_next { v.tr.t("result.next_level") } else { v.tr.t("result.level_list") };
        g.button(btns[0], &v.tr.t("result.show_code"), Kind::Primary, widget_state(ptr, &btns[0], false, self.codes.is_empty()));
        g.button(btns[1], &next_label, Kind::Good, widget_state(ptr, &btns[1], false, false));
        g.button(btns[2], &v.tr.t("result.level_list"), Kind::Normal, widget_state(ptr, &btns[2], false, false));
        g.button(btns[3], &v.tr.t("result.keep_editing"), Kind::Normal, widget_state(ptr, &btns[3], false, false));
        if self.t > 100 && (v.tick / 30) % 2 == 0 {
            g.cursor(btns[0].x - 11, btns[0].y + (btns[0].h - 9) / 2);
        }
    }

    fn draw_code(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        let (code, footer, btns) = ResultScreen::code_rects(&vp);
        let tabs = ResultScreen::tab_rects(&vp);
        let ptr = v.ptr;
        g.rect(vp.screen(), 'k');
        let slide = (1.0 - ease_out(self.stage_t as f32 / 14.0)) * 30.0;
        let off = if v.progress.settings.reduce_motion { 0 } else { slide as i32 };
        let head = Rect::new(vp.safe.x + 3, vp.safe.y + 2, vp.safe.w - 6, 16);
        let title = if self.verdict.is_some() { v.tr.t("result.code_title") } else { v.tr.t("result.code") };
        g.header(head, &title, false);
        if let Some(l) = &self.play.st.level {
            g.text_r(head.right() - 6, head.y + 5, &clip(&l.title, ((head.w - 140) / 6).max(4) as usize), 'y', 1);
        }
        let saved_off = g.off;
        g.off = (0, off);
        for (i, r) in tabs.iter().enumerate() {
            let sel = i == self.lang;
            let kind = if sel { Kind::Primary } else { Kind::Normal };
            g.button(*r, Lang::ALL[i].title(), kind, widget_state(ptr, r, false, false));
        }
        let scale = v.text_scale();
        let line_h = 9 * scale;
        g.round_rect(code, 'W', 'K');
        let inner = code.inset(3);
        let gutter = 18;
        g.rect(Rect::new(inner.x, inner.y, gutter - 2, inner.h), 'w');
        let saved = g.c.set_clip(inner.offset(g.off.0, g.off.1));
        if let Some(ct) = self.codes.get(self.lang) {
            let rows = ((inner.h) / line_h).max(1);
            for (i, line) in ct.lines.iter().enumerate().skip(self.scroll.0 as usize).take(rows as usize + 1) {
                let y = inner.y + (i as i32 - self.scroll.0) * line_h;
                let num = (i + 1).to_string();
                g.tiny(inner.x + gutter - 4 - (num.len() as i32 * 4 - 1), y + 1, &num, 'a');
                let mut x = inner.x + gutter + 2 - self.scroll.1 * 6 * scale;
                for (s, c) in line {
                    g.text_s(x, y, s, *c, scale);
                    x += s.chars().count() as i32 * 6 * scale;
                }
            }
            let total = ct.lines.len() as i32 * line_h;
            g.c.set_clip(saved);
            g.scrollbar(Rect::new(code.right() - 6, code.y + 3, 3, code.h - 6), self.scroll.0 * line_h, total, inner.h);
        } else {
            g.c.set_clip(saved);
            g.text(inner.x + 6, inner.y + 6, &v.tr.t("result.no_code"), 'r');
        }
        // footer: stats, the clerk's note and buttons
        g.panel(footer, 'W');
        let mut y = footer.y + 5;
        if let Some(vd) = &self.verdict {
            let s = format!("{} {}  {} {:.1}  {} {}", v.tr.t("brief.size"), vd.size, v.tr.t("result.speed_avg"), vd.avg_steps, v.tr.t("result.speed_worst"), vd.worst_steps);
            g.text(footer.x + 6, y, &clip(&s, ((footer.w - 12) / 6) as usize), 'K');
            y += 11;
        }
        if let Some(ct) = self.codes.get(self.lang) {
            let note = match &ct.note {
                Some((a, t)) => format!("{}: {}. {}", v.tr.t("result.what_clerk_did"), a.trim_end_matches('.'), t),
                None if !ct.structured => v.tr.t("result.goto_note"),
                None => String::new(),
            };
            let cols = ((footer.w - 12) / 6) as usize;
            let max_lines = ((btns[0].y - y - 2) / 9).max(0) as usize;
            let lines = algoclerk_media::Font::wrap(&note, cols);
            for (i, l) in lines.iter().take(max_lines).enumerate() {
                let text = if i + 1 == max_lines && lines.len() > max_lines { clip(l, cols) } else { l.clone() };
                g.text(footer.x + 6, y + i as i32 * 9, &text, 'k');
            }
        }
        g.button(btns[0], &v.tr.t("result.copy_code"), Kind::Normal, widget_state(ptr, &btns[0], false, self.codes.is_empty()));
        g.button(btns[1], &v.tr.t("result.back"), Kind::Normal, widget_state(ptr, &btns[1], false, false));
        let next = if self.verdict.is_some() && self.has_next { v.tr.t("result.next_level") } else { v.tr.t("result.level_list") };
        g.button(btns[2], &next, Kind::Good, widget_state(ptr, &btns[2], false, false));
        g.off = saved_off;
    }
}

/// Draw one frame of the clerk at an integer scale (hero shots).
pub fn blit_scaled(g: &mut Gfx<'_>, art: &ClerkArt, frame: usize, x: i32, y: i32, scale: i32) {
    let s = art.sprite();
    let f = s.frame(frame);
    for sy in 0..s.h {
        for sx in 0..s.w {
            let ch = f[sy * s.w + sx];
            if ch != b'.' {
                let c = g.col(ch as char);
                g.c.rect_fill(x + sx as i32 * scale + g.off.0, y + sy as i32 * scale + g.off.1, scale, scale, c);
            }
        }
    }
}

/// Bank accessor used by tools.
pub fn bank_of(_b: &ArtBank) {}

/// Re-export of the tap helper for tests.
pub fn tap(r: Rect, min: i32, b: Rect) -> Rect {
    tap_rect(r, min, b)
}

/// State helper for tests.
pub fn idle() -> State {
    State::default()
}
