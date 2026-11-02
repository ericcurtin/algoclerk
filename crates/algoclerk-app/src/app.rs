// SPDX-License-Identifier: Apache-2.0
//! The application shell: screens, navigation, settings, persistence.

use crate::office::{Mood, OfficeInput, OfficeView};
use crate::play::PlayState;
use crate::play_ui::play_screen;
use crate::result::{ResultView, result_screen};
use crate::sound::Sound;
use crate::theme::{self, Palette};
use crate::widgets::{Icon, big_button, draw_icon, icon_button, stars};
use algoclerk_core::i18n::{self, Strings};
use algoclerk_core::progress::Progress;
use algoclerk_levels::Campaign;
use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, Sense, Ui, vec2};

/// Top-level screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Packs,
    Play,
    Result,
    Settings,
    About,
}

/// Navigation requests from screens.
pub enum Nav {
    Screen(Screen),
    Level(String),
    Sandbox,
    Result(Box<ResultView>),
}

/// Shared read/write context handed to screens.
pub struct Env<'a> {
    pub pal: Palette,
    pub tr: &'a Strings,
    pub progress: &'a mut Progress,
    pub campaign: &'a Campaign,
    pub sound: &'a Sound,
    pub narrow: bool,
    pub touch: bool,
    pub dt: f32,
}

/// The whole game.
pub struct AlgoClerkApp {
    pub campaign: Campaign,
    pub progress: Progress,
    pub strings: Strings,
    pub screen: Screen,
    pub play: Option<Box<PlayState>>,
    pub result: Option<Box<ResultView>>,
    pub sound: Sound,
    pub title_scene: OfficeView,
    pub confirm_reset: bool,
    pub toast: Option<String>,
    applied_theme: Option<(String, bool, u32, bool)>,
    narrow_override: Option<bool>,
    loaded_language: String,
}

fn load_extra_levels(campaign: &mut Campaign) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let dir = std::env::var_os("ALGOCLERK_LEVELS")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
                let base = std::path::PathBuf::from(home);
                Some(if cfg!(target_os = "macos") {
                    base.join("Library/Application Support/AlgoClerk/levels")
                } else if cfg!(windows) {
                    std::env::var_os("APPDATA").map_or_else(
                        || base.join("AlgoClerk/levels"),
                        |a| std::path::PathBuf::from(a).join("AlgoClerk/levels"),
                    )
                } else {
                    std::env::var_os("XDG_DATA_HOME").map_or_else(
                        || base.join(".local/share/algoclerk/levels"),
                        |x| std::path::PathBuf::from(x).join("algoclerk/levels"),
                    )
                })
            });
        if let Some(d) = dir {
            campaign.load_dir(&d);
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = campaign;
}

impl AlgoClerkApp {
    /// Create the app, restoring saved progress from `saved` (JSON) if present.
    pub fn new(saved: Option<&str>) -> AlgoClerkApp {
        let mut campaign = Campaign::shipped();
        load_extra_levels(&mut campaign);
        let progress = saved.map(Progress::from_json).unwrap_or_default();
        let strings = i18n::load_language(&progress.settings.language)
            .unwrap_or_else(|| i18n::english().clone());
        let loaded_language = progress.settings.language.clone();
        let sound = Sound::new();
        sound.set_enabled(progress.settings.sound);
        AlgoClerkApp {
            campaign,
            progress,
            strings,
            screen: Screen::Title,
            play: None,
            result: None,
            sound,
            title_scene: OfficeView::default(),
            confirm_reset: false,
            toast: None,
            applied_theme: None,
            narrow_override: None,
            loaded_language,
        }
    }

    /// Force the narrow or wide layout (used by screenshots).
    pub fn force_layout(&mut self, narrow: Option<bool>) {
        self.narrow_override = narrow;
    }

    /// The JSON to persist.
    pub fn save_json(&self) -> String {
        self.progress.to_json()
    }

    /// Start a level by id.
    pub fn start_level(&mut self, id: &str) {
        let Some(level) = self.campaign.find(id).cloned() else {
            return;
        };
        let rec = self.progress.record(id);
        match PlayState::for_level(&level, &rec.draft, rec.hints_used) {
            Ok(p) => {
                self.progress.last_level = id.to_string();
                self.play = Some(Box::new(p));
                self.screen = Screen::Play;
            }
            Err(e) => self.toast = Some(e),
        }
    }

