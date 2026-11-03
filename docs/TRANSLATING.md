# Translating AlgoClerk

The game is localisation-ready: every player-visible string comes from a catalogue keyed by message
id, with English as the fallback for any missing key. The catalogues live in
`crates/algoclerk-core/lang/` (one TOML file per language) and are embedded at build time.

## Add a language

1. Copy `crates/algoclerk-core/lang/en.toml` to `<code>.toml` where `<code>` is a short language
   code (`de`, `fr`, `pt-BR`, ...).
2. Set `[meta] language = "Deutsch"` (the name in the language itself) and translate the values.
3. **Keep the keys and `{placeholders}` unchanged.** `{n}`, `{pos}`, `{total}`, `{cost}`, `{out}`, `{want}`, `{size}`, `{technique}` and similar names are replaced at run time. Keep sentences short: phone screens are small.
4. Run `cargo test -p algoclerk-core` (checks that the file parses) and start the game: the language
   appears in Settings, Language.
5. Open a pull request. You do not need to translate everything at once; untranslated keys fall back
   to English. Please keep C++ code and names (`std::vector`, `push_back`, `#include`, ...) exactly as they are: they are code.

## What is translated

- All UI chrome of the pixel front end (menus, buttons, memo, editor, victory, settings, about).
- **Not yet:** compiler and runtime messages (English strings with stable codes such as `type.mismatch`).
- **Not yet:** level stories, statements and hints (they are part of the level files). The plan is
  to allow `levels/<pack>/<id>.<lang>.toml` overlay files that replace the text fields only; see the
  [roadmap](../ROADMAP.md).
- Generated code comments are English.

## Fonts

The pixel font (`assets/art/font.txt`) is a 5x7 bitmap font covering printable ASCII. A test checks it against
every string of every shipped catalogue, so a translation using other letters needs new glyphs in
`font.txt` (original or OFL/CC0 designs, credited in `CREDITS.md`). Adding accented Latin letters is a good
first issue.
