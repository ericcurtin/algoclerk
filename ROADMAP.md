# Roadmap

AlgoClerk 0.2 teaches ClerkScript with 80 levels on five platforms. Where it could go next:

## Language and levels

- More wings: tries and union-find, intervals and sweep lines, backtracking packs, system-design flavoured
  "office tools" (an LRU cache as a level with methods), string matching (KMP), graph weights (Dijkstra with
  `heap`).
- User-defined struct types and methods in the sandbox; first-class lambdas for custom sort keys.
- Level text translations (overlay files `levels/<pack>/<id>.<lang>.toml`) and translated diagnostics.
- A level editor in the game that writes the TOML (generators, oracle skeleton, naive solution).

## Front end

- Screen reader support through AccessKit; full keyboard navigation of the card editor.
- Drag and drop of cards between blocks (today a card moves among its siblings and can be re-created inside
  another card with the palette).
- Larger clerk wardrobe (hats, glasses) and more office decorations unlocked by wings.
- A real "step into helper function" animation with a second tray row for the callee.
- More languages and bitmap-font glyphs for accented Latin, Greek and Cyrillic.

## Platforms

- Published store builds for Android and iOS, signed desktop installers, Flatpak.
- iOS and Android safe-area queries from the OS when winit exposes them.