    /// Start the sandbox.
    pub fn start_sandbox(&mut self) {
        self.play = Some(Box::new(PlayState::sandbox(&self.progress.sandbox)));
        self.screen = Screen::Play;
    }

    fn apply_nav(&mut self, nav: Nav) {
        match nav {
            Nav::Screen(Screen::Play) if self.play.is_none() => self.screen = Screen::Packs,
            Nav::Screen(s) => self.screen = s,
            Nav::Level(id) => self.start_level(&id),
            Nav::Sandbox => self.start_sandbox(),
            Nav::Result(rv) => {
                self.result = Some(rv);
                self.screen = Screen::Result;
            }
        }
    }

    fn ensure_theme(&mut self, ctx: &egui::Context, touch: bool) -> Palette {
        let s = &self.progress.settings;
        let sig = (s.theme.clone(), s.colorblind, s.text_scale.to_bits(), touch);
        let pal = theme::palette(&s.theme, s.colorblind);
        if self.applied_theme.as_ref() != Some(&sig) {
            theme::apply(ctx, &pal, s.text_scale, touch);
            self.applied_theme = Some(sig);
        }
        pal
    }

    /// Draw one frame of the whole UI.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let touch = matches!(
            ctx.os(),
            egui::os::OperatingSystem::Android | egui::os::OperatingSystem::IOS
        ) || ctx.input(|i| i.any_touches());
        let pal = self.ensure_theme(&ctx, touch);
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let insets = ctx.input(|i| i.safe_area_insets());
        let full = ui.max_rect();
        let inner = full - insets;
        ui.painter().rect_filled(full, 0.0, pal.bg);
        let narrow = self.narrow_override.unwrap_or(inner.width() < 760.0);
        let strings = self.strings.clone();
        let mut nav: Option<Nav> = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            let mut play = self.play.take();
            let mut result = self.result.take();
            {
                let mut env = Env {
                    pal,
                    tr: &strings,
                    progress: &mut self.progress,
                    campaign: &self.campaign,
                    sound: &self.sound,
                    narrow,
                    touch,
                    dt,
                };
                match self.screen {
                    Screen::Title => {
                        nav = title_screen(ui, &mut env, &mut self.title_scene, &self.toast)
                    }
                    Screen::Packs => nav = packs_screen(ui, &mut env),
                    Screen::Play => {
                        if let Some(p) = play.as_deref_mut() {
                            nav = play_screen(ui, &mut env, p);
                        } else {
                            nav = Some(Nav::Screen(Screen::Packs));
                        }
                    }
                    Screen::Result => {
                        if let Some(r) = result.as_deref_mut() {
                            nav = result_screen(ui, &mut env, r);
                        } else {
                            nav = Some(Nav::Screen(Screen::Title));
                        }
                    }
                    Screen::Settings => {
                        let (n, reset) = settings_screen(ui, &mut env, self.confirm_reset);
                        nav = n;
                        self.confirm_reset = reset.0;
                        if reset.1 {
                            *env.progress = Progress::default();
                        }
                    }
                    Screen::About => nav = about_screen(ui, &mut env),
                }
            }
            self.play = play;
            self.result = result;
        });
        // Settings may have changed language or sound.
        if self.loaded_language != self.progress.settings.language {
            self.strings = i18n::load_language(&self.progress.settings.language)
                .unwrap_or_else(|| i18n::english().clone());
            self.loaded_language
                .clone_from(&self.progress.settings.language);
        }
        self.sound.set_enabled(self.progress.settings.sound);
        if let Some(n) = nav {
            self.apply_nav(n);
        }
    }
}

impl eframe::App for AlgoClerkApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        AlgoClerkApp::ui(self, ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("progress", self.save_json());
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
}

fn centered_column<R>(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_width();
    let w = avail.min(max_width);
    let pad = ((avail - w) / 2.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(w);
            add(ui)
        })
        .inner
    })
    .inner
}

