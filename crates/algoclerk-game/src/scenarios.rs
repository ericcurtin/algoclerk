// SPDX-License-Identifier: Apache-2.0
//! Scripted screens for documentation screenshots, golden-image smoke tests and the layout tests.
//! Everything runs through the real [`Game`] (no GPU, no window): the scenarios build a game state, tick it
//! a number of frames and draw.

use crate::input::{Input, Key};
use crate::layout::{Display, Viewport};
use crate::play::Mode;
use crate::result::Stage;
use crate::storage::MemoryStorage;
use crate::ui::{GameConfig, Nav};
use crate::Game;
use algoclerk_core::asm;
use algoclerk_media::{Assets, Canvas};

/// All scenario names, in documentation order.
pub const NAMES: [&str; 16] = [
    "title", "select", "brief", "play", "play-error", "play-text", "play-palette", "victory", "code-c", "code-cpp",
    "code-rust", "sandbox", "settings", "about", "play-keyboard", "select-locked",
];

/// What to render.
#[derive(Debug, Clone)]
pub struct Spec {
    /// Scenario name from [`NAMES`].
    pub name: String,
    /// Portrait phone layout.
    pub portrait: bool,
    /// `warm`, `colorblind` or `contrast`.
    pub palette: String,
    /// Reading text size (1 or 2).
    pub text_size: u8,
    /// Reduce motion.
    pub reduce_motion: bool,
    /// Touch sizing (implied by portrait).
    pub touch: bool,
}

impl Spec {
    /// A landscape scenario with default settings.
    pub fn new(name: &str) -> Spec {
        Spec {
            name: name.to_string(),
            portrait: false,
            palette: "warm".into(),
            text_size: 1,
            reduce_motion: false,
            touch: false,
        }
    }
}

fn tick_n(g: &mut Game, n: u32) {
    let i = Input::default();
    for _ in 0..n {
        g.tick(&i);
    }
}

fn press(g: &mut Game, k: Key) {
    let i = Input { keys: vec![k], ..Input::default() };
    g.tick(&i);
}

fn click(g: &mut Game, x: i32, y: i32) {
    let down = Input { pointer: Some((x, y)), down: true, pressed: true, ..Input::default() };
    g.tick(&down);
    let up = Input { pointer: Some((x, y)), released: true, ..Input::default() };
    g.tick(&up);
}

/// Load the reference solution of the current level into the play screen.
fn load_solution(g: &mut Game) -> Result<(), String> {
    let p = g.play_mut().ok_or("not on the play screen")?;
    let text = p.st.level.as_ref().and_then(|l| l.solution.as_ref()).map(|s| s.asm.clone()).ok_or("no solution")?;
    p.st.listing = asm::parse(&text).map_err(|e| e[0].to_string())?;
    p.st.after_edit(true);
    p.st.reset_machine();
    p.text.set_text(&p.st.text);
    Ok(())
}

