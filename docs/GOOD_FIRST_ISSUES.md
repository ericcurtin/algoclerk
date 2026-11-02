# Good first issues

Ideas that are well-scoped and friendly to newcomers. Open an issue (or comment on one) to claim an
item. "Level" items only touch `levels/`; "Translation" items only touch `lang/`.

## Levels (data only)

1. **Second-largest**: output the second-largest distinct value of a list (counting-house pack).
2. **Average (integer)**: output the integer average of a list using `DIV`.
3. **Count negatives**: filter-and-count with `JUMPN`.
4. **Pairwise maximum**: for pairs (a, b) output `max(a, b)`.
5. **Triangular numbers**: n -> n(n+1)/2 without `MUL`, then with `MUL`.
6. **Leap year**: output 1 if a year is a leap year (`MOD`).
7. **Binary digits**: output the binary digits of n, most significant first (needs a stack on the floor).
8. **Reverse digits of a number** and **palindromic number**.
9. **Anagram check** for two short words over a small alphabet (histogram trick).
10. **First unique element** of a list.
11. **Move zeroes to the end** (stable, in place on the floor).
12. **Rotate the floor array right by one** using pointer tiles.
13. **Running maximum** (prefix maximum) and **running minimum**.
14. **Two-sum with indices**: output the positions of the pair (two pointers on sorted input).
15. **Intersection of two sorted lists** (merge-like).
16. **Stock profit**: best single buy/sell gain over a list of prices.
17. **House robber** (DP with a rolling window) and **min-cost climbing stairs**.
18. **Longest common prefix length** of two words.
19. **Queue with two stacks**: simulate with floor arrays.
20. **Detect a cycle in a linked list** using index pointers (tortoise and hare).
21. **Find the middle of a linked list**.
22. **Pascal's triangle row** n using a floor array.
23. **Greatest common divisor of a list** (fold with Euclid).
24. **Perfect numbers** up to n.
25. A whole new **pack** (for example "Trees and Heaps": array-based heap push/pop).

## Translations (data only)

26. Add a UI language (`crates/algoclerk-core/lang/<code>.toml`): German, French, Spanish, Portuguese,
    Japanese, Chinese, Hindi, Arabic, Russian ... (for non-Latin scripts also see the font item below).
27. Proof-read `en.toml`: shorter wording for phone screens, friendlier error messages.

## Game and UI

28. **New theme**: a sepia or solarized palette in `theme.rs`, with screenshots from `scripts/screenshots.sh`.
29. **Fallback fonts** for CJK or Arabic (OFL only) and the matching `CREDITS.md` entry.
30. **Clerk expressions**: animate the clerk's eyebrows or a blink on success (`office.rs`).
31. **Sound design**: new synthesised effects in `sound.rs` (still no audio files).
32. **Keyboard navigation** of the program list (arrow keys to select, Enter to edit, Delete to remove).
33. **Undo/redo** for program edits in the list.
34. **Share as QR code** (pure-Rust generation, permissive licence) for phone-to-phone solution sharing.
35. **Level search** in the level list.
36. **Per-level notes**: let the player jot a note that is saved with the draft.

## Tooling and quality

37. `algoclerk-cli compare <dir>`: verify a directory of shared solutions and print a table.
38. A `cargo xtask`-style script (in Rust) that regenerates the platform icons from the icon renderer.
39. More property tests for `Structurer` (for example comparing against a tree interpreter).
40. Fuzz targets (`cargo-fuzz`) for the parser and level loader.
41. Improve the CI caching and add a job that builds with `--features audio` on Windows and macOS.
42. Documentation: an illustrated tutorial for the text syntax, or a "solve level 1-10" walkthrough
    (please keep walkthroughs separate from the in-game reference solutions).
