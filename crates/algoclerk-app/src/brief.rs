// SPDX-License-Identifier: Apache-2.0
//! The level brief: story, statement, examples, rules, targets, hints and the spoiler-protected reference.

use crate::app::Env;
use crate::play::PlayState;
use crate::widgets::stars;
use algoclerk_core::level::{Display, Level};
use eframe::egui::{self, Frame, Margin, RichText, Ui};

fn fmt_values(v: &[i32], display: Display) -> String {
    let items: Vec<String> = v
        .iter()
        .map(
            |x| match (display, u32::try_from(*x).ok().and_then(char::from_u32)) {
                (Display::Char, Some(c)) if !c.is_control() && c != ' ' => format!("'{c}'"),
                _ => x.to_string(),
            },
        )
        .collect();
    format!("[{}]", items.join(", "))
}

fn card(ui: &mut Ui, env: &Env<'_>, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(env.pal.panel_alt)
        .corner_radius(8)
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// Draw the brief of the current level.
pub fn brief_ui(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState) {
    let Some(level) = play.level.clone() else {
        sandbox_brief(ui, env, play);
        return;
    };
    let pal = env.pal;
    let tr = env.tr;
    let rec = env.progress.record(&level.id);
    ui.label(RichText::new(&level.title).heading().color(pal.accent));
    ui.label(RichText::new(&level.summary).color(pal.dim));
    ui.add_space(4.0);
    card(ui, env, |ui| {
        ui.label(RichText::new(&level.story).italics());
    });
    ui.add_space(6.0);
    ui.label(RichText::new(tr.t("brief.task")).strong().color(pal.accent));
    ui.label(&level.statement);
    if !level.constraints.is_empty() {
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr.t("brief.constraints"))
                .strong()
                .color(pal.accent),
        );
        for c in &level.constraints {
            ui.label(format!("• {c}"));
        }
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new(tr.t("brief.examples"))
            .strong()
            .color(pal.accent),
    );
    for (i, ex) in level.examples.iter().enumerate() {
        card(ui, env, |ui| {
            ui.label(
                RichText::new(format!("{} {}", tr.t("brief.example"), i + 1))
                    .small()
                    .color(pal.dim),
            );
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    tr.t("brief.inbox"),
                    fmt_values(&ex.input.to_vec(), level.display)
                ))
                .monospace(),
            );
            for f in &ex.floor {
                ui.label(
                    RichText::new(format!(
                        "{} {} {}",
                        tr.t("brief.floor_from"),
                        f.at,
                        fmt_values(&f.values.to_vec(), level.display)
                    ))
                    .monospace(),
                );
            }
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    tr.t("brief.outbox"),
                    fmt_values(&ex.output.to_vec(), level.display)
                ))
                .monospace(),
            );
            if !ex.note.is_empty() {
                ui.label(RichText::new(&ex.note).small().color(pal.dim));
            }
        });
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new(tr.t("brief.tools"))
            .strong()
            .color(pal.accent),
    );
    ui.horizontal_wrapped(|ui| {
        for op in play.allowed.opcodes() {
            ui.label(
                RichText::new(op.mnemonic())
                    .monospace()
                    .background_color(pal.panel_alt),
            );
        }
        if play.allowed.allows_indirect() {
            ui.label(
                RichText::new("[n]")
                    .monospace()
                    .background_color(pal.panel_alt),
            );
        }
    });
    ui.label(
        RichText::new(format!("{} {}", tr.t("brief.tiles"), level.machine.floor))
            .small()
            .color(pal.dim),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(tr.t("brief.targets"))
            .strong()
            .color(pal.accent),
    );
    targets_ui(ui, env, &level, &rec);

    ui.add_space(8.0);
    ui.label(
        RichText::new(tr.t("brief.hints"))
            .strong()
            .color(pal.accent),
    );
    for (i, h) in level.hints.iter().take(play.hints_shown).enumerate() {
        card(ui, env, |ui| {
            ui.label(
                RichText::new(format!("{} {}", tr.t("brief.hint"), i + 1))
                    .small()
                    .color(pal.dim),
            );
            ui.label(h);
        });
    }
    if play.hints_shown < level.hints.len() {
        let text = format!(
            "{} ({}/{})",
            tr.t("brief.show_hint"),
            play.hints_shown + 1,
            level.hints.len()
        );
        if ui.button(text).clicked() {
            play.hints_shown += 1;
            env.progress.record_mut(&level.id).hints_used = play.hints_shown.min(255) as u8;
        }
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(tr.t("brief.reference"))
            .strong()
            .color(pal.accent),
    );
    let revealed = rec.reference_revealed || rec.solved;
    if revealed {
        if let Some(sol) = &level.solution {
            card(ui, env, |ui| {
                ui.label(RichText::new(sol.asm.trim_end()).monospace());
            });
        }
    } else if play.confirm_reference {
        ui.label(RichText::new(tr.t("brief.reference_warning")).color(pal.warn));
        ui.horizontal(|ui| {
            if ui.button(tr.t("brief.reference_yes")).clicked() {
                env.progress.record_mut(&level.id).reference_revealed = true;
                play.confirm_reference = false;
            }
            if ui.button(tr.t("ui.cancel")).clicked() {
                play.confirm_reference = false;
            }
        });
    } else if ui.button(tr.t("brief.reference_reveal")).clicked() {
        play.confirm_reference = true;
    }
    ui.add_space(12.0);
}

