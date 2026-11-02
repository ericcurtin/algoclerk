// SPDX-License-Identifier: Apache-2.0
//! Developer preview: renders the office scene to a PNG. `cargo run -p algoclerk-game --example scene -- out.png [tiles]`
use algoclerk_game::gfx::Gfx;
use algoclerk_game::office::{Office, SCENE_H, SCENE_W};
use algoclerk_media::{ArtBank, Assets, Canvas, Outfit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "scene.png".into());
    let tiles: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
    let bank = ArtBank::load(&Assets::embedded()).expect("art");
    let mut o = Office::new(&bank, &Outfit::default(), tiles);
    let floor: Vec<Option<i32>> = (0..tiles).map(|i| if i % 3 == 0 { Some(i as i32 * 7 - 3) } else { None }).collect();
    o.sync(&[3, 14, 15, 92, 65, 35, 89, 79, 32], 2, &floor, Some(42), &[3, 14, 99], 17);
    o.set_expected(&[3, 14, 15, 92, 65]);
    o.vis.hand = Some(42);
    o.clerk.carrying = true;
    o.clerk.x = 120.0;
    o.clerk.y = 100.0;
    for _ in 0..30 {
        o.tick();
    }
    let mut c = Canvas::new(SCENE_W as usize, SCENE_H as usize);
    let mut g = Gfx { c: &mut c, bank: &bank, pal: &bank.palettes[0], tick: 0, reduce_motion: false, off: (0, 0) };
    o.draw(&mut g, (0, 0));
    std::fs::write(&out, c.upscaled(4).to_png()).expect("write");
}
