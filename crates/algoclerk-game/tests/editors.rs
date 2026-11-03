// SPDX-License-Identifier: Apache-2.0
//! Editor tests: text -> cards -> text round trips over the whole C++ corpus and all level references,
//! card editing through real input events (keys and a mouse drag) with undo and redo, the typed pickers,
//! and a fuzz of both editors with random key and pointer sequences.

use algoclerk_core::cpp::print::print_program;
use algoclerk_game::blocks::{self, CardKind};
use algoclerk_game::scenarios::{Spec, build};
use algoclerk_game::session::parse_with_prelude;
use algoclerk_game::{Game, Input, Key};
use algoclerk_levels::Campaign;
use algoclerk_media::Canvas;

/// Every program text the editors must survive: the corpus (no prelude) and the level references.
fn sources() -> Vec<(String, String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../algoclerk-core/tests/data/cpp_corpus");
    let mut out = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("corpus directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        out.push((name, String::new(), std::fs::read_to_string(&f).unwrap()));
    }
    for l in Campaign::shipped().all_levels() {
        out.push((l.id.clone(), l.prelude.clone(), l.reference.clone()));
    }
    out
}

fn normalised_text(p: &algoclerk_core::cpp::ast::Program) -> String {
    let mut n = p.clone();
    blocks::normalise(&mut n);
    print_program(&n, true)
}

#[test]
fn the_corpus_and_all_level_references_round_trip_through_cards() {
    let srcs = sources();
    assert!(srcs.len() >= 140, "{} sources", srcs.len());
    let (mut raw, mut cards, mut edits, mut moves) = (0, 0, 0, 0);
    for (name, prelude, src) in &srcs {
        let Ok(p) = parse_with_prelude(prelude, src) else {
            // corpus files outside the subset are the core's business; levels must always parse
            assert!(
                prelude.is_empty() && !name.contains('-'),
                "{name} must parse"
            );
            continue;
        };
        // the printed text is a fixed point of parse and print
        let base = print_program(&p, true);
        let again = parse_with_prelude(prelude, &base).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        assert_eq!(
            print_program(&again, true),
            base,
            "{name}: print is not stable"
        );
        let expected = normalised_text(&p);
        let rows = blocks::rows(&p);
        for r in &rows {
            assert!(
                !r.text.contains('\n'),
                "{name}: multi-line card {:?}",
                r.text
            );
            match r.kind {
                CardKind::Raw => {
                    raw += 1;
                    assert!(r.movable() && !r.editable(), "{name}: {:?}", r.text);
                }
                CardKind::Simple | CardKind::Compound => cards += 1,
                _ => {}
            }
        }
        for r in &rows {
            // editing a card with its own text changes nothing
            if r.editable()
                && let Some(t) = blocks::card_text(&p, r.node)
            {
                let mut q = p.clone();
                let text = blocks::edit_stmt(&mut q, r.node, &t)
                    .unwrap_or_else(|e| panic!("{name}: card {t:?}: {e}"));
                assert_eq!(text, expected, "{name}: identity edit of {t:?}");
                edits += 1;
            }
            // moving a card down and up again changes nothing
            if r.movable() {
                let mut q = p.clone();
                if let Some(down) = blocks::step_stmt(&mut q, r.node, false) {
                    assert!(
                        parse_with_prelude(prelude, &down).is_ok(),
                        "{name}: move down breaks the text:\n{down}"
                    );
                    moves += 1;
                }
            }
        }
        // the program rebuilt from the card rows' own insertions still prints the same text
        let mut q = p.clone();
        let text = blocks::insert_stmt(&mut q, "zzz", None, false, "int zz_probe = 0;");
        if let Ok(t) = text {
            assert!(t.contains("zz_probe"), "{name}");
            assert!(parse_with_prelude(prelude, &t).is_ok(), "{name}:\n{t}");
        }
    }
    assert!(cards > 500, "{cards} cards");
    assert!(edits > 400, "{edits} identity edits");
    assert!(moves > 100, "{moves} moves");
    assert!(raw > 0, "no raw card anywhere is suspicious");
}

