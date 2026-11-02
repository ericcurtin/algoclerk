// SPDX-License-Identifier: Apache-2.0
//! Victory (stars, medals, measured complexity) and the code reveal (C, C++ and Rust tabs).

use crate::gfx::{Gfx, Kind, State, clip};
use crate::input::Key;
use crate::layout::{Hit, Viewport};
use crate::play::Play;
use crate::session::Session;
use crate::syntax::{self, Piece};
use crate::ui::{Ctx, GameEvent, Nav, View, widget_state};
use crate::words;
use algoclerk_core::judge::Medal;
use algoclerk_media::{Font, Rect};

/// Which part of the result is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Stars and stats.
    Victory,
    /// Generated code.
    Code,
}

/// The solution in one language, for the code reveal.
#[derive(Debug, Clone)]
pub struct Translation {
    /// Tab label.
    pub label: &'static str,
    /// Short id (`c`, `cpp`, `rust`), the value stored in the settings.
    pub id: &'static str,
    /// Read-only support code above the player's (the level prelude for C++).
    pub prelude: String,
    /// The solution.
    pub code: String,
    /// The translation is not available yet: the tab shows a friendly message instead of code.
    pub pending: bool,
}

impl Translation {
    /// Prelude and code as one text.
    pub fn full(&self) -> String {
        if self.prelude.trim().is_empty() {
            self.code.clone()
        } else {
            format!("{}\n\n{}", self.prelude.trim_end(), self.code)
        }
    }
}

/// The tabs of the code reveal, in tab order (C, C++, Rust).
///
/// C++ is the player's own program. The C and Rust translators live in the code generation package: until
/// they are connected here the tabs are marked `pending` and the screen says that the translation is coming.
/// Connect them by filling `code` (and `prelude`) and clearing `pending`.
pub fn translations(session: &Session) -> Vec<Translation> {
    let prelude = session
        .level
        .as_ref()
        .map(|l| l.prelude.trim().to_string())
        .unwrap_or_default();
    let pending = |label, id| Translation {
        label,
        id,
        prelude: String::new(),
        code: String::new(),
        pending: true,
    };
    vec![
        pending("C", "c"),
        Translation {
            label: "C++",
            id: "cpp",
            prelude,
            code: session.text(),
            pending: false,
        },
        pending("Rust", "rust"),
    ]
}

/// The screen.
pub struct ResultScreen {
    /// The play screen we came from.
    pub play: Box<Play>,
    /// Current stage.
    pub stage: Stage,
    /// Selected language tab (0 C, 1 C++, 2 Rust).
    pub lang: usize,
    /// Scroll in lines.
    pub scroll: i32,
    /// Show the helper prelude.
    pub show_prelude: bool,
    /// Ticks since the screen opened.
    pub t: u32,
    /// The solution beat the previous best.
    pub new_best: bool,
    code: Vec<Translation>,
    solved: bool,
}

fn medal_sprite(m: Medal) -> &'static str {
    match m {
        Medal::None => "ui/medal_empty",
        Medal::Bronze => "ui/medal_clip",
        Medal::Silver => "ui/medal_staple",
        Medal::Gold => "ui/medal_seal",
    }
}

struct Layout {
    panel: Rect,
    next: Rect,
    code: Rect,
    retry: Rect,
    levels: Rect,
    tabs: Vec<Rect>,
    copy: Rect,
    back: Rect,
    prelude: Rect,
    body: Rect,
}

fn layout(vp: &Viewport) -> Layout {
    let bh = vp.btn_h();
    let panel = Rect::new(8, 8, vp.w - 16, vp.h - 16);
    let by = panel.bottom() - bh - 6;
    let bw = (panel.w - 20) / 4;
    let cols: Vec<Rect> = (0..4)
        .map(|i| Rect::new(panel.x + 6 + i * (bw + 2), by, bw, bh))
        .collect();
    let tabs: Vec<Rect> = (0..3)
        .map(|i| Rect::new(panel.x + 6 + i * 62, panel.y + 16, 60, bh))
        .collect();
    let body = Rect::new(
        panel.x + 6,
        panel.y + 18 + bh + 2,
        panel.w - 12,
        by - (panel.y + 18 + bh + 2) - 14,
    );
    Layout {
        panel,
        next: cols[0],
        code: cols[1],
        retry: cols[2],
        levels: cols[3],
        tabs,
        copy: Rect::new(panel.right() - 66, panel.y + 16, 60, bh),
        back: cols[0],
        prelude: Rect::new(panel.x + 6, by - 12, 120, 11),
        body,
    }
}

