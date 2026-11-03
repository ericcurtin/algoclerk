// SPDX-License-Identifier: Apache-2.0
//! GPU-free UI tests: every scenario renders in both orientations, no interactive rectangle leaves the
//! virtual screen at several window sizes, the pixel font covers every string and every generated code
//! sample, and the software renderer is deterministic and close to stored coarse "golden" maps.

use algoclerk_core::i18n;
use algoclerk_core::judge::Judge;
use algoclerk_game::layout::{TOUCH_MIN, choose};
use algoclerk_game::scenarios::{NAMES, Spec, build, render};
use algoclerk_game::{Game, GameConfig, MemoryStorage};
use algoclerk_levels::Campaign;
use algoclerk_media::{ArtBank, Assets, Canvas};
use std::collections::HashSet;

fn bank() -> ArtBank {
    ArtBank::load(&Assets::embedded()).expect("art loads")
}

fn distinct_colours(c: &Canvas) -> usize {
    let mut set = HashSet::new();
    for y in 0..c.h {
        for x in 0..c.w {
            set.insert(c.get(x, y));
        }
    }
    set.len()
}

#[test]
fn every_scenario_renders_in_both_orientations() {
    for name in NAMES {
        for portrait in [false, true] {
            let spec = Spec {
                portrait,
                touch: portrait,
                ..Spec::new(name)
            };
            let c = render(&spec).unwrap_or_else(|e| panic!("{name} portrait={portrait}: {e}"));
            assert_eq!(
                (c.w, c.h),
                if portrait { (270, 480) } else { (480, 270) },
                "{name}"
            );
            assert!(
                distinct_colours(&c) >= 6,
                "{name} portrait={portrait} looks blank"
            );
            let again = render(&spec).unwrap();
            assert_eq!(
                c.checksum(),
                again.checksum(),
                "{name} portrait={portrait} is not deterministic"
            );
        }
    }
}

#[test]
fn palettes_and_text_sizes_render() {
    for palette in ["warm", "colorblind", "contrast"] {
        for text_size in [1u8, 2] {
            for name in ["title", "play", "brief", "victory"] {
                let spec = Spec {
                    palette: palette.into(),
                    text_size,
                    ..Spec::new(name)
                };
                render(&spec).unwrap_or_else(|e| panic!("{name}/{palette}/{text_size}: {e}"));
            }
        }
    }
}

#[test]
fn no_interactive_rectangle_leaves_the_screen() {
    let windows = [
        (1920.0, 1080.0),
        (1280.0, 720.0),
        (1024.0, 768.0),
        (800.0, 600.0),
        (1080.0, 1920.0),
        (720.0, 1280.0),
        (390.0, 844.0),
        (844.0, 390.0),
        (480.0, 270.0),
    ];
    for name in NAMES {
        for (w, h) in windows {
            let touch = h > w || w < 900.0;
            let spec = Spec {
                touch,
                ..Spec::new(name)
            };
            let mut g = build(&spec).unwrap();
            let d = choose(w, h, (0.0, 0.0, 0.0, 0.0), touch);
            g.set_display(&d);
            // let layout-dependent state (scroll positions) settle
            let i = algoclerk_game::Input::default();
            for _ in 0..3 {
                g.tick(&i);
            }
            let screen = d.vp.screen();
            for hit in g.hits() {
                assert!(
                    hit.rect.inside(&screen),
                    "{name} @{w}x{h}: {} {:?} leaves the {}x{} screen",
                    hit.name,
                    hit.rect,
                    screen.w,
                    screen.h
                );
                if hit.tap && touch && w >= 720.0 && h > w {
                    assert!(
                        hit.rect.w >= TOUCH_MIN - 1 && hit.rect.h >= TOUCH_MIN - 1,
                        "{name} @{w}x{h}: {} {:?} is too small to tap",
                        hit.name,
                        hit.rect
                    );
                }
            }
        }
    }
}

fn strings_of_catalogue(code: &str) -> Vec<String> {
    let s = i18n::load_language(code).expect("catalogue");
    s.keys()
        .iter()
        .filter_map(|k| s.get(k).map(String::from))
        .collect()
}

