# Code reveal: the player's C++ as C++, Rust and C

When a level is solved the game shows the player's program in three tabs. The first is the player's own
accepted solution, formatted. The other two are **translations of the checked C++ syntax tree** into
idiomatic Rust and plain C: the translators walk the typed tree (`cpp::ast` plus the checker's side tables),
keep the player's names and structure, add the level's signature, and never use `goto`.

```
algoclerk-cli codegen LEVEL [FILE] --lang c|cpp|rust [--verify] [--no-prelude]
```

prints a tab (`--no-prelude` leaves out the helper prelude); `--verify` compiles the translation together with
a generated test harness (`rustc`, `gcc` and `clang`; a missing compiler is skipped) and compares its output on
the level's standard tests with the interpreter.

## API (`crates/algoclerk-core/src/cpp/codegen/`)

```rust
pub enum Lang { C, Cpp, Rust }
pub struct Generated { pub lang: Lang, pub prelude: String, pub code: String } // full() joins them
pub fn generate(c: &Compiled, lang: Lang, entry: &str) -> Result<Generated, String>
```

`Err` is a friendly sentence such as `this clerk cannot translate a lambda that captures variables (C has no
closures) to C yet`. The translators **refuse rather than guess**: a construct whose meaning cannot be kept
faithfully gives an `Err`, never wrong code, and a panic inside a translator is caught and reported the same
way. The game shows the message in the tab instead of code. `prelude` is the part the UI collapses by default
(imports, helpers, node arenas, container templates), `code` the translated functions.

The module replaces the earlier generator, which has been deleted.

## C++ tab

The level's prelude (includes, `ListNode`, `TreeNode`) is `prelude`, the player's items formatted with
`cpp::print::print_program` are `code`.

## Rust (2021)

| C++ | Rust |
| --- | --- |
| `int`, `unsigned`, `long long`, `unsigned long long`, `size_t`, `char`, `bool` | `i32`, `u32`, `i64`, `u64`, `usize`, `u8`, `bool` |
| `std::vector<T>`, `std::array<T,N>`, `std::stack<T>` | `Vec<T>`, `[T; N]`, `Vec<T>` |
| `std::string`, `std::string_view`, `const std::string&` | `String`, `&str`, `&str` |
| `std::unordered_map`, `std::map`, `std::unordered_set`, `std::set` | `HashMap`, `BTreeMap`, `HashSet`, `BTreeSet` |
| `std::queue`, `std::deque`, `std::list` | `VecDeque` |
| `std::priority_queue<T>` / with `std::greater<T>` | `BinaryHeap<T>` / `BinaryHeap<Reverse<T>>` |
| `std::pair`, `std::tuple`, structured bindings | tuples, tuple patterns |
| `std::optional<T>` | `Option<T>` |
| `const T&` / `T&` parameters | `&T` / `&mut T` |
| `ListNode*`, `TreeNode*`, player structs of scalar fields | `Copy` handle into a small `thread_local!` arena (`ListNode::new`, `.val()`, `.set_next(..)`), so nodes alias and cycles work as with pointers |
| lambdas | closures (`move` when captured by copy) |
| `for (int i = a; i < b; ++i)` | `for i in a..b` (also `<=`, `--`, `i += k`, `step_by`) |
| range-for | `for x in v.iter()`, `for (k, v) in m.iter()`, `for &x in ..` |
| `m[k]` on maps | `m.entry(k).or_default()` |
| `m.find(k) != m.end()`, `m.count(k)` | `contains_key`, `contains` |
| `it = m.find(k); it->second` | `Option` from `get_key_value` / `get_mut` |
| `std::sort(v.begin(), v.end(), cmp)` | `v.sort()` / `v.sort_by(..)` |
| `switch` | `match` (cases that end in `break` or `return`) |

Arithmetic is plain `i32` / `i64` (the interpreter reports signed overflow as a runtime error, so the
translation never has to wrap); `unsigned` and `size_t` use `wrapping_add` / `wrapping_sub` / `wrapping_mul`
(`+=` on `usize` with a literal is written directly). Casts between integral types are explicit `as`; indexes
are converted to `usize` explicitly. Division and remainder truncate toward zero in both languages. Copies of
containers are explicit `.clone()`; the last `return` of a function is its tail expression.

**Refused in Rust** (friendly message): overloaded functions, functions taking lambdas or `std::function`,
generic lambdas (except comparators), lambdas that capture a variable that is also changed outside them or that
copy a container into their closure, `std::priority_queue` ordered by a lambda, iterators other than the result
of `find`/`begin`/`end` used as plain positions, copying a struct by value, struct constructors with bodies or
several constructors, `switch` cases that fall through or a `default` that is not last, `continue` inside
`do`-`while`, assigning to a character of a string (`s[i] = c`), swapping two parts of one container with
`std::swap`, globals that are not integer constants, the comma operator inside an expression.

Known limitation: `char` is `u8` (the subset is ASCII), so programs that depend on `char` values of 128 and
above being negative differ; programs whose references alias a variable that is modified while the reference is
alive are not valid Rust (`rustc` rejects them: the harness test lists these two classes).

## C (C11)

The prelude is generated per program from small macro templates, only for the types the program uses (collapsed
by default in the game):

| C++ | C |
| --- | --- |
| `std::string` | `Str` (`Str_from`, `Str_cat`, `Str_substr`, `Str_find`, ...) |
| `std::vector<T>`, `std::array`, `std::stack` | `Vec_T` (`VEC` template: `_push`, `_pop`, `_insert`, `_erase`, `_sort`, `_lower`, ...) |
| `std::deque`, `std::queue`, `std::list` | `Deq_T` (ring buffer) |
| `std::priority_queue` | `Heap_T_max` / `Heap_T_min` (binary heap) |
| `std::unordered_map`, `std::unordered_set` | `Hmap_K_V` / `Hset_K` (open addressing; iterate over `cap` slots) |
| `std::map`, `std::set` | `Smap_K_V` / `Sset_K` (sorted arrays; iterate in key order) |
| `std::pair`, `std::tuple`, `std::optional` | `Pair_A_B`, `Tup_A_B_C`, `Opt_T` structs |
| `ListNode*`, `TreeNode*`, player structs | plain structs with `X_make` (value) and `X_new` (heap) |
| `T&`, `const T&` of containers | `T *` |
| lambdas used as comparators or called by name | lifted `static` functions (no captures) |

Element types name their helpers (`int_copy`, `Str_eq`, `Pair_int_int_lt`, ...), so containers nest
(`Vec_Vec_int`, `Hmap_Str_Vec_Str`). Copies of containers are explicit (`Vec_int_copy`); the prelude does not
free memory (these programs are short lived, which keeps the body readable). Operators and control flow are
written as in the C++ source; only `std::` calls become function calls on the prelude types.

**Refused in C**: everything refused in Rust that is about overloading, plus lambdas that capture variables
(C has no closures), generic lambdas (except comparators), `std::function`, a struct stored by value inside a
container, pair or optional, `std::priority_queue` ordered by a lambda, iterators other than the result of
`find`, a mutable reference to a map entry (`for (auto& kv : m)`), loops over native arrays other than
fixed-size ones, `std::set` / `std::map` constructors from ranges other than sets of a vector or string.

## Semantics that are preserved

Integer arithmetic and conversions follow C++ (the unsigned wrap cases are explicit in Rust); collection
assignment copies; `&` parameters alias; comparison and sorting are lexicographic on strings, pairs, tuples and
vectors; `std::map` and `std::set` iterate by key; hash containers iterate in an unspecified order in all three
languages (the judge rejects solutions whose answer depends on it, and the harness prints them sorted).

## Tests

* `crates/algoclerk-levels/tests/cpp_codegen_levels.rs`: **every** shipped level's reference and naive
  solution is translated to Rust and C, wrapped in a generated harness (a `main` that builds every standard
  test's arguments from the level's literals, calls the translated function and prints the result in the
  notation of `cpp/value.rs`), compiled with `rustc`, `gcc -std=c11 -Wall -Wextra` and `clang` (every compiler
  and binary under `timeout -s KILL`; missing compilers are skipped) and compared line by line with the
  interpreter. The allowlist of untranslatable levels (`ALLOW`) holds one level, `drop-negatives` (an
  `it = l.erase(it)` loop over a `std::list`, for Rust and C); an entry that translates after all is an error.
* `crates/algoclerk-core/tests/cpp_codegen_random.rs`: the programs of the interpreter's differential corpus
  (`tests/data/cpp_interp`, 170+ snippets that exercise the whole subset) and of `tests/data/cpp_corpus` (called
  with generated arguments) are translated where possible and compared the same way; the number of verified
  translations has a floor so that a regression in coverage is noticed.
* Unit tests in `cpp/codegen/mod.rs`: friendly errors instead of panics, the C++ tab, missing entry.

The harness code (`cpp/codegen/harness*.rs`) is public so that `algoclerk-cli codegen --verify` and the tests
share it.
