// SPDX-License-Identifier: Apache-2.0
//! Headless rendering: screenshots of real screens and the application icon, through the
//! software rasteriser. No window, display or GPU is involved.

use crate::app::{AlgoClerkApp, Screen};
use crate::play::Tab;
use crate::raster::Rasterizer;
use crate::result::ResultView;
use crate::theme;
use algoclerk_core::asm;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, pos2, vec2};

/// What to render.
#[derive(Clone, Debug)]
pub struct Spec {
    pub screen: String,
    pub width: usize,
    pub height: usize,
    pub scale: f32,
    pub theme: String,
    pub colorblind: bool,
    pub level: String,
    pub steps: usize,
    pub narrow: Option<bool>,
    pub tab: Option<String>,
    /// Language of the code view (`c`, `cpp`, `rust`).
    pub lang: Option<String>,
    /// Number of UI passes to run before painting (animations settle).
    pub frames: usize,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            screen: "title".into(),
            width: 1280,
            height: 800,
            scale: 1.0,
            theme: "dark".into(),
            colorblind: false,
            level: "max-of".into(),
            steps: 14,
            narrow: None,
            tab: None,
            lang: None,
            frames: 12,
        }
    }
}

fn raw_input(w: f32, h: f32, time: f64) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(w, h))),
        time: Some(time),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    }
}

/// Fill the progress with a believable mid-game state for the level list screenshot.
fn demo_progress(app: &mut AlgoClerkApp) {
    let ids: Vec<String> = app.campaign.all_levels().map(|l| l.id.clone()).collect();
    for (i, id) in ids.iter().enumerate().take(19) {
        let r = app.progress.record_mut(id);
        r.solved = true;
        r.stars = [3, 2, 3, 1, 2, 3, 3, 2][i % 8];
    }
    app.progress.last_level = ids.get(19).cloned().unwrap_or_default();
}

fn prepare(spec: &Spec) -> Result<AlgoClerkApp, String> {
    let mut app = AlgoClerkApp::new(None);
    app.progress.settings.theme = spec.theme.clone();
    app.progress.settings.colorblind = spec.colorblind;
    if let Some(l) = &spec.lang {
        app.progress.settings.code_lang.clone_from(l);
    }
    app.force_layout(spec.narrow);
    match spec.screen.as_str() {
        "title" => {}
        "levels" => {
            demo_progress(&mut app);
            app.screen = Screen::Packs;
        }
        "play" | "victory" | "code" | "text" => {
            let level = app
                .campaign
                .find(&spec.level)
                .cloned()
                .ok_or_else(|| format!("no level {}", spec.level))?;
            app.start_level(&level.id);
            let play = app.play.as_deref_mut().ok_or("level did not start")?;
            let sol = level.solution.as_ref().ok_or("no reference")?.asm.clone();
            play.listing = asm::parse(&sol).map_err(|e| e[0].to_string())?;
            play.hints_shown = 1;
            play.after_edit(true);
            play.reset_machine();
            play.tab = match spec.tab.as_deref() {
                Some("brief") => Tab::Brief,
                Some("office") => Tab::Office,
                Some("text") => Tab::Text,
                _ => Tab::Program,
            };
            if spec.screen == "text" {
                play.show_text = true;
            }
            let tr = app.strings.clone();
            let play = app.play.as_deref_mut().ok_or("no play")?;
            for _ in 0..spec.steps {
                play.step(&tr);
            }
            if spec.screen == "victory" || spec.screen == "code" {
                let play = app.play.as_deref_mut().ok_or("no play")?;
                if !play.check() {
                    return Err("the reference solution does not pass".into());
                }
                let solved = spec.screen == "victory";
                let v = play.verdict.clone();
                if let (Some(v), Some(l)) = (v.as_ref(), play.level.as_ref()) {
                    app.progress.record_verdict(l, v, &play.listing.to_text());
                }
                let rv = {
                    let play_ref = app.play.as_deref().ok_or("no play")?;
                    let env_progress = &mut app.progress;
                    let pal = theme::palette(&spec.theme, spec.colorblind);
                    let env = crate::app::Env {
                        pal,
                        tr: &tr,
                        progress: env_progress,
                        campaign: &app.campaign,
                        sound: &app.sound,
                        narrow: false,
                        touch: false,
                        dt: 0.0,
                    };
                    ResultView::new(play_ref, &env, solved)
                };
                app.result = Some(Box::new(rv));
                app.screen = Screen::Result;
            }
        }
        "sandbox" => {
            app.progress.sandbox = "-- double each number\nloop:\n    INBOX\n    COPYTO 0\n    ADD 0\n    OUTBOX\n    JUMP loop\n".into();
            app.start_sandbox();
            if let Some(p) = app.play.as_deref_mut() {
                p.reset_machine();
            }
            let tr = app.strings.clone();
            if let Some(p) = app.play.as_deref_mut() {
                for _ in 0..spec.steps.min(12) {
                    p.step(&tr);
                }
            }
        }
        "settings" => app.screen = Screen::Settings,
        "about" => app.screen = Screen::About,
        other => return Err(format!("unknown screen \"{other}\"")),
    }
    Ok(app)
}

