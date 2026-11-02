# Code reveal: ClerkScript to C, C++ and Rust

When a level is solved the game shows the player's program as the code they would write in an interview, in
three tabs. The translation walks the typed syntax tree directly (no `goto`), keeps the player's variable
names and adds the level's signature. `algoclerk-cli codegen LEVEL [FILE] --lang c|cpp|rust [--verify]
[--no-prelude]` prints it (and `--verify` compiles and compares it with the interpreter).

## C++ (C++17)

`std::vector`, `std::string` (`"text"s`), `std::unordered_map` / `std::unordered_set` (with a small
`ac::Hash` for tuples and vectors), `std::stack`, `std::queue`, `std::deque`, `std::priority_queue`,
`std::optional`, range-for with structured bindings, `ListNode*` and `TreeNode*`. A tiny `namespace ac`
prelude supplies what the standard library lacks (`pop` returning a value, `sorted`, `split`, `join`, `get_or`,
and `keys`/`values`/`items` which return the entries **sorted by key** so output is deterministic).

## Rust (2021)

`Vec`, `String`, `HashMap`, `HashSet`, `VecDeque`, `BinaryHeap`, iterators and `for` loops, tail `return`
as an expression, `&T` for read-only helper parameters and `&mut T` for `&mut` parameters, `.clone()` exactly
where ClerkScript copies a collection. Linked structures use a tiny arena (`ListRef`, `TreeRef`) in the prelude.

## C (C11)

A clearly separated helper prelude (collapsed by default in the game) instantiates small typed containers for
exactly the types the program uses: `Vec_int`, `Set_str`, `Map_int_Vec_int`, `Deq_..`, `Heap_..`, `Tup_..`,
`Opt_..` (open-addressing hash tables, ring-buffer deques, binary heaps). The solution body is written plainly
against them. The prelude does not free memory: these programs are short-lived, which keeps the body readable.

## Semantics that are preserved

Integer division and remainder truncate toward zero; `s += c` appends; collection assignment copies;
`&mut` parameters alias; comparison and sorting are lexicographic on strings, tuples and vectors; map and
set iteration is by sorted key; a `for` counter is never modified inside its loop (the checker forbids it).

## Tests (`crates/algoclerk-levels/tests/codegen_compile.rs`, `crates/algoclerk-core/tests/codegen_random.rs`)

For **every reference solution** and for a corpus of **random type-correct programs** (24 functions per
seed, many seeds) the generated code is wrapped in a generated harness, compiled with `gcc`, `g++`, `clang`,
`clang++` and `rustc` (every runner is wrapped in `timeout -s KILL`; a missing compiler is skipped) and its
output is compared line by line with the interpreter. The pixel font is linted against every character the
generators emit (`crates/algoclerk-game/tests/ui.rs`).