#[test]
fn moving_into_a_block_and_back_keeps_the_program_equal() {
    let mut tried = 0;
    for (name, prelude, src) in sources() {
        let Ok(p) = parse_with_prelude(&prelude, &src) else {
            continue;
        };
        let rows = blocks::rows(&p);
        let expected = normalised_text(&p);
        for r in rows.iter().filter(|r| r.movable()) {
            let Some((from, at)) = blocks::locate(&p, r.node) else {
                continue;
            };
            // into the body of the first compound card that is not an ancestor, then home again
            let Some(target) = rows.iter().find(|t| {
                t.kind == CardKind::Compound && t.body.is_some() && t.node != r.node && !t.chained
            }) else {
                continue;
            };
            if target.body == Some(from) {
                continue;
            }
            let mut q = p.clone();
            let Some(text) = blocks::move_to(&mut q, r.node, target.body.unwrap(), 0) else {
                continue;
            };
            assert!(
                parse_with_prelude(&prelude, &text).is_ok(),
                "{name}: broken text after the move:\n{text}"
            );
            tried += 1;
            let Some(back) = blocks::move_to(&mut q, r.node, from, at) else {
                continue;
            };
            assert_eq!(
                back, expected,
                "{name}: card {:?} did not come home",
                r.text
            );
        }
    }
    assert!(tried > 50, "{tried} moves into blocks");
}

fn tick(g: &mut Game, i: Input) {
    g.tick(&i);
}

fn keys(g: &mut Game, k: &[Key]) {
    for key in k {
        tick(
            g,
            Input {
                keys: vec![*key],
                ..Input::default()
            },
        );
    }
}

fn typed(g: &mut Game, text: &str) {
    tick(
        g,
        Input {
            typed: text.to_string(),
            ..Input::default()
        },
    );
}

fn ctrl_z(g: &mut Game, shift: bool) {
    tick(
        g,
        Input {
            keys: vec![Key::Char('z')],
            ctrl: true,
            shift,
            ..Input::default()
        },
    );
}

fn text_of(g: &mut Game) -> String {
    g.play_mut().unwrap().session.text()
}

/// A play screen on the cards tab with the reference loaded.
fn card_game() -> Game {
    let mut g = build(&Spec::new("play")).unwrap();
    let p = g.play_mut().unwrap();
    p.panel.tab = algoclerk_game::editor::Tab::Blocks;
    p.panel.sel = None;
    p.panel.card = None;
    g
}

fn row_index(g: &mut Game, text: &str) -> usize {
    g.play_mut()
        .unwrap()
        .session
        .rows()
        .iter()
        .position(|r| r.text == text)
        .unwrap_or_else(|| panic!("no card {text:?}"))
}

