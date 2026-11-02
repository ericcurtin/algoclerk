# Level format

A level is one TOML file under `levels/<pack>/<id>.toml`; the file name (without `.toml`) is the id. A pack is
a directory with a `pack.toml` (`title`, `blurb`, `order`). Levels are embedded in the binary at build time
and extra packs can be loaded from a user directory on desktop (`ALGOCLERK_LEVELS=/path` or the data
directory's `levels/`). Run `algoclerk-cli validate` to check everything.

```toml
title = "Matching Pair"
order = 1                       # sort key inside the pack
story = "The accountant has a shelf of invoices ..."          # original office flavour
statement = "Find two different positions i < j ..."          # the plain problem
signature = "fn two_sum(nums: vec<int>, target: int) -> vec<int>"   # fixed header the player cannot change
constraints = ["0 <= len(nums) <= 20000", "all values in nums are different"]
technique = "Hash map lookup"   # the standard technique, shown on victory
big_o = "O(n) time, O(n) space" # must match what the estimator measures (it may be better, never worse)
types = ["map", "vec"]          # unlocked type families (those in the signature are always unlocked)
builtins = ["contains", "len"]  # unlocked tools; ["*"] unlocks everything
locked = [{ name = "sorted", why = "The point is to do the sorting yourself." }]
compare = "exact"               # or "sort_outer" / "sort_all" when the answer order is free
hints = ["...", "...", "..."]   # at least three, shown as sticky notes
reference = '''fn two_sum(...) -> vec<int> { ... }'''
naive = '''fn two_sum(...) -> vec<int> { ... }'''   # optional: correct but slow; must TIME OUT on the large tests

[unlock]                        # optional
helpers = false                 # allow extra functions
recursion = false               # allow recursion

[budgets]                       # optional
steps = 200000                  # cost budget per standard test
large_steps = 3000000           # cost budget per large test
sweep_n0 = 64                   # first size of the complexity sweep (0 disables the sweep)

[medals]
size = [60, 48, 38]             # bronze, silver, gold: program size (lower is better)
speed = [900, 520, 400]         # bronze, silver, gold: average cost on the standard tests

[[examples]]                    # at least two; shown in the memo
args = ["[2, 7, 11, 15]", "9"]  # one literal per parameter
expect = "[0, 1]"
note = "optional explanation"

[[tests]]                       # hidden hand-written edge cases (same shape, no note)
args = ["[]", "0"]
expect = "[]"

[[generate]]                    # seeded generators; expected values come from the reference solution
seed = 202
count = 8
large = true                    # performance tests: run with large_steps
n = 20000                       # value of n in size expressions
args = [
  { kind = "vec_int", len = "n", range = [-1000000, 1000000], distinct = true },
  { kind = "int", range = [3000000, 3000000] },
]

[sweep]                         # optional generator for the complexity sweep (defaults to the first large block)
args = [ ... ]
```

## Value literals

Arguments and expected values use the same text form the interpreter prints: `42`, `true`, `'a'`,
`"text"`, `[1, 2]`, `(1, "a")`, `{1, 2}` (set), `{"a": 1}` (map), `some(3)` / `none`, a linked list as
`[1, 2, 3]` (or `[1, 2, 3] @1` for a tail that points back to index 1), and a tree in level order with holes
`[1, 2, 3, null, 4]`.

## Generators

`kind` is one of `int`, `bool`, `char`, `string`, `vec_int`, `vec_bool`, `vec_char`, `vec_string`, `grid_int`,
`grid_char`, `pairs`, `edges`, `list`, `tree`, `sorted_lists`, `rpn`. Sizes (`len`, `rows`, `nodes`, `m`, ...) are
a number, a `[lo, hi]` range or an expression in `n` (`"n"`, `"n/2"`, `"3*n/2"`, `"sqrt(n)"`, `"n-1"`).
Useful options: `range`, `sorted`, `distinct`, `rotated`, `descending`, `perm_missing`, `single_out`, `alphabet`,
`words`, `palindrome`, `periodic`, `prefix`, `suffix`, `brackets` + `balanced_pct` + `nested`,
`anagram_of = <earlier argument>` + `change_pct`, `pick_from`, `sum_of`, `index_of`, `in_len_of`, `below`
(relations to earlier arguments), `intervals`, `directed`, `dag`, `connected`, `chain`, `bst`, `swap_pct`,
`cycle_pct`, `square`. The generator source is `crates/algoclerk-core/src/testgen.rs`; a SplitMix64 generator
keeps the tests identical on every platform.

## What the validator proves (`algoclerk-cli validate`, `crates/algoclerk-levels/tests/shipped.rs`)

- the signature parses, the reference type-checks under the level's unlock lists and passes **all** tests
  within the step budgets (with 3x headroom), earning gold in both medals;
- the measured time and space class are no worse than `big_o`;
- the `naive` solution compiles, passes every standard test, and **times out on a large test**;
- at least two examples, three hints, a story, a technique, and at least one large generator;
- independent oracles written in plain Rust (`crates/algoclerk-levels/tests/oracles.rs`) agree with the
  expected value of every test of every level;
- the generated C, C++ and Rust of every reference solution compile and match the interpreter
  (`crates/algoclerk-levels/tests/codegen_compile.rs`).

Medal targets are computed with `algoclerk-cli tune --all --write` from the reference solution (size
x1.6 / x1.25 / x1.0 and cost x3 / x1.6 / x1.1).