fn title_screen(
    ui: &mut Ui,
    env: &mut Env<'_>,
    scene: &mut OfficeView,
    toast: &Option<String>,
) -> Option<Nav> {
    let mut nav = None;
    let pal = env.pal;
    let tr = env.tr;
    let avail = ui.available_rect_before_wrap();
    let head_h = (avail.height() * 0.1).clamp(36.0, 70.0);
    let scene_h = (avail.height() * 0.4).clamp(150.0, 340.0);
    let scene_rect =
        egui::Rect::from_min_size(avail.min + vec2(0.0, head_h), vec2(avail.width(), scene_h));
    let demo_in = [3, 1, 4, 1, 5];
    let demo_out = [3, 1];
    let input = OfficeInput {
        inbox_left: &demo_in,
        outbox: &demo_out,
        expected: &demo_out,
        floor: &[Some(0), None, Some(7), None],
        names: &[],
        hand: Some(4),
        char_mode: false,
        bubble: None,
        mood: Mood::Happy,
    };
    let sps = 1.0;
    scene.show(ui, scene_rect, &input, &pal, env.dt, sps);
    ui.painter().text(
        egui::pos2(avail.center().x, avail.top() + 4.0),
        egui::Align2::CENTER_TOP,
        "AlgoClerk",
        egui::FontId::proportional((head_h * 0.8).clamp(28.0, 56.0)),
        pal.accent,
    );
    ui.allocate_rect(
        egui::Rect::from_min_max(avail.min, scene_rect.max),
        Sense::hover(),
    );
    ui.add_space(12.0);
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            centered_column(ui, 420.0, |ui| {
                ui.label(RichText::new(tr.t("title.tagline")).color(pal.dim));
                ui.add_space(8.0);
                let has_progress = !env.progress.last_level.is_empty();
                let label = if has_progress {
                    tr.t("title.continue")
                } else {
                    tr.t("title.play")
                };
                if big_button(ui, &label, &pal, true).clicked() {
                    nav = Some(
                        if has_progress && env.campaign.find(&env.progress.last_level).is_some() {
                            Nav::Level(env.progress.last_level.clone())
                        } else {
                            Nav::Screen(Screen::Packs)
                        },
                    );
                }
                if has_progress && big_button(ui, &tr.t("title.levels"), &pal, false).clicked() {
                    nav = Some(Nav::Screen(Screen::Packs));
                }
                if big_button(ui, &tr.t("title.sandbox"), &pal, false).clicked() {
                    nav = Some(Nav::Sandbox);
                }
                if big_button(ui, &tr.t("title.settings"), &pal, false).clicked() {
                    nav = Some(Nav::Screen(Screen::Settings));
                }
                if big_button(ui, &tr.t("title.about"), &pal, false).clicked() {
                    nav = Some(Nav::Screen(Screen::About));
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        tr.t("title.stars"),
                        env.progress.total_stars()
                    ))
                    .small()
                    .color(pal.dim),
                );
                if let Some(t) = toast {
                    ui.label(RichText::new(t).color(pal.bad));
                }
            });
        });
    nav
}

fn packs_screen(ui: &mut Ui, env: &mut Env<'_>) -> Option<Nav> {
    let mut nav = None;
    let pal = env.pal;
    let tr = env.tr;
    egui::Panel::top("packs-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Home, &pal, true, &tr.t("ui.back")).clicked() {
                nav = Some(Nav::Screen(Screen::Title));
            }
            ui.label(
                RichText::new(tr.t("packs.title"))
                    .heading()
                    .color(pal.accent),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        env.progress.total_stars(),
                        tr.t("packs.stars")
                    ))
                    .color(pal.warn),
                );
            });
        });
    });
    egui::CentralPanel::default_margins().show(ui, |ui| {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (pi, pack) in env.campaign.packs.iter().enumerate() {
                    let unlocked = env.campaign.pack_unlocked(env.progress, pi);
                    let solved = pack
                        .levels
                        .iter()
                        .filter(|l| env.progress.record(&l.id).solved)
                        .count();
                    Frame::new()
                        .fill(pal.panel_alt)
                        .corner_radius(10)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{}. {}", pi + 1, pack.info.title))
                                        .strong()
                                        .size(18.0)
                                        .color(if unlocked { pal.accent } else { pal.dim }),
                                );
                                ui.label(
                                    RichText::new(format!("{solved}/{}", pack.levels.len()))
                                        .small()
                                        .color(pal.dim),
                                );
                                if !unlocked {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                                    draw_icon(ui.painter(), Icon::Lock, r, pal.dim);
                                }
                            });
                            ui.label(RichText::new(&pack.info.blurb).small().color(pal.dim));
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                for (li, level) in pack.levels.iter().enumerate() {
                                    let open = env.campaign.level_unlocked(env.progress, pi, li);
                                    let rec = env.progress.record(&level.id);
                                    level_button(
                                        ui,
                                        env,
                                        &level.title,
                                        li + 1,
                                        open,
                                        rec.stars,
                                        rec.solved,
                                        &mut nav,
                                        &level.id,
                                    );
                                }
                            });
                        });
                    ui.add_space(8.0);
                }
            });
    });
    nav
}