#[test]
fn the_pixel_font_covers_every_shipped_string() {
    let b = bank();
    for code in i18n::available_languages() {
        for text in strings_of_catalogue(code) {
            let m = b.font.missing(&text);
            assert!(m.is_empty(), "{code}: font lacks {m:?} in {text:?}");
        }
    }
}

#[test]
fn the_pixel_font_covers_all_level_text_and_all_reference_code() {
    let b = bank();
    let campaign = Campaign::shipped();
    let mut missing: Vec<String> = Vec::new();
    for level in campaign.all_levels() {
        let mut texts: Vec<String> = vec![
            level.title.clone(),
            level.story.clone(),
            level.statement.clone(),
            level.technique.clone(),
            level.big_o.clone(),
            level.signature_text.clone(),
            level.prelude.clone(),
        ];
        texts.extend(level.constraints.iter().cloned());
        texts.extend(level.hints.iter().cloned());
        texts.extend(level.locked.iter().map(|l| l.why.clone()));
        texts.extend(level.examples.iter().flat_map(|e| {
            e.args
                .iter()
                .cloned()
                .chain([e.expect.clone(), e.note.clone()])
        }));
        texts.push(level.reference.clone());
        texts.extend(level.naive.iter().cloned());
        for t in texts {
            let m = b.font.missing(&t.replace('\n', " "));
            if !m.is_empty() {
                missing.push(format!("{}: {m:?}", level.id));
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "glyphs missing from the pixel font:\n{}",
        missing.join("\n")
    );
}

#[test]
fn ascii_the_code_generators_can_emit_is_covered() {
    let b = bank();
    let all: String = (0x20u8..0x7f).map(|c| c as char).collect();
    assert!(
        b.font.missing(&all).is_empty(),
        "{:?}",
        b.font.missing(&all)
    );
}

/// A coarse map of the frame: 48x27 cells, each the most common palette-ish class of its pixels.
fn coarse(c: &Canvas) -> String {
    let (cw, ch) = (c.w / 40 + 1, c.h / 23 + 1);
    let mut out = String::new();
    for cy in 0..c.h.div_ceil(ch) {
        for cx in 0..c.w.div_ceil(cw) {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for y in cy * ch..((cy + 1) * ch).min(c.h) {
                for x in cx * cw..((cx + 1) * cw).min(c.w) {
                    let p = c.get(x, y);
                    r += u32::from(p[0]);
                    g += u32::from(p[1]);
                    b += u32::from(p[2]);
                    n += 1;
                }
            }
            let lum = (r + g + b) / (3 * n.max(1));
            out.push(char::from(b'0' + (lum / 26).min(9) as u8));
        }
        out.push('\n');
    }
    out
}

#[test]
fn rendering_stays_close_to_the_stored_coarse_golden_maps() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    for name in ["title", "play", "brief", "victory", "code-rust"] {
        let c = render(&Spec::new(name)).unwrap();
        let map = coarse(&c);
        let path = dir.join(format!("{name}.txt"));
        if update || !path.exists() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &map).unwrap();
            continue;
        }
        // Windows checkouts may turn the golden files into CRLF.
        let want = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\r\n", "\n");
        let (a, b): (Vec<char>, Vec<char>) = (map.chars().collect(), want.chars().collect());
        assert_eq!(a.len(), b.len(), "{name}: shape changed");
        let same = a
            .iter()
            .zip(b.iter())
            .filter(|(x, y)| (**x as i32 - **y as i32).abs() <= 1)
            .count();
        assert!(
            same * 100 >= a.len() * 93,
            "{name}: only {}% of the coarse map matches the golden (run with UPDATE_GOLDEN=1 after an intended change)",
            same * 100 / a.len()
        );
    }
}

