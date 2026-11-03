# ADR 0007: real C++ as the player language

Status: accepted. Supersedes ADR 0006 (the invented high-level language) as the language players write. The
interpreter, cost model, judge and level pipeline of ADR 0002, 0003 and 0006 stay; only the language changes.

## Context

ADR 0006 replaced the assembly puzzles with an invented, typed, high-level language. That fixed the level of
abstraction (players think in hash maps and sliding windows), but the owner's review found a new problem: a
language that exists only inside the game cannot be used anywhere else. Practising interview questions is
only worth the time if the thing practised is the thing used in the interview. Real C++ is the most common
language for algorithm interviews alongside Python and Java, it has exactly the containers and algorithms the
puzzles are about (`std::unordered_map`, `std::priority_queue`, `std::sort`, iterators), and its costs and
pitfalls (copying a vector by value, iterator invalidation, signed overflow) are themselves things worth
learning.

## Decision

Players write a carefully defined **subset of real C++17**, documented in `docs/CPP_SUBSET.md`.

- **Every accepted program is valid C++17 with one meaning.** It compiles with `g++ -std=c++17` and
  `clang++ -std=c++17` when placed in the level's translation unit (the level's read-only prelude, then the
  player's code) and behaves identically on both. Anything that would make the two compilers (or two
  platforms) disagree is outside the subset: `long`, `int64_t`, `double`, templates, I/O, exceptions,
  `std::move`, C++20 library additions, and so on. The checker explains each refusal politely.
- **The game never runs a compiler.** It has its own lexer, parser, type checker and tree-walking interpreter
  for the subset, written in Rust. There is no libclang, no tree-sitter, no WebAssembly and no external
  process at run time, so the game works on phones and on machines with no toolchain (ADR 0002 is unchanged).
