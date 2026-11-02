# Translating AlgoClerk

The game is localisation-ready: every player-visible string comes from a catalogue keyed by message
id, with English as the fallback for any missing key. The catalogues live in
`crates/algoclerk-core/lang/` (one TOML file per language) and are embedded at build time.

## Add a language

1. Copy `crates/algoclerk-core/lang/en.toml` to `<code>.toml` where `<code>` is a short language
   code (`de`, `fr`, `pt-BR`, ...).
2. Set `[meta] language = "Deutsch"` (the name in the language itself) and translate the values.
3. **Keep the keys and `{placeholders}` unchanged.** `{slot}`, `{line}`, `{text}`, `{op}`, `{limit}`,
   `{test}`, `{got}`, `{expected}` are replaced at run time. Keep sentences short: phone screens are small.
4. Run `cargo test -p algoclerk-core` (checks that the file parses) and start the game: the language
   appears in Settings, Language.
5. Open a pull request. You do not need to translate everything at once; untranslated keys fall back
   to English. Please keep technical names of instructions (`INBOX`, `COPYTO`, ...) as they are.

## What is translated

- UI texts, error messages ("The clerk's hand is empty..."), tooltips, About text.
- **Not yet:** level stories, statements and hints (they are part of the level files). The plan is
  to allow `levels/<pack>/<id>.<lang>.toml` overlay files that replace the text fields only; see the
  [roadmap](../ROADMAP.md).
- Generated code comments are English.

## Fonts

The bundled font covers Latin scripts, Greek and Cyrillic. Scripts that need other glyphs (CJK,
Arabic, Indic scripts) require a font that is permissively licensed (OFL is fine). Adding a fallback
font is a good first issue; see `crates/algoclerk-app/src/fonts.rs` and `CREDITS.md`.