#[test]
fn a_full_game_session_through_real_input_events() {
    use algoclerk_game::{Input, Key};
    let cfg = GameConfig {
        can_quit: true,
        touch: false,
        system_clipboard: false,
    };
    let mut g = Game::new(&Assets::embedded(), Box::new(MemoryStorage::default()), cfg).unwrap();
    let d = choose(1920.0, 1080.0, (0.0, 0.0, 0.0, 0.0), false);
    g.set_display(&d);
    let tick = |g: &mut Game, i: Input| g.tick(&i);
    for _ in 0..5 {
        tick(&mut g, Input::default());
    }
    assert_eq!(g.scene.name(), "title");
    // Start the campaign with the keyboard: the first menu entry opens the next level's memo
    tick(
        &mut g,
        Input {
            keys: vec![Key::Enter],
            ..Input::default()
        },
    );
    for _ in 0..60 {
        tick(&mut g, Input::default());
    }
    assert_eq!(g.scene.name(), "play");
    // dismiss the memo, load the reference, run and check
    tick(
        &mut g,
        Input {
            keys: vec![Key::Escape],
            ..Input::default()
        },
    );
    let p = g.play_mut().unwrap();
    p.load_reference();
    g.play_mut().unwrap().jump(3);
    for _ in 0..30 {
        tick(&mut g, Input::default());
    }
    // press Ctrl+Enter to check
    tick(
        &mut g,
        Input {
            keys: vec![Key::Enter],
            ctrl: true,
            ..Input::default()
        },
    );
    for _ in 0..60 {
        tick(&mut g, Input::default());
    }
    assert_eq!(
        g.scene.name(),
        "result",
        "a correct program should lead to the victory screen"
    );
    assert!(g.progress.solved_count() >= 1);
    let _ = Judge::new;
}

