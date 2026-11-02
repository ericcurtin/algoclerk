// SPDX-License-Identifier: Apache-2.0
//! The level brief: a cream paper memo with an ink border (title, flavour text, task, examples drawn as
//! crates, constraints, the instruction set as icons, size/speed medal targets and hints pinned as sticky
//! notes). The body scrolls with the wheel, with a finger drag and with the arrow keys.

use crate::gfx::{Gfx, Kind, TEXT_H};
use crate::input::{Key, Pointer};
use crate::office::draw_crate_sprite;
use crate::ui::{Ctx, View, widget_state};
use algoclerk_core::isa::Opcode;
use algoclerk_core::level::{Display, Level};
use algoclerk_core::progress::LevelRecord;
use algoclerk_media::{Font, Rect};

/// A piece of memo content.
#[derive(Debug, Clone)]
enum Block {
    Heading(String),
    Title(String),
    Para(String, char),
    Bullets(Vec<String>),
    Crates { label: String, values: Vec<i32>, char_mode: bool },
    Ops(Vec<Opcode>, bool),
    Targets(Box<TargetsBlock>),
    Sticky(String),
    Gap(i32),
}

#[derive(Debug, Clone)]
struct TargetsBlock {
    size_par: u32,
    size_ace: u32,
    speed_par: u32,
    speed_ace: u32,
    best_size: Option<u32>,
    best_speed: Option<f64>,
    medal_size: Option<algoclerk_core::judge::Medal>,
    medal_speed: Option<algoclerk_core::judge::Medal>,
}

/// Memo state (scroll position).
#[derive(Debug, Clone, Default)]
pub struct Memo {
    /// Scroll offset in pixels.
    pub scroll: i32,
    content_h: i32,
    view_h: i32,
}

/// What the memo footer asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoAction {
    /// Nothing.
    None,
    /// The primary button (Start shift / Close).
    Primary,
    /// Pin the next hint.
    Hint,
    /// Ask to reveal the reference solution.
    Reveal,
}

/// Rectangles of a memo within `r`: `(header, body, footer, buttons...)`.
#[derive(Debug, Clone)]
pub struct MemoRects {
    /// Title ribbon.
    pub header: Rect,
    /// Scrolling paper.
    pub body: Rect,
    /// Footer strip.
    pub footer: Rect,
    /// Hint button.
    pub hint: Rect,
    /// Spoiler button.
    pub reveal: Rect,
    /// Primary button.
    pub primary: Rect,
}

impl Memo {
    /// Rectangles for a memo drawn in `r`.
    pub fn rects(r: Rect, btn_h: i32, portrait: bool) -> MemoRects {
        let header = Rect::new(r.x + 4, r.y + 4, r.w - 8, 16);
        let foot_h = if portrait { btn_h * 2 + 10 } else { btn_h + 8 };
        let footer = Rect::new(r.x + 4, r.bottom() - 4 - foot_h, r.w - 8, foot_h);
        let body = Rect::new(r.x + 6, header.bottom() + 3, r.w - 12, footer.y - header.bottom() - 6);
        let inner = footer.inset(2);
        if portrait {
            let half = (inner.w - 4) / 2;
            let hint = Rect::new(inner.x, inner.y, half, btn_h);
            let reveal = Rect::new(inner.x + half + 4, inner.y, half, btn_h);
            let primary = Rect::new(inner.x, inner.y + btn_h + 4, inner.w, btn_h);
            MemoRects { header, body, footer, hint, reveal, primary }
        } else {
            let w3 = (inner.w - 8) / 3;
            let hint = Rect::new(inner.x, inner.y, w3, btn_h);
            let reveal = Rect::new(inner.x + w3 + 4, inner.y, w3, btn_h);
            let primary = Rect::new(inner.x + 2 * (w3 + 4), inner.y, w3, btn_h);
            MemoRects { header, body, footer, hint, reveal, primary }
        }
    }