/// Render a screen to an image.
pub fn render(spec: &Spec) -> Result<Rasterizer, String> {
    let mut app = prepare(spec)?;
    let ctx = egui::Context::default();
    crate::fonts::install(&ctx);
    ctx.set_pixels_per_point(spec.scale);
    let (w, h) = (
        spec.width as f32 / spec.scale,
        spec.height as f32 / spec.scale,
    );
    let pal = theme::palette(&spec.theme, spec.colorblind);
    let mut raster = Rasterizer::new(spec.width, spec.height, pal.bg);
    let mut time = 0.0;
    let frames = spec.frames.max(2);
    for i in 0..frames {
        let mut out = ctx.run_ui(raw_input(w, h, time), |ui| app.ui(ui));
        raster.apply_textures(&out.textures_delta);
        out.textures_delta.clear();
        if i + 1 == frames {
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            raster.paint(&prims, out.pixels_per_point);
        }
        time += 1.0 / 60.0;
    }
    Ok(raster)
}

/// Draw the app icon: a clerk behind a crate on a rounded badge.
pub fn render_icon(size: usize) -> Result<Rasterizer, String> {
    let ctx = egui::Context::default();
    crate::fonts::install(&ctx);
    let pal = theme::palette("dark", false);
    let s = size as f32;
    let mut raster = Rasterizer::new(size, size, Color32::TRANSPARENT);
    for i in 0..3 {
        let mut out = ctx.run_ui(raw_input(s, s, f64::from(i) / 60.0), |ui| {
            let p = ui.painter();
            let r = Rect::from_min_size(Pos2::ZERO, vec2(s, s));
            p.rect_filled(r, s * 0.22, pal.accent.gamma_multiply(0.9));
            p.rect_filled(r.shrink(s * 0.04), s * 0.19, pal.panel);
            let c = pos2(s * 0.5, s * 0.46);
            // Body and tie.
            p.rect_filled(
                Rect::from_center_size(pos2(c.x, c.y + s * 0.33), vec2(s * 0.52, s * 0.34)),
                s * 0.1,
                pal.suit,
            );
            p.add(egui::Shape::convex_polygon(
                vec![
                    pos2(c.x - s * 0.03, c.y + s * 0.19),
                    pos2(c.x + s * 0.03, c.y + s * 0.19),
                    pos2(c.x + s * 0.025, c.y + s * 0.4),
                    pos2(c.x, c.y + s * 0.44),
                    pos2(c.x - s * 0.025, c.y + s * 0.4),
                ],
                pal.tie,
                Stroke::NONE,
            ));
            // Head.
            p.circle_filled(c, s * 0.22, pal.skin);
            p.add(egui::Shape::convex_polygon(
                vec![
                    pos2(c.x - s * 0.22, c.y - s * 0.03),
                    pos2(c.x - s * 0.15, c.y - s * 0.2),
                    pos2(c.x + s * 0.15, c.y - s * 0.2),
                    pos2(c.x + s * 0.22, c.y - s * 0.03),
                    pos2(c.x, c.y - s * 0.12),
                ],
                pal.crate_edge,
                Stroke::NONE,
            ));
            for dx in [-0.085, 0.085] {
                let e = pos2(c.x + s * dx, c.y + s * 0.02);
                p.circle_stroke(e, s * 0.062, Stroke::new(s * 0.014, Color32::from_gray(40)));
                p.circle_filled(e, s * 0.02, Color32::from_gray(30));
            }
            p.add(egui::Shape::line(
                (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0 * 2.0 - 1.0;
                        pos2(
                            c.x + t * s * 0.07,
                            c.y + s * 0.125 + s * 0.04 * (1.0 - t * t),
                        )
                    })
                    .collect(),
                Stroke::new(s * 0.016, Color32::from_gray(60)),
            ));
            // Crate with an "A" on the desk.
            let b = Rect::from_center_size(pos2(s * 0.5, s * 0.8), vec2(s * 0.3, s * 0.2));
            p.rect_filled(b, s * 0.03, pal.crate_fill);
            p.rect_stroke(
                b,
                s * 0.03,
                Stroke::new(s * 0.014, pal.crate_edge),
                egui::StrokeKind::Inside,
            );
            p.text(
                b.center(),
                egui::Align2::CENTER_CENTER,
                "A",
                egui::FontId::monospace(s * 0.15),
                Color32::from_rgb(30, 22, 12),
            );
        });
        raster.apply_textures(&out.textures_delta);
        out.textures_delta.clear();
        if i == 2 {
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            raster.paint(&prims, out.pixels_per_point);
        }
    }
    Ok(raster)
}

