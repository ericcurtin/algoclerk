// SPDX-License-Identifier: Apache-2.0
//! The victory screen and the "equivalent code" view.

use crate::app::{Env, Nav, Screen};
use crate::play::PlayState;
use crate::theme::Palette;
use crate::widgets::{Icon, icon_text_button, stars};
use algoclerk_core::codegen::highlight::{TokenKind, highlight};
use algoclerk_core::codegen::{self, Generated, Lang, Options, Stats};
use algoclerk_core::judge::{Medal, Verdict};
use algoclerk_core::level::Level;
use algoclerk_core::share;
use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, Ui, text::LayoutJob};

/// What the result screen shows.
pub struct ResultView {
    pub level: Option<Level>,
    pub verdict: Option<Verdict>,
    pub solution_text: String,
    pub codes: Vec<Generated>,
    pub lang: usize,
    pub new_best: bool,
}

impl ResultView {
    /// Build the view for the current program. `solved` is true when the judge accepted it.
    pub fn new(play: &PlayState, env: &Env<'_>, solved: bool) -> ResultView {
        let verdict = play.verdict.clone().filter(|v| v.passed && solved);
        let opts = Options {
            stats: verdict.as_ref().map(|v| Stats {
                size: v.size,
                avg_steps: v.avg_steps,
                worst_steps: v.worst_steps,
            }),
            floor_len: Some(play.floor_len),
            ..Options::default()
        };
        let mut codes = Vec::new();
        if let Some(p) = &play.program {
            for lang in Lang::ALL {
                codes.push(codegen::generate(p, lang, play.level.as_ref(), &opts));
            }
        }
        let lang = Lang::ALL
            .iter()
            .position(|l| l.key() == env.progress.settings.code_lang)
            .unwrap_or(2);
        ResultView {
            level: play.level.clone(),
            verdict,
            solution_text: play.listing.to_text(),
            codes,
            lang,
            new_best: false,
        }
    }
}

fn medal_label(env: &Env<'_>, m: Option<Medal>) -> String {
    m.map_or_else(|| "-".to_string(), |m| env.tr.t(m.key()))
}

/// Syntax-highlighted code as a layout job.
pub fn code_job(g: &Generated, pal: &Palette, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for t in highlight(g.lang, &g.code) {
        let color = match t.kind {
            TokenKind::Keyword => pal.syn_keyword,
            TokenKind::Type => pal.syn_type,
            TokenKind::Number => pal.syn_number,
            TokenKind::Comment => pal.syn_comment,
            TokenKind::Str => pal.syn_string,
            TokenKind::Preproc => pal.syn_preproc,
            TokenKind::Ident | TokenKind::Punct | TokenKind::Space => pal.text,
        };
        job.append(
            t.text,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(size),
                color,
                ..Default::default()
            },
        );
    }
    job
}

