// SPDX-License-Identifier: Apache-2.0
//! The play screen: brief, program editor, text panel, office view and run controls.

use crate::app::{Env, Nav, Screen};
use crate::brief::brief_ui;
use crate::editor::{self, list_ui, palette_ui};
use crate::office::{Mood, OfficeInput};
use crate::play::{PlayState, Tab};
use crate::result::ResultView;
use crate::sound::Sfx;
use crate::widgets::{Icon, icon_button, icon_text_button};
use algoclerk_core::judge::Outcome;
use algoclerk_core::share;
use algoclerk_core::vm::Status;
use eframe::egui::{self, Frame, Key, Margin, Panel, RichText, ScrollArea, Sense, Ui, vec2};

/// Draw the play screen. Returns navigation requests.
pub fn play_screen(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState) -> Option<Nav> {
    let mut nav = None;
    play.tick(env.dt, env.tr);
    if play.running || play.machine.is_some() {
        ui.ctx().request_repaint();
    }
    shortcuts(ui, env, play);

    Panel::top("play-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Home, &env.pal, true, &env.tr.t("ui.back")).clicked() {
                nav = Some(Nav::Screen(if play.level.is_some() {
                    Screen::Packs
                } else {
                    Screen::Title
                }));
            }
            let title = play
                .level
                .as_ref()
                .map_or_else(|| env.tr.t("sandbox.title"), |l| l.title.clone());
            ui.label(
                RichText::new(title)
                    .size(if env.narrow { 18.0 } else { 26.0 })
                    .color(env.pal.accent),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(
                    ui,
                    Icon::Copy,
                    &env.pal,
                    play.listing.instruction_count() > 0,
                    &env.tr.t("play.share"),
                )
                .clicked()
                {
                    let stats =
                        play.verdict
                            .as_ref()
                            .filter(|v| v.passed)
                            .map(|v| share::ShareStats {
                                size: v.size,
                                avg_steps: v.avg_steps,
                                worst_steps: v.worst_steps,
                            });
                    let text = share::export(&play.id(), &play.listing, stats);
                    ui.ctx().copy_text(text);
                    play.message = Some(env.tr.t("play.copied_solution"));
                }
                let solved = play
                    .level
                    .as_ref()
                    .is_some_and(|l| env.progress.record(&l.id).solved);
                let may_peek = play.level.is_none() || solved;
                if icon_button(
                    ui,
                    Icon::Eye,
                    &env.pal,
                    play.runnable() || play.program.is_some(),
                    &env.tr.t("play.peek"),
                )
                .clicked()
                {
                    if may_peek || env.progress.record(&play.id()).code_revealed {
                        nav = Some(Nav::Result(Box::new(ResultView::new(play, env, false))));
                    } else {
                        play.confirm_peek = true;
                    }
                }
            });
        });
        if play.confirm_peek {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(env.tr.t("play.peek_warning")).color(env.pal.warn));
                if ui.button(env.tr.t("play.peek_yes")).clicked() {
                    env.progress.record_mut(&play.id()).code_revealed = true;
                    play.confirm_peek = false;
                    nav = Some(Nav::Result(Box::new(ResultView::new(play, env, false))));
                }
                if ui.button(env.tr.t("ui.cancel")).clicked() {
                    play.confirm_peek = false;
                }
            });
        }
    });

    if env.narrow {
        narrow_layout(ui, env, play, &mut nav);
    } else {
        wide_layout(ui, env, play, &mut nav);
    }

    if play.draft_dirty {
        play.draft_dirty = false;
        let text = play.listing.to_text();
        match &play.level {
            Some(l) => env.progress.record_mut(&l.id).draft = text,
            None => env.progress.sandbox = text,
        }
    }
    nav
}

fn shortcuts(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState) {
    if ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let (run, step, back, reset) = ui.input(|i| {
        (
            i.key_pressed(Key::Space),
            i.key_pressed(Key::ArrowRight),
            i.key_pressed(Key::ArrowLeft),
            i.key_pressed(Key::R),
        )
    });
    if run {
        play.toggle_run();
    }
    if step {
        play.step(env.tr);
    }
    if back {
        play.step_back();
    }
    if reset {
        play.reset_machine();
    }
}

fn wide_layout(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState, nav: &mut Option<Nav>) {
    if play.show_brief {
        Panel::left("brief-panel")
            .default_size(320.0)
            .size_range(240.0..=440.0)
            .resizable(true)
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| brief_ui(ui, env, play));
            });
    }
    if play.show_text {
        Panel::right("text-panel")
            .default_size(270.0)
            .size_range(200.0..=500.0)
            .resizable(true)
            .show(ui, |ui| text_panel(ui, env, play));
    }
    Panel::right("program-panel")
        .default_size(400.0)
        .size_range(330.0..=520.0)
        .resizable(true)
        .show(ui, |ui| program_panel(ui, env, play));
    egui::CentralPanel::default_margins().show(ui, |ui| office_panel(ui, env, play, nav));
}

