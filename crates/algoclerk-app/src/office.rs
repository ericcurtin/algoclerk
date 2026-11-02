// SPDX-License-Identifier: Apache-2.0
//! The office scene: inbox belt, clerk, hand, floor tiles and outbox belt, drawn procedurally.

use crate::theme::Palette;
use algoclerk_core::vm::{Effect, StepReport};
use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Ui, Vec2, pos2, vec2,
};
use std::collections::HashMap;

/// The clerk's mood.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Neutral,
    Happy,
    Sad,
}

/// Everything the scene needs to draw one frame.
pub struct OfficeInput<'a> {
    pub inbox_left: &'a [i32],
    pub outbox: &'a [i32],
    pub expected: &'a [i32],
    pub floor: &'a [Option<i32>],
    pub names: &'a [String],
    pub hand: Option<i32>,
    pub char_mode: bool,
    pub bubble: Option<&'a str>,
    pub mood: Mood,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Focus {
    Home,
    Inbox,
    Outbox,
    Tile(usize),
}

/// Animation state of the scene.
#[derive(Debug)]
pub struct OfficeView {
    hand: Option<Pos2>,
    focus: Focus,
    focus_left: f32,
    in_shift: f32,
    out_shift: f32,
    flashes: HashMap<usize, (f32, bool)>,
    time: f32,
}

impl Default for OfficeView {
    fn default() -> Self {
        OfficeView {
            hand: None,
            focus: Focus::Home,
            focus_left: 0.0,
            in_shift: 0.0,
            out_shift: 0.0,
            flashes: HashMap::new(),
            time: 0.0,
        }
    }
}

struct Layout {
    inbox_belt: Rect,
    outbox_belt: Rect,
    clerk: Pos2,
    home: Pos2,
    tiles: Vec<Rect>,
    crate_size: f32,
    unit: f32,
}

fn layout(rect: Rect, floor_len: usize) -> Layout {
    let w = rect.width();
    let h = rect.height();
    let unit = (w / 640.0).min(h / 400.0).clamp(0.45, 1.6);
    let crate_size = (40.0 * unit).clamp(22.0, 56.0);
    let top_h = (h * 0.44).max(130.0).min(h * 0.6);
    let belt_h = crate_size * 1.25;
    let belt_y = rect.top() + top_h * 0.28;
    let inbox_belt =
        Rect::from_min_size(pos2(rect.left() + w * 0.03, belt_y), vec2(w * 0.35, belt_h));
    let outbox_belt = Rect::from_min_size(
        pos2(rect.right() - w * 0.38, belt_y),
        vec2(w * 0.35, belt_h),
    );
    let clerk = pos2(rect.center().x, rect.top() + top_h * 0.62);
    let home = pos2(clerk.x, clerk.y + 70.0 * unit);

    // Floor tiles below the top zone.
    let area = Rect::from_min_max(
        pos2(rect.left() + 8.0, rect.top() + top_h + 14.0),
        pos2(rect.right() - 8.0, rect.bottom() - 6.0),
    );
    let n = floor_len.max(1);
    let gap = 4.0;
    let mut t = (54.0 * unit).clamp(26.0, 64.0);
    let (cols, rows) = loop {
        let cols = (((area.width() + gap) / (t + gap)).floor() as usize).clamp(1, n);
        let rows = n.div_ceil(cols);
        if (rows as f32) * (t + gap + 10.0) <= area.height() || t <= 20.0 {
            break (cols, rows);
        }
        t -= 2.0;
    };
    let grid_w = cols as f32 * (t + gap) - gap;
    let x0 = area.center().x - grid_w / 2.0;
    let tiles = (0..n)
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            Rect::from_min_size(
                pos2(
                    x0 + c as f32 * (t + gap),
                    area.top() + r as f32 * (t + gap + 10.0),
                ),
                vec2(t, t),
            )
        })
        .collect();
    let _ = rows;
    Layout {
        inbox_belt,
        outbox_belt,
        clerk,
        home,
        tiles,
        crate_size,
        unit,
    }
}