const HELP: &str = "algoclerk (desktop game)

USAGE:
    algoclerk                          Open the game window
    algoclerk --screenshot <screen> --out <file.png> [options]
    algoclerk --icon <size> --out <file.png>

SCREENSHOT SCREENS: title levels play victory code text sandbox settings about
OPTIONS: --width N --height N --scale F --theme dark|light|contrast --colorblind
         --level <id> --steps N --narrow | --wide --tab brief|program|office|text
         --lang c|cpp|rust --frames N
Screenshots use a software renderer: no window, display or GPU needed.
";

/// Command line for headless rendering. Returns the process exit code.
pub fn cli(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return 0;
    }
    let mut spec = Spec::default();
    let mut out: Option<String> = None;
    let mut icon: Option<usize> = None;
    let mut i = 0;
    let get = |i: &mut usize| -> Option<String> {
        *i += 1;
        args.get(*i).cloned()
    };
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => spec.screen = get(&mut i).unwrap_or_default(),
            "--out" => out = get(&mut i),
            "--width" => {
                spec.width = get(&mut i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(spec.width)
            }
            "--height" => {
                spec.height = get(&mut i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(spec.height)
            }
            "--scale" => spec.scale = get(&mut i).and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--theme" => spec.theme = get(&mut i).unwrap_or_default(),
            "--level" => spec.level = get(&mut i).unwrap_or_default(),
            "--steps" => {
                spec.steps = get(&mut i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(spec.steps)
            }
            "--tab" => spec.tab = get(&mut i),
            "--lang" => spec.lang = get(&mut i),
            "--frames" => {
                spec.frames = get(&mut i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(spec.frames)
            }
            "--icon" => icon = get(&mut i).and_then(|v| v.parse().ok()),
            "--colorblind" => spec.colorblind = true,
            "--narrow" => spec.narrow = Some(true),
            "--wide" => spec.narrow = Some(false),
            other => {
                eprintln!("unknown option {other}\n{HELP}");
                return 2;
            }
        }
        i += 1;
    }
    let Some(out) = out else {
        eprintln!("--out <file.png> is required\n{HELP}");
        return 2;
    };
    let result = match icon {
        Some(size) => render_icon(size),
        None => render(&spec),
    };
    match result.and_then(|r| {
        let (lum, colours) = r.stats();
        eprintln!(
            "rendered {}x{} (mean luminance {lum:.2}, {colours} distinct colours)",
            r.width, r.height
        );
        r.to_png()
    }) {
        Ok(png) => match std::fs::write(&out, png) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("cannot write {out}: {e}");
                1
            }
        },
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}