    fn blocks(level: &Level, v: &View<'_>, hints_shown: usize, order: usize) -> Vec<Block> {
        let tr = v.tr;
        let rec = v.progress.record(&level.id);
        let char_mode = level.display == Display::Char;
        let mut b = vec![Block::Title(format!("{:02} {}", order, level.title)), Block::Para(level.summary.clone(), 'k'), Block::Gap(4)];
        if !level.story.trim().is_empty() {
            b.push(Block::Para(level.story.trim().to_string(), 'K'));
            b.push(Block::Gap(4));
        }
        b.push(Block::Heading(tr.t("brief.task")));
        b.push(Block::Para(level.statement.trim().to_string(), 'K'));
        if !level.constraints.is_empty() {
            b.push(Block::Heading(tr.t("brief.constraints")));
            b.push(Block::Bullets(level.constraints.clone()));
        }
        if !level.examples.is_empty() {
            b.push(Block::Heading(tr.t("brief.examples")));
            for (i, e) in level.examples.iter().enumerate() {
                b.push(Block::Crates {
                    label: format!("{} {}", tr.t("brief.example"), i + 1),
                    values: e.input.to_vec(),
                    char_mode,
                });
                b.push(Block::Crates { label: tr.t("brief.outbox"), values: e.output.to_vec(), char_mode });
                if !e.note.trim().is_empty() {
                    b.push(Block::Para(e.note.trim().to_string(), 'a'));
                }
                b.push(Block::Gap(3));
            }
        }
        b.push(Block::Heading(tr.t("brief.tools")));
        let allowed = level.allowed();
        if let Ok(a) = &allowed {
            b.push(Block::Ops(a.opcodes(), a.allows_indirect()));
        }
        b.push(Block::Para(format!("{} {}", tr.t("brief.tiles"), level.machine.floor), 'K'));
        b.push(Block::Heading(tr.t("brief.targets")));
        b.push(Block::Targets(Box::new(TargetsBlock {
            size_par: level.targets.size_par,
            size_ace: level.targets.size_ace,
            speed_par: level.targets.speed_par,
            speed_ace: level.targets.speed_ace,
            best_size: rec.best_size,
            best_speed: rec.best_speed,
            medal_size: rec.medals.size,
            medal_speed: rec.medals.speed,
        })));
        b.push(Block::Heading(tr.t("brief.hints")));
        if hints_shown == 0 {
            b.push(Block::Para(tr.t("brief.no_hints"), 'a'));
        }
        for h in level.hints.iter().take(hints_shown) {
            b.push(Block::Sticky(h.clone()));
        }
        b
    }

    fn block_height(f: &Block, width: i32, scale: i32) -> i32 {
        let cols = ((width / (6 * scale)).max(1)) as usize;
        let lh = 9 * scale + 2;
        match f {
            Block::Title(t) => {
                let c = ((width / 12).max(1)) as usize;
                Font::wrap(t, c).len() as i32 * 16 + 2
            }
            Block::Heading(_) => 7 * scale + 9,
            Block::Para(t, _) => Font::wrap(t, cols).len() as i32 * lh + 3,
            Block::Bullets(items) => items.iter().map(|t| Font::wrap(t, cols.saturating_sub(2).max(1)).len() as i32 * lh).sum::<i32>() + 3,
            Block::Crates { values, .. } => {
                let per_row = ((width - 6 * 6 * scale.min(1) - 52) / 17).max(1);
                let rows = ((values.len().min(24)) as i32 + per_row - 1) / per_row.max(1);
                (rows.max(1) * 18).max(18) + 2
            }
            Block::Ops(ops, ind) => {
                let per_row = (width / 62).max(1);
                let rows = (ops.len() as i32 + per_row - 1) / per_row;
                rows * 14 + i32::from(*ind) * lh + 4
            }
            Block::Targets(_) => 2 * 20 + 6,
            Block::Sticky(t) => {
                let c = ((width - 24) / (6 * scale)).max(1) as usize;
                Font::wrap(t, c).len() as i32 * lh + 12
            }
            Block::Gap(h) => *h,
        }
    }

