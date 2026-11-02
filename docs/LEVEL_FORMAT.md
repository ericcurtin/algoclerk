# Level file format

Levels are **data, not code**. Each level is one TOML file at `levels/<pack>/<id>.toml`; each pack
directory has a `pack.toml`. The files are embedded in the binary at build time (no runtime file
access on mobile) and extra packs can be loaded from a user folder on desktop. You can add a puzzle
with a pull request that only touches `levels/`.

Check your work with the headless CLI:

```sh
cargo run -p algoclerk-cli -- validate levels/my-pack          # static + dynamic checks
cargo run -p algoclerk-cli -- judge --level my-level --reference
cargo run -p algoclerk-cli -- tune --level my-level            # suggests [targets]
cargo test -p algoclerk-levels                                 # everything, incl. codegen compile tests
```

## `pack.toml`

```toml
title = "Mail Room"                 # shown in the level list
blurb = "Your first day."           # one line
order = 1                           # position of the pack
```

## `<id>.toml`

The file name (without `.toml`) must equal `id`. Ids are lowercase letters, digits and dashes and unique
across all packs. Unknown keys are errors (typos are caught).

```toml
id = "max-of"
title = "Top of the Pile"
order = 3                           # position inside the pack
summary = "Find the largest number in a list."          # one line (menus)
story = "..."                                           # short ORIGINAL flavour text
statement = "..."                                       # the problem, in your own words
constraints = ["The list has between 1 and 8 numbers."] # shown as bullets
hints = ["gentle", "more specific", "nearly the answer"]  # the hint ladder (at least 1)
display = "number"                  # or "char": draw values as characters
```

### Examples

Shown in the statement and also run as visible tests. `input` and `output` are either arrays of
integers or a string (each character becomes its code point; `"ab\u0000"` ends with a 0 marker).

```toml
[[examples]]
input = [3, -2, 7, 0]
output = [7]
note = "optional explanation"
floor = [{ at = 0, values = [1, 2, 3] }]   # optional: extra values on floor tiles for this test
```

### Machine

```toml
[machine]
floor = 4                          # number of floor tiles (1..128)
names = ["best", "candidate"]      # optional tile names (index = tile); used in the UI and generated code
allowed = ["inbox", "outbox", "copyto", "copyfrom", "sub", "jump", "jumpz", "jumpn", "indirect"]

[machine.prefill]                  # tiles that start with a value (constants!)
0 = 0
3 = 99
```

`allowed` accepts the instruction names (`inbox`, `outbox`, `copyto`, `copyfrom`, `add`, `sub`,
`bumpup`, `bumpdn`, `mul`, `div`, `mod`, `jump`, `jumpz`, `jumpn`) and the pseudo key `indirect`.
`inbox` and `outbox` are mandatory. Remember there is no "load constant" instruction: constants such
as 0, 1 or 10 come from prefilled tiles, which also makes levels a little puzzle of their own.

### Tests

```toml
[tests]
seed = 203                         # makes generated cases deterministic on every platform
step_limit = 20000                 # optional, instructions per test run

[[tests.fixed]]                    # hand-written edge cases: at least 3 with an explicit output
input = [-5, 0]
output = [-5]
note = "single negative number"

[[tests.generators]]               # generated cases
count = 9
inbox = [{ kind = "list", len = [1, 8], range = [-9, 9], nonzero = true, terminator = 0 }]
```

Expected outputs of generated cases are computed by running the reference solution, so a level needs
at least 8 tests overall, a correct `[solution]`, and hand-written cases that pin down the intended
behaviour. The repository additionally cross-checks every shipped level against an independent Rust
implementation (`crates/algoclerk-levels/tests/oracles.rs`); adding an oracle for your level is
welcome but not required.

#### Generator segments

A generator has up to three lists of **segments**: `vars` (generated first, never emitted), `floor`
(`{ at = <first tile>, seg = {...} }`) and `inbox` (concatenated into the inbox). `of = N` refers to
the N-th earlier segment, counting `vars`, then `floor`, then `inbox` segments.