impl ResultScreen {
    /// Build from the play screen. `victory` selects the stage.
    pub fn new(cx: &mut Ctx<'_>, play: Box<Play>, victory: bool) -> ResultScreen {
        let code = translations(&play.session);
        let lang = match cx.progress.settings.code_lang.as_str() {
            "c" => 0,
            "cpp" => 1,
            _ => 2,
        };
        let solved = play.solved;
        let lang = if code.get(lang).is_some_and(|t| t.pending) {
            1
        } else {
            lang
        };
        ResultScreen {
            play,
            stage: if victory { Stage::Victory } else { Stage::Code },
            lang,
            scroll: 0,
            show_prelude: false,
            t: 0,
            new_best: false,
            code,
            solved,
        }
    }

    /// Hit rectangles for layout tests.
    pub fn hits(&self, vp: &Viewport) -> Vec<Hit> {
        let l = layout(vp);
        let mut v = vec![Hit::area("panel", l.panel)];
        match self.stage {
            Stage::Victory => {
                v.push(Hit::tap("next", l.next));
                v.push(Hit::tap("code", l.code));
                v.push(Hit::tap("retry", l.retry));
                v.push(Hit::tap("levels", l.levels));
            }
            Stage::Code => {
                for (i, t) in l.tabs.iter().enumerate() {
                    v.push(Hit::tap(["tab c", "tab cpp", "tab rust"][i], *t));
                }
                v.push(Hit::tap("copy", l.copy));
                v.push(Hit::tap("back", l.back));
                v.push(Hit::area("code body", l.body));
            }
        }
        v
    }

    fn text_for(&self, cols: usize) -> (Vec<Vec<Piece>>, usize) {
        let Some(g) = self.code.get(self.lang) else {
            return (Vec::new(), 0);
        };
        if g.pending {
            return (
                Font::wrap(words::TRANSLATION_COMING, cols.max(10))
                    .into_iter()
                    .map(|l| vec![(l, 'A')])
                    .collect(),
                0,
            );
        }
        let pre_lines = g.prelude.lines().count();
        let mut text = String::new();
        if self.show_prelude {
            text.push_str(&g.prelude);
            text.push('\n');
        }
        text.push_str(&g.code);
        (syntax::lines(&text, syntax::dark, 'W'), pre_lines)
    }