fn narrow_layout(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState, nav: &mut Option<Nav>) {
    Panel::bottom("tab-bar").show(ui, |ui| {
        ui.columns(4, |cols| {
            for (i, (tab, key)) in [
                (Tab::Brief, "tab.brief"),
                (Tab::Program, "tab.program"),
                (Tab::Office, "tab.office"),
                (Tab::Text, "tab.text"),
            ]
            .into_iter()
            .enumerate()
            {
                let selected = play.tab == tab;
                let b = egui::Button::new(RichText::new(env.tr.t(key)).size(14.0))
                    .selected(selected)
                    .min_size(vec2(0.0, 44.0));
                if cols[i]
                    .add_sized([cols[i].available_width(), 44.0], b)
                    .clicked()
                {
                    play.tab = tab;
                }
            }
        });
    });
    egui::CentralPanel::default_margins().show(ui, |ui| match play.tab {
        Tab::Brief => {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| brief_ui(ui, env, play));
        }
        Tab::Program => {
            program_panel(ui, env, play);
        }
        Tab::Office => office_panel(ui, env, play, nav),
        Tab::Text => text_panel(ui, env, play),
    });
}

fn program_panel(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState) {
    let pal = env.pal;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(env.tr.t("editor.title"))
                .strong()
                .color(pal.accent),
        );
        ui.label(
            RichText::new(format!(
                "{} {}",
                env.tr.t("editor.size"),
                play.listing.instruction_count()
            ))
            .small()
            .color(pal.dim),
        );
        if !env.narrow {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .toggle_value(&mut play.show_text, env.tr.t("tab.text"))
                    .changed()
                    && play.show_text
                    && ui.ctx().content_rect().width() < 1500.0
                {
                    // Make room for the text panel on medium-sized windows.
                    play.show_brief = false;
                }
                ui.toggle_value(&mut play.show_brief, env.tr.t("tab.brief"));
            });
        }
    });
    if let Some(pick) = palette_ui(ui, play, env.tr, &pal) {
        editor::insert_line(play, pick);
        env.sound.play(Sfx::Click);
    }
    ui.separator();
    for p in play.problems_text(env.tr) {
        ui.label(RichText::new(p).color(pal.bad));
    }
    let reserve = if env.narrow { 150.0 } else { 20.0 };
    let avail = (ui.available_height() - reserve).max(120.0);
    ScrollArea::vertical()
        .id_salt("program-scroll")
        .max_height(avail)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            list_ui(ui, play, env.tr, &pal);
        });
    if env.narrow {
        ui.separator();
        let mut nav = None;
        controls(ui, env, play, &mut nav);
    }
}

fn text_panel(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState) {
    let pal = env.pal;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(env.tr.t("tab.text"))
                .strong()
                .color(pal.accent),
        );
        if ui.button(env.tr.t("text.format")).clicked() {
            play.text = play.listing.to_text();
            play.text_errors.clear();
        }
    });
    ui.label(RichText::new(env.tr.t("text.hint")).small().color(pal.dim));
    let h = (ui.available_height() - 80.0).max(120.0);
    ScrollArea::vertical()
        .id_salt("text-scroll")
        .max_height(h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let r = ui.add(
                egui::TextEdit::multiline(&mut play.text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(16)
                    .lock_focus(true)
                    .hint_text("INBOX\nOUTBOX"),
            );
            if r.changed() {
                play.set_text(env.tr);
            }
        });
    for e in &play.text_errors {
        ui.label(RichText::new(e).color(pal.bad).small());
    }
    if env.narrow {
        ui.separator();
        let mut nav = None;
        controls(ui, env, play, &mut nav);
    }
}

fn office_panel(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState, nav: &mut Option<Nav>) {
    let pal = env.pal;
    let controls_h = if env.narrow { 120.0 } else { 150.0 };
    let avail = ui.available_size();
    let scene_h = if env.narrow {
        (avail.x * 0.95).clamp(240.0, (avail.y * 0.5).max(240.0))
    } else {
        (avail.y - controls_h).max(160.0)
    };
    let (rect, _) = ui.allocate_exact_size(vec2(avail.x, scene_h), Sense::hover());
    // Scene input from the machine or the initial test.
    let test = play.current_test().clone();
    let (inbox_left, outbox, floor, hand) = match &play.machine {
        Some(m) => (
            m.inbox_remaining().to_vec(),
            m.outbox.clone(),
            m.floor.clone(),
            m.hand,
        ),
        None => {
            let mut f = test.floor.clone();
            f.resize(play.floor_len, None);
            (test.inbox.clone(), Vec::new(), f, None)
        }
    };
    let status = play.machine.as_ref().map(|m| m.status.clone());
    let mood = match (&status, play.finished_ok) {
        (Some(Status::Error(_)), _) | (_, Some(false)) => Mood::Sad,
        (_, Some(true)) => Mood::Happy,
        _ => Mood::Neutral,
    };
    let input = OfficeInput {
        inbox_left: &inbox_left,
        outbox: &outbox,
        expected: &test.expected,
        floor: &floor,
        names: &play.tile_names,
        hand,
        char_mode: play.char_mode(),
        bubble: play.error_bubble.as_deref(),
        mood,
    };
    let sps = play.steps_per_sec();
    play.view.show(ui, rect, &input, &pal, env.dt, sps);
    ui.add_space(4.0);
    controls(ui, env, play, nav);
    message_area(ui, env, play);
    if env.narrow {
        ui.separator();
        let h = ui.available_height().max(80.0);
        ScrollArea::vertical()
            .id_salt("office-list")
            .max_height(h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                list_ui(ui, play, env.tr, &pal);
            });
    }
}

