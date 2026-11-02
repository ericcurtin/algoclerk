# The C++ subset of AlgoClerk

Players write **real C++17**. Every program the game accepts is valid C++17 that compiles and behaves
identically with `g++ -std=c++17` and `clang++ -std=c++17` when placed in the level's generated translation
unit (the level's prelude, then the player's code). The game does **not** run a compiler: it has its own
lexer, parser, type checker and interpreter for the subset below (no libclang, no tree-sitter, no WebAssembly),
and the repository's differential tests compare that interpreter with real `g++` and `clang++` on every
reference solution, every naive solution and a corpus of random programs (ADR 0007).

If a program is outside the subset the game says so politely ("this floor's supply cupboard does not stock
..."). If a program is inside the subset but would be **undefined behaviour** in real C++, the interpreter stops
with a clerk-style error and the judge counts it as wrong (see "Undefined behaviour policy").

## The translation unit

```cpp
#include <vector>            // the level's prelude (read-only): includes and provided structs
#include <unordered_map>

std::vector<int> twoSum(const std::vector<int>& nums, int target) {   // the level's signature
    std::unordered_map<int, int> seen;                                // the player's body
    for (int i = 0; i < static_cast<int>(nums.size()); ++i) {
        if (auto it = seen.find(target - nums[i]); it != seen.end()) {
            return {it->second, i};
        }
        seen[nums[i]] = i;
    }
    return {};
}
```

* The player writes the whole function (the signature is fixed and checked) plus optional helper functions,
  `using` aliases, `const`/`constexpr` globals and extra `#include` lines.
* `#include` is allowed only for headers the level has **unlocked**. A name is usable only when the header
  that declares it (as the C++ standard specifies, not what happens to be transitively included) is included
  by the prelude or by the player. Using a locked or unstocked name gives a friendly message naming the thing
  and the reason.
* `using namespace std;` is accepted (then `vector<int>` works as in real C++); without it `std::` is required.
* `<bits/stdc++.h>` is not standard and is refused.

## Types

| Type | Notes |
| --- | --- |
| `bool`, `char` | `char` is a signed 8 bit ASCII character |
| `int` | 32 bit signed; overflow is undefined behaviour (reported) |
| `unsigned` / `unsigned int` | 32 bit, wraps modulo 2^32 (defined) |
| `long long`, `unsigned long long` | 64 bit; unsigned wraps |
| `size_t`, `std::size_t` | 64 bit unsigned (`.size()` returns it); wraps |
| `std::string`, `std::string_view` | ASCII bytes |
| `std::vector<T>`, `std::array<T, N>`, `T a[N]` (C array) | `vector<vector<T>>` works |
| `std::pair`, `std::tuple` | `std::get<I>`, `.first`/`.second`, structured bindings, `std::make_pair`, `std::tie`-free |
| `std::optional<T>` | `has_value`, `value`, `*`, `->`, `value_or`, `std::nullopt` |
| `std::unordered_map`, `std::map`, `std::unordered_set`, `std::set` | |
| `std::stack`, `std::queue`, `std::deque`, `std::list` | |
| `std::priority_queue<T>` | also with `std::greater<T>` or a lambda comparator (`decltype(cmp)`) |
| `std::function<R(A...)>` | for recursive lambdas |
| `T*` / `ListNode*`, `TreeNode*` | pointers to level-provided (or unlocked) structs, `->`, `nullptr`, `new`, `delete` |
| `auto`, `const`, `T&`, `const T&`, `auto&`, `const auto&` | `auto&&` and `T&&` are not in the subset |

**Not in v1** (documented gaps, checker says so politely): `long`, `short`, `float`, `double`, `<cstdint>` types
(`int64_t` is `long` on Linux but `long long` elsewhere, which would break the "identical on all compilers"
guarantee; write `long long`), template definitions, classes with methods, operator overloading, inheritance,
exceptions (`throw`, `try`; `at()` that would throw is reported as a runtime error), `goto`, `std::cin` /
`std::cout` and every other I/O, `std::shared_ptr` / `unique_ptr`, `std::move` and rvalue references,
`std::ranges`, C++20 features (`contains`, `starts_with`, `<=>`, `std::span`...).

## Statements and expressions

Declarations (`int x = 0;`, `int a = 1, b;`, `std::vector<int> v(n, 0);`, `std::vector<int> v{1, 2};`,
`auto [k, v] = *it;`), expression statements, `if` / `else if` / `else` (with C++17 init statement), `switch`
with `case`/`default`/`break` and fall-through (integral conditions), classic `for` (comma operator allowed in
the increment, several declarators in the init), range-`for` (`for (auto& [k, v] : m)`), `while`, `do`-`while`,
`break`, `continue`, `return`, blocks, comments (kept by the text-to-cards-to-text round trip).

Operators with C++ precedence and the usual arithmetic conversions: `+ - * / %`, `& | ^ ~ << >>`, comparisons,
`&& || !`, `?:`, assignment and compound assignment, `++`/`--` (prefix and postfix), `,`, unary `*` and `&`,
`static_cast<T>(e)`, `(T)e` and `T(e)` casts between scalar types, lambdas `[&](auto a, int b) { ... }`
(capture by reference or value, generic `auto` parameters), brace initialisation `{a, b}`, `new T(args)`.
Integer literals take their C++ type from value and suffix (`u`, `ll`, `ull`).

Helper functions (free functions) and recursion are allowed where the level unlocks them (call depth limit 200
frames in the interpreter).

## Undefined behaviour policy

Real C++ silently accepts many bugs. The interpreter detects and reports each of these with the line
highlighted and a speech-bubble message; the judge rejects the program:

| Class | Detected as |
| --- | --- |
| `operator[]` out of range on `vector`/`string`/`array`/`deque`; `at()` out of range (which would throw) | `index` |
| signed integer overflow (`int` and `long long`, including `INT_MIN / -1`, `-INT_MIN`, `abs(INT_MIN)`) | `overflow` |
| division or remainder by zero | `div-zero` |
| shift count negative or not smaller than the width, left shift of a negative value | `shift` |
| null pointer dereference, use after `delete`, pointer to a local that left scope | `null`, `dangling` |
| reading an uninitialised scalar or pointer | `uninit` |
| use of an iterator or reference after the container was modified (insert, erase, push_back, clear, resize, rehash) or `end()` dereference | `invalidated` |
| unsequenced modification of the same scalar twice or modification and read (`i + i++`, `f(i++, i)`) | reported by the checker |
| falling off the end of a non-void function | `missing-return` |
| infinite loop (cost budget), too deep recursion (200 frames), huge allocation (4 million elements) | `time`, `depth`, `memory` |
| popping or reading `front`/`back`/`top` of an empty container, `*v.end()`, `optional::value` on empty | `empty` |
| comparator that is not a strict weak ordering where detectable | `bad-compare` |

Iteration order of `unordered_map` / `unordered_set` is unspecified in C++. The interpreter takes an explicit
iteration order and the judge re-runs the standard tests with the reverse order; a solution whose answer
changes is reported as depending on unordered iteration order.

The full table of supported library members, their cost in the abstract cost model and whether the card editor
can express them is in the "Library" section below (generated from `crates/algoclerk-core/src/cpp/stdlib.rs`
and verified by tests).