#[allow(clippy::too_many_arguments)]
fn level_button(
    ui: &mut Ui,
    env: &Env<'_>,
    title: &str,
    number: usize,
    open: bool,
    star_count: u8,
    solved: bool,
    nav: &mut Option<Nav>,
    id: &str,
) {
    let pal = env.pal;
    let w = if env.narrow {
        ui.available_width()
    } else {
        190.0
    };
    let h = if env.touch { 64.0 } else { 54.0 };
    let (rect, resp) = ui.allocate_exact_size(
        vec2(w, h),
        if open { Sense::click() } else { Sense::hover() },
    );
    let fill = if !open {
        pal.panel
    } else if resp.is_pointer_button_down_on() {
        pal.accent.gamma_multiply(0.6)
    } else if resp.hovered() {
        pal.tile.gamma_multiply(1.2)
    } else {
        pal.tile
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().rect_stroke(
        rect,
        8.0,
        egui::Stroke::new(
            if resp.has_focus() || resp.hovered() {
                2.0
            } else {
                1.0
            },
            if solved { pal.good } else { pal.tile_edge },
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_top() + vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        format!("{number:>2}"),
        egui::FontId::monospace(14.0),
        pal.dim,
    );
    let max_chars = (((w - 70.0) / 8.0) as usize).max(8);
    let t: String = if title.chars().count() > max_chars {
        title.chars().take(max_chars - 1).chain(['~']).collect()
    } else {
        title.to_string()
    };
    ui.painter().text(
        rect.left_top() + vec2(40.0, 8.0),
        egui::Align2::LEFT_TOP,
        t,
        egui::FontId::proportional(14.0),
        if open { pal.text } else { pal.dim },
    );
    if open {
        let mut sx = rect.left() + 40.0;
        for i in 0..3u8 {
            draw_icon(
                ui.painter(),
                Icon::Star,
                egui::Rect::from_min_size(egui::pos2(sx, rect.bottom() - 22.0), vec2(14.0, 14.0)),
                if i < star_count {
                    pal.warn
                } else {
                    pal.dim.gamma_multiply(0.4)
                },
            );
            sx += 16.0;
        }
        if solved {
            draw_icon(
                ui.painter(),
                Icon::Check,
                egui::Rect::from_min_size(
                    egui::pos2(rect.right() - 26.0, rect.top() + 8.0),
                    vec2(16.0, 16.0),
                ),
                pal.good,
            );
        }
    } else {
        draw_icon(
            ui.painter(),
            Icon::Lock,
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 40.0, rect.bottom() - 24.0),
                vec2(16.0, 16.0),
            ),
            pal.dim,
        );
    }
    if open && resp.clicked() {
        *nav = Some(Nav::Level(id.to_string()));
    }
}

