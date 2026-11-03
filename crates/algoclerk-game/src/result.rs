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
use algoclerk_core::cpp::codegen::{Lang, generate};
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
    /// Read-only support code above the player's (includes and helpers).
    pub prelude: String,
    /// The solution.
    pub code: String,
    /// The translator could not express the program in this language: the friendly reason.
    pub error: Option<String>,
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

    /// There is code to show.
    pub fn available(&self) -> bool {
        self.error.is_none()
    }
}

/// The tabs of the code reveal, in tab order (C, C++, Rust).
///
/// Every tab comes from `algoclerk_core::cpp::codegen::generate` on the accepted program: C and Rust are
/// translations (helper prelude collapsed by default), C++ is the player's own formatted program with the
/// level's includes. A construct a translator cannot express turns into the friendly message of the
/// translator on that tab only.
pub fn translations(session: &Session) -> Vec<Translation> {
    let entry = session.entry_name();
    let compiled = session.compiled.as_ref().ok().or(session.last_ok.as_ref());
    [
        (Lang::C, "C", "c"),
        (Lang::Cpp, "C++", "cpp"),
        (Lang::Rust, "Rust", "rust"),
    ]
    .into_iter()
    .map(|(lang, label, id)| {
        let made = match compiled {
            Some(c) => generate(c, lang, &entry),
            None => Err(words::NOTHING_TO_TRANSLATE.to_string()),
        };
        match made {
            Ok(g) => Translation {
                label,
                id,
                prelude: g.prelude.trim().to_string(),
                code: g.code.trim_end().to_string(),
                error: None,
            },
            Err(e) => Translation {
                label,
                id,
                prelude: String::new(),
                code: String::new(),
                error: Some(e),
            },
        }
    })
    .collect()
}

/// The screen.
pub struct ResultScreen {
    /// The play screen we came from.
    pub play: Box<Play>,
    /// Current stage.
    pub stage: Stage,
    /// Selected language tab (0 C, 1 C++, 2 Rust).
    pub lang: usize,
    /// Vertical scroll in rows.
    pub scroll: i32,
    /// Horizontal scroll in columns (only used when lines are not wrapped).
    pub hscroll: i32,
    /// Wrap long lines (otherwise scroll sideways).
    pub wrap: bool,
    /// Show the helper prelude, per tab.
    pub show_prelude: [bool; 3],
    /// Ticks since the screen opened.
    pub t: u32,
    /// The solution beat the previous best.
    pub new_best: bool,
    code: Vec<Translation>,
    solved: bool,
    drag: (i32, i32),
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
    wrap: Rect,
    copy: Rect,
    back: Rect,
    prelude: Rect,
    note: Rect,
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
    // tab row: C, C++, Rust, wrap toggle, copy
    let row = Rect::new(panel.x + 6, panel.y + 16, panel.w - 12, bh);
    let five = crate::layout::columns(row, 5, 2);
    let tog_h = if vp.touch { 22 } else { 12 };
    // the note under the code may need four lines on a narrow screen (technique and complexity of the C++ tab)
    let note_h = if vp.portrait { 40 } else { 22 };
    let note = Rect::new(panel.x + 6, by - note_h - 2, panel.w - 12, note_h);
    let prelude = Rect::new(panel.x + 6, note.y - tog_h - 1, panel.w - 12, tog_h);
    let top = panel.y + 18 + bh + 2;
    let body = Rect::new(panel.x + 6, top, panel.w - 12, prelude.y - top - 2);
    Layout {
        panel,
        next: cols[0],
        code: cols[1],
        retry: cols[2],
        levels: cols[3],
        tabs: five[..3].to_vec(),
        wrap: five[3],
        copy: five[4],
        back: Rect::new(panel.x + 6, by, bw * 2 + 2, bh),
        prelude,
        note,
        body,
    }
}

/// Width of the line number gutter in pixels for `n` lines.
fn gutter_w(n: usize) -> i32 {
    (n.max(1).to_string().len() as i32 + 1) * 6 + 2
}

