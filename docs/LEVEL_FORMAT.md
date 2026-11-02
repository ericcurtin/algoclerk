# Level format

A level is one TOML file under `levels/<pack>/<id>.toml`; the file name (without `.toml`) is the id. A pack is
a directory with a `pack.toml` (`title`, `blurb`, `order`). Levels are embedded in the binary at build time
and extra packs can be loaded from a user directory on desktop (`ALGOCLERK_LEVELS=/path` or the data
directory's `levels/`). Files that do not parse (for example a level still written for the old ClerkScript
language) are skipped and reported in `Campaign::load_errors`; they never stop the game.

Players write real C++17 (see [CPP_SUBSET.md](CPP_SUBSET.md)). The level supplies a read-only **prelude**
(includes and helper structs) and a fixed **signature**; the player's text is the prelude, then the player's
code, in one translation unit:

```cpp
#include <vector>                                                        // prelude (read only)

#include <unordered_map>                                                 // player: unlocked headers
std::vector<int> twoSum(const std::vector<int>& nums, int target) {      // the level's signature
    ...                                                                  // the player's body
}
```

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `title`, `order`, `story`, `statement`, `constraints` | text | brief shown to the player. `order` is the sort key inside the pack |
| `prelude` | text | read-only code above the player's: `#include` lines, helper structs (`ListNode`, `TreeNode`), `using` aliases. May be empty |
| `signature` | text | one real C++ function signature, **with parameter names**, no body, no `;`. Parsed into an `EntrySig` |
| `unlocked` | list of headers | headers the player may `#include` in addition to the prelude's: `["<algorithm>", "<unordered_map>"]`. `["*"]` unlocks every stocked header |
| `locked` | list of `{ name, why }` | names the player must not use, with the reason shown on the brief and in the error message (see below) |
| `unlock` | table `{ helpers, recursion, structs }` | booleans (default false): extra functions, recursion, player-defined structs |
| `hints` | list of text | the hint ladder, at least three |
| `technique`, `big_o` | text | the standard technique and `"O(n) time, O(n) space"`; the estimator must not measure worse than `big_o` |
| `reference` | text | the reference solution: the **player's part** (own `#include`s, helpers, the function), not the prelude |
| `naive` | text, optional | a correct but slow solution (same rules as `reference`); must exceed the large budget on large tests |
| `compare` | `"exact"`, `"sort_outer"`, `"sort_all"` | how outputs are compared (see below) |
| `[budgets]` | `steps`, `large_steps`, `sweep_n0` | cost budgets per standard / large test; first size of the complexity sweep (0 disables) |
| `[medals]` | `size = [b, s, g]`, `speed = [b, s, g]` | bronze, silver, gold targets (lower is better): program size (statement and expression nodes of the player's code) and average cost on the standard tests |
| `[[examples]]`, `[[tests]]` | `args`, `expect`, `note` | visible examples (at least two) and hidden hand-written tests |
| `[[generate]]`, `[sweep]` | generator blocks | seeded generators (below) |

Removed fields: `types` and `builtins` (the old unlock lists). A file that still contains them fails to load.

### Prelude

* `#include` lines for every header the **signature** needs (`<vector>`, `<string>`, ...) and for every header
  the player gets "for free". Headers listed here are usable without being listed in `unlocked`.
* Helper structs must be written like this (the real-compiler harness and the interpreter's list/tree values
  rely on these names, fields and the `int` constructor):

```cpp
struct ListNode {
    int val;
    ListNode* next;
    ListNode(int v) : val(v), next(nullptr) {}
};

struct TreeNode {
    int val;
    TreeNode* left;
    TreeNode* right;
    TreeNode(int v) : val(v), left(nullptr), right(nullptr) {}
};
```

* `using Grid = std::vector<std::vector<char>>;` aliases are understood by the signature parser (one alias
  per line, `using` or `typedef`).
* The prelude is read-only for the player and never counts for the size medal.

### Signature

Only types of the subset (see CPP_SUBSET.md): `bool char int unsigned long long unsigned long long size_t`,
`std::string`, `std::vector<T>`, `std::array<T, N>`, `std::pair`, `std::tuple`, `std::optional`, `std::map`,
`std::unordered_map`, sets, `std::stack/queue/deque/list/priority_queue`, `T*` for prelude structs. `long`,
`double`, `short` are refused. `const T&` parameters make the entry read-only and cost 1 to pass; by-value
containers cost their size (the cost model, see COST_MODEL.md), which is a good topic for a hint.
Entry functions must return a value (not `void`) so the judge and the harness can compare results; to model
"modify in place" ask for the new container to be returned.

Use camelCase names (`twoSum`): the file name is the kebab-case id and stays the stable key.

### `unlocked` and `locked`

* Names are **headers** (`"<algorithm>"`), **`std::` names** (`"std::sort"`, `"std::accumulate"`) or **members**
  written like the standard does (`"std::string::substr"`, `"std::vector::insert"`).
* A header that is neither in the prelude nor in `unlocked` cannot be included ("the supply cupboard does
  not stock <x> yet"). A name listed in `locked` produces "the supply cupboard on this floor does not stock
  <x>: <why>" and wins over everything else: use it to explain *why* the obvious library call is not available
  (migrate the old lock notes: `sum` -> `std::accumulate` + `<numeric>`, `max` -> `std::max`,
  `std::max_element`, `sorted` -> `std::sort`, ...). List the header too when the whole drawer would defeat
  the lesson.
* A name is usable only when its **real standard header** is included (by the prelude or by the player):
  the reference and naive texts must contain their own `#include` lines and nothing is accepted because a
  header happens to include another.

### Compare modes

`exact`: equal. `sort_outer`: the outer sequence may be in any order. `sort_all`: every nested sequence and the
outer one may be in any order. Elements of `unordered_map` and `unordered_set` results are **always** sorted
before comparing (their iteration order is unspecified in C++). The judge additionally re-runs the standard
tests with unordered containers iterating in reverse order and rejects a solution whose answer changes
("depends on unordered iteration order"); a level must therefore never have an order dependent expected value.
`std::sort` is not stable: never write a level whose answer depends on the order of equal elements.

## Value literals

Arguments and expected values use the C++ brace-initialiser notation of `crates/algoclerk-core/src/cpp/value.rs`
(the notation the interpreter prints and the real-compiler harness prints):

| Type | Literal |
| --- | --- |
| `int`, `long long`, `unsigned`, `size_t` | `42`, `-7` (must fit the type) |
| `bool`, `char` | `true`, `'a'`, `'\n'`, `'\x80'` |
| `std::string` | `"text"` with `\" \\ \n \t \0 \xNN` escapes |
| `vector`, `array`, `deque`, `list`, sets | `{1, 2, 3}`, `{}` |
| nested | `{{1, 3}, {2, 6}}`, `{{'1', '0'}, {'0', '1'}}` |
| `pair`, `tuple` | `{1, "a"}` |
| `map`, `unordered_map` | `{{"a", 1}, {"b", 2}}` |
| `optional` | `nullopt`, `{5}` |
| `ListNode*` | `{1, 2, 3}`, `{}` or `nullptr` for the empty list, `{1, 2, 3} @1` when the last node points back to node 1 |
| `TreeNode*` | level order with holes: `{3, 9, 20, nullptr, nullptr, 15, 7}` |

Old `[..]` becomes `{..}`, `null` becomes `nullptr`, `none` becomes `nullopt`.

Remember C++ integer semantics: `int` is 32 bits and overflow is undefined behaviour (reported by the
interpreter and by `-fsanitize=undefined`). The old ClerkScript `int` was 64 bits: when a value or an
intermediate result can exceed about 2 billion, the signature and the reference must use `long long`. Check
the generators' ranges (a `sum_of` argument, `n * n`, prefix sums, products).

## Generators

`kind` is one of `int`, `bool`, `char`, `string`, `vec_int`, `vec_bool`, `vec_char`, `vec_string`, `grid_int`,
`grid_char`, `pairs`, `edges`, `list`, `tree`, `sorted_lists`, `rpn`; each emits the literal notation above
(`vec_int` for `std::vector<long long>` too: the digits are the same). Sizes (`len`, `rows`, `nodes`, `m`, ...)
are a number, a `[lo, hi]` range or an expression in `n` (`"n"`, `"n/2"`, `"3*n/2"`, `"sqrt(n)"`, `"n-1"`).
Useful options: `range`, `sorted`, `distinct`, `rotated`, `descending`, `perm_missing`, `single_out`, `alphabet`,
`words`, `palindrome`, `periodic`, `prefix`, `suffix`, `brackets` + `balanced_pct` + `nested`,
`anagram_of = <earlier argument>` + `change_pct`, `pick_from`, `sum_of`, `index_of`, `in_len_of`, `below`
(relations to earlier arguments), `intervals`, `directed`, `dag`, `connected`, `chain`, `bst`, `swap_pct`,
`cycle_pct`, `square`. The generator source is `crates/algoclerk-core/src/testgen.rs`; a SplitMix64 generator
keeps the tests identical on every platform. Expected values of generated tests come from the reference
solution (run by the interpreter and, in the differential test, by real compilers).

## A complete example

```toml
title = "Matching Pair"
order = 1
story = "The accountant has a shelf of invoices and one cheque. Two invoices add up exactly to the cheque, and she wants to know which two."
statement = "Find two different positions i < j with nums[i] + nums[j] == target. If several pairs work, return the one with the smallest j. Return {} when no pair works. All numbers are different."
prelude = '''
#include <vector>
'''
signature = "std::vector<int> twoSum(const std::vector<int>& nums, int target)"
constraints = ["0 <= nums.size() <= 20000", "all values in nums are different", "result is {i, j}"]
technique = "Hash map lookup"
big_o = "O(n) time, O(n) space"
unlocked = ["<unordered_map>"]
locked = [{ name = "std::sort", why = "Sorting loses the positions you must report." }]
compare = "exact"
hints = [
  "For each number x you need target - x. Have you seen it already?",
  "Keep a `std::unordered_map<int, int>` from value to index of every number you have walked past.",
  "`if (auto it = seen.find(want); it != seen.end()) { return {it->second, i}; }`, then `seen[nums[i]] = i;`.",
]
reference = '''
#include <unordered_map>

std::vector<int> twoSum(const std::vector<int>& nums, int target) {
    std::unordered_map<int, int> seen;
    for (int i = 0; i < static_cast<int>(nums.size()); ++i) {
        const int want = target - nums[i];
        if (auto it = seen.find(want); it != seen.end()) {
            return {it->second, i};
        }
        seen[nums[i]] = i;
    }
    return {};
}
'''
naive = '''
std::vector<int> twoSum(const std::vector<int>& nums, int target) {
    const int n = static_cast<int>(nums.size());
    for (int j = 0; j < n; ++j) {
        for (int i = 0; i < j; ++i) {
            if (nums[i] + nums[j] == target) {
                return {i, j};
            }
        }
    }
    return {};
}
'''

[unlock]                        # optional: helpers = true, recursion = true, structs = true
helpers = false

[budgets]
large_steps = 3000000

[medals]
size = [48, 38, 30]
speed = [78, 42, 30]

[[examples]]
args = ["{2, 7, 11, 15}", "9"]
expect = "{0, 1}"

[[examples]]
args = ["{1, 2}", "10"]
expect = "{}"
note = "No pair adds up."

[[tests]]
args = ["{5, -5}", "0"]
expect = "{0, 1}"

[[generate]]
seed = 201
count = 10
args = [
  { kind = "vec_int", len = [0, 8], range = [-15, 15], distinct = true },
  { kind = "int", sum_of = 0, miss = 20, range = [-30, 30] },
]

[[generate]]
seed = 202
count = 1
large = true
n = 20000
args = [
  { kind = "vec_int", len = "n", range = [-1000000, 1000000], distinct = true },
  { kind = "int", range = [3000000, 3000000] },
]
```

The editor starts with `Level::starter_code()`: the signature with an empty body.

## Writing code that is valid on every compiler

Everything in a level (reference, naive) must be valid C++17 that compiles **warning-free** with
`g++ -std=c++17 -Wall -Wextra` and `clang++ -std=c++17 -Wall -Wextra` and has no undefined behaviour. The usual
traps:

* `-Wsign-compare`: `i < v.size()` with `int i` warns. Use `std::size_t i`, or `static_cast<int>(v.size())`.
  Counting down with an unsigned index: `for (std::size_t i = v.size(); i > 0; --i) { v[i - 1]; }`.
* `size_t` and `int` do not mix in `std::max` / `std::min` (`std::max(int, long long)` is a template error).
* Implicit narrowing inside `{}` is an error in C++: `return {it->second, i};` is fine for two `int`s, but
  `return {size, 1};` with a `size_t` is not.
* Do not rely on the order of `unordered_map` iteration, on evaluation order of function arguments, on the
  order of equal elements after `std::sort`, or on a moved-from value.
* Every non-void path must return; reading an uninitialised variable or `v[v.size()]` is undefined.
* `std::vector<bool>` is not supported by the subset; use `std::vector<int>` or `std::vector<char>`.
* Spell the header of every library name you use (`<algorithm>` for `std::max`, `<utility>` for `std::pair`,
  `<functional>` for `std::greater`, `<cstddef>`-family for `std::size_t` (any of `<vector>`, `<string>` suffices)).

## What the validator proves (`algoclerk-cli validate`, `crates/algoclerk-levels/tests/shipped.rs`)

- the signature parses, the reference type-checks under the level's unlock lists and passes **all** tests
  within the step budgets (with 3x headroom), earning gold in both medals;
- the measured time and space class are no worse than `big_o`;
- the `naive` solution compiles, passes every standard test, and **times out on a large test**;
- the reference does not depend on unordered iteration order;
- at least two examples, three hints, a story, a technique, and at least one large generator;
- independent oracles written in plain Rust (`crates/algoclerk-levels/tests/oracles.rs`) agree with the
  expected value of every test of every level.

## Differential testing against real compilers

`crates/algoclerk-core/src/cpp/harness.rs` turns a level, a solution text and a list of cases into a complete
C++ translation unit: the prelude, the solution, generated printers and `ListNode*` / `TreeNode*` builders, and
a `main` that builds every case's arguments, calls the entry function and prints one canonical line per case
(exactly the notation of `value::render`; unordered containers are printed sorted by key).

```
cargo test -p algoclerk-levels --test cpp_diff_levels            # all levels, g++ and clang++, large tests capped
ALGOCLERK_LEVEL=two-sum cargo test -p algoclerk-levels --test cpp_diff_levels -- --nocapture
ALGOCLERK_DIFF_FULL=1 cargo test -p algoclerk-levels --test cpp_diff_levels     # large tests at full size
cargo test -p algoclerk-core --test cpp_harness_selftest         # printers/builders vs value.rs for every type
algoclerk-cli diff two-sum [--full] [--no-sanitize]              # the same check from the command line
algoclerk-cli harness two-sum                                    # print the generated test program
```

For every level and for the reference and the naive solution it checks: both compilers accept the code with
no warnings (`-Wall -Wextra`); the printed results equal the level's declared expected values (and, for
generated tests, the reference's results) in canonical form; the independent oracle agrees; the reference
built with `-fsanitize=undefined,address` runs clean (skipped with a note when the sanitizer runtime is not
installed; clang's is used when g++'s is missing); and, when the interpreter is available, the interpreter's
results equal the compilers'. Compilers and binaries run under `timeout -s KILL`.