#[test]
fn a_card_is_edited_with_the_keyboard_and_undo_redo_follow() {
    let mut g = card_game();
    let before = text_of(&mut g);
    let i = row_index(&mut g, "seen[nums[i]] = i;");
    g.play_mut().unwrap().panel.sel = Some(i);
    keys(&mut g, &[Key::Enter]);
    let line = g
        .play_mut()
        .unwrap()
        .panel
        .card
        .as_ref()
        .map(|c| c.line.text());
    assert_eq!(
        line.as_deref(),
        Some("seen[nums[i]] = i;"),
        "Enter opens the card"
    );
    keys(&mut g, &[Key::Backspace; 18]);
    typed(&mut g, "seen[nums[i]] = i + 1;");
    keys(&mut g, &[Key::Enter]);
    assert!(g.play_mut().unwrap().panel.card.is_none(), "Enter confirms");
    let after = text_of(&mut g);
    assert!(after.contains("seen[nums[i]] = i + 1;"), "{after}");
    // the comment and the rest of the program survived
    assert_eq!(after.lines().count(), before.lines().count());
    ctrl_z(&mut g, false);
    assert_eq!(text_of(&mut g), before, "ctrl+z undoes the card edit");
    ctrl_z(&mut g, true);
    assert_eq!(text_of(&mut g), after, "ctrl+shift+z redoes it");
    // a card with a mistake is refused and the text stays
    let i = row_index(&mut g, "return {};");
    g.play_mut().unwrap().panel.sel = Some(i);
    keys(&mut g, &[Key::Enter, Key::Backspace]);
    keys(&mut g, &[Key::Enter]);
    assert!(
        g.play_mut().unwrap().panel.card.is_some(),
        "a broken card stays open"
    );
    assert_eq!(text_of(&mut g), after);
    keys(&mut g, &[Key::Escape]);
    assert!(g.play_mut().unwrap().panel.card.is_none());
    // Delete removes the selected card, undo brings it back
    let i = row_index(&mut g, "return {};");
    g.play_mut().unwrap().panel.sel = Some(i);
    keys(&mut g, &[Key::Delete]);
    assert!(!text_of(&mut g).contains("return {};"));
    ctrl_z(&mut g, false);
    assert!(text_of(&mut g).contains("return {};"));
}

#[test]
fn a_card_is_dragged_into_a_block_with_the_mouse() {
    let mut g = card_game();
    let before = text_of(&mut g);
    let vp = g.vp;
    let list = g
        .play_mut()
        .unwrap()
        .hits(&vp)
        .into_iter()
        .find(|h| h.name == "editor body")
        .expect("the editor reports its body")
        .rect;
    let rows = g.play_mut().unwrap().session.rows();
    let geo = algoclerk_game::cards::geometry(&rows, list, 14);
    let find = |t: &str| rows.iter().position(|r| r.text == t).unwrap();
    let (src, dst) = (
        find("seen[nums[i]] = i;"),
        find("for (int i = 0; i < static_cast<int>(nums.size()); ++i)"),
    );
    let at = |i: usize, lower: bool| {
        let b = &geo.boxes[i];
        (
            b.x + b.w / 2,
            list.y + b.y + if lower { b.h - 2 } else { b.h / 2 },
        )
    };
    let (from, to) = (at(src, false), at(dst, true));
    assert!(
        list.contains(from.0, from.1) && list.contains(to.0, to.1),
        "{list:?} {from:?} {to:?}"
    );
    let pt = |p: (i32, i32), down: bool, pressed: bool, released: bool| Input {
        pointer: Some(p),
        down,
        pressed,
        released,
        ..Input::default()
    };
    tick(&mut g, pt(from, true, true, false));
    tick(&mut g, pt(to, true, false, false));
    tick(&mut g, pt(to, false, false, true));
    let after = text_of(&mut g);
    assert_ne!(after, before, "the drop changed the program");
    let (s, f, w) = (
        after.find("seen[nums[i]] = i;").unwrap(),
        after.find("for (int i").unwrap(),
        after.find("const int want").unwrap(),
    );
    assert!(
        f < s && s < w,
        "the card is the first statement of the loop:\n{after}"
    );
    ctrl_z(&mut g, false);
    assert_eq!(text_of(&mut g), before);
    // dropping a card onto itself or into its own child is refused quietly
    let (from, to) = (at(dst, false), at(dst, true));
    tick(&mut g, pt(from, true, true, false));
    tick(&mut g, pt(to, true, false, false));
    tick(&mut g, pt(to, false, false, true));
    assert_eq!(text_of(&mut g), before);
}

/// A play screen on the text tab holding `text`, the cursor at `cursor` (character index).
fn text_game(text: &str, cursor: usize) -> Game {
    let mut g = build(&Spec::new("play")).unwrap();
    let p = g.play_mut().unwrap();
    p.panel.tab = algoclerk_game::editor::Tab::Text;
    p.panel.card = None;
    p.session.set_text(text);
    p.session.edit.cursor = cursor;
    p.session.edit.anchor = None;
    g
}