| `kind` | Produces | Options |
| --- | --- | --- |
| `list` (default) | random list | `len = [min, max]`, `range = [lo, hi]`, `sort = "asc"/"desc"`, `distinct`, `nonzero`, `len_multiple` |
| `int` | one random integer | `range`, `nonzero` |
| `const` | fixed values | `values = [...]` |
| `pick` | an element of segment `of` (probability `chance` %), else random in `range` | `of`, `chance`, `range` |
| `perm` | shuffled consecutive integers starting at `range[0]` | `len`, `drop` (removes elements) |
| `balanced` | `+k` / `-k` bracket sequence, balanced with probability `chance` % | `len`, `kinds`, `chance` |
| `palindrome` | palindromic list with probability `chance` %, else a near miss | `len`, `range`, `chance` |
| `majority` | shuffled list where one value occurs in more than half the positions | `len`, `range` |
| `linked` | a linked list in floor slots (value, next index or -1; `stride` slots per node), aux = head | `len`, `range`, `stride` |
| `aux` | the auxiliary value (head) of segment `of` | `of` |
| `copy` | the values of segment `of` again | `of` |
| `len` | the length of segment `of` as one value | `of` |
| `pairsum` | the sum of two different elements of segment `of` (probability `chance` %), else random | `of`, `chance`, `range` |

Any segment can add `prefix_len = true` (emit the length first) and `terminator = 0` (append a marker).
Everything is driven by a built-in SplitMix64 generator (`crates/algoclerk-core/src/testgen.rs`), so
the same seed gives the same tests on every platform and in every release.

### Targets

```toml
[targets]
size_par = 13      # instructions for a staple
size_ace = 12      # instructions for a gold seal
speed_par = 80     # average steps for a staple
speed_ace = 60     # average steps for a gold seal
```

`algoclerk-cli tune` suggests numbers from your reference solution. The validator requires the
reference to meet both `par` targets.

### Reference solution

```toml
[solution]
asm = """
    INBOX
    ...
"""
```

The reference is used by tests and for generated expectations. The game hides it until the player
has solved the level or explicitly asks for it behind a warning.

### Code generation metadata (`[code]`) and `[explain]`

These make the translated C, C++ and Rust read like an answer to the interview question. See
[CODEGEN.md](CODEGEN.md) for details.

```toml
[code]
function = "max_of"                 # snake_case name
doc = "Returns the largest value."  # one-line comment
returns = "scalar"                  # "scalar" (first OUTBOX value) or "list" (all outputs)
return_type = "int"                 # or "bool"
params = [
  { name = "target", kind = "scalar" },          # one inbox value
  { name = "values", kind = "terminated" },      # inbox values followed by a 0 marker (terminator = 0)
]
```

Parameter `kind`s:

- `scalar`: one inbox value,
- `list`: all remaining inbox values (last inbox parameter only),
- `counted`: the length, then that many values,
- `terminated`: the values, then a terminator (default `terminator = 0`),
- `length`: the length of an earlier list parameter named by `of` (not a function argument).

Add `at = <tile>` to put a parameter on the floor instead of the inbox (a scalar goes to one tile, a
list fills tiles from `at`; `cap` limits the list). The validator checks that for every test the
arguments rebuild exactly the original inbox and floor.

```toml
[explain]
algorithm = "Linear scan for the maximum"
text = "A few sentences: the standard name of the idea and why it works."
```

## Rules for contributed levels

- Write the story, statement, hints and names yourself. Do not copy text, puzzles or level designs
  from other games or from problem sites; classic algorithm *topics* are fair game.
- Keep tile names different from parameter names and from `hand`, `next`, `inbox`, `outbox`, `floor`
  (a test checks this).
- Tile names must be unique within a level.
- Run `cargo test -p algoclerk-levels` before opening the pull request.