fn settings_screen(
    ui: &mut Ui,
    env: &mut Env<'_>,
    confirm_reset: bool,
) -> (Option<Nav>, (bool, bool)) {
    let mut nav = None;
    let pal = env.pal;
    let tr = env.tr;
    let mut confirm = confirm_reset;
    let mut do_reset = false;
    egui::Panel::top("settings-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Home, &pal, true, &tr.t("ui.back")).clicked() {
                nav = Some(Nav::Screen(Screen::Title));
            }
            ui.label(
                RichText::new(tr.t("title.settings"))
                    .heading()
                    .color(pal.accent),
            );
        });
    });
    egui::CentralPanel::default_margins().show(ui, |ui| {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                centered_column(ui, 560.0, |ui| {
                    let s = &mut env.progress.settings;
                    ui.label(
                        RichText::new(tr.t("settings.theme"))
                            .strong()
                            .color(pal.accent),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for (key, label) in [
                            ("dark", "settings.theme_dark"),
                            ("light", "settings.theme_light"),
                            ("contrast", "settings.theme_contrast"),
                        ] {
                            ui.selectable_value(&mut s.theme, key.to_string(), tr.t(label));
                        }
                    });
                    ui.checkbox(&mut s.colorblind, tr.t("settings.colorblind"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr.t("settings.text_size"))
                            .strong()
                            .color(pal.accent),
                    );
                    ui.add(egui::Slider::new(&mut s.text_scale, 0.8..=1.6));
                    ui.label(
                        RichText::new(tr.t("settings.speed"))
                            .strong()
                            .color(pal.accent),
                    );
                    ui.add(egui::Slider::new(&mut s.speed, 0.5..=3.0));
                    ui.add_space(8.0);
                    ui.checkbox(&mut s.sound, tr.t("settings.sound"));
                    ui.checkbox(&mut s.unlock_all, tr.t("settings.unlock_all"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr.t("settings.language"))
                            .strong()
                            .color(pal.accent),
                    );
                    egui::ComboBox::from_id_salt("language")
                        .selected_text(s.language.clone())
                        .show_ui(ui, |ui| {
                            for code in i18n::available_languages() {
                                ui.selectable_value(&mut s.language, code.to_string(), code);
                            }
                        });
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr.t("settings.levels_dir"))
                            .small()
                            .color(pal.dim),
                    );
                    ui.add_space(8.0);
                    if confirm {
                        ui.label(RichText::new(tr.t("settings.reset_warning")).color(pal.bad));
                        ui.horizontal(|ui| {
                            if ui.button(tr.t("settings.reset_yes")).clicked() {
                                do_reset = true;
                                confirm = false;
                            }
                            if ui.button(tr.t("ui.cancel")).clicked() {
                                confirm = false;
                            }
                        });
                    } else if ui.button(tr.t("settings.reset")).clicked() {
                        confirm = true;
                    }
                });
            });
    });
    (nav, (confirm, do_reset))
}