/// Targets and the player's best results.
pub fn targets_ui(
    ui: &mut Ui,
    env: &Env<'_>,
    level: &Level,
    rec: &algoclerk_core::progress::LevelRecord,
) {
    let pal = env.pal;
    let tr = env.tr;
    egui::Grid::new("targets-grid")
        .num_columns(4)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new(tr.t("medal.staple")).small().color(pal.dim));
            ui.label(RichText::new(tr.t("medal.seal")).small().color(pal.dim));
            ui.label(RichText::new(tr.t("brief.best")).small().color(pal.dim));
            ui.end_row();
            ui.label(tr.t("brief.size"));
            ui.label(RichText::new(format!("≤ {}", level.targets.size_par)).monospace());
            ui.label(RichText::new(format!("≤ {}", level.targets.size_ace)).monospace());
            ui.label(
                RichText::new(rec.best_size.map_or("-".to_string(), |v| v.to_string())).monospace(),
            );
            ui.end_row();
            ui.label(tr.t("brief.speed"));
            ui.label(RichText::new(format!("≤ {}", level.targets.speed_par)).monospace());
            ui.label(RichText::new(format!("≤ {}", level.targets.speed_ace)).monospace());
            ui.label(
                RichText::new(
                    rec.best_speed
                        .map_or("-".to_string(), |v| format!("{v:.1}")),
                )
                .monospace(),
            );
            ui.end_row();
        });
    stars(ui, rec.stars, 3, &pal, 18.0);
}

fn sandbox_brief(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState) {
    let pal = env.pal;
    let tr = env.tr;
    ui.label(
        RichText::new(tr.t("sandbox.title"))
            .heading()
            .color(pal.accent),
    );
    ui.label(tr.t("sandbox.intro"));
    ui.add_space(8.0);
    let mut changed = false;
    ui.label(tr.t("sandbox.inbox"));
    changed |= ui
        .add(
            egui::TextEdit::singleline(&mut play.sandbox_input)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        )
        .changed();
    ui.label(tr.t("sandbox.tiles"));
    changed |= ui
        .add(egui::Slider::new(&mut play.sandbox_floor, 1..=64))
        .changed();
    if changed {
        play.apply_sandbox_settings();
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new(tr.t("brief.tools"))
            .strong()
            .color(pal.accent),
    );
    ui.horizontal_wrapped(|ui| {
        for op in play.allowed.opcodes() {
            ui.label(
                RichText::new(op.mnemonic())
                    .monospace()
                    .background_color(pal.panel_alt),
            );
        }
        ui.label(
            RichText::new("[n]")
                .monospace()
                .background_color(pal.panel_alt),
        );
    });
}