#[test]
fn every_catalogue_key_used_in_the_code_exists() {
    let en = i18n::english();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for marker in [".t(\"", ".format(\""] {
            let mut rest = text.as_str();
            while let Some(i) = rest.find(marker) {
                rest = &rest[i + marker.len()..];
                if let Some(j) = rest.find('"') {
                    let key = &rest[..j];
                    if key.contains('.')
                        && key.chars().all(|c| {
                            c.is_ascii_lowercase() || c == '.' || c == '_' || c.is_ascii_digit()
                        })
                        && en.get(key).is_none()
                    {
                        missing.push(format!(
                            "{}: {key}",
                            path.file_name().unwrap().to_string_lossy()
                        ));
                    }
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "keys used in the code but missing from en.toml:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_song_parses_and_the_game_has_all_it_asks_for() {
    use algoclerk_media::synth::Song;
    let assets = Assets::embedded();
    for name in [
        "title", "office", "victory", "button", "pick", "put", "step", "correct", "wrong", "stars",
    ] {
        let text = assets
            .text(&format!("music/{name}.song"))
            .unwrap_or_else(|| panic!("missing song {name}"));
        let song = Song::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!song.render().is_empty(), "{name} renders to silence");
    }
    // every sound the code asks for exists
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(src).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        let mut rest = text.as_str();
        while let Some(i) = rest.find("sfx(\"") {
            rest = &rest[i + 5..];
            let name = &rest[..rest.find('"').unwrap()];
            assert!(
                assets.text(&format!("music/{name}.song")).is_some(),
                "sound {name} has no song"
            );
        }
    }
}

#[test]
fn random_input_never_panics_on_any_screen() {
    use algoclerk_core::testgen::Rng;
    use algoclerk_game::{Input, Key};
    const KEYS: [Key; 15] = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Escape,
        Key::Backspace,
        Key::Delete,
        Key::Tab,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
        Key::Space,
        Key::Char('r'),
    ];
    let seed = std::env::var("ALGOCLERK_FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let rng = Rng::new(seed);
    for name in NAMES {
        for portrait in [false, true] {
            let spec = Spec {
                portrait,
                touch: portrait,
                ..Spec::new(name)
            };
            let mut g = build(&spec).unwrap();
            let (w, h) = (g.vp.w, g.vp.h);
            let mut down = false;
            for _ in 0..500 {
                let mut i = Input::default();
                if rng.chance(30) {
                    i.keys
                        .push(KEYS[rng.range(0, KEYS.len() as i64 - 1) as usize]);
                }
                if rng.chance(20) {
                    i.typed = ["a", "{", ")", "\"", " ", "\n", "9", "é", "\t"]
                        [rng.range(0, 8) as usize]
                        .to_string();
                }
                i.pointer = Some((
                    rng.range(-5, i64::from(w) + 5) as i32,
                    rng.range(-5, i64::from(h) + 5) as i32,
                ));
                i.ctrl = rng.chance(5);
                i.shift = rng.chance(10);
                i.touch = portrait;
                if rng.chance(15) {
                    i.wheel = rng.range(-3, 3) as f32;
                }
                if !down && rng.chance(25) {
                    i.pressed = true;
                    down = true;
                } else if down && rng.chance(40) {
                    i.released = true;
                    down = false;
                }
                i.down = down;
                g.tick(&i);
                if rng.chance(10) {
                    let mut c = Canvas::new(w as usize, h as usize);
                    g.draw(&mut c);
                }
                let _ = g.take_events();
            }
        }
    }
}

#[test]
fn the_autocomplete_popup_opens_navigates_and_accepts() {
    use algoclerk_game::{Input, Key};
    let mut g = build(&Spec::new("play-complete")).unwrap();
    let pop = g
        .play_mut()
        .unwrap()
        .panel
        .ac
        .clone()
        .expect("popup after `seen.`");
    assert!(pop.items.len() > 3);
    let second = pop.items[1].insert.clone();
    let tick = |g: &mut Game, keys: Vec<Key>, typed: &str| {
        g.tick(&Input {
            keys,
            typed: typed.to_string(),
            ..Input::default()
        });
    };
    // Escape closes the popup without leaving the screen
    tick(&mut g, vec![Key::Escape], "");
    assert_eq!(g.scene.name(), "play");
    assert!(g.play_mut().unwrap().panel.ac.is_none());
    // typing a letter reopens it with the filtered list; Down then Enter picks the second suggestion
    tick(&mut g, vec![], "e");
    let pop = g
        .play_mut()
        .unwrap()
        .panel
        .ac
        .clone()
        .expect("popup after `seen.e`");
    assert!(pop.items.iter().all(|i| i.label.starts_with('e')));
    tick(&mut g, vec![Key::Down], "");
    let want = pop.items[1].insert.clone();
    tick(&mut g, vec![Key::Enter], "");
    let text = g.play_mut().unwrap().session.text();
    assert!(text.contains(&format!("seen.{want}")), "{text}");
    assert!(!text.contains("seen.ee"), "{text}");
    assert!(g.play_mut().unwrap().panel.ac.is_none());
    let _ = second;
}

#[test]
fn an_old_save_loads_with_a_one_time_note_and_progress_kept() {
    use algoclerk_game::{Input, Key};
    let mut g = build(&Spec::new("migrated")).unwrap();
    assert_eq!(g.scene.name(), "title");
    assert!(!g.progress.migration_note.is_empty());
    assert!(
        g.progress.record("two-sum").draft.is_empty(),
        "old solutions are discarded"
    );
    assert!(g.progress.sandbox.is_empty());
    // the notice swallows input until it is dismissed
    assert_eq!(g.hits().len(), 3);
    g.tick(&Input {
        keys: vec![Key::Enter],
        ..Input::default()
    });
    assert!(g.progress.migration_note.is_empty());
    assert!(g.hits().len() > 3);
}

#[test]
fn the_code_reveal_translates_every_reference_solution_or_says_why_not() {
    use algoclerk_game::result::translations;
    use algoclerk_game::session::Session;
    let campaign = Campaign::shipped();
    let (mut ok_c, mut ok_rust, mut total) = (0, 0, 0);
    for level in campaign.all_levels() {
        let mut s = Session::for_level(level, &level.reference).unwrap();
        s.recompile();
        let tabs = translations(&s);
        assert_eq!(tabs.len(), 3);
        total += 1;
        // the C++ tab is the player's own code, always available, with the level's includes
        let cpp = &tabs[1];
        assert!(cpp.available(), "{}: {:?}", level.id, cpp.error);
        if !level.prelude.trim().is_empty() {
            assert!(!cpp.prelude.trim().is_empty(), "{}", level.id);
        }
        assert!(cpp.code.contains(&level.signature.name), "{}", level.id);
        for t in &tabs {
            match &t.error {
                Some(e) => assert!(e.len() > 10, "{}: unhelpful message {e:?}", level.id),
                None => assert!(!t.code.trim().is_empty(), "{} {}", level.id, t.label),
            }
        }
        ok_c += usize::from(tabs[0].available());
        ok_rust += usize::from(tabs[2].available());
    }
    assert!(ok_c * 10 >= total * 7, "C translated {ok_c}/{total}");
    assert!(
        ok_rust * 10 >= total * 7,
        "Rust translated {ok_rust}/{total}"
    );
}
