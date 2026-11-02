// SPDX-License-Identifier: Apache-2.0
//! Render every screen headlessly (software rasteriser) in several sizes and themes.

use algoclerk_app::shot::{Spec, render};

fn check(spec: &Spec) {
    let r = render(spec).unwrap_or_else(|e| panic!("{spec:?}: {e}"));
    let (lum, colours) = r.stats();
    assert!(
        colours > 40,
        "{spec:?}: only {colours} colours, the screen looks empty"
    );
    assert!((0.01..0.95).contains(&lum), "{spec:?}: odd luminance {lum}");
    let png = r.to_png().unwrap();
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
}

#[test]
fn all_screens_render_on_desktop_and_phone_sizes() {
    for screen in [
        "title", "levels", "play", "victory", "code", "text", "sandbox", "settings", "about",
    ] {
        for (w, h, scale, narrow) in [
            (1280, 800, 1.0, None),
            (800, 1600, 2.0, None),
            (1600, 720, 2.0, None),
        ] {
            for tab in [None, Some("brief"), Some("office"), Some("text")] {
                if screen != "play" && tab.is_some() {
                    continue;
                }
                check(&Spec {
                    screen: screen.into(),
                    width: w,
                    height: h,
                    scale,
                    narrow,
                    tab: tab.map(Into::into),
                    ..Spec::default()
                });
            }
        }
    }
}

#[test]
fn themes_and_colourblind_palette_render() {
    for theme in ["dark", "light", "contrast"] {
        for colorblind in [false, true] {
            check(&Spec {
                screen: "play".into(),
                theme: theme.into(),
                colorblind,
                width: 1000,
                height: 700,
                ..Spec::default()
            });
        }
    }
}

#[test]
fn every_shipped_level_renders_its_play_screen() {
    let c = algoclerk_levels::Campaign::shipped();
    for l in c.all_levels() {
        check(&Spec {
            screen: "play".into(),
            level: l.id.clone(),
            width: 1000,
            height: 640,
            steps: 25,
            ..Spec::default()
        });
    }
}

#[test]
fn icon_renders_at_all_sizes() {
    for size in [16, 64, 256] {
        let r = algoclerk_app::shot::render_icon(size).unwrap();
        assert!(r.stats().1 > 5);
    }
}
