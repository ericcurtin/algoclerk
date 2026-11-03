# The cost model of the C++ interpreter

The interpreter (`crates/algoclerk-core/src/cpp/interp.rs` and `interp/`) measures **abstract cost units**,
not wall-clock time. A solution's cost is the number of units a run consumed; the judge compares it with the
level's budget ("time limit") and with the reference solutions. The units mirror the *real complexity* of
every operation, so an `O(n)` solution costs about `n` units and an `O(n^2)` one about `n^2`, whatever the
host machine does.

Rules that apply everywhere:

* Every **executed statement** costs 1 (a declaration statement costs 1 however many names it declares,
  `;` costs 1). A braced block `{ }` and the `case` / `default` labels cost nothing themselves; the
  statements inside do.
* Every **loop iteration** costs 1 more (`for`, `while`, `do`-`while`, range-`for`), so an empty endless loop
  still burns the budget.
* Every **operator node** of an expression costs 1: unary and binary operators, assignment and compound
  assignment, `?:`, `[]`, member access `.` / `->`, casts, `new`, `delete`, lambda creation, braced lists,
  constructor expressions and every call. Plain names and numeric / character literals are free; a string
  literal costs 1 (it builds a `std::string`).
* A **call** costs 1 (the call node) plus its arguments plus whatever the callee executes. Library
  operations add the extra cost listed below on top of that 1.
* **Copies are charged** (see "Copies and references"), references are not.
* Costs in the tables are the **total** for the call including the call node (so "1" means "only the call
  node"), **not** counting the evaluation of the arguments: an argument costs what it costs on its own (a
  literal or a plain scalar name 0, `v.begin()` 1 for the call, a named iterator 1 because it is copied, a
  lambda expression 1). The tests in `tests/cpp_cost_model.rs` lock the rows against the interpreter.
* Calling a lambda or function from a library algorithm (`for_each`, `count_if`, comparators ...) costs 1
  for the call plus whatever the body executes.

`log` below is `ceil(log2(n + 1))` (number of bits of `n`) unless written `cl2(n) = ceil(log2 n)`.

## Copies and references

| Situation | Cost |
| --- | --- |
| Copy of a scalar (`bool`, `char`, integers) | 0 extra |
| Copy of a pointer, iterator, lambda, `std::function` or `string_view` held in a variable (`auto c = b;`, an iterator passed by name) | 1 |
| Copy of a `std::string` | its length |
| Copy of a container (`vector`, `deque`, `list`, `map`, `set`, `unordered_*`, stacks, queues) | the number of **leaf elements**: for `vector<vector<int>>` all the ints, for a `vector<string>` the sum of the string lengths |
| Copy of a `pair`, `tuple`, `optional`, struct | the leaves inside |
| Passing by value (`std::vector<int> v`) | the copy cost of the argument |
| Passing by reference (`T&`, `const T&`) | 0 (the reference aliases the object) |
| `auto x = v;` / `T x = v;` / `x = v;` (copy initialisation or assignment from an lvalue) | the copy cost |
| `auto& x = v;`, `const auto& x = v;`, range-`for` over `auto&` | 0 |
| `for (auto x : v)` with container elements | the copy cost of each element |
| Initialising from a temporary (`T x = f();`, `return f();`, `v = g();`) | 0: the temporary is moved |
| `return x;` where `x` is a plain local variable or by-value parameter | 0: the variable is moved out (C++ does a move or copy elision as well) |
| Capturing by value in a lambda (`[=]`, `[v]`) | the copy cost, once, when the lambda is created |
| Constructing `n` copies (`vector<T>(n, x)`, `assign(n, x)`, `resize(n, x)`, `insert(pos, n, x)`) | `n` times the leaves of `x` (at least `n`) |
| Entry function arguments | by-value parameters are charged their copy cost, references 0 |

Moving a container out of a function costs nothing; `std::move` is not part of the subset (the checker
refuses it), so the only moves are the implicit ones above.

## Strings (`std::string`, `std::string_view`)

