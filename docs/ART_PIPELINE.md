# Art, font and music pipeline

Everything you see and hear is a **text file** in `assets/`, embedded into the binary at build time by
`crates/algoclerk-media/build.rs` (or loaded from a directory with `--assets DIR` for modding). The pipeline
design is shared with the sibling project Falabichos; all AlgoClerk art, the font and the songs are new.

| What | Where | Format |
| --- | --- | --- |
| Palettes | `assets/art/palette.txt` (warm), `palette_cb.txt` (colour-blind safe), `palette_hc.txt` (high contrast) | one letter per colour, `. = transparent`; **at most 32 opaque colours** (linted) |
| Sprites | `assets/art/<category>/<name>.px` | optional `size:`/`outline:` header lines, then frames of palette letters separated by `---` |
| Clerk | `assets/art/clerk/_body_*.px` + `assets/art/outfits.txt` | template with role digits (1 hair, 2 skin, 3 shirt, 4 pants, 5 shoes, 6 extra) recoloured per outfit |
| Fonts | `assets/art/font.txt` (5x7), `tinyfont.txt` (3x5) | one glyph per line, rows of `#` and `.` |
| Music and sound | `assets/music/*.song` | channels of `NOTE:STEPS`, rendered to PCM at run time by `algoclerk-media/src/synth.rs` |

The three palettes define the same letters, so art never knows which palette is active; the player switches at
runtime in Settings. Every screen draws only with palette letters (`'K'` ink, `'W'` cream paper, ...).

## Contributing art

1. Draw a `.px` file in the right category (see existing ones for the size convention: tiles are 16x16, icons
   9x9, props vary). Use only letters of `palette.txt`.
2. Run `cargo test -p algoclerk-media` (parsing, colour limit, frame counts) and `cargo test -p algoclerk-game`
   (the font and the screens).
3. `scripts/screenshots.sh` renders every screen headlessly to `docs/screenshots` so you can look at it.

## Songs

```text
name: title
bpm: 132           # a step is a sixteenth note
loop: yes
lead = square 0.35 : E5:2 G5:2 C6:4 -:2   # waveform, volume : NOTE:STEPS ('-' is a rest)
bass = triangle 0.7 : C3:4 G3:4
perc = noise 0.18 : x:2 h:2                # x = hit, h = hat
```

The game asks for songs by name: `title`, `office` (loops while you play), `victory`, and the effects
`button`, `pick`, `put`, `step`, `correct`, `wrong`, `stars`. Audio failure never blocks the game, and it is
fully playable muted. The Linux build needs the ALSA headers for audio (`alsa-lib-devel` / `libasound2-dev`);
without the `audio` cargo feature everything else still builds.

## Colour-blind and high-contrast play

Meaning is never colour alone: stamps use tick and cross shapes, locks use an icon, medals have different
shapes, errors are text. `Settings > Palette` swaps the whole screen at once.