fn value_text(v: i32, char_mode: bool) -> String {
    if char_mode
        && let Some(c) = u32::try_from(v)
            .ok()
            .and_then(char::from_u32)
            .filter(|c| !c.is_control() && *c != ' ')
    {
        return c.to_string();
    }
    v.to_string()
}

fn draw_crate(
    p: &Painter,
    center: Pos2,
    size: f32,
    text: &str,
    pal: &Palette,
    edge: Option<Color32>,
) {
    let r = Rect::from_center_size(center, Vec2::splat(size));
    p.rect_filled(r, size * 0.12, pal.crate_fill);
    p.rect_stroke(
        r,
        size * 0.12,
        Stroke::new(2.0, edge.unwrap_or(pal.crate_edge)),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [pos2(r.left(), center.y), pos2(r.right(), center.y)],
        Stroke::new(1.0, pal.crate_edge.gamma_multiply(0.5)),
    );
    let fs = (size * if text.chars().count() > 3 { 0.34 } else { 0.5 }).max(9.0);
    let ink = if pal.crate_fill.r() > 150 {
        Color32::from_rgb(30, 22, 12)
    } else {
        Color32::WHITE
    };
    p.text(
        center,
        Align2::CENTER_CENTER,
        text,
        FontId::monospace(fs),
        ink,
    );
}

impl OfficeView {
    /// Forget animation state (new run).
    pub fn reset(&mut self) {
        *self = OfficeView::default();
    }

    /// React to an executed step: move the hand, flash tiles, slide belts.
    pub fn on_report(&mut self, report: &StepReport, steps_per_sec: f32) {
        let hold = (0.55 / steps_per_sec.max(0.5)).min(0.7);
        for e in &report.effects {
            match e {
                Effect::InboxTake(_) => {
                    self.focus = Focus::Inbox;
                    self.focus_left = hold;
                    self.in_shift = 1.0;
                }
                Effect::OutboxPut(_) => {
                    self.focus = Focus::Outbox;
                    self.focus_left = hold;
                    self.out_shift = 1.0;
                }
                Effect::FloorWrite { slot, .. } => {
                    self.focus = Focus::Tile(*slot);
                    self.focus_left = hold;
                    self.flashes.insert(*slot, (1.0, true));
                }
                Effect::FloorRead(slot) => {
                    self.focus = Focus::Tile(*slot);
                    self.focus_left = hold;
                    self.flashes.entry(*slot).or_insert((1.0, false));
                }
                _ => {}
            }
        }
    }