- **Levels keep the shape of ADR 0003.** A level supplies a prelude (includes and helper structs such as
  `ListNode`) and a real C++ signature such as `std::vector<int> twoSum(const std::vector<int>& nums, int
  target)`. Locks are real names: headers (`<algorithm>`), `std::` names and members, each with a reason that
  the clerk quotes ("the supply cupboard on this floor does not stock `<numeric>`: adding things up is the
  exercise").
- **The cost model mirrors the real complexity of the STL** (`docs/COST_MODEL.md`), including copies: passing
  a `std::vector<int>` by value costs its size, `const&` costs nothing. This keeps the hidden large tests,
  naive-solution timeouts, medals and the empirical time and space classes working unchanged.
- **Two editing surfaces stay in sync.** The program text is the source of truth; cards are statements of the
  C++ tree, and anything cards cannot express is shown as a read-only "raw C++" card.
- **The code reveal changes meaning.** The C++ tab is the player's own accepted program, formatted. The Rust and
  C tabs are translations of the checked C++ tree (`docs/CODEGEN.md`), and a construct that cannot be
  translated faithfully gives a friendly message rather than wrong code.

### Undefined behaviour policy

Real C++ silently accepts many bugs, and a practice game that accepts them teaches the wrong lesson. The
interpreter therefore **detects every undefined-behaviour class it can name and reports it as a friendly clerk
error** (the offending line highlighted, a speech-bubble message), and the judge counts such a program as
wrong:

out-of-range `operator[]` and `at()`, signed integer overflow (including `INT_MIN / -1` and `-INT_MIN`),
division and remainder by zero, invalid shift counts, null, dangling and deleted pointer use, reading an
uninitialised scalar, using an iterator or reference after the container changed, unsequenced modification of
one scalar, falling off the end of a non-void function, reading from an empty container, and comparators
that are not a strict weak ordering where this is detectable. Runaway loops, too-deep recursion and huge
allocations stop with `time`, `depth` and `memory` errors. The full table is in `docs/CPP_SUBSET.md`. The policy
is the goal; the few cases where the interpreter still misses or misclassifies a class are listed, honestly,
in `docs/PROGRESS.md` and covered by a shrinking `UB_KNOWN` list in the tests.

Where C++ leaves behaviour unspecified instead of undefined, the interpreter takes a fixed choice and the judge
guards against dependence on it: the iteration order of `unordered_map` and `unordered_set` is unspecified, so
the judge re-runs the standard tests with the reverse order and rejects a solution whose answer changes, and
levels never rely on the order of equal elements under `std::sort`.

### How fidelity is guaranteed

Trust comes from tests against the real compilers, not from argument (the same approach as ADR 0004 and 0006):

- **Level differential tests.** Every reference and naive solution of every level is built into a translation
  unit with a generated `main`, compiled with `g++` and `clang++` (`-std=c++17 -Wall -Wextra -O0`, warnings are
  failures), run on the standard tests plus reduced large tests, and compared with the declared expected
  values, with the independent Rust oracles (`crates/algoclerk-levels/tests/common/mod.rs`) and with the
  interpreter. The reference is also built with `-fsanitize=undefined,address` to prove it is UB free.
- **Property tests.** A proptest generator produces random type-correct programs that are UB free by
  construction (loops, containers, algorithms, lambdas, structured bindings, integer promotion, unsigned
  wraparound, shifts, casts, `switch`, `do`-`while`). Each is checked, interpreted and compared with the
  output of the compiled program (`g++`, and `clang++` for a sample or for all with `ALGOCLERK_DIFF_CLANG`).
  Failures are shrunk to a minimal program.
- **Negative tests.** A table with one or more programs per UB class above must stop the interpreter with the
  expected error at the expected line; where a sanitizer runtime is installed the same program must also fail
  under `-fsanitize=undefined,address`.
- **Corpus and fuzz tests.** A corpus of realistic solutions must compile with both compilers, parse and
  print back to an equal tree, and pass the checker. The lexer, parser, checker and interpreter are fuzzed
  (token soup, mutated corpus files) for no panics and bounded time and memory.
- **Translation tests.** Rust and C translations are compiled with `rustc` and `gcc`/`clang` together with a
  generated harness and must print what the interpreter returns.

CI installs `g++`, `clang++` and `rustc`; a developer machine without one skips that part of the suite with a
message. Where the interpreter and a compiler are found to disagree, the disagreement is recorded as a known
divergence, excluded from the generator by name, and removed from that list when fixed (see `docs/PROGRESS.md`
for the current state). A known divergence is a bug of the game, never an accepted difference.

## Alternatives considered

- **Python.** Very common in interviews and trivial to run on a phone in principle, but our own interpreter
  would still have to reimplement it, and the pitfalls the game wants to teach (copies, invalidation, overflow,
  container cost) mostly do not exist there. A Python player language through the same typed IR remains a
  possible future direction (see `ROADMAP.md`).
- **Keep the invented language (ADR 0006), maybe with C++-like syntax.** Cheapest, and the front end and levels
  already worked, but "C++-flavoured" programs that real compilers reject, or that mean something different,
  are worse than either option: the player learns habits that do not transfer. The honest choices are a clearly
  invented language or the real thing.
- **Full C++ through a real toolchain** (call `g++` or `clang++`, or embed libclang or clang compiled to
  WebAssembly). Complete and exact, but it needs a compiler on every device, cannot run on iOS or Android
  without a large embedded toolchain, breaks the no-WebAssembly and permissive-licence policies, cannot give
  step-by-step animation, abstract cost, hidden-large-test timeouts with a deterministic budget, or friendly
  errors for undefined behaviour (the compiler runs the bug silently). Ruled out.

## Consequences

- The campaign is rewritten in C++: signatures, preludes, reference and naive solutions, hint ladders, lock
  lists with reasons. The old language, its documentation and its tests are removed; ADR 0006 stays as history.
- Saves from the previous version cannot be migrated (their drafts and solutions are in a language that no
  longer exists). The progress file format is bumped to version 2; older files are loaded without crashing, keep
  settings, solved flags and medals, and drop drafts and saved solutions, with a note shown to the player.
- The subset has documented gaps (no templates or classes with methods, no `double`, no I/O, no C++20 library).
  Each is a candidate for a later level wing or a good first issue; none may be filled in a way that makes the
  interpreter and the compilers disagree.
- Adding a library member is real work: the stdlib table, the checker rule, the interpreter behaviour, the cost,
  the card-editor flag and the documentation table must all be added, and the differential tests must cover it
  (see `CONTRIBUTING.md`).
- The interpreter, not the compiler, defines the cost model and the friendly error messages. The compilers
  define the meaning of every program that is not an error. The differential tests are what keeps the two
  consistent, so they are part of the product and must stay green in CI.
- Maintenance cost is higher than for an invented language: the subset follows a real standard and two
  compilers. Staying within C++17 and refusing anything the compilers disagree on keeps it bounded.
