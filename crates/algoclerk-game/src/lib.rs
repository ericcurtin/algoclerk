// SPDX-License-Identifier: Apache-2.0
//! The AlgoClerk game, independent of any engine.
//!
//! The platform shell (the Bevy client, a test, or the headless screenshot tool) does four things:
//! 1. build a [`Game`] from [`Assets`] and a [`Storage`];
//! 2. call [`Game::set_display`] when the window changes (this picks landscape or portrait and the scale);
//! 3. call [`Game::tick`] 60 times a second with the current [`Input`], then [`Game::draw`] to render the
//!    low-resolution frame into a [`Canvas`] (480x270 or 270x480 virtual pixels);
//! 4. drain [`Game::take_events`] to play sounds and music, use the clipboard, toggle fullscreen or quit.
//!
//! Because all drawing goes through a software canvas, the whole game can run (and be screenshotted) with no
//! window and no GPU. See `docs/ARCHITECTURE.md`.
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    missing_debug_implementations,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::needless_lifetimes,
    clippy::type_complexity,
    clippy::needless_range_loop,
    clippy::if_same_then_else,
    clippy::useless_format,
    clippy::manual_range_contains,
    clippy::manual_is_multiple_of,
    clippy::assertions_on_constants,
    clippy::single_match,
    clippy::erasing_op,
    clippy::identity_op,
    clippy::needless_return
)]

pub mod about;
pub mod blocks;
pub mod brief;
pub mod clerk;
pub mod editor;
pub mod gfx;
pub mod input;
pub mod layout;
pub mod modal;
pub mod office;
pub mod play;
pub mod result;
pub mod scenarios;
pub mod select;
pub mod session;
pub mod settings;
pub mod storage;
pub mod textedit;
pub mod title;
pub mod transition;
pub mod ui;
pub mod value_view;

pub use input::{Input, Key};
pub use storage::{FileStorage, MemoryStorage, Storage, default_data_dir};
pub use ui::{GameConfig, GameEvent, Nav};

use algoclerk_core::i18n::{self, Strings};
use algoclerk_core::progress::Progress;
use algoclerk_levels::Campaign;
use algoclerk_media::bank::PALETTES;
use algoclerk_media::{ArtBank, Assets, Canvas};
use gfx::Gfx;
use input::Pointer;
use layout::{Display, Hit, Viewport};
use std::sync::Arc;
use transition::Iris;
use ui::{Ctx, View};

/// The file progress is saved to.
pub const PROGRESS_FILE: &str = "progress.json";
/// Crate version (shown on the title screen).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The active screen.
pub enum Scene {
    /// Placeholder while a scene is being replaced.
    Blank,
    /// Title.
    Title(Box<title::Title>),
    /// Level select.
    Select(Box<select::Select>),
    /// Play (also the brief and the sandbox).
    Play(Box<play::Play>),
    /// Victory and code reveal.
    Result(Box<result::ResultScreen>),
    /// Settings.
    Settings(Box<settings::Settings>),
    /// About.
    About(Box<about::About>),
}

impl Scene {
    /// A short name for logs and tests.
    pub fn name(&self) -> &'static str {
        match self {
            Scene::Blank => "blank",
            Scene::Title(_) => "title",
            Scene::Select(_) => "select",
            Scene::Play(_) => "play",
            Scene::Result(_) => "result",
            Scene::Settings(_) => "settings",
            Scene::About(_) => "about",
        }
    }
}

/// The game.
pub struct Game {
    /// Art, fonts and palettes.
    pub bank: Arc<ArtBank>,
    /// All levels.
    pub campaign: Arc<Campaign>,
    /// Saved progress and settings.
    pub progress: Progress,
    /// Text for the current language.
    pub tr: Strings,
    storage: Box<dyn Storage>,
    /// Shell configuration.
    pub config: GameConfig,
    /// Screen geometry.
    pub vp: Viewport,
    /// The active screen.
    pub scene: Scene,
    iris: Option<(Iris, Option<Nav>)>,
    ptr: Pointer,
    events: Vec<GameEvent>,
    /// Frame counter (60 per second).
    pub tick: u64,
    dirty: bool,
    clipboard: String,
    pending_paste: Option<String>,
    music: Option<&'static str>,
    last_pointer: (i32, i32),
}