| Operation | Cost |
| --- | --- |
| `size`, `length`, `empty`, `[]`, `at`, `front`, `back`, `push_back`, `pop_back`, `begin`, `end` | 1 |
| `s += t`, `s.append(t)` | 1 + length of the added part (`s += c` is 1 + 1) |
| `s + t`, `s + c`, `c + s` (new string) | 1 + `|s| + |t|` (the `+` node is the 1) |
| `substr(i, k)` | 1 + length of the result (a `string_view` slice costs 1) |
| `find`, `rfind` | 2 + positions examined (up to and including the hit, all of them on a miss) + needle length |
| `find_first_of`, `find_last_of`, `find_first_not_of`, `find_last_not_of` | 2 + positions examined |
| `compare`, `==`, `<` ... | 2 + min of the two lengths (the operator forms: the operator node is the extra 1) |
| `insert(pos, t)` | 1 + the characters moved (tail length) + the inserted length |
| `erase(pos, n)` | 1 + the tail length; `erase(it)` 1 + the string length |
| `replace` | 1 + the characters moved plus the inserted length (see `tests/cpp_cost_model.rs` for exact rows) |
| `clear` | 1 + the old length |
| `resize(n)` | 1 + `n` when growing, 1 + the new length when shrinking |
| `assign(t)` | 1 + the new length |
| `std::string(n, c)`, `s.append(n, c)` | 1 + `n` |
| `std::to_string(x)` | 1 + number of digits |
| `std::stoi`, `stoll` | 1 + length of the text |

## `std::vector`, `std::array`

| Operation | Cost |
| --- | --- |
| `[]`, `at`, `size`, `empty`, `front`, `back`, `begin`, `end`, `rbegin`, `rend` | 1 |
| `push_back`, `emplace_back`, `pop_back` | 1 (amortised) |
| `insert(pos, x)` at index `i` of `n` | `n - i + 1` |
| `insert(pos, k, x)` / range insert | `n - i + k + 1` |
| `erase(pos)` / `erase(a, b)` at index `i` | `n - i + 1` |
| `resize(n)`, `assign(n, x)`, `fill` (array) | `1 + n` (the new size) |
| `clear` | `1 +` the leaves released |
| `vector<T>(n)`, `vector<T>(n, x)` (a declaration, no call node) | `n` |
| `reserve`, `shrink_to_fit` | 1 (`reserve` is refused when huge) |
| `swap` | 1 (the contents are exchanged) |
| copy (`vector<T> b = a;` ...) | leaves (see above) |
| `==`, `<` on vectors | 2 + min length (the operator node is the extra 1) |

## `std::deque`, `std::queue`, `std::stack`, `std::list`

| Operation | Cost |
| --- | --- |
| deque: `push_front`, `push_back`, `pop_front`, `pop_back`, `front`, `back`, `[]`, `at`, `size`, `empty` | 1 |
| deque: `insert(pos, x)` at index `i` of `n` | `min(i, n - i) + k + 1` (`k` elements inserted) |
| deque: `erase(pos)` at index `i` of `n` | `min(i, n - i) + 1` |
| deque: `clear` | `1 +` the leaves released |
| queue: `push`, `pop`, `front`, `back`, `size`, `empty`, `emplace`; stack: `push`, `pop`, `top`, `size`, `empty`, `emplace` | 1 |
| list: `push_*`, `pop_*`, `front`, `back`, `size`, `empty`, `erase(it)` | 1 |
| list: `insert(it, x)` | 1 + the number of elements inserted |
| list: `erase(a, b)` | the number of elements removed (at least 1) |
| list: `sort`, `sort(cmp)` | `1 + n * cl2(n)` (comparator bodies are charged on top) |
| list: `reverse`, `remove`, `remove_if`, `unique`, `clear` | `1 + n` |
| list: `resize(k)` | `1 + k` |

## `std::map`, `std::set`, `std::unordered_map`, `std::unordered_set`, `std::priority_queue`

