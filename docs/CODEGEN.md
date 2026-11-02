# Equivalent-code reveal

When a level is solved, the victory screen shows the player's own program as **C, C++ and Rust**
(tabs, copy button, syntax highlighting, size/speed stats and a note naming the algorithm). The same
translator is available in the CLI (`algoclerk-cli codegen --lang c|cpp|rust --level X --solution file`)
and in sandbox mode. The code is a reward: before solving, the game only shows it after an explicit
"show it anyway" confirmation.

Implementation: `crates/algoclerk-core/src/codegen/`. It is general: it works for **any valid
program**, not for particular levels.

## The machine model

The generated function models the clerk as

- `hand`: an `int` (the hand is never "empty" in generated code; programs that would stop with an
  error in the game are outside the contract),
- the floor: named local variables (one per tile that is used with a direct address, named after the
  level's tile names) or, when the program uses indirect addressing, an array `floor[N]`,
- the inbox as an input sequence with a `next` index; running out of input returns from the function,
- the outbox as an output vector (or a returned value, see below).

`INBOX` becomes a bounds check and a read, `OUTBOX` a push (or `return`), `COPYTO`/`COPYFROM`/`ADD`...
plain assignments with a trailing comment naming the original instruction.

## Control flow recovery

`structure.rs` rebuilds structured code from the jump pattern of the instruction list:

- back edges become `loop` / `while (true)` / `do .. while` / `while (cond)`, with `break` and `continue`,
- forward conditional jumps become `if` / `if .. else`,
- jumps that leave the current construct towards a later point become `break 'label` out of a labelled
  block in Rust and `goto` in C and C++,
- a jump to the end of the program becomes `return`.

Every jump is classified exactly once; if any jump does not fit (for example loops with two entries)
the translator **falls back to a form that is always correct**: labelled `goto` in C and C++ (the
clerk's own labels are kept) and a `pc` state machine in Rust. The result is shown with a note.
Correctness of both forms is checked by compiling and running them (below).

## Calling convention

*Generic*: `clerk_run(inbox) -> outbox` (C: `size_t clerk_run(const int *inbox, size_t inbox_len,
int *outbox)`). Levels with per-test floor data additionally take `floor_start`.

*Level metadata*: a level may add a `[code]` section (see [LEVEL_FORMAT.md](LEVEL_FORMAT.md)) that
describes how its inbox and floor map to parameters and what the result is, so the generated code reads
like an answer to the question:

```rust
pub fn max_of(values: &[i32]) -> i32 { ... }            // returns = "scalar"
pub fn merge_sorted(first: &[i32], second: &[i32]) -> Vec<i32> { ... }   // returns = "list"
pub fn has_pair_with_sum(values: &[i32], n: i32, target: i32) -> bool    // bool return
```

The generated function first builds the clerk's inbox from the parameters, exactly as the level's
tests would have supplied it. A `scalar` return is returned at the first `OUTBOX` (a program that
solves the level outputs exactly one value). The validator checks that, for every test of the level,
splitting the inbox and floor into arguments and rebuilding them reproduces the original input.

## Verification

Generated code is verified, not trusted:

- `crates/algoclerk-levels/tests/codegen_compile.rs`: for every shipped reference solution, in both the
  level-signature and the generic form, in C, C++ and Rust, with every installed compiler (gcc, clang,
  g++, clang++, rustc), **with warnings as errors**, the generated code and a generated test harness are
  compiled and run on all test cases; outputs must equal the interpreter's exactly.
- `crates/algoclerk-core/tests/codegen_random.rs`: proptest corpora of random valid programs
  (well structured ones and ones with arbitrary jumps), translated both normally and with the
  structurer disabled (forcing the goto/state-machine form), compiled and compared. A property test also
  asserts that every well-structured program is actually recovered as structured code.
- All external processes run under a hard kill timeout (`timeout -s KILL` when available).
- `algoclerk-cli codegen --verify` runs the same check for your own program and level.

A language whose compiler is missing is skipped with a message in the test output (CI installs all of them).

## Syntax highlighting

`highlight.rs` is a small dependency-free tokenizer for the generated code (keywords, types, numbers,
comments, strings, preprocessor lines) used by the game to colour the text. It is pure Rust.

## Limits

- Programs that stop with a runtime error in the game (empty hand, empty tile, out-of-range pointer,
  overflow, division by zero) have no defined translation: the generated code uses `0` for empty values
  and plain `int` arithmetic.
- Overflow of the six-digit paper limit is not modelled; values stay far below the range of `int`
  for all valid runs.