fn extra_levels(campaign: &mut Campaign) {
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

impl Game {
    /// Build the game. Fails with a readable message if the art is invalid.
    ///
    /// # Errors
    /// All art problems, one per line.
    pub fn new(
        assets: &Assets,
        storage: Box<dyn Storage>,
        config: GameConfig,
    ) -> Result<Game, String> {
        let bank = ArtBank::load(assets).map_err(|e| e.join("\n"))?;
        let mut campaign = Campaign::shipped();
        extra_levels(&mut campaign);
        let progress = storage
            .read(PROGRESS_FILE)
            .map_or_else(Progress::default, |t| Progress::from_json(&t));
        let tr = i18n::load_language(&progress.settings.language)
            .unwrap_or_else(|| i18n::english().clone());
        let bank = Arc::new(bank);
        let vp = Viewport::new(false, config.touch);
        let outfit = play::outfit_of(&progress, &bank);
        let mut g = Game {
            scene: Scene::Title(Box::new(title::Title::new(&bank, &outfit))),
            bank,
            campaign: Arc::new(campaign),
            progress,
            tr,
            storage,
            config,
            vp,
            iris: None,
            ptr: Pointer::default(),
            events: Vec::new(),
            tick: 0,
            dirty: false,
            clipboard: String::new(),
            pending_paste: None,
            music: None,
            last_pointer: (240, 135),
        };
        g.set_music();
        Ok(g)
    }

    /// Tell the game about the window: orientation, scale and safe area are resolved here.
    pub fn set_display(&mut self, d: &Display) {
        self.vp = d.vp;
        self.vp.touch = self.config.touch;
    }

    /// Switch language (falls back to English).
    pub fn set_language(&mut self, code: &str) {
        self.tr = i18n::load_language(code).unwrap_or_else(|| i18n::english().clone());
    }

    /// Index of the active palette.
    pub fn palette_index(&self) -> usize {
        PALETTES
            .iter()
            .position(|(id, _)| *id == self.progress.settings.palette)
            .unwrap_or(0)
    }

    /// Take the events produced since the last call.
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// Save progress now.
    pub fn save(&mut self) {
        if self
            .storage
            .write(PROGRESS_FILE, &self.progress.to_json())
            .is_ok()
        {
            self.events.push(GameEvent::Saved);
        }
        self.dirty = false;
    }

    /// Called when the app is about to be suspended or closed.
    pub fn on_suspend(&mut self) {
        if self.dirty {
            self.save();
        }
    }

    fn set_music(&mut self) {
        let want = match &self.scene {
            Scene::Play(_) => Some("office"),
            Scene::Result(r) if r.stage == result::Stage::Victory => Some("victory"),
            Scene::Result(_) => Some("office"),
            _ => Some("title"),
        };
        if want != self.music {
            self.music = want;
            self.events.push(GameEvent::Music(want));
        }
    }

    /// Hit rectangles of the active screen (layout tests).
    pub fn hits(&self) -> Vec<Hit> {
        let vp = self.vp;
        match &self.scene {
            Scene::Title(_) => title::Title::hits(&vp, self.config.can_quit),
            Scene::Select(_) => {
                let counts: Vec<usize> =
                    self.campaign.packs.iter().map(|p| p.levels.len()).collect();
                select::Select::hits(&vp, &counts)
            }
            Scene::Play(p) => {
                let mut v = p.hits(&vp);
                if p.modal.is_some() {
                    v.extend(modal::Modal::hits(&vp));
                }
                if p.memo.is_some() {
                    let r = play::memo_rect(&vp);
                    let m = brief::Memo::rects(r, vp.btn_h(), vp.portrait);
                    v.push(Hit::area("memo", r));
                    v.push(Hit::tap("memo hint", m.hint));
                    v.push(Hit::tap("memo reveal", m.reveal));
                    v.push(Hit::tap("memo primary", m.primary));
                }
                v
            }
            Scene::Result(r) => r.hits(&vp),
            Scene::Settings(_) => settings::Settings::hits(&vp, self.config.can_quit),
            Scene::About(_) => about::About::hits(&vp),
            Scene::Blank => Vec::new(),
        }
    }

    /// Advance the game by one tick (60 per second).
    pub fn tick(&mut self, input: &Input) {
        self.tick += 1;
        let mut inp = input.clone();
        if let Some(p) = self.pending_paste.take() {
            inp.paste = Some(p);
        }
        if let Some(p) = inp.pointer {
            self.last_pointer = p;
        }
        self.ptr.update(&inp);
        // a transition in progress swallows input
        if let Some((iris, nav)) = &mut self.iris {
            if iris.tick() {
                if let Some(n) = nav.take() {
                    self.apply_nav(n);
                }
            }
            if self.iris.as_ref().is_some_and(|(i, _)| i.done()) {
                self.iris = None;
            }
            return;
        }
        let nav = self.update_scene(&inp);
        if let Some(n) = nav {
            self.start_nav(n);
        }
        // events that need the game's own handling
        let mut keep = Vec::new();
        for e in std::mem::take(&mut self.events) {
            match &e {
                GameEvent::Copy(t) => {
                    self.clipboard.clone_from(t);
                    keep.push(e);
                }
                GameEvent::PasteRequest if !self.config.system_clipboard => {
                    self.pending_paste = Some(self.clipboard.clone());
                }
                GameEvent::SettingsChanged | GameEvent::Fullscreen(_) => {
                    keep.push(e);
                }
                _ => keep.push(e),
            }
        }
        self.events = keep;
        // language and palette changes take effect immediately
        if self.tr.get("meta.language")
            != i18n::load_language(&self.progress.settings.language)
                .and_then(|s| s.get("meta.language").map(String::from))
                .as_deref()
        {
            let l = self.progress.settings.language.clone();
            self.set_language(&l);
        }
        if self.dirty && self.tick % 300 == 0 {
            self.save();
        }
        self.set_music();
    }

    fn update_scene(&mut self, input: &Input) -> Option<Nav> {
        let mut cx = Ctx {
            input,
            ptr: &self.ptr,
            vp: self.vp,
            tr: &self.tr,
            progress: &mut self.progress,
            campaign: &self.campaign,
            bank: &self.bank,
            events: &mut self.events,
            tick: self.tick,
            config: &self.config,
            dirty: &mut self.dirty,
        };
        match &mut self.scene {
            Scene::Title(s) => s.update(&mut cx),
            Scene::Select(s) => s.update(&mut cx),
            Scene::Play(s) => s.update(&mut cx),
            Scene::Result(s) => s.update(&mut cx),
            Scene::Settings(s) => s.update(&mut cx),
            Scene::About(s) => s.update(&mut cx),
            Scene::Blank => None,
        }
    }

    /// Start a transition to another screen (a plain cut with reduced motion).
    pub fn start_nav(&mut self, nav: Nav) {
        if self.progress.settings.reduce_motion {
            self.apply_nav(nav);
        } else {
            self.iris = Some((Iris::new(self.last_pointer), Some(nav)));
        }
    }

    /// Switch screens immediately.
    pub fn goto(&mut self, nav: Nav) {
        self.iris = None;
        self.apply_nav(nav);
    }

    fn apply_nav(&mut self, nav: Nav) {
        let old = std::mem::replace(&mut self.scene, Scene::Blank);
        // a Ctx for scene construction
        let mut cx = Ctx {
            input: &Input::default(),
            ptr: &self.ptr,
            vp: self.vp,
            tr: &self.tr,
            progress: &mut self.progress,
            campaign: &self.campaign,
            bank: &self.bank,
            events: &mut self.events,
            tick: self.tick,
            config: &self.config,
            dirty: &mut self.dirty,
        };
        let outfit = play::outfit_of(cx.progress, cx.bank);
        let next = match nav {
            Nav::Title => Scene::Title(Box::new(title::Title::new(cx.bank, &outfit))),
            Nav::Select => {
                let id = cx.progress.last_level.clone();
                Scene::Select(Box::new(select::Select::new(&cx, &id)))
            }
            Nav::Brief(id) => match play::Play::level(&mut cx, &id, true) {
                Ok(p) => Scene::Play(Box::new(p)),
                Err(_) => Scene::Select(Box::new(select::Select::new(&cx, &id))),
            },
            Nav::Play(id) => match play::Play::level(&mut cx, &id, false) {
                Ok(p) => Scene::Play(Box::new(p)),
                Err(_) => Scene::Select(Box::new(select::Select::new(&cx, &id))),
            },
            Nav::Sandbox => Scene::Play(Box::new(play::Play::sandbox(&mut cx))),
            Nav::Settings => Scene::Settings(Box::new(settings::Settings::new(&cx))),
            Nav::About => Scene::About(Box::default()),
            Nav::Victory => match old {
                Scene::Play(p) => {
                    let improved = p.improved;
                    let mut r = result::ResultScreen::new(&mut cx, p, true);
                    r.new_best = improved;
                    Scene::Result(Box::new(r))
                }
                Scene::Result(mut r) => {
                    r.stage = result::Stage::Victory;
                    Scene::Result(r)
                }
                other => other,
            },
            Nav::Code => match old {
                Scene::Play(p) => {
                    Scene::Result(Box::new(result::ResultScreen::new(&mut cx, p, false)))
                }
                Scene::Result(mut r) => {
                    r.stage = result::Stage::Code;
                    Scene::Result(r)
                }
                other => other,
            },
            Nav::BackToPlay => match old {
                Scene::Result(r) => Scene::Play(r.play),
                other => other,
            },
            Nav::Quit => {
                cx.events.push(GameEvent::Quit);
                old
            }
        };
        self.scene = next;
        if self.dirty {
            self.save();
        }
        self.set_music();
    }

    /// Render the current frame. The canvas is resized to the virtual screen when needed.
    pub fn draw(&self, canvas: &mut Canvas) {
        let vp = self.vp;
        if canvas.w != vp.w as usize || canvas.h != vp.h as usize {
            *canvas = Canvas::new(vp.w as usize, vp.h as usize);
        }
        canvas.reset_clip();
        let pal = &self.bank.palettes[self.palette_index()];
        let ink = pal.col('K');
        {
            let mut g = Gfx {
                c: canvas,
                bank: &self.bank,
                pal,
                tick: self.tick,
                reduce_motion: self.progress.settings.reduce_motion,
                off: (0, 0),
            };
            let v = View {
                vp,
                tr: &self.tr,
                progress: &self.progress,
                campaign: &self.campaign,
                ptr: &self.ptr,
                tick: self.tick,
                config: &self.config,
            };
            g.c.clear(ink);
            match &self.scene {
                Scene::Title(s) => s.draw(&mut g, &v),
                Scene::Select(s) => s.draw(&mut g, &v),
                Scene::Play(s) => s.draw(&mut g, &v),
                Scene::Result(s) => s.draw(&mut g, &v),
                Scene::Settings(s) => s.draw(&mut g, &v),
                Scene::About(s) => s.draw(&mut g, &v),
                Scene::Blank => {}
            }
        }
        canvas.reset_clip();
        if let Some((iris, _)) = &self.iris {
            iris.draw(canvas, ink);
        }
    }

    /// The play screen, if that is the active scene.
    pub fn play_mut(&mut self) -> Option<&mut play::Play> {
        match &mut self.scene {
            Scene::Play(p) => Some(p),
            _ => None,
        }
    }

    /// The result screen, if that is the active scene.
    pub fn result_mut(&mut self) -> Option<&mut result::ResultScreen> {
        match &mut self.scene {
            Scene::Result(r) => Some(r),
            _ => None,
        }
    }

    /// Run a closure with a construction context (used by screenshot scenarios).
    pub fn with_ctx<R>(&mut self, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
        let mut cx = Ctx {
            input: &Input::default(),
            ptr: &self.ptr,
            vp: self.vp,
            tr: &self.tr,
            progress: &mut self.progress,
            campaign: &self.campaign,
            bank: &self.bank,
            events: &mut self.events,
            tick: self.tick,
            config: &self.config,
            dirty: &mut self.dirty,
        };
        f(&mut cx)
    }
}
