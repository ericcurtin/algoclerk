# Credits and asset licences

## Code

AlgoClerk is licensed under the Apache License 2.0 (see `LICENSE` and `NOTICE`). Third-party Rust
crates are all permissively licensed; `cargo deny check licenses` verifies this in CI and the
accepted licence list is in `deny.toml`. The full list of dependencies and their licences can be
produced with `cargo deny list`.

## Fonts

| Asset | Author | Licence | Where |
| --- | --- | --- | --- |
| JetBrains Mono (NL, Regular) | The JetBrains Mono Project Authors | SIL Open Font License 1.1 | `assets/fonts/JetBrainsMonoNL-Regular.ttf`, licence text in `assets/fonts/OFL.txt` |

egui's own default fonts are **not** used (they include fonts under other licences); the
`default_fonts` feature is disabled and the font above is used for everything.

## Art, sounds, levels and text

- All visuals (the clerk, belts, crates, tiles, icons, the application icon) are drawn in code with
  vector shapes (`crates/algoclerk-app/src/{office,widgets,shot}.rs`); the icon PNGs in
  `assets/icons/` and the platform icon sets are rendered from that code. No external image files.
- Sound effects (optional `audio` feature) are synthesised in code
  (`crates/algoclerk-app/src/sound.rs`); there are no audio files.
- All levels, stories, statements, hints and UI strings were written for this project.

## Inspiration

AlgoClerk is inspired by the genre of programming puzzle games and by classic algorithm interview
topics. No assets, text, names, characters, level designs or problem statements were copied from
any game or website.
