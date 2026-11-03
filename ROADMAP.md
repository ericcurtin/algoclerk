# Roadmap

AlgoClerk 0.2 teaches real C++17 with 86 levels in eleven wings. Linux is built and run; the other four
platforms share the code but their app builds are not re-verified (`docs/BUILDING.md`). Where it could go next:

## Language and levels

- **More of C++**, always under the rule that the interpreter and `g++` / `clang++` must agree
  (`docs/CPP_SUBSET.md`): more of the STL (`<ranges>`-free algorithms, `std::string_view` operations,
  `std::bitset`, more `<numeric>`), then small `struct`s with methods and constructors, operator overloading
  for comparators, simple function templates, `std::move` and smart pointers, and `double` once the
  differential tests can pin its behaviour down (`long`, `short` and `double` are not in the subset yet, see
  "Not supported (yet)" in `docs/CPP_SUBSET.md`). Each needs checker rules, interpreter semantics with the
  matching undefined-behaviour checks, cost-model rows and translations.
- More wings: tries and union-find, intervals and sweep lines, backtracking packs, system-design flavoured
  "office tools" (an LRU cache as a level with classes), string matching (KMP), graph weights (Dijkstra with
  `std::priority_queue` and `std::greater`), and more C++-semantics levels for The Supply Cupboard, the wing
  that already covers references versus copies, erase-remove, custom comparators, `priority_queue` with
  `greater`, `string_view` and the iterator `erase` idiom.
- An "interview polish" list of suggestions on the C++ tab (not implemented).
- Teach the Rust and C translators the one idiom they refuse in the campaign (`drop-negatives`, an iterator
  erase loop over `std::list`).
- Level text translations (overlay files `levels/<pack>/<id>.<lang>.toml`) and translated diagnostics.
- A level editor in the game that writes the TOML (generators, oracle skeleton, naive solution).
- Possible future direction: additional player languages (Python, Java, Rust, Go) through the same typed IR, so
  one campaign could be practised in several languages. This is only a note; nothing is planned or promised.

## Front end

- Wide editor mode in landscape, and checking touch drag on a real device (both unverified).
- Screen reader support through AccessKit; full keyboard navigation of the card editor.
- Drag and drop of cards between blocks (today a card moves among its siblings and can be re-created inside
  another card with the palette).
- Larger clerk wardrobe (hats, glasses) and more office decorations unlocked by wings.
- A real "step into helper function" animation with a second tray row for the callee.
- More interface languages and bitmap-font glyphs for accented Latin, Greek and Cyrillic.

## Platforms

- Published store builds for Android and iOS, signed desktop installers, Flatpak.
- iOS and Android safe-area queries from the OS when winit exposes them.