/// Draw the result screen.
pub fn result_screen(ui: &mut Ui, env: &mut Env<'_>, rv: &mut ResultView) -> Option<Nav> {
    let pal = env.pal;
    let tr = env.tr;
    let mut nav = None;
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = ui.available_width().min(900.0);
            ui.set_max_width(width);
            let solved = rv.verdict.is_some();
            ui.label(
                RichText::new(if solved {
                    tr.t("result.solved")
                } else {
                    tr.t("result.code_title")
                })
                .heading()
                .color(if solved { pal.good } else { pal.accent }),
            );
            if let Some(l) = &rv.level {
                ui.label(RichText::new(&l.title).color(pal.dim));
            }
            if let (Some(v), Some(l)) = (&rv.verdict, &rv.level) {
                ui.add_space(6.0);
                stars(ui, v.medals.stars(), 3, &pal, 30.0);
                egui::Grid::new("result-stats")
                    .num_columns(4)
                    .spacing([18.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("");
                        ui.label(RichText::new(tr.t("result.yours")).small().color(pal.dim));
                        ui.label(RichText::new(tr.t("medal.staple")).small().color(pal.dim));
                        ui.label(RichText::new(tr.t("medal.seal")).small().color(pal.dim));
                        ui.end_row();
                        ui.label(tr.t("brief.size"));
                        ui.label(
                            RichText::new(format!(
                                "{} ({})",
                                v.size,
                                medal_label(env, v.medals.size)
                            ))
                            .monospace()
                            .strong(),
                        );
                        ui.label(RichText::new(format!("≤ {}", l.targets.size_par)).monospace());
                        ui.label(RichText::new(format!("≤ {}", l.targets.size_ace)).monospace());
                        ui.end_row();
                        ui.label(tr.t("result.speed_avg"));
                        ui.label(
                            RichText::new(format!(
                                "{:.1} ({})",
                                v.avg_steps,
                                medal_label(env, v.medals.speed)
                            ))
                            .monospace()
                            .strong(),
                        );
                        ui.label(RichText::new(format!("≤ {}", l.targets.speed_par)).monospace());
                        ui.label(RichText::new(format!("≤ {}", l.targets.speed_ace)).monospace());
                        ui.end_row();
                        ui.label(tr.t("result.speed_worst"));
                        ui.label(
                            RichText::new(v.worst_steps.to_string())
                                .monospace()
                                .strong(),
                        );
                        ui.end_row();
                    });
            }
            // What the clerk did.
            if let Some(e) = rv.level.as_ref().and_then(|l| l.explain.as_ref()) {
                ui.add_space(8.0);
                Frame::new()
                    .fill(pal.panel_alt)
                    .corner_radius(8)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(tr.t("result.what_clerk_did"))
                                .small()
                                .color(pal.dim),
                        );
                        ui.label(
                            RichText::new(&e.algorithm)
                                .strong()
                                .color(pal.accent)
                                .size(18.0),
                        );
                        ui.label(&e.text);
                    });
            }
            // Code.
            ui.add_space(10.0);
            ui.label(
                RichText::new(tr.t("result.code"))
                    .strong()
                    .color(pal.accent),
            );
            if rv.codes.is_empty() {
                ui.label(RichText::new(tr.t("result.no_code")).color(pal.dim));
            } else {
                ui.horizontal(|ui| {
                    for (i, g) in rv.codes.iter().enumerate() {
                        if ui
                            .selectable_label(rv.lang == i, RichText::new(g.lang.title()))
                            .clicked()
                        {
                            rv.lang = i;
                            env.progress.settings.code_lang = g.lang.key().to_string();
                        }
                    }
                    if let Some(g) = rv.codes.get(rv.lang)
                        && icon_text_button(ui, Icon::Copy, &tr.t("result.copy_code"), &pal, true)
                            .clicked()
                    {
                        ui.ctx().copy_text(g.code.clone());
                    }
                });
                if let Some(g) = rv.codes.get(rv.lang) {
                    if !g.structured {
                        ui.label(
                            RichText::new(tr.t("result.goto_note"))
                                .small()
                                .color(pal.dim),
                        );
                    }
                    Frame::new()
                        .fill(pal.code_bg)
                        .corner_radius(8)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ScrollArea::horizontal().id_salt("code-h").show(ui, |ui| {
                                let size = egui::TextStyle::Monospace.resolve(ui.style()).size;
                                ui.add(
                                    egui::Label::new(code_job(g, &pal, size))
                                        .extend()
                                        .selectable(true),
                                );
                            });
                        });
                }
            }
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if icon_text_button(ui, Icon::Copy, &tr.t("result.copy_solution"), &pal, true)
                    .clicked()
                {
                    let stats = rv.verdict.as_ref().map(|v| share::ShareStats {
                        size: v.size,
                        avg_steps: v.avg_steps,
                        worst_steps: v.worst_steps,
                    });
                    let id = rv
                        .level
                        .as_ref()
                        .map_or("sandbox".to_string(), |l| l.id.clone());
                    if let Ok(listing) = algoclerk_core::asm::parse(&rv.solution_text) {
                        ui.ctx().copy_text(share::export(&id, &listing, stats));
                    }
                }
                if icon_text_button(ui, Icon::Back, &tr.t("result.keep_editing"), &pal, true)
                    .clicked()
                {
                    nav = Some(Nav::Screen(Screen::Play));
                }
                if solved
                    && let Some(l) = &rv.level
                    && let Some(next) = env.campaign.next_after(&l.id)
                    && icon_text_button(ui, Icon::Next, &tr.t("result.next_level"), &pal, true)
                        .clicked()
                {
                    nav = Some(Nav::Level(next.id.clone()));
                }
                if icon_text_button(ui, Icon::Home, &tr.t("result.level_list"), &pal, true)
                    .clicked()
                {
                    nav = Some(Nav::Screen(if rv.level.is_some() {
                        Screen::Packs
                    } else {
                        Screen::Title
                    }));
                }
            });
            ui.add_space(20.0);
        });
    nav
}