    /// Draw the scene into `rect`.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        input: &OfficeInput<'_>,
        pal: &Palette,
        dt: f32,
        steps_per_sec: f32,
    ) {
        let lay = layout(rect, input.floor.len());
        self.time += dt;
        // Advance animation.
        self.focus_left -= dt;
        if self.focus_left <= 0.0 {
            self.focus = Focus::Home;
        }
        let target = match self.focus {
            Focus::Home => lay.home,
            Focus::Inbox => pos2(
                lay.inbox_belt.right() - lay.crate_size * 0.6,
                lay.inbox_belt.center().y,
            ),
            Focus::Outbox => pos2(
                lay.outbox_belt.left() + lay.crate_size * 0.6,
                lay.outbox_belt.center().y,
            ),
            Focus::Tile(i) => lay.tiles.get(i).map_or(lay.home, |t| t.center()),
        };
        let rate = (7.0 + steps_per_sec * 2.0).min(60.0);
        let cur = self.hand.unwrap_or(lay.home);
        let k = 1.0 - (-rate * dt).exp();
        self.hand = Some(cur + (target - cur) * k);
        self.in_shift = (self.in_shift - dt * (2.0 + steps_per_sec)).max(0.0);
        self.out_shift = (self.out_shift - dt * (2.0 + steps_per_sec)).max(0.0);
        for (t, _) in self.flashes.values_mut() {
            *t -= dt * 1.5;
        }
        self.flashes.retain(|_, (t, _)| *t > 0.0);

        let p = ui.painter_at(rect);
        // Background.
        let top_h = lay.clerk.y - rect.top() + 100.0 * lay.unit;
        p.rect_filled(rect, 0.0, pal.floor);
        p.rect_filled(
            Rect::from_min_max(
                rect.min,
                pos2(rect.right(), rect.top() + top_h.min(rect.height() * 0.6)),
            ),
            0.0,
            pal.wall,
        );

        // Belts.
        self.draw_belt(&p, lay.inbox_belt, pal);
        self.draw_belt(&p, lay.outbox_belt, pal);
        let label_font = FontId::proportional((12.0 * lay.unit).clamp(10.0, 15.0));
        p.text(
            pos2(lay.inbox_belt.left(), lay.inbox_belt.top() - 4.0),
            Align2::LEFT_BOTTOM,
            "INBOX",
            label_font.clone(),
            pal.dim,
        );
        p.text(
            pos2(lay.outbox_belt.right(), lay.outbox_belt.top() - 4.0),
            Align2::RIGHT_BOTTOM,
            "OUTBOX",
            label_font.clone(),
            pal.dim,
        );

        // Inbox crates: front crate at the right end (nearest the clerk).
        let cs = lay.crate_size;
        let spacing = cs * 1.12;
        let max_visible = ((lay.inbox_belt.width() / spacing) as usize).max(1);
        for (i, v) in input.inbox_left.iter().take(max_visible).enumerate() {
            let x = lay.inbox_belt.right() - cs * 0.6 - (i as f32 - self.in_shift) * spacing;
            if x > lay.inbox_belt.left() - cs {
                let c = pos2(x, lay.inbox_belt.center().y);
                draw_crate(&p, c, cs * 0.9, &value_text(*v, input.char_mode), pal, None);
            }
        }
        if input.inbox_left.len() > max_visible {
            p.text(
                pos2(lay.inbox_belt.left() + 2.0, lay.inbox_belt.bottom() + 10.0),
                Align2::LEFT_CENTER,
                format!("+{}", input.inbox_left.len() - max_visible),
                label_font.clone(),
                pal.dim,
            );
        }
        // Outbox crates: the newest next to the clerk, older ones pushed outwards.
        let out_visible = ((lay.outbox_belt.width() / spacing) as usize).max(1);
        for (age, (idx, v)) in input
            .outbox
            .iter()
            .enumerate()
            .rev()
            .take(out_visible)
            .enumerate()
        {
            let x = lay.outbox_belt.left() + cs * 0.6 + (age as f32 + self.out_shift) * spacing;
            if x < lay.outbox_belt.right() + cs * 0.3 {
                let ok = input.expected.get(idx).is_some_and(|e| e == v);
                let edge = if ok { pal.good } else { pal.bad };
                draw_crate(
                    &p,
                    pos2(x, lay.outbox_belt.center().y),
                    cs * 0.9,
                    &value_text(*v, input.char_mode),
                    pal,
                    Some(edge),
                );
                // A tick or cross under the crate (never colour alone).
                let m = pos2(x, lay.outbox_belt.bottom() + cs * 0.3);
                crate::widgets::draw_icon(
                    &p,
                    if ok {
                        crate::widgets::Icon::Check
                    } else {
                        crate::widgets::Icon::Cross
                    },
                    Rect::from_center_size(m, Vec2::splat(cs * 0.35)),
                    edge,
                );
            }
        }
        // Expected output line.
        let exp: Vec<String> = input
            .expected
            .iter()
            .map(|v| value_text(*v, input.char_mode))
            .collect();
        let exp_text = format!(
            "expected: {}",
            if exp.is_empty() {
                "-".to_string()
            } else {
                exp.join(" ")
            }
        );
        p.text(
            pos2(
                lay.outbox_belt.right(),
                lay.outbox_belt.bottom() + cs * 0.75,
            ),
            Align2::RIGHT_TOP,
            truncate(
                &exp_text,
                ((lay.outbox_belt.width() / (7.0 * lay.unit.max(0.8))) as usize).max(12),
            ),
            FontId::monospace((11.0 * lay.unit).clamp(9.0, 13.0)),
            pal.dim,
        );

        // Clerk and desk.
        self.draw_clerk(&p, &lay, input.mood, pal);

        // Floor tiles.
        let tile_font = FontId::monospace(
            (lay.tiles.first().map_or(30.0, |t| t.width()) * 0.38).clamp(9.0, 22.0),
        );
        let small = FontId::proportional(
            (lay.tiles.first().map_or(30.0, |t| t.width()) * 0.24).clamp(8.0, 12.0),
        );
        for (i, r) in lay.tiles.iter().enumerate() {
            let flash = self.flashes.get(&i).copied();
            let fill = match flash {
                Some((t, true)) => pal.tile.lerp_to_gamma(pal.highlight, t.clamp(0.0, 1.0)),
                Some((t, false)) => pal
                    .tile
                    .lerp_to_gamma(pal.accent, (t * 0.6).clamp(0.0, 1.0)),
                None => pal.tile,
            };
            p.rect_filled(*r, 5.0, fill);
            p.rect_stroke(
                *r,
                5.0,
                Stroke::new(
                    if flash.is_some() { 2.5 } else { 1.5 },
                    if flash.is_some() {
                        pal.accent
                    } else {
                        pal.tile_edge
                    },
                ),
                egui::StrokeKind::Inside,
            );
            match input.floor.get(i).copied().flatten() {
                Some(v) => {
                    let ink = if matches!(flash, Some((_, true))) && !pal.dark {
                        Color32::BLACK
                    } else {
                        pal.text
                    };
                    p.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        value_text(v, input.char_mode),
                        tile_font.clone(),
                        ink,
                    )
                }
                None => p.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    "-",
                    tile_font.clone(),
                    pal.dim.gamma_multiply(0.5),
                ),
            };
            let name = input
                .names
                .get(i)
                .map(String::as_str)
                .filter(|s| !s.is_empty());
            let caption = match name {
                Some(n) if r.width() >= 40.0 => {
                    format!("{i} {}", truncate(n, ((r.width() / 5.5) as usize).max(3)))
                }
                _ => i.to_string(),
            };
            p.text(
                pos2(r.center().x, r.bottom() + 1.0),
                Align2::CENTER_TOP,
                caption,
                small.clone(),
                pal.dim,
            );
        }

        // Arm and hand with its crate on top.
        let hand = self.hand.unwrap_or(lay.home);
        let shoulder = pos2(lay.clerk.x, lay.clerk.y + 38.0 * lay.unit);
        p.line_segment([shoulder, hand], Stroke::new(8.0 * lay.unit, pal.suit));
        p.circle_filled(hand, 7.0 * lay.unit, pal.skin);
        if let Some(v) = input.hand {
            draw_crate(
                &p,
                hand + vec2(0.0, -cs * 0.55),
                cs * 0.9,
                &value_text(v, input.char_mode),
                pal,
                None,
            );
        }

        if let Some(msg) = input.bubble {
            self.draw_bubble(&p, rect, &lay, msg, pal);
        }
    }

    fn draw_belt(&self, p: &Painter, r: Rect, pal: &Palette) {
        p.rect_filled(r, r.height() * 0.2, pal.belt);
        let step = r.height() * 0.5;
        let offset = (self.time * 20.0) % step;
        let mut x = r.left() - step + offset;
        while x < r.right() {
            if x > r.left() {
                p.line_segment(
                    [pos2(x, r.top() + 3.0), pos2(x, r.bottom() - 3.0)],
                    Stroke::new(1.5, pal.belt_stripe),
                );
            }
            x += step;
        }
        p.rect_stroke(
            r,
            r.height() * 0.2,
            Stroke::new(2.0, pal.belt_stripe),
            egui::StrokeKind::Inside,
        );
    }

    fn draw_clerk(&self, p: &Painter, lay: &Layout, mood: Mood, pal: &Palette) {
        let u = lay.unit;
        let c = lay.clerk;
        // Desk.
        let desk = Rect::from_center_size(pos2(c.x, c.y + 62.0 * u), vec2(170.0 * u, 26.0 * u));
        p.rect_filled(desk, 4.0, pal.desk);
        p.rect_stroke(
            desk,
            4.0,
            Stroke::new(2.0, pal.crate_edge),
            egui::StrokeKind::Inside,
        );
        // Body.
        let body = Rect::from_center_size(pos2(c.x, c.y + 28.0 * u), vec2(64.0 * u, 54.0 * u));
        p.rect_filled(body, 10.0 * u, pal.suit);
        p.add(egui::Shape::convex_polygon(
            vec![
                pos2(c.x - 6.0 * u, c.y + 6.0 * u),
                pos2(c.x + 6.0 * u, c.y + 6.0 * u),
                pos2(c.x + 4.0 * u, c.y + 42.0 * u),
                pos2(c.x, c.y + 48.0 * u),
                pos2(c.x - 4.0 * u, c.y + 42.0 * u),
            ],
            pal.tie,
            Stroke::NONE,
        ));
        // Head.
        let head = pos2(c.x, c.y - 8.0 * u);
        p.circle_filled(head, 26.0 * u, pal.skin);
        p.circle_stroke(
            head,
            26.0 * u,
            Stroke::new(2.0, pal.crate_edge.gamma_multiply(0.8)),
        );
        // Hair / cap.
        p.add(egui::Shape::convex_polygon(
            vec![
                pos2(c.x - 26.0 * u, head.y - 4.0 * u),
                pos2(c.x - 18.0 * u, head.y - 24.0 * u),
                pos2(c.x + 18.0 * u, head.y - 24.0 * u),
                pos2(c.x + 26.0 * u, head.y - 4.0 * u),
                pos2(c.x, head.y - 14.0 * u),
            ],
            pal.crate_edge,
            Stroke::NONE,
        ));
        // Glasses, eyes (blink every few seconds).
        let blink = (self.time % 4.0) > 3.85;
        for dx in [-10.0, 10.0] {
            let e = pos2(c.x + dx * u, head.y);
            p.circle_stroke(
                e,
                8.0 * u,
                Stroke::new(1.5 * u.max(0.8), Color32::from_gray(40)),
            );
            if blink {
                p.line_segment(
                    [e - vec2(4.0 * u, 0.0), e + vec2(4.0 * u, 0.0)],
                    Stroke::new(2.0, Color32::from_gray(40)),
                );
            } else {
                p.circle_filled(e, 2.6 * u, Color32::from_gray(30));
            }
        }
        p.line_segment(
            [pos2(c.x - 2.0 * u, head.y), pos2(c.x + 2.0 * u, head.y)],
            Stroke::new(1.5, Color32::from_gray(40)),
        );
        // Mouth.
        let my = head.y + 15.0 * u;
        let mouth: Vec<Pos2> = (0..=8)
            .map(|i| {
                let t = i as f32 / 8.0 * 2.0 - 1.0;
                let curve = match mood {
                    Mood::Happy => 5.0 * (1.0 - t * t),
                    Mood::Sad => -5.0 * (1.0 - t * t),
                    Mood::Neutral => 0.0,
                };
                pos2(c.x + t * 8.0 * u, my + curve * u)
            })
            .collect();
        p.add(egui::Shape::line(
            mouth,
            Stroke::new(2.0, Color32::from_gray(60)),
        ));
    }

    fn draw_bubble(&self, p: &Painter, rect: Rect, lay: &Layout, msg: &str, pal: &Palette) {
        let font = FontId::proportional((13.0 * lay.unit).clamp(11.0, 17.0));
        let max_w = (rect.width() * 0.5).clamp(160.0, 380.0);
        let galley = p.layout(msg.to_string(), font, Color32::from_gray(20), max_w);
        let size = galley.size() + vec2(20.0, 14.0);
        let mut tl = pos2(
            lay.clerk.x + 38.0 * lay.unit,
            lay.clerk.y - 60.0 * lay.unit - size.y * 0.3,
        );
        if tl.x + size.x > rect.right() - 4.0 {
            tl.x = rect.right() - 4.0 - size.x;
        }
        if tl.y < rect.top() + 2.0 {
            tl.y = rect.top() + 2.0;
        }
        let r = Rect::from_min_size(tl, size);
        p.rect_filled(r, 10.0, Color32::from_rgb(255, 250, 235));
        p.rect_stroke(r, 10.0, Stroke::new(2.0, pal.bad), egui::StrokeKind::Inside);
        p.galley(r.min + vec2(10.0, 7.0), galley, Color32::from_gray(20));
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('~');
        t
    }
}