fn controls(ui: &mut Ui, env: &mut Env<'_>, play: &mut PlayState, nav: &mut Option<Nav>) {
    let pal = env.pal;
    let tr = env.tr;
    ui.horizontal_wrapped(|ui| {
        if icon_button(ui, Icon::Reset, &pal, true, &tr.t("play.reset")).clicked() {
            play.reset_machine();
        }
        let can_back = play.machine.as_ref().is_some_and(|m| m.can_step_back());
        if icon_button(ui, Icon::Back, &pal, can_back, &tr.t("play.back")).clicked() {
            play.step_back();
        }
        let icon = if play.running {
            Icon::Pause
        } else {
            Icon::Play
        };
        let tip = if play.running {
            tr.t("play.pause")
        } else {
            tr.t("play.run")
        };
        if icon_button(ui, icon, &pal, play.runnable(), &tip).clicked() {
            play.toggle_run();
            env.sound.play(Sfx::Click);
        }
        let can_step = play.runnable() && play.machine.as_ref().is_none_or(|m| !m.is_done());
        if icon_button(ui, Icon::Step, &pal, can_step, &tr.t("play.step")).clicked() {
            play.running = false;
            play.step(tr);
        }
        ui.add(
            egui::Slider::new(&mut play.speed, 0.0..=1.0)
                .show_value(false)
                .text(tr.t("play.speed")),
        );
    });
    ui.horizontal_wrapped(|ui| {
        if play.level.is_some() {
            let tests = play.tests.len();
            let label = |i: usize| -> String {
                if i < play.level.as_ref().map_or(0, |l| l.examples.len()) {
                    format!("{} {}", tr.t("brief.example"), i + 1)
                } else {
                    format!("{} {}", tr.t("play.test"), i + 1)
                }
            };
            let mut idx = play.test_idx;
            egui::ComboBox::from_id_salt("test-select")
                .selected_text(label(idx))
                .show_ui(ui, |ui| {
                    for i in 0..tests {
                        ui.selectable_value(&mut idx, i, label(i));
                    }
                });
            if idx != play.test_idx {
                play.test_idx = idx;
                play.reset_machine();
            }
            if icon_text_button(ui, Icon::Check, &tr.t("play.check"), &pal, play.runnable())
                .clicked()
            {
                if play.check() {
                    let rv = ResultView::new(play, env, true);
                    env.sound.play(Sfx::Win);
                    *nav = Some(Nav::Result(Box::new(rv)));
                } else {
                    env.sound.play(Sfx::Error);
                    if let Some(f) = play.verdict.as_ref().and_then(|v| v.first_failure()) {
                        play.test_idx = f.index;
                        play.reset_machine();
                    }
                }
            }
        } else if icon_text_button(
            ui,
            Icon::Eye,
            &tr.t("play.peek"),
            &pal,
            play.program.is_some(),
        )
        .clicked()
        {
            *nav = Some(Nav::Result(Box::new(ResultView::new(play, env, false))));
        }
    });
}

fn message_area(ui: &mut Ui, env: &Env<'_>, play: &PlayState) {
    let pal = env.pal;
    let tr = env.tr;
    if let Some(v) = &play.verdict
        && !v.passed
    {
        Frame::new()
            .fill(pal.panel_alt)
            .corner_radius(8)
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::draw_icon(
                        ui.painter(),
                        Icon::Cross,
                        egui::Rect::from_min_size(
                            ui.cursor().min + vec2(0.0, 2.0),
                            vec2(14.0, 14.0),
                        ),
                        pal.bad,
                    );
                    ui.add_space(18.0);
                    ui.label(RichText::new(v.headline(tr)).color(pal.bad));
                });
                ui.label(
                    RichText::new(format!(
                        "{} {}/{}",
                        tr.t("play.tests_passed"),
                        v.passed_count(),
                        v.tests.len()
                    ))
                    .small()
                    .color(pal.dim),
                );
                if let Some(f) = v.first_failure()
                    && !matches!(f.outcome, Outcome::Pass)
                {
                    ui.label(
                        RichText::new(format!("{} {:?}", tr.t("brief.inbox"), f.inbox))
                            .monospace()
                            .small(),
                    );
                }
            });
    } else if let Some(m) = &play.message {
        let color = match play.finished_ok {
            Some(false) => pal.bad,
            Some(true) => pal.good,
            None => pal.dim,
        };
        ui.label(RichText::new(m).color(color));
    }
    if let Some(m) = &play.machine {
        ui.label(
            RichText::new(format!(
                "{} {}   {} {}",
                tr.t("play.steps"),
                m.steps,
                tr.t("play.size"),
                play.listing.instruction_count()
            ))
            .small()
            .color(pal.dim),
        );
    }
}