    /// Handle input.
    pub fn update(&mut self, cx: &mut Ctx<'_>) -> Option<Nav> {
        self.t += 1;
        let vp = cx.vp;
        let l = layout(&vp);
        match self.stage {
            Stage::Victory => {
                if cx.ptr.tapped_in(&l.next) || cx.input.key(Key::Enter) {
                    return Some(match self.next_id(cx) {
                        Some(id) => Nav::Brief(id),
                        None => Nav::Select,
                    });
                }
                if cx.ptr.tapped_in(&l.code) || cx.input.ch('c') {
                    self.stage = Stage::Code;
                    self.scroll = 0;
                    cx.sfx("button");
                }
                if cx.ptr.tapped_in(&l.retry) {
                    return Some(Nav::BackToPlay);
                }
                if cx.ptr.tapped_in(&l.levels) || cx.input.key(Key::Escape) {
                    return Some(Nav::Select);
                }
            }
            Stage::Code => {
                for (i, t) in l.tabs.iter().enumerate() {
                    if cx.ptr.tapped_in(t) {
                        self.lang = i;
                        self.scroll = 0;
                        cx.progress.settings.code_lang = ["c", "cpp", "rust"][i].to_string();
                        cx.touch_progress();
                        cx.sfx("button");
                    }
                }
                if cx.input.key(Key::Right) {
                    self.lang = (self.lang + 1) % 3;
                    self.scroll = 0;
                }
                if cx.input.key(Key::Left) {
                    self.lang = (self.lang + 2) % 3;
                    self.scroll = 0;
                }
                let (lines, _) = self.text_for(((l.body.w - 8) / 6) as usize);
                let rows = (l.body.h / 9).max(1);
                let max_scroll = (lines.len() as i32 - rows).max(0);
                if cx.ptr.wheel != 0.0 {
                    self.scroll += -(cx.ptr.wheel.signum() as i32) * 3;
                }
                if cx.ptr.down && cx.ptr.moved && l.body.contains(cx.ptr.start.0, cx.ptr.start.1) {
                    self.scroll -= cx.ptr.delta.1 / 3 + cx.ptr.delta.1.signum();
                }
                if cx.input.key(Key::Down) {
                    self.scroll += 1;
                }
                if cx.input.key(Key::Up) {
                    self.scroll -= 1;
                }
                self.scroll = self.scroll.clamp(0, max_scroll);
                if cx.ptr.tapped_in(&l.prelude) {
                    self.show_prelude = !self.show_prelude;
                    self.scroll = 0;
                }
                if cx.ptr.tapped_in(&l.copy) || cx.input.ctrl_ch('c') {
                    if let Some(g) = self.code.get(self.lang) {
                        if !g.pending {
                            cx.events.push(GameEvent::Copy(g.full()));
                        }
                        cx.sfx("button");
                    }
                }
                if cx.ptr.tapped_in(&l.back) || cx.input.key(Key::Escape) {
                    return Some(if self.solved {
                        Nav::Victory
                    } else {
                        Nav::BackToPlay
                    });
                }
            }
        }
        None
    }

    fn next_id(&self, cx: &Ctx<'_>) -> Option<String> {
        let id = self.play.session.level.as_ref()?.id.clone();
        cx.campaign.next_after(&id).map(|l| l.id.clone())
    }

    /// Draw the screen.
    pub fn draw(&self, g: &mut Gfx<'_>, v: &View<'_>) {
        let vp = v.vp;
        g.rect(vp.screen(), 'k');
        let l = layout(&vp);
        g.panel(l.panel, 'W');
        if self.stage == Stage::Victory && !v.progress.settings.reduce_motion {
            // slow confetti drifting down inside the memo
            for i in 0..46u32 {
                let x =
                    l.panel.x + 4 + ((i * 131 + i * i * 17 + 29) % (l.panel.w as u32 - 8)) as i32;
                let y = l.panel.y
                    + 16
                    + ((i * 89 + (i * i) % 23 * 7 + self.t * (1 + i % 3)) % (l.panel.h as u32 - 40))
                        as i32;
                g.rect(
                    Rect::new(x, y, 2, 3),
                    ['R', 'y', 'g', 'B', 'p', 'c'][(i % 6) as usize],
                );
            }
        }
        match self.stage {
            Stage::Victory => self.draw_victory(g, v, &l),
            Stage::Code => self.draw_code(g, v, &l),
        }
    }

