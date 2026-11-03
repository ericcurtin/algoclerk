// SPDX-License-Identifier: Apache-2.0
//! Polish found in the real windowed game: an untouched starter is a nudge and not an error, the note under
//! the code is never clipped nor repeated, the cards offer an obvious first card, and the brief has real bullets.

use algoclerk_game::cards;
use algoclerk_game::editor::Tab;
use algoclerk_game::scenarios::{Spec, build};
use algoclerk_game::{Game, Input};

fn tick(g: &mut Game, i: Input) {
    g.tick(&i);
}

fn click(g: &mut Game, p: (i32, i32)) {
    let at = |down: bool, pressed: bool, released: bool| Input {
        pointer: Some(p),
        down,
        pressed,
        released,
        ..Input::default()
    };
    tick(g, at(true, true, false));
    tick(g, at(false, false, true));
    tick(g, Input::default());
}

fn editor_body(g: &mut Game) -> algoclerk_media::Rect {
    let vp = g.vp;
    g.play_mut()
        .unwrap()
        .hits(&vp)
        .into_iter()
        .find(|h| h.name == "editor body")
        .expect("the editor reports its body")
        .rect
}

fn empty_game(tab: Tab) -> Game {
    let mut g = build(&Spec::new("play-empty")).unwrap();
    let p = g.play_mut().unwrap();
    p.panel.tab = tab;
    p.panel.card = None;
    g
}

#[test]
fn an_untouched_starter_shows_a_nudge_and_no_error_anywhere() {
    for tab in [Tab::Blocks, Tab::Text] {
        let mut g = empty_game(tab);
        let tr = algoclerk_core::i18n::english();
        let p = g.play_mut().unwrap();
        assert!(p.session.nudge());
        let note = p.panel.note_of(&p.session).expect("a friendly note");
        assert!(!note.1, "the nudge is not an error");
        assert!(note.0.contains("between the braces"), "{}", note.0);
        assert!(p.panel.shown_error(&p.session).is_none());
        let (status, bad) = p.status_line(tr);
        assert!(!bad, "{status}");
        assert!(!status.contains("must give back"), "{status}");
    }
}

#[test]
fn pressing_run_on_an_empty_body_reports_once_by_the_code_and_once_in_the_status() {
    for tab in [Tab::Blocks, Tab::Text] {
        let mut g = empty_game(tab);
        let tr = algoclerk_core::i18n::english();
        {
            let p = g.play_mut().unwrap();
            p.session.start_run().unwrap_err();
            p.status = Some((p.session.problem().unwrap().msg, true));
        }
        let p = g.play_mut().unwrap();
        let note = p.panel.note_of(&p.session).unwrap();
        assert!(note.1 && note.0.contains("must give back"), "{}", note.0);
        let (status, bad) = p.status_line(tr);
        assert!(bad);
        // the clerk's sentence appears beside the code only, the status points at it
        assert!(!status.contains("must give back"), "{status}");
        assert!(status.contains("note"), "{status}");
    }
}

#[test]
fn the_note_box_is_tall_enough_for_every_line_at_every_size() {
    for (portrait, touch) in [(false, false), (false, true), (true, true)] {
        let mut spec = Spec::new("play-empty");
        spec.portrait = portrait;
        spec.touch = touch;
        let mut g = build(&spec).unwrap();
        let vp = g.vp;
        let area = vp.screen();
        for tab in [Tab::Blocks, Tab::Text] {
            let p = g.play_mut().unwrap();
            p.panel.tab = tab;
            p.session.start_run().unwrap_err();
            let (rect, lines) = p.panel.note_layout(&vp, area, &p.session);
            assert!(lines >= 1, "there is a message");
            assert!(
                rect.h >= lines as i32 * 9 + 2,
                "{lines} lines do not fit {rect:?} (portrait {portrait}, {tab:?})"
            );
            assert!(lines <= 6, "message would be capped: {lines} lines");
        }
    }
}

#[test]
fn an_empty_body_offers_an_obvious_first_card() {
    let mut g = empty_game(Tab::Blocks);
    let list = editor_body(&mut g);
    let rows = g.play_mut().unwrap().session.rows();
    assert!(cards::is_empty_body(&rows));
    let geo = cards::geometry(&rows, list, 14);
    let slot = cards::empty_slot(&rows, &geo, list, 0, 22).expect("the add button");
    assert!(list.contains(slot.x, slot.y) && list.contains(slot.right() - 1, slot.bottom() - 1));
    click(&mut g, (slot.x + slot.w / 2, slot.y + slot.h / 2));
    let p = g.play_mut().unwrap();
    assert_eq!(p.panel.palette, Some(false), "the card palette opened");
    // the first palette cell adds a card into the body
    let cell = (list.x + 4 + 5, list.y + 14 + 5);
    click(&mut g, cell);
    let p = g.play_mut().unwrap();
    assert!(p.panel.palette.is_none());
    let rows = p.session.rows();
    assert!(!cards::is_empty_body(&rows), "{}", p.session.text());
    assert_eq!(p.panel.sel, Some(1), "the new card is selected");
    assert!(!p.session.nudge());
    // the first card leaves the missing return as a yellow heads-up, not a red error
    let tr = algoclerk_core::i18n::english();
    assert!(p.session.soft_problem(), "pressed: {}", p.session.pressed);
    let status = p.status_line(tr);
    assert!(!status.1, "{status:?}");
}

