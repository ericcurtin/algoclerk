// SPDX-License-Identifier: Apache-2.0
//! Colour palettes (light, dark, high contrast, colour-blind friendly) and egui styling.

use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle};

/// All colours used by the game.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub good: Color32,
    pub bad: Color32,
    pub warn: Color32,
    pub belt: Color32,
    pub belt_stripe: Color32,
    pub crate_fill: Color32,
    pub crate_edge: Color32,
    pub tile: Color32,
    pub tile_edge: Color32,
    pub wall: Color32,
    pub floor: Color32,
    pub skin: Color32,
    pub suit: Color32,
    pub tie: Color32,
    pub desk: Color32,
    pub highlight: Color32,
    pub code_bg: Color32,
    pub syn_keyword: Color32,
    pub syn_type: Color32,
    pub syn_number: Color32,
    pub syn_comment: Color32,
    pub syn_string: Color32,
    pub syn_preproc: Color32,
}

fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Build the palette for a theme name (`"light"`, `"dark"`, `"contrast"`).
pub fn palette(theme: &str, colorblind: bool) -> Palette {
    let mut p = match theme {
        "light" => Palette {
            dark: false,
            bg: rgb(238, 232, 218),
            panel: rgb(250, 247, 238),
            panel_alt: rgb(228, 220, 202),
            text: rgb(38, 34, 30),
            dim: rgb(104, 96, 86),
            accent: rgb(30, 110, 160),
            on_accent: rgb(255, 255, 255),
            good: rgb(30, 130, 60),
            bad: rgb(190, 40, 40),
            warn: rgb(200, 130, 20),
            belt: rgb(120, 118, 122),
            belt_stripe: rgb(150, 148, 152),
            crate_fill: rgb(214, 170, 104),
            crate_edge: rgb(128, 90, 44),
            tile: rgb(246, 240, 222),
            tile_edge: rgb(150, 138, 112),
            wall: rgb(206, 220, 214),
            floor: rgb(190, 170, 140),
            skin: rgb(240, 196, 160),
            suit: rgb(64, 84, 116),
            tie: rgb(190, 60, 50),
            desk: rgb(150, 104, 64),
            highlight: rgb(255, 224, 120),
            code_bg: rgb(244, 240, 228),
            syn_keyword: rgb(150, 40, 130),
            syn_type: rgb(30, 100, 150),
            syn_number: rgb(170, 90, 10),
            syn_comment: rgb(110, 120, 100),
            syn_string: rgb(30, 130, 80),
            syn_preproc: rgb(130, 90, 50),
        },
        "contrast" => Palette {
            dark: true,
            bg: Color32::BLACK,
            panel: Color32::BLACK,
            panel_alt: rgb(28, 28, 28),
            text: Color32::WHITE,
            dim: rgb(210, 210, 210),
            accent: rgb(255, 230, 0),
            on_accent: Color32::BLACK,
            good: rgb(80, 255, 120),
            bad: rgb(255, 90, 90),
            warn: rgb(255, 190, 40),
            belt: rgb(70, 70, 70),
            belt_stripe: rgb(240, 240, 240),
            crate_fill: rgb(255, 200, 90),
            crate_edge: Color32::WHITE,
            tile: Color32::BLACK,
            tile_edge: Color32::WHITE,
            wall: Color32::BLACK,
            floor: rgb(20, 20, 20),
            skin: rgb(255, 214, 180),
            suit: rgb(120, 160, 255),
            tie: rgb(255, 80, 80),
            desk: rgb(200, 150, 90),
            highlight: rgb(255, 230, 0),
            code_bg: Color32::BLACK,
            syn_keyword: rgb(255, 130, 255),
            syn_type: rgb(110, 220, 255),
            syn_number: rgb(255, 200, 80),
            syn_comment: rgb(190, 200, 190),
            syn_string: rgb(120, 255, 160),
            syn_preproc: rgb(255, 180, 120),
        },
        _ => Palette {
            dark: true,
            bg: rgb(26, 28, 36),
            panel: rgb(36, 39, 50),
            panel_alt: rgb(46, 50, 64),
            text: rgb(232, 230, 222),
            dim: rgb(160, 164, 178),
            accent: rgb(96, 176, 232),
            on_accent: rgb(10, 20, 30),
            good: rgb(110, 200, 120),
            bad: rgb(236, 100, 96),
            warn: rgb(238, 178, 70),
            belt: rgb(70, 74, 86),
            belt_stripe: rgb(96, 100, 114),
            crate_fill: rgb(204, 156, 90),
            crate_edge: rgb(110, 76, 36),
            tile: rgb(52, 56, 70),
            tile_edge: rgb(110, 116, 140),
            wall: rgb(42, 48, 60),
            floor: rgb(58, 52, 50),
            skin: rgb(236, 190, 154),
            suit: rgb(88, 108, 150),
            tie: rgb(210, 76, 66),
            desk: rgb(130, 90, 56),
            highlight: rgb(250, 214, 96),
            code_bg: rgb(24, 26, 34),
            syn_keyword: rgb(210, 130, 220),
            syn_type: rgb(110, 190, 240),
            syn_number: rgb(240, 170, 90),
            syn_comment: rgb(124, 134, 120),
            syn_string: rgb(130, 210, 140),
            syn_preproc: rgb(220, 170, 120),
        },
    };
    if colorblind {
        // Okabe-Ito inspired: blue for good, orange for bad; never rely on red versus green.
        p.good = rgb(0, 114, 178);
        p.bad = rgb(230, 130, 0);
        p.warn = rgb(240, 228, 66);
        if p.dark {
            p.good = rgb(86, 180, 233);
        }
    }
    p
}

