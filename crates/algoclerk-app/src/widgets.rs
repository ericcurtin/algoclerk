// SPDX-License-Identifier: Apache-2.0
//! Small drawing helpers: vector icons and icon buttons (no image assets, no emoji fonts).

use crate::theme::Palette;
use eframe::egui::{
    self, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2,
};

/// Icons drawn with painter shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Step,
    Back,
    Reset,
    Trash,
    Up,
    Down,
    Grip,
    Check,
    Cross,
    Star,
    Lock,
    Home,
    Copy,
    Plus,
    Minus,
    Eye,
    Next,
}

fn poly(p: &egui::Painter, pts: Vec<Pos2>, fill: Color32) {
    p.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
}

/// Draw an icon inside `rect`.
pub fn draw_icon(p: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    let s = Stroke::new((r * 0.22).max(1.5), color);
    let pt = |dx: f32, dy: f32| pos2(c.x + dx * r, c.y + dy * r);
    match icon {
        Icon::Play => poly(
            p,
            vec![pt(-0.45, -0.6), pt(0.65, 0.0), pt(-0.45, 0.6)],
            color,
        ),
        Icon::Pause => {
            p.rect_filled(
                Rect::from_min_max(pt(-0.55, -0.6), pt(-0.15, 0.6)),
                1.0,
                color,
            );
            p.rect_filled(
                Rect::from_min_max(pt(0.15, -0.6), pt(0.55, 0.6)),
                1.0,
                color,
            );
        }
        Icon::Step => {
            poly(
                p,
                vec![pt(-0.6, -0.55), pt(0.35, 0.0), pt(-0.6, 0.55)],
                color,
            );
            p.rect_filled(
                Rect::from_min_max(pt(0.45, -0.55), pt(0.65, 0.55)),
                1.0,
                color,
            );
        }
        Icon::Back => {
            poly(
                p,
                vec![pt(0.6, -0.55), pt(-0.35, 0.0), pt(0.6, 0.55)],
                color,
            );
            p.rect_filled(
                Rect::from_min_max(pt(-0.65, -0.55), pt(-0.45, 0.55)),
                1.0,
                color,
            );
        }
        Icon::Reset => {
            p.add(Shape::line(
                (0..=14)
                    .map(|i| {
                        let a = 0.6 + (i as f32 / 14.0) * 4.6;
                        pt(a.cos() * 0.6, a.sin() * 0.6)
                    })
                    .collect(),
                s,
            ));
            poly(
                p,
                vec![pt(0.35, -0.75), pt(0.85, -0.35), pt(0.2, -0.25)],
                color,
            );
        }
        Icon::Trash => {
            p.rect_filled(
                Rect::from_min_max(pt(-0.5, -0.35), pt(0.5, 0.7)),
                2.0,
                color,
            );
            p.rect_filled(
                Rect::from_min_max(pt(-0.65, -0.6), pt(0.65, -0.42)),
                1.0,
                color,
            );
            p.rect_filled(
                Rect::from_min_max(pt(-0.2, -0.78), pt(0.2, -0.6)),
                1.0,
                color,
            );
        }
        Icon::Up => poly(p, vec![pt(-0.6, 0.35), pt(0.0, -0.4), pt(0.6, 0.35)], color),
        Icon::Down => poly(
            p,
            vec![pt(-0.6, -0.35), pt(0.0, 0.4), pt(0.6, -0.35)],
            color,
        ),
        Icon::Grip => {
            for dy in [-0.5, 0.0, 0.5] {
                for dx in [-0.3, 0.3] {
                    p.circle_filled(pt(dx, dy), r * 0.14, color);
                }
            }
        }
        Icon::Check => {
            p.add(Shape::line(
                vec![pt(-0.6, 0.05), pt(-0.15, 0.5), pt(0.65, -0.5)],
                s,
            ));
        }
        Icon::Cross => {
            p.line_segment([pt(-0.5, -0.5), pt(0.5, 0.5)], s);
            p.line_segment([pt(-0.5, 0.5), pt(0.5, -0.5)], s);
        }
        Icon::Star => {
            let tau5 = std::f32::consts::TAU / 5.0;
            let outer: Vec<Pos2> = (0..5)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * tau5;
                    pt(a.cos() * 0.9, a.sin() * 0.9)
                })
                .collect();
            let inner: Vec<Pos2> = (0..5)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * tau5 + tau5 / 2.0;
                    pt(a.cos() * 0.4, a.sin() * 0.4)
                })
                .collect();
            poly(p, inner.clone(), color);
            for i in 0..5 {
                poly(p, vec![inner[(i + 4) % 5], outer[i], inner[i]], color);
            }
        }
        Icon::Lock => {
            p.rect_filled(
                Rect::from_min_max(pt(-0.55, -0.05), pt(0.55, 0.75)),
                2.0,
                color,
            );
            p.add(Shape::line(
                (0..=10)
                    .map(|i| {
                        let a = std::f32::consts::PI + (i as f32 / 10.0) * std::f32::consts::PI;
                        pt(a.cos() * 0.35, -0.05 + a.sin() * 0.45)
                    })
                    .collect(),
                s,
            ));
        }
        Icon::Home => {
            poly(
                p,
                vec![pt(-0.8, -0.05), pt(0.0, -0.8), pt(0.8, -0.05)],
                color,
            );
            p.rect_filled(
                Rect::from_min_max(pt(-0.55, -0.05), pt(0.55, 0.7)),
                1.0,
                color,
            );
        }
        Icon::Copy => {
            p.rect_stroke(
                Rect::from_min_max(pt(-0.7, -0.4), pt(0.2, 0.7)),
                2.0,
                s,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_min_max(pt(-0.2, -0.7), pt(0.7, 0.4)),
                2.0,
                s,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Plus => {
            p.line_segment([pt(-0.6, 0.0), pt(0.6, 0.0)], s);
            p.line_segment([pt(0.0, -0.6), pt(0.0, 0.6)], s);
        }
        Icon::Minus => {
            p.line_segment([pt(-0.6, 0.0), pt(0.6, 0.0)], s);
        }
        Icon::Eye => {
            p.add(Shape::line(
                (0..=12)
                    .map(|i| {
                        let t = i as f32 / 12.0 * 2.0 - 1.0;
                        pt(t * 0.85, -0.45 * (1.0 - t * t))
                    })
                    .collect(),
                s,
            ));
            p.add(Shape::line(
                (0..=12)
                    .map(|i| {
                        let t = i as f32 / 12.0 * 2.0 - 1.0;
                        pt(t * 0.85, 0.45 * (1.0 - t * t))
                    })
                    .collect(),
                s,
            ));
            p.circle_filled(pt(0.0, 0.0), r * 0.2, color);
        }
        Icon::Next => {
            poly(
                p,
                vec![pt(-0.7, -0.55), pt(0.0, 0.0), pt(-0.7, 0.55)],
                color,
            );
            poly(p, vec![pt(0.0, -0.55), pt(0.7, 0.0), pt(0.0, 0.55)], color);
        }
    }
}