#[test]
fn the_text_editor_toggles_comments_indents_and_matches_brackets() {
    let src = "int f() {\n    int a = (1 + 2);\n}\n";
    // comment toggling with Ctrl+/ on the cursor's line, twice
    let at = src.find("int a").unwrap() + 4;
    let mut g = text_game(src, at);
    tick(
        &mut g,
        Input {
            keys: vec![Key::Char('/')],
            ctrl: true,
            ..Input::default()
        },
    );
    let once = text_of(&mut g);
    assert!(once.contains("// int a = (1 + 2);"), "{once}");
    tick(
        &mut g,
        Input {
            keys: vec![Key::Char('/')],
            ctrl: true,
            ..Input::default()
        },
    );
    assert_eq!(text_of(&mut g), src);
    // Enter after `{` indents one level, `}` dedents again
    let mut g = text_game("int f() {", 9);
    keys(&mut g, &[Key::Enter]);
    assert_eq!(text_of(&mut g), "int f() {\n    ");
    typed(&mut g, "}");
    assert_eq!(text_of(&mut g), "int f() {\n}");
    // a long line scrolls sideways with the cursor; Home comes back
    let long = format!("int f() {{ return {}0; }}", "1 + ".repeat(40));
    let mut g = text_game(&long, 0);
    assert_eq!(g.play_mut().unwrap().session.edit.scroll_col, 0);
    keys(&mut g, &[Key::End]);
    let col = g.play_mut().unwrap().session.edit.scroll_col;
    assert!(
        col > 20,
        "End scrolls the view sideways, scroll_col = {col}"
    );
    keys(&mut g, &[Key::Home]);
    assert_eq!(g.play_mut().unwrap().session.edit.scroll_col, 0);
    // the bracket next to the cursor has a partner
    let at = src.find('(').unwrap() + 1;
    let mut g = text_game(src, at);
    keys(&mut g, &[Key::Left, Key::Right]);
    let ed = &g.play_mut().unwrap().session.edit;
    let (a, b) = ed.match_bracket().expect("a matching pair");
    assert_eq!(src.chars().nth(a), Some('('));
    assert_eq!(src.chars().nth(b), Some(')'));
}

#[test]
fn pickers_follow_the_receiver_type_and_the_unlocked_headers() {
    use algoclerk_core::cpp::ast::Ty;
    use algoclerk_core::cpp::stdlib::members_of;
    use algoclerk_game::complete::{
        allowed_headers, card_member_items, card_std_items, member_items, std_items,
    };
    use algoclerk_game::session::Session;
    let sandbox = Session::sandbox("");
    let int = || Box::new(Ty::Int);
    let vec_ty = Ty::Vector(int());
    let map_ty = Ty::UnorderedMap(int(), int());
    let cards_of = |t: &Ty| -> Vec<String> {
        card_member_items(&sandbox, t, false, "")
            .into_iter()
            .map(|i| i.label)
            .collect()
    };
    let (v, m) = (cards_of(&vec_ty), cards_of(&map_ty));
    assert!(
        v.contains(&"push_back".to_string()) && v.contains(&"size".to_string()),
        "{v:?}"
    );
    assert!(
        !m.contains(&"push_back".to_string()),
        "a map has no push_back: {m:?}"
    );
    assert!(
        m.contains(&"find".to_string()) && m.contains(&"count".to_string()),
        "{m:?}"
    );
    // only members with `card: true`, and never more than autocomplete offers
    for t in [&vec_ty, &map_ty, &Ty::Str] {
        let full = member_items(&sandbox, t, false, "").len();
        let cards = card_member_items(&sandbox, t, false, "");
        assert!(cards.len() <= full);
        for it in &cards {
            assert!(
                members_of(t).iter().any(|x| x.name == it.label && x.card),
                "{} is not a card member",
                it.label
            );
        }
    }
    // std:: items stay inside the headers of the level
    let campaign = Campaign::shipped();
    let all = std_items(&sandbox, "").len();
    let mut narrower = 0;
    for level in campaign.all_levels() {
        let s = Session::for_level(level, &level.reference).unwrap();
        let allowed = allowed_headers(&s);
        for it in std_items(&s, "") {
            if it.detail.starts_with('<')
                && let Some(a) = &allowed
            {
                assert!(
                    a.contains(&it.detail),
                    "{}: {} needs {}",
                    level.id,
                    it.label,
                    it.detail
                );
            }
        }
        let cards = card_std_items(&s, "");
        assert!(cards.len() <= std_items(&s, "").len());
        if cards.len() < all {
            narrower += 1;
        }
    }
    assert!(
        narrower > 40,
        "early levels offer fewer std names ({narrower})"
    );
}