/// Apply a palette and size settings to an egui context.
pub fn apply(ctx: &egui::Context, p: &Palette, text_scale: f32, touch: bool) {
    let mut visuals = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.override_text_color = Some(p.text);
    visuals.panel_fill = p.panel;
    visuals.window_fill = p.panel;
    visuals.extreme_bg_color = p.code_bg;
    visuals.faint_bg_color = p.panel_alt;
    visuals.hyperlink_color = p.accent;
    visuals.selection.bg_fill = p.accent.gamma_multiply(0.5);
    visuals.selection.stroke = egui::Stroke::new(1.5, p.accent);
    let stroke_w = if p.bg == Color32::BLACK { 2.0 } else { 1.0 };
    let edge = egui::Stroke::new(stroke_w, p.tile_edge);
    visuals.widgets.noninteractive.bg_fill = p.panel;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.panel_alt);
    visuals.widgets.inactive.bg_fill = p.panel_alt;
    visuals.widgets.inactive.weak_bg_fill = p.panel_alt;
    visuals.widgets.inactive.bg_stroke = edge;
    visuals.widgets.hovered.bg_fill = p.panel_alt.gamma_multiply(1.25);
    visuals.widgets.hovered.weak_bg_fill = p.panel_alt.gamma_multiply(1.25);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(stroke_w + 0.5, p.accent);
    visuals.widgets.active.bg_fill = p.accent.gamma_multiply(0.45);
    visuals.widgets.active.weak_bg_fill = p.accent.gamma_multiply(0.45);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, p.text);
    visuals.widgets.open.bg_fill = p.panel_alt;
    ctx.set_visuals(visuals);

    let s = text_scale.clamp(0.7, 2.0) * if touch { 1.1 } else { 1.0 };
    ctx.global_style_mut(|style| {
        let mono = FontFamily::Monospace;
        let prop = FontFamily::Proportional;
        style.text_styles = [
            (TextStyle::Heading, FontId::new(26.0 * s, prop.clone())),
            (TextStyle::Body, FontId::new(15.0 * s, prop.clone())),
            (TextStyle::Button, FontId::new(15.0 * s, prop.clone())),
            (TextStyle::Small, FontId::new(12.0 * s, prop.clone())),
            (TextStyle::Monospace, FontId::new(14.0 * s, mono)),
        ]
        .into();
        let target = if touch { 48.0 } else { 30.0 };
        style.spacing.interact_size = egui::vec2(target, target);
        style.spacing.button_padding = egui::vec2(10.0, if touch { 10.0 } else { 5.0 });
        style.spacing.item_spacing = egui::vec2(8.0, if touch { 10.0 } else { 6.0 });
        style.spacing.slider_width = 140.0;
        style.spacing.scroll.bar_width = if touch { 14.0 } else { 8.0 };
        style.interaction.tooltip_delay = 0.4;
    });
}
