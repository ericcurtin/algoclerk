// SPDX-License-Identifier: Apache-2.0
//! Scripted screens for documentation screenshots, golden-image smoke tests and the layout tests.
//! Everything runs through the real [`Game`] (no GPU, no window): the scenarios build a game state, tick it
//! a number of frames and draw.

use crate::Game;
use crate::editor::Tab;
use crate::input::{Input, Key};
use crate::layout::Display;
use crate::result::Stage;
use crate::storage::MemoryStorage;
use crate::ui::{GameConfig, Nav};
use algoclerk_media::{Assets, Canvas};

/// All scenario names, in documentation order.
pub const NAMES: [&str; 24] = [
    "title",
    "select",
    "select-locked",
    "brief",
    "brief-examples",
    "brief-tools",
    "play",
    "play-grid",
    "play-error",
    "play-text",
    "play-card",
    "play-members",
    "play-drag",
    "play-palette",
    "play-keyboard",
    "play-complete",
    "migrated",
    "victory",
    "code-c",
    "code-cpp",
    "code-rust",
    "sandbox",
    "settings",
    "about",
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

fn click(g: &mut Game, x: i32, y: i32) {
    let down = Input {
        pointer: Some((x, y)),
        down: true,
        pressed: true,
        ..Input::default()
    };
    g.tick(&down);
    let up = Input {
        pointer: Some((x, y)),
        released: true,
        ..Input::default()
    };
    g.tick(&up);
}

fn press(g: &mut Game, k: Key) {
    let i = Input {
        keys: vec![k],
        ..Input::default()
    };
    g.tick(&i);
}

/// Build a game in a scenario and return it (before the final draw).
///
/// # Errors
/// A message when the scenario cannot be set up.
pub fn build(spec: &Spec) -> Result<Game, String> {
    let cfg = GameConfig {
        can_quit: !spec.portrait,
        touch: spec.portrait || spec.touch,
        system_clipboard: false,
    };
    let mut storage = MemoryStorage::default();
    if spec.name == "migrated" {
        // a save written by the ClerkScript era game: version 1, a solved level and a draft in the old syntax
        use crate::storage::Storage;
        let old = r#"{"version":1,"levels":{"two-sum":{"solved":true,"stars":3,"draft":"fn two_sum(nums: vec<int>) -> int { return 0; }"}},"sandbox":"fn main() -> int { return 1; }"}"#;
        let _ = storage.write(crate::PROGRESS_FILE, old);
    }
    let mut g = Game::new(&Assets::embedded(), Box::new(storage), cfg)?;
    g.progress.settings.palette.clone_from(&spec.palette);
    g.progress.settings.text_size = spec.text_size;
    g.progress.settings.reduce_motion = spec.reduce_motion;
    let (w, h) = if spec.portrait {
        (1080.0, 1920.0)
    } else {
        (1920.0, 1080.0)
    };
    let d: Display = crate::layout::choose(w, h, (0.0, 0.0, 0.0, 0.0), g.config.touch);
    g.set_display(&d);
    // a believable mid-game save for the map screens
    let ids: Vec<String> = g.campaign.all_levels().map(|l| l.id.clone()).collect();
    for (i, id) in ids.iter().enumerate().take(24) {
        let r = g.progress.record_mut(id);
        r.solved = true;
        r.stars = [3, 2, 3, 1, 2, 3, 3, 2][i % 8];
    }
    g.progress.last_level = ids.get(24).cloned().unwrap_or_default();
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
        "brief" | "brief-examples" | "brief-tools" => {
            g.goto(Nav::Brief("two-sum".into()));
            tick_n(&mut g, 6);
            let page = match spec.name.as_str() {
                "brief" => 0,
                "brief-examples" => 1,
                _ => 2,
            };
            if let Some(p) = g.play_mut() {
                if let Some(m) = &mut p.memo {
                    m.page = page;
                }
            }
            if page == 2 {
                g.goto(Nav::Brief("sum-the-bills".into()));
                if let Some(p) = g.play_mut() {
                    if let Some(m) = &mut p.memo {
                        m.page = 2;
                    }
                    p.show_hints(2);
                }
            }
            tick_n(&mut g, 4);
        }
        "play" => {
            g.goto(Nav::Play("two-sum".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.load_reference();
            p.session.case = 0;
            p.jump(24);
            tick_n(&mut g, 30);
            if let Some(p) = g.play_mut() {
                p.jump(26);
            }
            tick_n(&mut g, 14);
        }
        "play-grid" => {
            g.goto(Nav::Play("count-islands".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.load_reference();
            p.jump(60);
            tick_n(&mut g, 40);
        }
        "play-error" => {
            g.goto(Nav::Play("sum-the-bills".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.session.set_text("int sumTheBills(const std::vector<int>& bills) {\n    int total = 0;\n    for (std::size_t i = 0; i <= bills.size(); ++i) {\n        total += bills[i];\n    }\n    return total;\n}\n");
            p.run_to_end();
            tick_n(&mut g, 30);
        }
        "play-text" => {
            g.goto(Nav::Play("sum-the-bills".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.panel.tab = Tab::Text;
            p.session.set_text("int sumTheBills(const std::vector<int>& bills) {\n    int total = 0;\n    for (int bill : bills) {\n        total += \"1\";\n    }\n    return total;\n}\n");
            tick_n(&mut g, 4);
        }
        "play-keyboard" => {
            g.goto(Nav::Play("sum-the-bills".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.panel.tab = Tab::Text;
            p.panel.kb_open = true;
            p.load_reference();
            tick_n(&mut g, 4);
        }
        "migrated" => {
            tick_n(&mut g, 4);
            return Ok(g);
        }
        "play-complete" => {
            g.goto(Nav::Play("two-sum".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.panel.tab = Tab::Text;
            p.session.set_text("std::vector<int> twoSum(const std::vector<int>& nums, int target) {\n    std::unordered_map<int, int> seen;\n    seen.\n}\n");
            tick_n(&mut g, 2);
            // put the cursor after `seen.` and let the popup open
            if let Some(p) = g.play_mut() {
                let at = p.session.text().find("seen.").map_or(0, |i| i + 5);
                p.session.edit.cursor = at;
                p.session.edit.anchor = None;
                p.panel.suggest_now(&p.session);
            }
            tick_n(&mut g, 2);
        }
        "play-members" => {
            g.goto(Nav::Play("two-sum".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.load_reference();
            let rows = p.session.rows();
            p.panel.sel = rows.iter().position(|r| r.text == "seen[nums[i]] = i;");
            tick_n(&mut g, 2);
            if let Some(p) = g.play_mut() {
                p.open_card_editor();
                if let Some(c) = p.panel.card.as_mut() {
                    c.line.set_text("seen.");
                    c.pad = crate::editor::Pad::Members;
                }
            }
            tick_n(&mut g, 4);
        }
        "play-drag" => {
            g.goto(Nav::Play("two-sum".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.load_reference();
            tick_n(&mut g, 4);
            let vp = g.vp;
            let p = g.play_mut().ok_or("no play")?;
            let list = p
                .hits(&vp)
                .into_iter()
                .find(|h| h.name == "editor body")
                .map(|h| h.rect)
                .ok_or("no editor body")?;
            let rows = p.session.rows();
            let geo = crate::cards::geometry(&rows, list, if vp.touch { 24 } else { 14 });
            let find = |t: &str| rows.iter().position(|r| r.text.starts_with(t));
            let (src, dst) = (
                find("seen[nums[i]]").ok_or("no source card")?,
                find("for (int i").ok_or("no target card")?,
            );
            let at = |i: usize, lower: bool| {
                let b = &geo.boxes[i];
                (
                    b.x + b.w / 2,
                    list.y + b.y + if lower { b.h - 2 } else { b.h / 2 },
                )
            };
            let (from, to) = (at(src, false), at(dst, true));
            let ev = |pt: (i32, i32), pressed: bool| Input {
                pointer: Some(pt),
                down: true,
                pressed,
                ..Input::default()
            };
            g.tick(&ev(from, true));
            g.tick(&ev(to, false));
            g.tick(&ev(to, false));
        }
        "play-card" | "play-palette" => {
            g.goto(Nav::Play("count-vowels".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.load_reference();
            p.panel.sel = Some(4);
            tick_n(&mut g, 2);
            if let Some(p) = g.play_mut() {
                if spec.name == "play-card" {
                    p.open_card_editor();
                } else {
                    p.panel.palette = Some(false);
                }
            }
            tick_n(&mut g, 4);
        }
        "victory" | "code-c" | "code-cpp" | "code-rust" => {
            g.goto(Nav::Play("two-sum".into()));
            let p = g.play_mut().ok_or("no play")?;
            p.load_reference();
            p.solve_now();
            g.goto(Nav::Victory);
            tick_n(&mut g, if spec.name == "victory" { 80 } else { 4 });
            if spec.name != "victory" {
                if let Some(r) = g.result_mut() {
                    r.stage = Stage::Code;
                    r.lang = match spec.name.as_str() {
                        "code-c" => 0,
                        "code-cpp" => 1,
                        _ => 2,
                    };
                }
                tick_n(&mut g, 2);
            }
        }
        "sandbox" => {
            g.goto(Nav::Sandbox);
            let p = g.play_mut().ok_or("no play")?;
            p.jump(8);
            tick_n(&mut g, 30);
        }
        "settings" => {
            g.goto(Nav::Settings);
            tick_n(&mut g, 6);
        }
        "about" => {
            g.goto(Nav::About);
            tick_n(&mut g, 6);
        }
        other => return Err(format!("unknown scenario {other}")),
    }
    let _ = (
        click as fn(&mut Game, i32, i32),
        press as fn(&mut Game, Key),
    );
    Ok(g)
}

/// Render a scenario to a canvas.
///
/// # Errors
/// A message when the scenario cannot be set up.
pub fn render(spec: &Spec) -> Result<Canvas, String> {
    let g = build(spec)?;
    let mut c = Canvas::new(g.vp.w as usize, g.vp.h as usize);
    g.draw(&mut c);
    Ok(c)
}