    /// Update scrolling and the footer buttons.
    pub fn update(&mut self, cx: &mut Ctx<'_>, r: Rect, hints_left: bool, can_reveal: bool) -> MemoAction {
        let rects = Memo::rects(r, cx.vp.btn_h(), cx.vp.portrait);
        let ptr = cx.ptr;
        let max = (self.content_h - self.view_h).max(0);
        if ptr.over(&rects.body) {
            self.scroll -= (ptr.wheel * 24.0) as i32;
        }
        if ptr.down && ptr.moved && ptr.held_in(&rects.body) {
            self.scroll -= ptr.delta.1;
        }
        for k in &cx.input.keys {
            match k {
                Key::Down => self.scroll += 16,
                Key::Up => self.scroll -= 16,
                Key::PageDown => self.scroll += self.view_h - 20,
                Key::PageUp => self.scroll -= self.view_h - 20,
                Key::Home => self.scroll = 0,
                Key::End => self.scroll = max,
                _ => {}
            }
        }
        self.scroll = self.scroll.clamp(0, max);
        if ptr.tapped_in(&rects.primary) || cx.input.key(Key::Enter) {
            return MemoAction::Primary;
        }
        if ptr.tapped_in(&rects.hint) && hints_left || cx.input.ch('h') && hints_left {
            return MemoAction::Hint;
        }
        if ptr.tapped_in(&rects.reveal) && can_reveal || cx.input.ch('s') && can_reveal {
            return MemoAction::Reveal;
        }
        MemoAction::None
    }