    fn draw_victory(&self, g: &mut Gfx<'_>, v: &View<'_>, l: &Layout) {
        let vp = v.vp;
        let p = l.panel;
        g.header(Rect::new(p.x, p.y, p.w, 14), &v.tr.t("result.solved"), true);
        let verdict = self.play.session.verdict.as_ref();
        let medals = verdict.map(|x| x.medals).unwrap_or_default();
        let stars_on = (self.t / 18).min(u32::from(medals.stars())) as usize;
        let sx = p.x + p.w / 2 - 36;
        for i in 0..3 {
            g.star(
                sx + i as i32 * 26,
                p.y + 20 + if i == 1 { -2 } else { 2 },
                i < stars_on,
                true,
            );
        }
        // the clerk cheers
        let cx = if vp.portrait { p.x + 70 } else { p.x + 60 };
        let cy = if vp.portrait { p.y + 130 } else { p.y + 150 };
        let (frame, _) = {
            let mut c = crate::clerk::ClerkState::new(0.0, 0.0);
            c.pose = crate::clerk::Pose::Cheer;
            c.clock = self.t;
            c.frame(!v.progress.settings.reduce_motion)
        };
        let art = crate::clerk::ClerkArt::new(g.bank, &crate::play::outfit_of(v.progress, g.bank));
        g.round_rect(Rect::new(cx - 34, cy - 8, 68, 16), 'n', 'N');
        g.rect(Rect::new(cx - 32, cy - 6, 64, 3), 'd');
        blit_scaled(
            g,
            &art,
            frame,
            cx - 24,
            cy - 8 - 60 + if (self.t / 10) % 2 == 1 { -4 } else { 0 },
            3,
        );
        // stats
        let x = if vp.portrait { p.x + 10 } else { p.x + 130 };
        let mut y = if vp.portrait { p.y + 156 } else { p.y + 52 };
        let name = self
            .play
            .session
            .level
            .as_ref()
            .map_or(String::new(), |l| l.title.clone());
        g.text(x, y, &clip(&name, ((p.right() - x - 8) / 6) as usize), 'B');
        y += 14;
        if let Some(vd) = verdict {
            for (label, medal, value) in [
                (
                    v.tr.t("result.size"),
                    medals.size,
                    v.tr.format("result.nodes", &[("n", vd.size.to_string())]),
                ),
                (
                    v.tr.t("result.speed"),
                    medals.speed,
                    v.tr.format("result.cost", &[("n", format!("{:.0}", vd.avg_cost))]),
                ),
            ] {
                g.sprite(medal_sprite(medal), 0, x, y - 3);
                g.text(x + 20, y, &label, 'K');
                g.text(x + 62, y, &value, 'k');
                g.text(
                    x + 130,
                    y,
                    &v.tr.t(match medal {
                        Medal::Gold => "result.medal_gold",
                        Medal::Silver => "result.medal_silver",
                        _ => "result.medal_bronze",
                    }),
                    if medal == Medal::Gold { 'Y' } else { 'a' },
                );
                y += 16;
            }
            if let Some(c) = &vd.complexity {
                g.text(x, y, &v.tr.t("result.measured"), 'k');
                y += 11;
                g.round_rect(Rect::new(x, y, 118, 24), 'K', 'k');
                g.text(x + 5, y + 4, &v.tr.t("result.time"), 'a');
                g.text(x + 5, y + 13, &c.time.to_string(), 'y');
                g.round_rect(Rect::new(x + 124, y, 118, 24), 'K', 'k');
                g.text(x + 129, y + 4, &v.tr.t("result.space"), 'a');
                g.text(x + 129, y + 13, &c.space.to_string(), 'y');
                y += 30;
            }
            if let Some(l) = &self.play.session.level {
                g.text(x, y, &v.tr.t("result.clerk_did"), 'k');
                y += 10;
                for line in Font::wrap(
                    &format!("{} ({})", l.technique, l.big_o),
                    ((p.right() - x - 8) / 6) as usize,
                )
                .iter()
                .take(3)
                {
                    g.text(x, y, line, 'K');
                    y += 9;
                }
            }
        }
        let solved = v
            .campaign
            .all_levels()
            .filter(|lv| v.progress.record(&lv.id).solved)
            .count();
        let total = v.campaign.level_count();
        let tl = v.tr.format(
            "result.totals",
            &[
                ("solved", solved.to_string()),
                ("total", total.to_string()),
                ("stars", v.progress.total_stars().to_string()),
            ],
        );
        g.text_c(p.x + p.w / 2, l.next.y - 14, &tl, 'k', 1);
        if self.new_best {
            g.text_r(p.right() - 8, p.y + 20, &v.tr.t("result.new_best"), 'R', 1);
        }
        let st = |r: &Rect| widget_state(v.ptr, r, false, false);
        let has_next = self
            .play
            .session
            .level
            .as_ref()
            .is_some_and(|lv| v.campaign.next_after(&lv.id).is_some());
        g.button(
            l.next,
            &if has_next {
                v.tr.t("result.next")
            } else {
                v.tr.t("result.levels")
            },
            Kind::Primary,
            st(&l.next),
        );
        g.button(l.code, &v.tr.t("result.code"), Kind::Good, st(&l.code));
        g.button(l.retry, &v.tr.t("result.retry"), Kind::Normal, st(&l.retry));
        g.button(
            l.levels,
            &v.tr.t("result.menu"),
            Kind::Normal,
            st(&l.levels),
        );
    }