fn about_screen(ui: &mut Ui, env: &mut Env<'_>) -> Option<Nav> {
    let mut nav = None;
    let pal = env.pal;
    let tr = env.tr;
    egui::Panel::top("about-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Home, &pal, true, &tr.t("ui.back")).clicked() {
                nav = Some(Nav::Screen(Screen::Title));
            }
            ui.label(
                RichText::new(tr.t("title.about"))
                    .heading()
                    .color(pal.accent),
            );
        });
    });
    egui::CentralPanel::default_margins().show(ui, |ui| {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                centered_column(ui, 640.0, |ui| {
                    ui.label(
                        RichText::new(format!("AlgoClerk {}", env!("CARGO_PKG_VERSION"))).heading(),
                    );
                    ui.label(tr.t("about.text"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr.t("about.license"))
                            .strong()
                            .color(pal.accent),
                    );
                    ui.label(tr.t("about.license_text"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr.t("about.credits"))
                            .strong()
                            .color(pal.accent),
                    );
                    ui.label(tr.t("about.credits_text"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr.t("about.policy"))
                            .strong()
                            .color(pal.accent),
                    );
                    ui.label(tr.t("about.policy_text"));
                    ui.add_space(8.0);
                    stars(ui, 3, 3, &pal, 22.0);
                });
            });
    });
    nav
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every message key used in the sources must exist in the English catalogue.
    #[test]
    fn all_ui_message_keys_exist_in_english() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let en = i18n::english();
        let mut missing = HashSet::new();
        for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let mut rest = text.as_str();
            // Collect every string literal that looks like a catalogue key: a.b or a.b_c.
            while let Some(i) = rest.find('"') {
                let after = &rest[i + 1..];
                let Some(j) = after.find('"') else { break };
                let lit = &after[..j];
                rest = &after[j + 1..];
                let looks_like_key = lit.contains('.')
                    && !lit.contains(' ')
                    && !lit.contains(['/', '\\', ':', '(', '{', '}'])
                    && lit.split('.').all(|p| {
                        !p.is_empty()
                            && p.chars()
                                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
                    })
                    && [
                        "about", "brief", "editor", "medal", "packs", "play", "result", "sandbox",
                        "settings", "tab", "text", "title", "ui", "op",
                    ]
                    .contains(&lit.split('.').next().unwrap_or(""));
                if looks_like_key && en.get(lit).is_none() {
                    missing.insert(lit.to_string());
                }
            }
        }
        assert!(
            missing.is_empty(),
            "missing keys in lang/en.toml: {missing:?}"
        );
    }

    #[test]
    fn every_opcode_has_an_english_tooltip() {
        for op in algoclerk_core::isa::Opcode::ALL {
            assert!(
                i18n::english().get(&format!("op.{}", op.key())).is_some(),
                "{op}"
            );
        }
    }

    #[test]
    fn app_starts_a_level_and_solves_it_through_the_play_state() {
        let mut app = AlgoClerkApp::new(None);
        let id = "max-of";
        app.start_level(id);
        let tr = app.strings.clone();
        let level = app.campaign.find(id).unwrap().clone();
        let play = app.play.as_deref_mut().unwrap();
        // An empty program cannot run.
        assert!(!play.runnable());
        play.listing = algoclerk_core::asm::parse(&level.solution.as_ref().unwrap().asm).unwrap();
        play.after_edit(true);
        play.reset_machine();
        assert!(play.runnable());
        while play.step(&tr).is_some() {}
        assert_eq!(play.finished_ok, Some(true));
        assert!(play.check());
        let v = play.verdict.clone().unwrap();
        assert!(app.progress.record_verdict(&level, &v, "x"));
        assert!(app.progress.record(id).solved);
    }

    #[test]
    fn rewind_and_errors_are_friendly() {
        let mut app = AlgoClerkApp::new(None);
        app.start_level("echo");
        let tr = app.strings.clone();
        let play = app.play.as_deref_mut().unwrap();
        play.listing = algoclerk_core::asm::parse("OUTBOX").unwrap();
        play.after_edit(true);
        play.reset_machine();
        play.step(&tr);
        assert!(
            play.error_bubble
                .as_deref()
                .is_some_and(|m| m.contains("hand is empty"))
        );
        play.step_back();
        assert!(play.error_bubble.is_none());
    }

    #[test]
    fn text_edits_sync_into_the_list_and_bad_text_keeps_the_old_program() {
        let mut app = AlgoClerkApp::new(None);
        app.start_sandbox();
        let tr = app.strings.clone();
        let play = app.play.as_deref_mut().unwrap();
        play.text = "INBOX\nOUTBOX".to_string();
        play.set_text(&tr);
        assert_eq!(play.listing.instruction_count(), 2);
        play.text = "INBOX\nBOGUS".to_string();
        play.set_text(&tr);
        assert!(!play.text_errors.is_empty());
        assert_eq!(
            play.listing.instruction_count(),
            2,
            "the last good program is kept"
        );
        crate::editor::insert_line(
            play,
            crate::editor::NewLine::Op(algoclerk_core::isa::Opcode::Jump),
        );
        assert!(
            play.text.contains("JUMP"),
            "list edits are written back to the text"
        );
        assert!(
            play.text.contains("L1:"),
            "a jump without labels creates one"
        );
    }

    #[test]
    fn progress_roundtrips_through_save_json() {
        let mut app = AlgoClerkApp::new(None);
        app.progress.record_mut("echo").solved = true;
        app.progress.settings.theme = "light".into();
        let json = app.save_json();
        let again = AlgoClerkApp::new(Some(&json));
        assert!(again.progress.record("echo").solved);
        assert_eq!(again.progress.settings.theme, "light");
        let broken = AlgoClerkApp::new(Some("not json"));
        assert_eq!(broken.progress.solved_count(), 0);
    }
}