    /// Draw the memo into `r`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        g: &mut Gfx<'_>,
        v: &View<'_>,
        r: Rect,
        level: &Level,
        order: usize,
        hints_shown: usize,
        primary_label: &str,
        rec: &LevelRecord,
    ) {
        let _ = rec;
        let rects = Memo::rects(r, v.vp.btn_h(), v.vp.portrait);
        g.panel(r, 'W');
        g.header(rects.header, &v.tr.t("brief.heading"), false);
        // paper body
        g.rect(rects.body, 'W');
        g.frame(rects.body, 'w');
        let scale = v.text_scale();
        let inner = Rect::new(rects.body.x + 6, rects.body.y + 4, rects.body.w - 14, rects.body.h - 8);
        let blocks = Memo::blocks(level, v, hints_shown, order);
        self.view_h = inner.h;
        self.content_h = blocks.iter().map(|b| Memo::block_height(b, inner.w, scale)).sum::<i32>() + 4;
        let max = (self.content_h - self.view_h).max(0);
        self.scroll = self.scroll.clamp(0, max);
        let saved = g.c.set_clip(inner.offset(g.off.0, g.off.1));
        let mut y = inner.y - self.scroll;
        let cols = ((inner.w / (6 * scale)).max(1)) as usize;
        let lh = 9 * scale + 2;
        for b in &blocks {
            let h = Memo::block_height(b, inner.w, scale);
            if y + h >= inner.y && y <= inner.bottom() {
                Memo::draw_block(g, v, b, Rect::new(inner.x, y, inner.w, h), cols, scale, lh);
            }
            y += h;
        }
        g.c.set_clip(saved);
        g.scrollbar(Rect::new(rects.body.right() - 6, rects.body.y + 2, 4, rects.body.h - 4), self.scroll, self.content_h, self.view_h);
        // footer
        let hints_left = hints_shown < level.hints.len();
        let ptr = v.ptr;
        let label = format!("{} {}/{}", v.tr.t("brief.hint"), hints_shown, level.hints.len());
        g.button(rects.hint, &label, Kind::Normal, widget_state(ptr, &rects.hint, false, !hints_left));
        g.button(rects.reveal, &v.tr.t("brief.spoiler"), Kind::Normal, widget_state(ptr, &rects.reveal, false, level.solution.is_none()));
        g.button(rects.primary, primary_label, Kind::Primary, widget_state(ptr, &rects.primary, false, false));
    }

    fn draw_block(g: &mut Gfx<'_>, v: &View<'_>, b: &Block, r: Rect, cols: usize, scale: i32, lh: i32) {
        match b {
            Block::Title(t) => {
                let c = ((r.w / 12).max(1)) as usize;
                for (i, l) in Font::wrap(t, c).iter().enumerate() {
                    g.text_shadow(r.x, r.y + i as i32 * 16, l, 'K', 'y', 2);
                }
            }
            Block::Heading(t) => {
                let y = r.y + 3;
                g.rect(Rect::new(r.x, y + 7 * scale + 1, r.w, 2), 'y');
                g.text_s(r.x, y, &t.to_uppercase(), 'b', scale);
            }
            Block::Para(t, ink) => {
                for (i, l) in Font::wrap(t, cols).iter().enumerate() {
                    g.text_s(r.x, r.y + 1 + i as i32 * lh, l, *ink, scale);
                }
            }
            Block::Bullets(items) => {
                let mut y = r.y + 1;
                for t in items {
                    let lines = Font::wrap(t, cols.saturating_sub(2).max(1));
                    g.text_s(r.x, y, "*", 'R', scale);
                    for (i, l) in lines.iter().enumerate() {
                        g.text_s(r.x + 12 * scale, y + i as i32 * lh, l, 'K', scale);
                    }
                    y += lines.len() as i32 * lh;
                }
            }
            Block::Crates { label, values, char_mode } => {
                g.text(r.x, r.y + 5, &crate::gfx::clip(label, 9), 'k');
                let x0 = r.x + 56;
                let per_row = ((r.w - 6 * 6 * scale.min(1) - 52) / 17).max(1);
                for (i, val) in values.iter().take(24).enumerate() {
                    let (cx, cy) = (x0 + (i as i32 % per_row) * 17, r.y + (i as i32 / per_row) * 18);
                    draw_crate_sprite(g, cx, cy + 1, *val, *char_mode);
                }
                if values.is_empty() {
                    g.text(x0, r.y + 5, "-", 'a');
                }
                if values.len() > 24 {
                    g.text(x0 + per_row * 17, r.y + 5, "...", 'a');
                }
            }
            Block::Ops(ops, ind) => {
                let per_row = (r.w / 62).max(1);
                for (i, op) in ops.iter().enumerate() {
                    let (cx, cy) = (r.x + (i as i32 % per_row) * 62, r.y + (i as i32 / per_row) * 14);
                    g.sprite(&format!("icons/op_{}", op.key()), 0, cx, cy);
                    g.text(cx + 14, cy + 3, op.mnemonic(), 'K');
                }
                if *ind {
                    let rows = (ops.len() as i32 + per_row - 1) / per_row;
                    g.text_s(r.x, r.y + rows * 14 + 2, &v.tr.t("brief.indirect"), 'a', 1);
                }
            }
            Block::Targets(t) => {
                for (row, (name, par, ace, best, medal)) in [
                    (v.tr.t("brief.size"), t.size_par, t.size_ace, t.best_size.map(|x| x.to_string()), t.medal_size),
                    (v.tr.t("brief.speed"), t.speed_par, t.speed_ace, t.best_speed.map(|x| format!("{x:.1}")), t.medal_speed),
                ]
                .into_iter()
                .enumerate()
                {
                    let y = r.y + row as i32 * 20;
                    g.text(r.x, y + 5, &crate::gfx::clip(&name, 12), 'K');
                    let mx = r.x + 78;
                    use algoclerk_core::judge::Medal;
                    let slots = [(Medal::Clip, "ui/medal_clip", v.tr.t("brief.solved")), (Medal::Staple, "ui/medal_staple", format!("<={par}")), (Medal::Seal, "ui/medal_seal", format!("<={ace}"))];
                    for (i, (m, key, cap)) in slots.iter().enumerate() {
                        let x = mx + i as i32 * 44;
                        let earned = medal.is_some_and(|e| e >= *m);
                        g.sprite(if earned { key } else { "ui/medal_empty" }, 0, x, y);
                        g.text(x + 18, y + 5, &crate::gfx::clip(cap, 4), if earned { 'K' } else { 'a' });
                    }
                    if let Some(b) = best {
                        g.text_r(r.right() - 2, y + 5, &format!("{} {b}", v.tr.t("brief.best")), 'b', 1);
                    }
                }
            }
            Block::Sticky(t) => {
                let note = Rect::new(r.x + 4, r.y + 2, r.w - 8, r.h - 4);
                g.rect(note.offset(1, 1), 'E');
                g.rect(note, 'y');
                g.hline(note.x, note.bottom() - 1, note.w, 'Y');
                g.sprite("icons/pin", 0, note.x + note.w / 2 - 5, note.y - 3);
                let c = ((note.w - 8) / (6 * scale)).max(1) as usize;
                for (i, l) in Font::wrap(t, c).iter().enumerate() {
                    g.text_s(note.x + 4, note.y + 9 + i as i32 * lh, l, 'K', scale);
                }
            }
            Block::Gap(_) => {}
        }
        let _ = TEXT_H;
    }
}

/// Is the pointer-driven memo scrolled by this tick? (helper for tests)
pub fn wheel_scrolls(ptr: &Pointer, r: &Rect) -> bool {
    ptr.over(r) && ptr.wheel != 0.0
}