    fn draw_code(&self, g: &mut Gfx<'_>, v: &View<'_>, l: &Layout) {
        let p = l.panel;
        g.header(
            Rect::new(p.x, p.y, p.w, 14),
            &v.tr.t("result.real_code"),
            true,
        );
        let st = |r: &Rect| widget_state(v.ptr, r, false, false);
        for (i, t) in l.tabs.iter().enumerate() {
            g.button(
                *t,
                self.code.get(i).map_or("", |t| t.label),
                if self.lang == i {
                    Kind::On
                } else {
                    Kind::Normal
                },
                st(t),
            );
        }
        g.button(l.copy, &v.tr.t("result.copy"), Kind::Normal, st(&l.copy));
        g.rect(l.body, 'K');
        g.frame(l.body, 'k');
        let (lines, pre_lines) = self.text_for(((l.body.w - 8) / 6) as usize);
        let saved = g.c.set_clip(l.body.inset(1).offset(g.off.0, g.off.1));
        let rows = (l.body.h / 9).max(1);
        let cols = ((l.body.w - 8) / 6).max(10) as usize;
        for (i, spans) in lines
            .iter()
            .enumerate()
            .skip(self.scroll as usize)
            .take(rows as usize + 1)
        {
            let y = l.body.y + 2 + (i as i32 - self.scroll) * 9;
            let mut x = l.body.x + 3;
            let mut used = 0usize;
            for (text, class) in spans {
                let col = *class;
                let room = cols.saturating_sub(used);
                if room == 0 {
                    break;
                }
                let t: String = text.chars().take(room).collect();
                g.text(x, y, &t, col);
                x += t.chars().count() as i32 * 6;
                used += t.chars().count();
            }
        }
        g.c.set_clip(saved);
        g.scrollbar(
            Rect::new(l.body.right() - 4, l.body.y + 1, 3, l.body.h - 2),
            self.scroll * 9,
            lines.len() as i32 * 9,
            l.body.h,
        );
        // prelude toggle
        if pre_lines > 0 {
            g.text(
                l.prelude.x,
                l.prelude.y + 2,
                &v.tr.format(
                    "result.prelude",
                    &[
                        (
                            "sign",
                            (if self.show_prelude { "-" } else { "+" }).to_string(),
                        ),
                        ("n", pre_lines.to_string()),
                        ("s", (if pre_lines == 1 { "" } else { "s" }).to_string()),
                    ],
                ),
                'B',
            );
        }
        g.button(l.back, &v.tr.t("result.back"), Kind::Normal, st(&l.back));
        let hint = v.tr.t("result.same_program");
        g.text(
            l.code.x,
            l.code.y + (l.code.h - 7) / 2,
            &clip(&hint, ((l.panel.right() - l.code.x - 6) / 6) as usize),
            'k',
        );
        let _ = State::default();
    }
}

/// Draw one frame of the clerk at an integer scale (hero shots).
pub fn blit_scaled(
    g: &mut Gfx<'_>,
    art: &crate::clerk::ClerkArt,
    frame: usize,
    x: i32,
    y: i32,
    scale: i32,
) {
    let s = art.sprite();
    let f = s.frame(frame);
    for sy in 0..s.h {
        for sx in 0..s.w {
            let ch = f[sy * s.w + sx];
            if ch != b'.' {
                let c = g.col(ch as char);
                g.c.rect_fill(
                    x + sx as i32 * scale + g.off.0,
                    y + sy as i32 * scale + g.off.1,
                    scale,
                    scale,
                    c,
                );
            }
        }
    }
}