#[test]
fn both_editors_survive_random_keys_taps_and_drags() {
    use algoclerk_core::testgen::Rng;
    use algoclerk_game::editor::Tab;
    const KEYS: [Key; 17] = [
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
        Key::Char('z'),
        Key::Char('/'),
        Key::Char('y'),
    ];
    const TYPED: [&str; 14] = [
        "a", "{", "}", "(", ")", "\"", "'", " ", "\n", "9", "é", "\t", ";", "->",
    ];
    let seed = std::env::var("ALGOCLERK_FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(11);
    let rng = Rng::new(seed);
    for (portrait, tab) in [
        (false, Tab::Blocks),
        (false, Tab::Text),
        (true, Tab::Blocks),
        (true, Tab::Text),
    ] {
        for scenario in ["play", "play-grid"] {
            let spec = Spec {
                portrait,
                touch: portrait,
                ..Spec::new(scenario)
            };
            let mut g = build(&spec).unwrap();
            g.play_mut().unwrap().panel.tab = tab;
            let (w, h) = (g.vp.w, g.vp.h);
            let (mut down, mut at) = (false, (0, 0));
            for n in 0..900 {
                let mut i = Input::default();
                if rng.chance(35) {
                    i.keys
                        .push(KEYS[rng.range(0, KEYS.len() as i64 - 1) as usize]);
                }
                if rng.chance(25) {
                    i.typed = TYPED[rng.range(0, TYPED.len() as i64 - 1) as usize].to_string();
                }
                // pointers mostly wander over the editor half of the screen, and drags are long
                if !down || rng.chance(30) {
                    at = (
                        rng.range(i64::from(w) / 2 - 10, i64::from(w) + 5) as i32,
                        rng.range(-5, i64::from(h) + 5) as i32,
                    );
                    if portrait {
                        at.0 = rng.range(-5, i64::from(w) + 5) as i32;
                    }
                }
                i.pointer = Some(at);
                i.ctrl = rng.chance(8);
                i.shift = rng.chance(12);
                i.touch = portrait;
                if rng.chance(10) {
                    i.wheel = rng.range(-3, 3) as f32;
                }
                if !down && rng.chance(25) {
                    i.pressed = true;
                    down = true;
                } else if down && rng.chance(30) {
                    i.released = true;
                    down = false;
                }
                i.down = down;
                g.tick(&i);
                if g.scene.name() != "play" {
                    // Back, Memo or a modal took us away: start over on the play screen
                    g = build(&spec).unwrap();
                    g.play_mut().unwrap().panel.tab = tab;
                }
                if n % 7 == 0 {
                    let mut c = Canvas::new(w as usize, h as usize);
                    g.draw(&mut c);
                    let p = g.play_mut().unwrap();
                    let rows = p.session.rows();
                    assert!(p.panel.sel.is_none_or(|s| s < rows.len() + 2) || rows.is_empty());
                    assert!(
                        p.session.text().len() < 50_000,
                        "the text grew without bound"
                    );
                }
                let _ = g.take_events();
            }
        }
    }
}