/// A square icon button. Returns the response (use `.clicked()`).
pub fn icon_button(ui: &mut Ui, icon: Icon, pal: &Palette, enabled: bool, tip: &str) -> Response {
    let size = ui.spacing().interact_size.y.max(30.0);
    icon_button_sized(ui, icon, pal, enabled, tip, size)
}

/// An icon button with an explicit size.
pub fn icon_button_sized(
    ui: &mut Ui,
    icon: Icon,
    pal: &Palette,
    enabled: bool,
    tip: &str,
    size: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(size, size),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hovered = resp.hovered() && enabled;
    let fill = if resp.is_pointer_button_down_on() && enabled {
        pal.accent
    } else if hovered {
        pal.panel_alt.gamma_multiply(1.3)
    } else {
        pal.panel_alt
    };
    let edge = if resp.has_focus() || hovered {
        pal.accent
    } else {
        pal.tile_edge
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter()
        .rect_stroke(rect, 6.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    let color = if !enabled {
        pal.dim.gamma_multiply(0.5)
    } else if resp.is_pointer_button_down_on() {
        pal.on_accent
    } else {
        pal.text
    };
    draw_icon(ui.painter(), icon, rect.shrink(size * 0.22), color);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

/// A text button with an icon on the left.
pub fn icon_text_button(
    ui: &mut Ui,
    icon: Icon,
    text: &str,
    pal: &Palette,
    enabled: bool,
) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, pal.text);
    let h = ui.spacing().interact_size.y.max(30.0);
    let pad = 10.0;
    let w = h * 0.9 + galley.size().x + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(
        vec2(w, h),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let down = resp.is_pointer_button_down_on() && enabled;
    let fill = if down {
        pal.accent
    } else if resp.hovered() && enabled {
        pal.panel_alt.gamma_multiply(1.3)
    } else {
        pal.panel_alt
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(
        rect,
        6.0,
        Stroke::new(
            1.0,
            if resp.hovered() || resp.has_focus() {
                pal.accent
            } else {
                pal.tile_edge
            },
        ),
        egui::StrokeKind::Inside,
    );
    let color = if !enabled {
        pal.dim.gamma_multiply(0.5)
    } else if down {
        pal.on_accent
    } else {
        pal.text
    };
    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + pad + h * 0.3, rect.center().y),
        Vec2::splat(h * 0.5),
    );
    draw_icon(ui.painter(), icon, icon_rect, color);
    let text_pos = pos2(
        rect.left() + pad + h * 0.7,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, color);
    resp
}

/// A primary-styled wide button (menu entries).
pub fn big_button(ui: &mut Ui, text: &str, pal: &Palette, primary: bool) -> Response {
    let h = (ui.spacing().interact_size.y * 1.4).max(44.0);
    let w = ui.available_width().clamp(160.0, 360.0);
    let font = egui::FontId::proportional(egui::TextStyle::Button.resolve(ui.style()).size * 1.2);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let down = resp.is_pointer_button_down_on();
    let fill = if primary || down {
        if down {
            pal.accent.gamma_multiply(0.8)
        } else {
            pal.accent
        }
    } else if resp.hovered() {
        pal.panel_alt.gamma_multiply(1.3)
    } else {
        pal.panel_alt
    };
    ui.painter().rect_filled(rect, 10.0, fill);
    ui.painter().rect_stroke(
        rect,
        10.0,
        Stroke::new(
            if resp.has_focus() { 3.0 } else { 1.0 },
            if resp.hovered() || resp.has_focus() {
                pal.text
            } else {
                pal.tile_edge
            },
        ),
        egui::StrokeKind::Inside,
    );
    let color = if primary { pal.on_accent } else { pal.text };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font,
        color,
    );
    resp
}

/// Draw `n` stars (filled up to `filled`) in a row; returns the width used.
pub fn stars(ui: &mut Ui, filled: u8, total: u8, pal: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size * total as f32 * 1.1, size), Sense::hover());
    for i in 0..total {
        let c = pos2(
            rect.left() + size * 0.55 + i as f32 * size * 1.1,
            rect.center().y,
        );
        let on = i < filled;
        let color = if on {
            pal.warn
        } else {
            pal.dim.gamma_multiply(0.4)
        };
        draw_icon(
            ui.painter(),
            Icon::Star,
            Rect::from_center_size(c, Vec2::splat(size)),
            color,
        );
    }
}