| Operation | Cost |
| --- | --- |
| `map` / `set`: `find`, `count`, `insert`, `emplace`, `try_emplace`, `insert_or_assign`, `erase(key)`, `erase(it)`, `[]`, `at`, `lower_bound`, `upper_bound`, `equal_range` | `log + 1` |
| `map` / `set`: `begin`, `end`, `rbegin`, `rend`, `size`, `empty`, iterator `++` / `*` / `->` | 1 |
| `map` / `set`: `clear` | `1 + n` |
| `unordered_map` / `unordered_set`: `find`, `count`, `insert`, `emplace`, `try_emplace`, `erase`, `[]`, `at` | 1 |
| `unordered_*`: `clear` | `1 + n` |
| `priority_queue`: `push`, `emplace`, `pop` | `1 +` the number of bits of `n + 1`, `n` being the size before the call (comparator bodies on top) |
| `priority_queue`: `top`, `size`, `empty` | 1 |
| Range-`for` over a map or set | 1 per element, plus 1 per iteration |
| Construct from a braced list of `k` elements | `k` |

Iterating an `unordered_map` / `unordered_set` visits the elements in insertion order (or in reverse
insertion order when the run is configured with `UnorderedOrder::Reverse`); this is what the judge uses to
find solutions that depend on the unspecified order of the real containers.

## `<algorithm>`, `<numeric>`, `<utility>`, `<cctype>`, `<limits>`

| Function | Cost |
| --- | --- |
| `std::sort`, `std::stable_sort` | `1 + n * cl2(n)` (the sort is stable; each comparator call also costs 1 plus its body) |
| `reverse`, `fill`, `iota`, `unique`, `remove`, `remove_if`, `rotate`, `next_permutation`, `prev_permutation` | `1 + n` (the predicate of `remove_if` is called `n` times, 1 plus its body each) |
| `accumulate`, `count`, `count_if`, `find` (positions scanned), `find_if`, `any_of`, `all_of`, `none_of`, `min_element`, `max_element`, `equal` | `1 + n` (or the elements visited) |
| `copy` | `1 + 2n` (`n` read, `n` written) |
| `lexicographical_compare` | `1 +` both range lengths `+` the positions compared |
| `for_each` | `1 + n`, plus 1 plus the body of the function for each element |
| `partial_sum` | `1 + 2n` (`n` elements read, `n` written) |
| `binary_search`, `lower_bound`, `upper_bound` on a random access range | `cl2(n) + 1` |
| `std::distance`, `std::advance`, `std::next`, `std::prev` on random access iterators | 1 (on `map`, `set`, `list` iterators: 1 plus the number of steps) |
| `std::swap`, `std::iter_swap` | 1 (containers are swapped, not copied) |
| `std::max`, `std::min`, `std::abs`, `std::make_pair`, `std::get`, `std::make_tuple` | 1 (the initializer-list form of `max` / `min` adds the list length) |
| `std::isdigit` ... `std::toupper`, `std::numeric_limits<T>::max()` | 1 |
| `new T(...)` | 1 plus the construction |

## Memory (`peak_cells`, `Limits::cells`)

Live memory is estimated in *cells*: a scalar variable or element is 1 cell, a string `len / 8 + 1`, a
container `1 +` the cells of its elements, a struct `1 +` its fields, every heap node (`new`) its own
cells. The interpreter samples the whole state periodically (more often when `Limits::measure` is set, and
proportionally to the amount of live data so that sampling costs `O(1)` amortised per cost unit) and
reports the maximum as `peak_cells`; a sample above `Limits::cells` stops the run with `RtKind::Memory`.
Single huge requests (`vector<int>(1000000000)`, `resize`, `string(n, c)`) are refused up front.
`input_cells` is the same estimate for the arguments of the entry function.

## Undefined behaviour in the cost model

A detected undefined behaviour stops the run with a `RtError`; the cost already consumed is reported. The
checks themselves cost nothing, and neither does the optional (uncharged) verification of a sort
comparator.