/// One row of the code view.
struct DRow {
    pieces: Vec<Piece>,
    /// One-based line number on the first row of a line.
    num: Option<usize>,
    /// The row belongs to the helper prelude.
    pre: bool,
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
        ResultScreen {
            play,
            stage: if victory { Stage::Victory } else { Stage::Code },
            lang,
            scroll: 0,
            hscroll: 0,
            wrap: true,
            show_prelude: [false, true, false],
            t: 0,
            new_best: false,
            code,
            solved,
            drag: (0, 0),
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
                v.push(Hit::tap("wrap", l.wrap));
                v.push(Hit::tap("copy", l.copy));
                v.push(Hit::tap("back", l.back));
                v.push(Hit::area("code body", l.body));
                v.push(Hit::area("code note", l.note));
            }
        }
        v
    }

    /// Columns of code that fit in the body.
    fn code_cols(&self, body: Rect, total_lines: usize) -> usize {
        (((body.w - gutter_w(total_lines) - 10) / 6).max(8)) as usize
    }

    /// The rows to draw for the current tab at the given width, and how many prelude lines there are.
    fn rows(&self, body: Rect) -> (Vec<DRow>, usize) {
        let Some(g) = self.code.get(self.lang) else {
            return (Vec::new(), 0);
        };
        let pre_lines = if g.prelude.trim().is_empty() {
            0
        } else {
            g.prelude.lines().count()
        };
        let show = pre_lines > 0 && self.show_prelude.get(self.lang).copied().unwrap_or(false);
        let mut text = String::new();
        let mut pre_shown = 0;
        if show {
            text.push_str(g.prelude.trim_end());
            text.push_str("\n\n");
            pre_shown = pre_lines + 1;
        }
        text.push_str(&g.code);
        let colored = syntax::lines(&text, syntax::dark, 'W');
        let cols = self.code_cols(body, colored.len());
        let mut rows = Vec::new();
        for (i, line) in colored.into_iter().enumerate() {
            let pre = i < pre_shown;
            if self.wrap {
                for (k, r) in syntax::wrap_line(&line, cols).into_iter().enumerate() {
                    rows.push(DRow {
                        pieces: r,
                        num: (k == 0).then_some(i + 1),
                        pre,
                    });
                }
            } else {
                rows.push(DRow {
                    pieces: line,
                    num: Some(i + 1),
                    pre,
                });
            }
        }
        // the C++ tab shows its includes inline, there is nothing to fold away
        (rows, if self.lang == 1 { 0 } else { pre_lines })
    }

    /// The longest line in columns (scroll mode).
    fn longest(rows: &[DRow]) -> usize {
        rows.iter()
            .map(|r| syntax::width(&r.pieces))
            .max()
            .unwrap_or(0)
    }

    /// The text of the message for a tab that cannot be translated.
    fn error_lines(&self, reason: &str, cols: usize, label: &str) -> Vec<Vec<Piece>> {
        let mut out: Vec<Vec<Piece>> = Vec::new();
        for l in Font::wrap(&format!("{} {label}.", words::CANNOT_TRANSLATE), cols) {
            out.push(vec![(l, 'y')]);
        }
        out.push(Vec::new());
        for l in Font::wrap(reason, cols) {
            out.push(vec![(l, 'W')]);
        }
        out.push(Vec::new());
        for l in Font::wrap(words::STILL_VALID, cols) {
            out.push(vec![(l, 'A')]);
        }
        out
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
                        self.pick_lang(cx, i);
                    }
                }
                if cx.input.key(Key::Tab) {
                    self.pick_lang(cx, (self.lang + 1) % 3);
                }
                for (i, c) in ['1', '2', '3'].into_iter().enumerate() {
                    if cx.input.ch(c) {
                        self.pick_lang(cx, i);
                    }
                }
                if cx.ptr.tapped_in(&l.wrap) || cx.input.ch('w') {
                    self.wrap = !self.wrap;
                    self.hscroll = 0;
                    cx.sfx("button");
                }
                let (rows, _) = self.rows(l.body);
                let view_rows = ((l.body.h - 4) / 9).max(1);
                let max_scroll = (rows.len() as i32 - view_rows).max(0);
                let view_cols = self.code_cols(l.body, rows.len()) as i32;
                let max_h = if self.wrap {
                    0
                } else {
                    (Self::longest(&rows) as i32 - view_cols).max(0)
                };
                if cx.ptr.wheel != 0.0 {
                    let n = -(cx.ptr.wheel.signum() as i32) * 3;
                    if cx.input.shift && !self.wrap {
                        self.hscroll += n * 2;
                    } else {
                        self.scroll += n;
                    }
                }
                if cx.ptr.down && cx.ptr.moved && l.body.contains(cx.ptr.start.0, cx.ptr.start.1) {
                    self.drag.0 += cx.ptr.delta.0;
                    self.drag.1 += cx.ptr.delta.1;
                    let dy = self.drag.1 / 9;
                    self.drag.1 -= dy * 9;
                    self.scroll -= dy;
                    if !self.wrap {
                        let dx = self.drag.0 / 6;
                        self.drag.0 -= dx * 6;
                        self.hscroll -= dx;
                    } else {
                        self.drag.0 = 0;
                    }
                } else if !cx.ptr.down {
                    self.drag = (0, 0);
                }
                if cx.input.key(Key::Down) {
                    self.scroll += 1;
                }
                if cx.input.key(Key::Up) {
                    self.scroll -= 1;
                }
                if cx.input.key(Key::PageDown) {
                    self.scroll += view_rows - 1;
                }
                if cx.input.key(Key::PageUp) {
                    self.scroll -= view_rows - 1;
                }
                if cx.input.key(Key::Right) {
                    self.hscroll += 4;
                }
                if cx.input.key(Key::Left) {
                    self.hscroll -= 4;
                }
                if cx.input.key(Key::Home) {
                    self.hscroll = 0;
                    self.scroll = 0;
                }
                self.scroll = self.scroll.clamp(0, max_scroll);
                self.hscroll = self.hscroll.clamp(0, max_h);
                if cx.ptr.tapped_in(&l.prelude) && self.lang != 1 {
                    if let Some(f) = self.show_prelude.get_mut(self.lang) {
                        *f = !*f;
                    }
                    self.scroll = 0;
                }
                if cx.ptr.tapped_in(&l.copy) || cx.input.ctrl_ch('c') {
                    if let Some(g) = self.code.get(self.lang) {
                        if g.available() {
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

    fn pick_lang(&mut self, cx: &mut Ctx<'_>, i: usize) {
        if i != self.lang {
            self.lang = i;
            self.scroll = 0;
            self.hscroll = 0;
            cx.progress.settings.code_lang = ["c", "cpp", "rust"][i].to_string();
            cx.touch_progress();
            cx.sfx("button");
        }
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
                let bw = ((p.right() - x - 10 - 6) / 2).clamp(40, 118);
                g.round_rect(Rect::new(x, y, bw, 24), 'K', 'k');
                g.text(x + 5, y + 4, &v.tr.t("result.time"), 'a');
                g.text(x + 5, y + 13, &c.time.to_string(), 'y');
                g.round_rect(Rect::new(x + bw + 6, y, bw, 24), 'K', 'k');
                g.text(x + bw + 11, y + 4, &v.tr.t("result.space"), 'a');
                g.text(x + bw + 11, y + 13, &c.space.to_string(), 'y');
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
                .take(5)
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
        g.button(
            l.wrap,
            if self.wrap {
                words::SCROLL
            } else {
                words::WRAP
            },
            Kind::Normal,
            st(&l.wrap),
        );
        g.button(l.copy, &v.tr.t("result.copy"), Kind::Normal, st(&l.copy));
        g.rect(l.body, 'K');
        g.frame(l.body, 'k');
        let tab = self.code.get(self.lang);
        let mut pre_lines = 0;
        if let Some(reason) = tab.and_then(|t| t.error.as_deref()) {
            let cols = ((l.body.w - 12) / 6).max(8) as usize;
            let label = tab.map_or("", |t| t.label);
            let saved = g.c.set_clip(l.body.inset(1).offset(g.off.0, g.off.1));
            for (i, spans) in self.error_lines(reason, cols, label).iter().enumerate() {
                let y = l.body.y + 4 + i as i32 * 9;
                let mut x = l.body.x + 5;
                for (text, class) in spans {
                    g.text(x, y, text, *class);
                    x += text.chars().count() as i32 * 6;
                }
            }
            g.c.set_clip(saved);
        } else {
            let (rows, pl) = self.rows(l.body);
            pre_lines = pl;
            self.draw_rows(g, l, &rows);
        }
        // prelude toggle
        if pre_lines > 0 {
            let open = self.show_prelude.get(self.lang).copied().unwrap_or(false);
            let label = v.tr.format(
                "result.prelude",
                &[
                    ("sign", (if open { "-" } else { "+" }).to_string()),
                    ("n", pre_lines.to_string()),
                    ("s", (if pre_lines == 1 { "" } else { "s" }).to_string()),
                ],
            );
            g.rect(l.prelude, if st(&l.prelude).hover { 'y' } else { 'A' });
            g.text(
                l.prelude.x + 3,
                l.prelude.y + (l.prelude.h - 7) / 2,
                &clip(&label, ((l.prelude.w - 6) / 6) as usize),
                'B',
            );
        }
        self.draw_note(g, l);
        g.button(l.back, &v.tr.t("result.back"), Kind::Normal, st(&l.back));
        let _ = State::default();
    }

    fn draw_rows(&self, g: &mut Gfx<'_>, l: &Layout, rows: &[DRow]) {
        let gw = gutter_w(rows.iter().filter_map(|r| r.num).max().unwrap_or(1));
        let view_rows = ((l.body.h - 4) / 9).max(1);
        let text_x = l.body.x + gw + 3;
        let cols = self.code_cols(l.body, rows.iter().filter_map(|r| r.num).max().unwrap_or(1));
        g.rect(Rect::new(l.body.x + 1, l.body.y + 1, gw, l.body.h - 2), 'k');
        let saved = g.c.set_clip(l.body.inset(1).offset(g.off.0, g.off.1));
        for (i, row) in rows
            .iter()
            .enumerate()
            .skip(self.scroll as usize)
            .take(view_rows as usize + 1)
        {
            let y = l.body.y + 3 + (i as i32 - self.scroll) * 9;
            if let Some(n) = row.num {
                let t = n.to_string();
                g.text(
                    l.body.x + 1 + gw - 4 - t.len() as i32 * 6,
                    y,
                    &t,
                    if row.pre { 'm' } else { 'a' },
                );
            }
            let mut skip = if self.wrap { 0 } else { self.hscroll as usize };
            let mut x = text_x;
            let mut used = 0usize;
            for (text, class) in &row.pieces {
                let n = text.chars().count();
                if skip >= n {
                    skip -= n;
                    continue;
                }
                let t: String = text.chars().skip(skip).take(cols - used).collect();
                skip = 0;
                if t.is_empty() {
                    break;
                }
                let col = if row.pre { 'A' } else { *class };
                g.text(x, y, &t, col);
                x += t.chars().count() as i32 * 6;
                used += t.chars().count();
                if used >= cols {
                    break;
                }
            }
        }
        g.c.set_clip(saved);
        g.scrollbar(
            Rect::new(l.body.right() - 4, l.body.y + 1, 3, l.body.h - 2),
            self.scroll * 9,
            rows.len() as i32 * 9,
            l.body.h - 4,
        );
        if !self.wrap {
            let longest = Self::longest(rows).max(1) as i32;
            if longest > cols as i32 {
                let track = Rect::new(text_x, l.body.bottom() - 4, l.body.right() - text_x - 6, 2);
                g.rect(track, 'k');
                let thumb_w = (track.w * cols as i32 / longest).max(6);
                let thumb_x =
                    track.x + (track.w - thumb_w) * self.hscroll / (longest - cols as i32).max(1);
                g.rect(Rect::new(thumb_x, track.y, thumb_w, 2), 'a');
            }
        }
    }

    /// The line under the code: what this tab is, plus the technique and Big-O of the level.
    fn draw_note(&self, g: &mut Gfx<'_>, l: &Layout) {
        let cols = ((l.note.w - 2) / 6).max(8) as usize;
        let what = match self.code.get(self.lang).map(|t| t.id) {
            Some("c") => words::TAB_C,
            Some("rust") => words::TAB_RUST,
            _ => words::TAB_CPP,
        };
        let mut text = what.to_string();
        if self.lang == 1 {
            if let Some(lv) = &self.play.session.level {
                text = format!(
                    "{} {}: {} ({}).",
                    text,
                    words::TECHNIQUE,
                    lv.technique,
                    lv.big_o
                );
            }
        }
        for (i, line) in Font::wrap(&text, cols)
            .iter()
            .take(((l.note.h - 2) / 10).max(1) as usize)
            .enumerate()
        {
            g.text(l.note.x + 1, l.note.y + 2 + i as i32 * 10, line, 'k');
        }
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