#[test]
fn the_add_card_bar_is_there_before_anything_is_selected() {
    let mut g = empty_game(Tab::Blocks);
    let list = editor_body(&mut g);
    let vp = g.vp;
    let area = vp.screen();
    let _ = area;
    // the body leaves room under it for the bar: the editor body ends above the editor's bottom
    let editor = g
        .play_mut()
        .unwrap()
        .hits(&vp)
        .into_iter()
        .find(|h| h.name == "editor")
        .unwrap()
        .rect;
    assert!(list.bottom() < editor.bottom() - 8, "{list:?} {editor:?}");
}

#[test]
fn a_filled_body_has_no_add_button_in_the_list() {
    let mut g = build(&Spec::new("play")).unwrap();
    let list = editor_body(&mut g);
    let rows = g.play_mut().unwrap().session.rows();
    let geo = cards::geometry(&rows, list, 14);
    assert!(cards::empty_slot(&rows, &geo, list, 0, 22).is_none());
}

#[test]
fn constraints_use_a_bullet_with_a_gap() {
    assert!(algoclerk_game::brief::BULLET_GAP >= 4);
}

/// Type `src` the way a person does: no leading spaces (the editor indents), Return between the lines.
fn type_like_a_person(src: &str, auto_close: bool) -> String {
    let mut e = algoclerk_game::textedit::TextEdit::new("", true);
    e.smart = true;
    e.auto_close = auto_close;
    for ch in src.trim_start_matches('\n').chars() {
        match ch {
            '\n' => e.smart_newline(),
            c => e.type_char(c),
        }
        // leading spaces of a line are the editor's job
    }
    e.text()
}

fn strip_indent(src: &str) -> String {
    src.trim_start_matches('\n')
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn typing_every_reference_solution_gives_the_same_code() {
    let mut bad = Vec::new();
    for l in algoclerk_levels::Campaign::shipped().all_levels() {
        let want = l.reference.trim_start_matches('\n').trim_end().to_string();
        for auto_close in [false, true] {
            let typed = strip_indent(&l.reference);
            let got = type_like_a_person(&typed, auto_close);
            // continuation lines are aligned by hand in a few references: compare them without leading spaces,
            // and every other line exactly
            let exact = |a: &str, b: &str| {
                a.lines().count() == b.lines().count()
                    && a.lines().zip(b.lines()).all(|(x, y)| {
                        x == y || (y.starts_with("          ") && x.trim_start() == y.trim_start())
                    })
            };
            if !exact(got.trim_end(), &want) {
                bad.push(format!(
                    "--- {} (auto_close {auto_close})\n{}\n+++ typed gave\n{}",
                    l.id,
                    want,
                    got.trim_end()
                ));
            }
        }
    }
    let n = bad.len();
    bad.truncate(3);
    assert!(n == 0, "{n} mismatches, first:\n{}", bad.join("\n"));
}

#[test]
fn a_pointer_resting_on_a_door_does_not_steal_the_focus_when_level_select_opens() {
    use algoclerk_game::{Nav, Scene};
    let mut g = build(&Spec::new("title")).unwrap();
    g.goto(Nav::Select);
    let want = match &g.scene {
        Scene::Select(s) => s.focus,
        _ => panic!("level select"),
    };
    // the mouse rests near the bottom right of the window
    let rest = Input {
        pointer: Some((408, 245)),
        ..Input::default()
    };
    for _ in 0..3 {
        tick(&mut g, rest.clone());
    }
    match &g.scene {
        Scene::Select(s) => assert_eq!(s.focus, want, "the screen opened on the wrong level"),
        _ => panic!("level select"),
    }
}

#[test]
fn a_missing_return_is_a_yellow_heads_up_until_run_is_pressed() {
    let mut g = empty_game(Tab::Blocks);
    let tr = algoclerk_core::i18n::english();
    let p = g.play_mut().unwrap();
    p.session
        .set_text("int stampDuty(int price, int qty) {\n    int total = price * qty;\n}\n");
    assert!(!p.session.nudge() && p.session.soft_problem());
    for tab in [Tab::Blocks, Tab::Text] {
        p.panel.tab = tab;
        let note = p.panel.note_of(&p.session).expect("a note");
        assert!(!note.1, "not red yet: {}", note.0);
        assert!(note.0.contains("must give back"));
        assert!(p.panel.shown_error(&p.session).is_none());
        assert!(!p.status_line(tr).1);
    }
    // pressing Run makes it a real error
    p.session.start_run().unwrap_err();
    assert!(!p.session.soft_problem());
    assert!(p.panel.note_of(&p.session).unwrap().1);
}

#[test]
fn the_status_stays_calm_after_the_first_card_when_coming_from_the_brief() {
    use algoclerk_game::Nav;
    let mut g = build(&Spec::new("title")).unwrap();
    g.goto(Nav::Brief("sum-the-bills".into()));
    for _ in 0..5 {
        tick(&mut g, Input::default());
    }
    click(&mut g, (388, 247));
    for _ in 0..40 {
        tick(&mut g, Input::default());
    }
    let list = editor_body(&mut g);
    let rows = g.play_mut().unwrap().session.rows();
    let geo = cards::geometry(&rows, list, 14);
    let slot = cards::empty_slot(&rows, &geo, list, 0, 22).expect("the add button");
    click(&mut g, (slot.x + slot.w / 2, slot.y + slot.h / 2));
    click(&mut g, (list.x + 9, list.y + 19));
    let tr = algoclerk_core::i18n::english();
    let p = g.play_mut().unwrap();
    assert!(!p.session.nudge(), "{}", p.session.text());
    assert!(p.session.soft_problem(), "pressed {}", p.session.pressed);
    assert!(!p.status_line(tr).1, "{:?}", p.status_line(tr));
}
