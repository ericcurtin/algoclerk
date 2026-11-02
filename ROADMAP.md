# Roadmap

AlgoClerk v0.1 is playable on five platforms with 48 levels. This is where it could go; priorities
follow what the community contributes.

## 0.2: polish and reach

- Level text translations (overlay files `levels/<pack>/<id>.<lang>.toml`), more UI languages, fallback fonts for CJK/Arabic/Indic scripts.
- Screen reader support through AccessKit on desktop and mobile; full keyboard navigation of the program list.
- Sound effects on by default where the platform audio stack builds without extra system packages (Windows, macOS, Android, iOS), music-free by design.
- In-game level browser for extra packs and a "import pack from file" action on mobile.
- More themes (user-defined palettes from a TOML file).
- Better mobile text input for the program text panel.

## 0.3: language features

- `CALL` / `RET` and a visible call stack; matching translation to functions in the generated code.
- A "labelled data" mode: tiles with names usable in the text syntax (`COPYTO total`).
- Strings as first-class belts (`display = "char"` everywhere) with word-level puzzles.
- Optional "constants" instruction for tutorials that should not need prefilled tiles.

## 0.4: community

- **Level of the week**: a weekly community puzzle merged as a pull request, announced in the
  discussions, solutions shared as text and re-judged by anyone with the CLI.
- Community packs repository with a stable level-format version and a CI template that validates packs.
- `algoclerk-cli compare` to verify a directory of shared solutions and print a leaderboard table
  (see [docs/SHARING.md](docs/SHARING.md)); no hosting required.
- Daily puzzle generated from a seeded generator.
- Achievements for optimal size / speed, and a gallery of beautiful solutions.

## Later ideas

- Challenge levels with "minimise memory", "no jumps" or "at most N steps" rules.
- A level editor inside the game.
- A pack for graph and tree algorithms using pointer tiles.
- A teacher mode: share a set of levels and a verification script for a classroom.

## Non-goals

- A web or browser build, WebAssembly, embedding another scripting language: player programs run in
  the built-in interpreter by design.
- Online accounts and servers in the core game.