/// Build a game in a scenario and return it (before the final draw).
///
/// # Errors
/// A message when the scenario cannot be set up.
pub fn build(spec: &Spec) -> Result<Game, String> {
    let cfg = GameConfig { can_quit: !spec.portrait, touch: spec.portrait || spec.touch, system_clipboard: false };
    let mut g = Game::new(&Assets::embedded(), Box::new(MemoryStorage::default()), cfg)?;
    g.progress.settings.palette.clone_from(&spec.palette);
    g.progress.settings.text_size = spec.text_size;
    g.progress.settings.reduce_motion = spec.reduce_motion;
    let (w, h) = if spec.portrait { (1080.0, 1920.0) } else { (1920.0, 1080.0) };
    let d: Display = crate::layout::choose(w, h, (0.0, 0.0, 0.0, 0.0), g.config.touch);
    g.set_display(&d);
    // a believable mid-game save for the map screens
    let ids: Vec<String> = g.campaign.all_levels().map(|l| l.id.clone()).collect();
    for (i, id) in ids.iter().enumerate().take(11) {
        let r = g.progress.record_mut(id);
        r.solved = true;
        r.stars = [3, 2, 3, 1, 2, 3, 3, 2][i % 8];
    }
    g.progress.last_level = ids.get(11).cloned().unwrap_or_default();
    let level_id = "sum-pairs";
    match spec.name.as_str() {
        "title" => tick_n(&mut g, 90),
        "select" => {
            g.goto(Nav::Select);
            tick_n(&mut g, 10);
        }
        "select-locked" => {
            g.progress = algoclerk_core::progress::Progress::default();
            g.progress.settings.palette.clone_from(&spec.palette);
            g.goto(Nav::Select);
            tick_n(&mut g, 10);
        }
        "brief" => {
            g.goto(Nav::Brief("running-sum".into()));
            tick_n(&mut g, 6);
            if let Some(p) = g.play_mut() {
                p.st.hints_shown = 1;
            }
        }
        "play" | "play-error" | "play-text" | "play-palette" | "play-keyboard" | "victory" | "code-c" | "code-cpp" | "code-rust" => {
            g.goto(Nav::Play(level_id.into()));
            load_solution(&mut g)?;
            match spec.name.as_str() {
                "play" | "play-palette" | "play-text" | "play-keyboard" => {
                    if let Some(p) = g.play_mut() {
                        p.st.set_speed(0);
                        p.st.toggle_run();
                    }
                    // run until the clerk is walking with a crate overhead, a few instructions in
                    for _ in 0..1500 {
                        tick_n(&mut g, 1);
                        let ok = g.play_mut().is_some_and(|p| {
                            p.st.machine.as_ref().is_some_and(|m| m.steps >= 6) && p.st.office.vis.hand.is_some() && p.st.office.clerk.walking
                        });
                        if ok {
                            break;
                        }
                    }
                    if spec.name == "play-text" || spec.name == "play-keyboard" {
                        if let Some(p) = g.play_mut() {
                            p.mode = Mode::Text;
                            p.text.set_text(&p.st.text);
                            p.text.focused = spec.name == "play-keyboard";
                            p.kbd_open = spec.name == "play-keyboard";
                        }
                    }
                    if spec.name == "play-palette" {
                        if let Some(p) = g.play_mut() {
                            p.ed.sel = Some(3);
                            p.ed.palette_open = true;
                            p.ed.pal_focus = 4;
                        }
                    } else if spec.name == "play" {
                        if let Some(p) = g.play_mut() {
                            p.ed.sel = Some(2);
                        }
                    }
                }
                "play-error" => {
                    let p = g.play_mut().ok_or("no play")?;
                    p.st.listing = asm::parse("    OUTBOX\n").map_err(|e| e[0].to_string())?;
                    p.st.after_edit(true);
                    p.st.reset_machine();
                    p.st.set_speed(1);
                    p.text.set_text(&p.st.text);
                    p.st.toggle_run();
                    tick_n(&mut g, 400);
                }
                _ => {
                    // solve it: Check, then the victory screen (and the code tabs)
                    let ok = g.play_mut().is_some_and(|p| p.st.check());
                    if !ok {
                        return Err("the reference solution does not pass".into());
                    }
                    let level = g.play_mut().and_then(|p| p.st.level.clone()).ok_or("no level")?;
                    let verdict = g.play_mut().and_then(|p| p.st.verdict.clone()).ok_or("no verdict")?;
                    let text = g.play_mut().map(|p| p.st.listing.to_text()).ok_or("no play")?;
                    g.progress.record_verdict(&level, &verdict, &text);
                    g.goto(Nav::Victory);
                    if spec.name == "victory" {
                        tick_n(&mut g, 150);
                    } else {
                        tick_n(&mut g, 4);
                        g.goto(Nav::Code);
                        let idx = match spec.name.as_str() {
                            "code-c" => 0,
                            "code-cpp" => 1,
                            _ => 2,
                        };
                        if let Some(r) = g.result_mut() {
                            r.lang = idx;
                            r.stage = Stage::Code;
                        }
                        tick_n(&mut g, 24);
                    }
                }
            }
        }
        "sandbox" => {
            g.goto(Nav::Sandbox);
            if let Some(p) = g.play_mut() {
                p.st.listing = asm::parse("again:\n    INBOX\n    COPYTO 0\n    ADD 0\n    OUTBOX\n    JUMP again\n").map_err(|e| e[0].to_string())?;
                p.st.after_edit(true);
                p.st.reset_machine();
                p.text.set_text(&p.st.text);
                p.st.set_speed(0);
                p.st.toggle_run();
            }
            for _ in 0..800 {
                tick_n(&mut g, 1);
                if g.play_mut().is_some_and(|p| p.st.machine.as_ref().is_some_and(|m| m.steps >= 9) && p.st.office.vis.hand.is_some()) {
                    break;
                }
            }
        }
        "settings" => {
            g.goto(Nav::Settings);
            tick_n(&mut g, 10);
        }
        "about" => {
            g.goto(Nav::About);
            tick_n(&mut g, 10);
        }
        other => return Err(format!("unknown scenario `{other}`")),
    }
    let _ = (press as fn(&mut Game, Key), click as fn(&mut Game, i32, i32));
    Ok(g)
}

/// Render a scenario to a canvas of the virtual screen size.
///
/// # Errors
/// See [`build`].
pub fn render(spec: &Spec) -> Result<Canvas, String> {
    let g = build(spec)?;
    let vp: Viewport = g.vp;
    let mut c = Canvas::new(vp.w as usize, vp.h as usize);
    g.draw(&mut c);
    Ok(c)
}
