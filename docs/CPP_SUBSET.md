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
* Including the same header twice (or one the prelude already includes) is harmless, as in real C++ where
  every standard header has include guards.
* `<bits/stdc++.h>` is not standard and is refused.

## Types

| Type | Notes |
| --- | --- |
| `bool`, `char`, `signed char` | `char` is a signed 8 bit ASCII character |
| `unsigned char` | 8 bit, wraps modulo 256 (defined), promotes to `int` in arithmetic; a `std::string` does not accept it in `+=` / `push_back` (use `static_cast<char>`) |
| `int` | 32 bit signed; overflow is undefined behaviour (reported) |
| `unsigned` / `unsigned int` | 32 bit, wraps modulo 2^32 (defined) |
| `long long`, `unsigned long long` | 64 bit; unsigned wraps |
| `size_t`, `std::size_t` | 64 bit unsigned (`.size()` returns it); wraps |
| `std::string`, `std::string_view` | ASCII bytes |
| `std::vector<T>`, `std::array<T, N>`, `T a[N]` (C array) | `vector<vector<T>>` works |
| `std::pair`, `std::tuple` | `std::get<I>`, `.first`/`.second`, structured bindings, `std::make_pair`, `std::make_tuple`, `std::tie(a, b) = ...` (assignment only) |
| `std::optional<T>` | `has_value`, `value`, `*`, `->`, `value_or`, `std::nullopt` |
| `std::unordered_map`, `std::map`, `std::unordered_set`, `std::set` | |
| `std::stack`, `std::queue`, `std::deque`, `std::list` | |
| `std::priority_queue<T>` | also with `std::greater<T>` or a lambda comparator (`decltype(cmp)`) |
| `std::function<R(A...)>` | for recursive lambdas |
| `T*` / `ListNode*`, `TreeNode*` | pointers to level-provided (or unlocked) structs, `->`, `nullptr`, `new`, `delete` |
| `auto`, `const`, `T&`, `const T&`, `auto&`, `const auto&` | `auto&&` and `T&&` are not in the subset |

**Not in v1** (documented gaps, checker says so politely): `long`, `unsigned long`, `short`, `unsigned short`
(16 bit integers are not stocked: use `int`), `float`, `double`, `std::ptrdiff_t`, `<cstdint>` types
(`int64_t` is `long` on Linux but `long long` elsewhere, which would break the "identical on all compilers"
guarantee; write `long long`), `std::gcd` / `std::lcm` / `std::is_sorted` / `std::stoul` (not in the library
table below), template definitions, classes with methods, operator overloading, inheritance,
exceptions (`throw`, `try`; `at()` that would throw is reported as a runtime error), `goto`, `std::cin` /
`std::cout` and every other I/O, `std::shared_ptr` / `unique_ptr`, `std::move` and rvalue references,
`std::ranges`, C++20 features (`contains`, `starts_with`, `<=>`, `std::span`...).

Valid C++17 that the subset does not describe is refused by the checker, never silently mis-run: the game's
interpreter implements exactly the rows of the library table below (every row has a test that compiles its
example with real compilers, and the property tests compare the interpreter with them on random programs).

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
| reading an uninitialised scalar or pointer (including `int x = x;`) | `uninit` |
| use of an iterator or reference after the container was modified (insert, erase, push_back, clear, resize, rehash), including advancing an iterator whose element was erased, or `end()` dereference | `invalidated` |
| unsequenced modification of the same scalar twice or modification and read (`i + i++`, `f(i++, i)`) | reported by the checker |
| falling off the end of a non-void function | `missing-return` |
| infinite loop (cost budget), too deep recursion (200 frames), huge allocation (4 million elements) | `time`, `depth`, `memory` |
| popping or reading `front`/`back`/`top` of an empty container, `*v.end()`, `optional::value` on empty | `empty` |
| comparator that is not a strict weak ordering where detectable | `bad-compare` |

Iteration order of `unordered_map` / `unordered_set` is unspecified in C++. The interpreter takes an explicit
iteration order and the judge re-runs the standard tests with the reverse order; a solution whose answer
changes is reported as depending on unordered iteration order.

Arithmetic never panics the interpreter: unsigned operations wrap, signed ones are checked (see `overflow`),
and the products of two 64 bit values are computed in a wider type and wrapped (`~0ull * ~0ull` is `1`).

## Library

The full table of supported library members, their cost class in the abstract cost model (`docs/COST_MODEL.md`)
and whether the card editor can express them. It is generated from `crates/algoclerk-core/src/cpp/stdlib.rs`
(`algoclerk-cli subset-table` prints it) and a test (`tests/cpp_subset_doc.rs`) fails when it is stale.

<!-- library-table:begin -->
| Receiver | Name | Header | Signature | Cost | Card |
| --- | --- | --- | --- | --- | --- |
| `std::vector<T>` | `ctor` | `<vector>` | `vector(std::size_t n)` | n | no |
| `std::vector<T>` | `ctor` | `<vector>` | `vector(std::size_t n, const T& x)` | n | yes |
| `std::vector<T>` | `ctor` | `<vector>` | `vector(InputIt first, InputIt last)` | n | no |
| `std::vector<T>` | `ctor` | `<vector>` | `vector(std::initializer_list<T> il)` | n | yes |
| `std::vector<T>` | `ctor` | `<vector>` | `vector(const vector& other)` | n | yes |
| `std::vector<T>` | `operator=` | `<vector>` | `vector& operator=(const vector& other)` | n | yes |
| `std::vector<T>` | `size` | `<vector>` | `std::size_t size() const` | 1 | yes |
| `std::vector<T>` | `empty` | `<vector>` | `bool empty() const` | 1 | yes |
| `std::vector<T>` | `push_back` | `<vector>` | `void push_back(const T& x)` | 1 amortised | yes |
| `std::vector<T>` | `emplace_back` | `<vector>` | `T& emplace_back(Args... args)` | 1 amortised | yes |
| `std::vector<T>` | `pop_back` | `<vector>` | `void pop_back()` | 1 | yes |
| `std::vector<T>` | `back` | `<vector>` | `T& back()` | 1 | yes |
| `std::vector<T>` | `front` | `<vector>` | `T& front()` | 1 | yes |
| `std::vector<T>` | `clear` | `<vector>` | `void clear()` | n | yes |
| `std::vector<T>` | `operator[]` | `<vector>` | `T& operator[](std::size_t i)` | 1 | yes |
| `std::vector<T>` | `at` | `<vector>` | `T& at(std::size_t i)` | 1 | yes |
| `std::vector<T>` | `data` | `<vector>` | `T* data()` | 0 | no |
| `std::vector<T>` | `reserve` | `<vector>` | `void reserve(std::size_t n)` | 0 | no |
| `std::vector<T>` | `shrink_to_fit` | `<vector>` | `void shrink_to_fit()` | 0 | no |
| `std::vector<T>` | `insert` | `<vector>` | `iterator insert(iterator pos, const T& x)` | n | yes |
| `std::vector<T>` | `insert` | `<vector>` | `iterator insert(iterator pos, std::size_t n, const T& x)` | n | no |
| `std::vector<T>` | `insert` | `<vector>` | `iterator insert(iterator pos, InputIt first, InputIt last)` | n | no |
| `std::vector<T>` | `insert` | `<vector>` | `iterator insert(iterator pos, std::initializer_list<T> il)` | n | no |
| `std::vector<T>` | `emplace` | `<vector>` | `iterator emplace(iterator pos, Args... args)` | n | no |
| `std::vector<T>` | `erase` | `<vector>` | `iterator erase(iterator pos)` | n | yes |
| `std::vector<T>` | `erase` | `<vector>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::vector<T>` | `resize` | `<vector>` | `void resize(std::size_t n)` | n | yes |
| `std::vector<T>` | `resize` | `<vector>` | `void resize(std::size_t n, const T& x)` | n | yes |
| `std::vector<T>` | `assign` | `<vector>` | `void assign(std::size_t n, const T& x)` | n | yes |
| `std::vector<T>` | `assign` | `<vector>` | `void assign(InputIt first, InputIt last)` | n | no |
| `std::vector<T>` | `assign` | `<vector>` | `void assign(std::initializer_list<T> il)` | n | no |
| `std::vector<T>` | `begin` | `<vector>` | `iterator begin()` | 1 | no |
| `std::vector<T>` | `end` | `<vector>` | `iterator end()` | 1 | no |
| `std::vector<T>` | `rbegin` | `<vector>` | `reverse_iterator rbegin()` | 1 | no |
| `std::vector<T>` | `rend` | `<vector>` | `reverse_iterator rend()` | 1 | no |
| `std::vector<T>` | `cbegin` | `<vector>` | `const_iterator cbegin() const` | 1 | no |
| `std::vector<T>` | `cend` | `<vector>` | `const_iterator cend() const` | 1 | no |
| `std::vector<T>` | `swap` | `<vector>` | `void swap(vector& other)` | 1 | yes |
| `std::vector<T>` | `operator==` | `<vector>` | `bool operator==(const vector& other) const` | n | yes |
| `std::vector<T>` | `operator!=` | `<vector>` | `bool operator!=(const vector& other) const` | n | yes |
| `std::vector<T>` | `operator<` | `<vector>` | `bool operator<(const vector& other) const` | n | yes |
| `std::array<T,N>` | `size` | `<array>` | `std::size_t size() const` | 1 | yes |
| `std::array<T,N>` | `operator[]` | `<array>` | `T& operator[](std::size_t i)` | 1 | yes |
| `std::array<T,N>` | `at` | `<array>` | `T& at(std::size_t i)` | 1 | yes |
| `std::array<T,N>` | `front` | `<array>` | `T& front()` | 1 | yes |
| `std::array<T,N>` | `back` | `<array>` | `T& back()` | 1 | yes |
| `std::array<T,N>` | `fill` | `<array>` | `void fill(const T& x)` | n | yes |
| `std::array<T,N>` | `begin` | `<array>` | `iterator begin()` | 1 | no |
| `std::array<T,N>` | `end` | `<array>` | `iterator end()` | 1 | no |
| `std::string` | `ctor` | `<string>` | `string(const char* s)` | n | yes |
| `std::string` | `ctor` | `<string>` | `string(std::size_t n, char c)` | n | yes |
| `std::string` | `ctor` | `<string>` | `string(InputIt first, InputIt last)` | n | no |
| `std::string` | `ctor` | `<string>` | `string(const string& other)` | n | yes |
| `std::string` | `operator=` | `<string>` | `string& operator=(const string& other)` | n | yes |
| `std::string` | `npos` | `<string>` | `static const std::size_t npos` | 0 | no |
| `std::string` | `size` | `<string>` | `std::size_t size() const` | 1 | yes |
| `std::string` | `length` | `<string>` | `std::size_t length() const` | 1 | yes |
| `std::string` | `empty` | `<string>` | `bool empty() const` | 1 | yes |
| `std::string` | `push_back` | `<string>` | `void push_back(char c)` | 1 amortised | yes |
| `std::string` | `pop_back` | `<string>` | `void pop_back()` | 1 | yes |
| `std::string` | `back` | `<string>` | `char& back()` | 1 | yes |
| `std::string` | `front` | `<string>` | `char& front()` | 1 | yes |
| `std::string` | `clear` | `<string>` | `void clear()` | n | yes |
| `std::string` | `operator[]` | `<string>` | `char& operator[](std::size_t i)` | 1 | yes |
| `std::string` | `at` | `<string>` | `char& at(std::size_t i)` | 1 | yes |
| `std::string` | `operator+` | `<string>` | `string operator+(const string& other) const` | n | yes |
| `std::string` | `operator+=` | `<string>` | `string& operator+=(const string& other)` | n | yes |
| `std::string` | `append` | `<string>` | `string& append(const string& other)` | n | yes |
| `std::string` | `append` | `<string>` | `string& append(const string& other, std::size_t pos, std::size_t len = npos)` | n | no |
| `std::string` | `append` | `<string>` | `string& append(std::size_t n, char c)` | n | no |
| `std::string` | `append` | `<string>` | `string& append(InputIt first, InputIt last)` | n | no |
| `std::string` | `substr` | `<string>` | `string substr(std::size_t pos, std::size_t len = npos) const` | n | yes |
| `std::string` | `find` | `<string>` | `std::size_t find(const string& needle, std::size_t pos = 0) const` | n | yes |
| `std::string` | `find` | `<string>` | `std::size_t find(char c, std::size_t pos = 0) const` | n | yes |
| `std::string` | `rfind` | `<string>` | `std::size_t rfind(const string& needle, std::size_t pos = npos) const` | n | yes |
| `std::string` | `find_first_of` | `<string>` | `std::size_t find_first_of(const string& set, std::size_t pos = 0) const` | n | no |
| `std::string` | `find_last_of` | `<string>` | `std::size_t find_last_of(const string& set, std::size_t pos = npos) const` | n | no |
| `std::string` | `find_first_not_of` | `<string>` | `std::size_t find_first_not_of(const string& set, std::size_t pos = 0) const` | n | no |
| `std::string` | `find_last_not_of` | `<string>` | `std::size_t find_last_not_of(const string& set, std::size_t pos = npos) const` | n | no |
| `std::string` | `compare` | `<string>` | `int compare(const string& other) const` | n | yes |
| `std::string` | `compare` | `<string>` | `int compare(std::size_t pos, std::size_t len, const string& other) const` | n | no |
| `std::string` | `compare` | `<string>` | `int compare(std::size_t pos, std::size_t len, const string& other, std::size_t pos2, std::size_t len2 = npos) const` | n | no |
| `std::string` | `insert` | `<string>` | `string& insert(std::size_t pos, const string& s)` | n | yes |
| `std::string` | `insert` | `<string>` | `string& insert(std::size_t pos, const string& s, std::size_t subpos, std::size_t sublen = npos)` | n | no |
| `std::string` | `insert` | `<string>` | `string& insert(std::size_t pos, std::size_t n, char c)` | n | no |
| `std::string` | `insert` | `<string>` | `iterator insert(iterator pos, char c)` | n | no |
| `std::string` | `insert` | `<string>` | `iterator insert(iterator pos, std::size_t n, char c)` | n | no |
| `std::string` | `insert` | `<string>` | `iterator insert(iterator pos, InputIt first, InputIt last)` | n | no |
| `std::string` | `replace` | `<string>` | `string& replace(std::size_t pos, std::size_t len, const string& s)` | n | no |
| `std::string` | `replace` | `<string>` | `string& replace(std::size_t pos, std::size_t len, std::size_t n, char c)` | n | no |
| `std::string` | `replace` | `<string>` | `string& replace(std::size_t pos, std::size_t len, const string& s, std::size_t subpos, std::size_t sublen = npos)` | n | no |
| `std::string` | `replace` | `<string>` | `string& replace(iterator first, iterator last, const string& s)` | n | no |
| `std::string` | `erase` | `<string>` | `string& erase(std::size_t pos, std::size_t len = npos)` | n | yes |
| `std::string` | `erase` | `<string>` | `iterator erase(iterator pos)` | n | no |
| `std::string` | `erase` | `<string>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::string` | `resize` | `<string>` | `void resize(std::size_t n, char c = '\0')` | n | yes |
| `std::string` | `assign` | `<string>` | `string& assign(const string& other)` | n | no |
| `std::string` | `assign` | `<string>` | `string& assign(std::size_t n, char c)` | n | yes |
| `std::string` | `assign` | `<string>` | `void assign(InputIt first, InputIt last)` | n | no |
| `std::string` | `reserve` | `<string>` | `void reserve(std::size_t n)` | 0 | no |
| `std::string` | `shrink_to_fit` | `<string>` | `void shrink_to_fit()` | 0 | no |
| `std::string` | `begin` | `<string>` | `iterator begin()` | 1 | no |
| `std::string` | `end` | `<string>` | `iterator end()` | 1 | no |
| `std::string` | `rbegin` | `<string>` | `reverse_iterator rbegin()` | 1 | no |
| `std::string` | `rend` | `<string>` | `reverse_iterator rend()` | 1 | no |
| `std::string` | `cbegin` | `<string>` | `const_iterator cbegin() const` | 1 | no |
| `std::string` | `cend` | `<string>` | `const_iterator cend() const` | 1 | no |
| `std::string` | `swap` | `<string>` | `void swap(string& other)` | 1 | yes |
| `std::string` | `operator==` | `<string>` | `bool operator==(const string& other) const` | n | yes |
| `std::string` | `operator!=` | `<string>` | `bool operator!=(const string& other) const` | n | yes |
| `std::string` | `operator<` | `<string>` | `bool operator<(const string& other) const` | n | yes |
| `std::string_view` | `size` | `<string_view>` | `std::size_t size() const` | 1 | yes |
| `std::string_view` | `npos` | `<string_view>` | `static const std::size_t npos` | 0 | no |
| `std::string_view` | `empty` | `<string_view>` | `bool empty() const` | 1 | yes |
| `std::string_view` | `operator[]` | `<string_view>` | `const char& operator[](std::size_t i) const` | 1 | yes |
| `std::string_view` | `at` | `<string_view>` | `const char& at(std::size_t i) const` | 1 | yes |
| `std::string_view` | `front` | `<string_view>` | `const char& front() const` | 1 | yes |
| `std::string_view` | `back` | `<string_view>` | `const char& back() const` | 1 | yes |
| `std::string_view` | `substr` | `<string_view>` | `string_view substr(std::size_t pos, std::size_t len = npos) const` | 1 | yes |
| `std::string_view` | `find` | `<string_view>` | `std::size_t find(string_view needle, std::size_t pos = 0) const` | n | yes |
| `std::string_view` | `length` | `<string_view>` | `std::size_t length() const` | 1 | yes |
| `std::string_view` | `rfind` | `<string_view>` | `std::size_t rfind(string_view needle, std::size_t pos = npos) const` | n | no |
| `std::string_view` | `find_first_of` | `<string_view>` | `std::size_t find_first_of(string_view set, std::size_t pos = 0) const` | n | no |
| `std::string_view` | `find_last_of` | `<string_view>` | `std::size_t find_last_of(string_view set, std::size_t pos = npos) const` | n | no |
| `std::string_view` | `find_first_not_of` | `<string_view>` | `std::size_t find_first_not_of(string_view set, std::size_t pos = 0) const` | n | no |
| `std::string_view` | `find_last_not_of` | `<string_view>` | `std::size_t find_last_not_of(string_view set, std::size_t pos = npos) const` | n | no |
| `std::string_view` | `compare` | `<string_view>` | `int compare(string_view other) const` | n | no |
| `std::string_view` | `remove_prefix` | `<string_view>` | `void remove_prefix(std::size_t n)` | 1 | no |
| `std::string_view` | `remove_suffix` | `<string_view>` | `void remove_suffix(std::size_t n)` | 1 | no |
| `std::string_view` | `begin` | `<string_view>` | `const_iterator begin() const` | 1 | no |
| `std::string_view` | `end` | `<string_view>` | `const_iterator end() const` | 1 | no |
| `std::string_view` | `operator==` | `<string_view>` | `bool operator==(string_view other) const` | n | yes |
| `std::pair<A,B>` | `ctor` | `<utility>` | `pair(const A& a, const B& b)` | 1 | yes |
| `std::pair<A,B>` | `first` | `<utility>` | `A first` | 1 | yes |
| `std::pair<A,B>` | `second` | `<utility>` | `B second` | 1 | yes |
| `std::pair<A,B>` | `swap` | `<utility>` | `void swap(pair& other)` | 1 | no |
| `std::pair<A,B>` | `operator==` | `<utility>` | `bool operator==(const pair& other) const` | 1 | yes |
| `std::pair<A,B>` | `operator<` | `<utility>` | `bool operator<(const pair& other) const` | 1 | yes |
| `std::tuple<T...>` | `ctor` | `<tuple>` | `tuple(const T&... args)` | 1 | no |
| `std::tuple<T...>` | `operator==` | `<tuple>` | `bool operator==(const tuple& other) const` | 1 | no |
| `std::tuple<T...>` | `operator<` | `<tuple>` | `bool operator<(const tuple& other) const` | 1 | no |
| `std::optional<T>` | `has_value` | `<optional>` | `bool has_value() const` | 1 | yes |
| `std::optional<T>` | `value` | `<optional>` | `T& value()` | 1 | yes |
| `std::optional<T>` | `value_or` | `<optional>` | `T value_or(const U& fallback) const` | 1 | yes |
| `std::optional<T>` | `operator*` | `<optional>` | `T& operator*()` | 1 | yes |
| `std::optional<T>` | `operator->` | `<optional>` | `T* operator->()` | 1 | no |
| `std::optional<T>` | `reset` | `<optional>` | `void reset()` | 1 | yes |
| `std::optional<T>` | `operator=` | `<optional>` | `optional& operator=(const T& x)` | 1 | yes |
| `iterator` | `operator*` | (container's) | `T& operator*() const` | 1 | no |
| `iterator` | `operator->` | (container's) | `T* operator->() const` | 1 | no |
| `iterator` | `operator++` | (container's) | `iterator& operator++()` | 1 | no |
| `iterator` | `operator--` | (container's) | `iterator& operator--()` | 1 | no |
| `iterator` | `operator+` | (container's) | `iterator operator+(std::ptrdiff_t n) const` | 1 | no |
| `iterator` | `operator-` | (container's) | `std::ptrdiff_t operator-(iterator other) const` | 1 | no |
| `iterator` | `operator==` | (container's) | `bool operator==(iterator other) const` | 1 | no |
| `iterator` | `operator!=` | (container's) | `bool operator!=(iterator other) const` | 1 | no |
| `iterator` | `operator<` | (container's) | `bool operator<(iterator other) const` | 1 | no |
| `std::function<R(A...)>` | `operator()` | `<functional>` | `R operator()(A... args) const` | 1 | no |
| `std::unordered_map<K,V>` | `size` | `<unordered_map>` | `std::size_t size() const` | 1 | yes |
| `std::unordered_map<K,V>` | `empty` | `<unordered_map>` | `bool empty() const` | 1 | yes |
| `std::unordered_map<K,V>` | `clear` | `<unordered_map>` | `void clear()` | n | yes |
| `std::unordered_map<K,V>` | `count` | `<unordered_map>` | `std::size_t count(const K& key) const` | 1 average | yes |
| `std::unordered_map<K,V>` | `find` | `<unordered_map>` | `iterator find(const K& key)` | 1 average | yes |
| `std::unordered_map<K,V>` | `insert` | `<unordered_map>` | `std::pair<iterator, bool> insert(const std::pair<K, V>& kv)` | 1 average | yes |
| `std::unordered_map<K,V>` | `insert` | `<unordered_map>` | `void insert(InputIt first, InputIt last)` | n | no |
| `std::unordered_map<K,V>` | `try_emplace` | `<unordered_map>` | `std::pair<iterator, bool> try_emplace(const K& key, Args... args)` | log n | no |
| `std::unordered_map<K,V>` | `insert_or_assign` | `<unordered_map>` | `std::pair<iterator, bool> insert_or_assign(const K& key, const V& v)` | log n | no |
| `std::unordered_map<K,V>` | `emplace` | `<unordered_map>` | `std::pair<iterator, bool> emplace(Args... args)` | 1 average | yes |
| `std::unordered_map<K,V>` | `erase` | `<unordered_map>` | `std::size_t erase(const K& key)` | 1 average | yes |
| `std::unordered_map<K,V>` | `erase` | `<unordered_map>` | `iterator erase(iterator pos)` | 1 average | no |
| `std::unordered_map<K,V>` | `erase` | `<unordered_map>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::unordered_map<K,V>` | `operator[]` | `<unordered_map>` | `V& operator[](const K& key)` | 1 average | yes |
| `std::unordered_map<K,V>` | `at` | `<unordered_map>` | `V& at(const K& key)` | 1 average | yes |
| `std::unordered_map<K,V>` | `begin` | `<unordered_map>` | `iterator begin()` | 1 | no |
| `std::unordered_map<K,V>` | `end` | `<unordered_map>` | `iterator end()` | 1 | no |
| `std::map<K,V>` | `size` | `<map>` | `std::size_t size() const` | 1 | yes |
| `std::map<K,V>` | `empty` | `<map>` | `bool empty() const` | 1 | yes |
| `std::map<K,V>` | `clear` | `<map>` | `void clear()` | n | yes |
| `std::map<K,V>` | `count` | `<map>` | `std::size_t count(const K& key) const` | log n | yes |
| `std::map<K,V>` | `find` | `<map>` | `iterator find(const K& key)` | log n | yes |
| `std::map<K,V>` | `insert` | `<map>` | `std::pair<iterator, bool> insert(const std::pair<K, V>& kv)` | log n | yes |
| `std::map<K,V>` | `insert` | `<map>` | `void insert(InputIt first, InputIt last)` | n | no |
| `std::map<K,V>` | `try_emplace` | `<map>` | `std::pair<iterator, bool> try_emplace(const K& key, Args... args)` | log n | no |
| `std::map<K,V>` | `insert_or_assign` | `<map>` | `std::pair<iterator, bool> insert_or_assign(const K& key, const V& v)` | log n | no |
| `std::map<K,V>` | `emplace` | `<map>` | `std::pair<iterator, bool> emplace(Args... args)` | log n | yes |
| `std::map<K,V>` | `erase` | `<map>` | `std::size_t erase(const K& key)` | log n | yes |
| `std::map<K,V>` | `erase` | `<map>` | `iterator erase(iterator pos)` | 1 amortised | no |
| `std::map<K,V>` | `erase` | `<map>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::map<K,V>` | `operator[]` | `<map>` | `V& operator[](const K& key)` | log n | yes |
| `std::map<K,V>` | `at` | `<map>` | `V& at(const K& key)` | log n | yes |
| `std::map<K,V>` | `lower_bound` | `<map>` | `iterator lower_bound(const K& key)` | log n | yes |
| `std::map<K,V>` | `upper_bound` | `<map>` | `iterator upper_bound(const K& key)` | log n | yes |
| `std::map<K,V>` | `equal_range` | `<map>` | `std::pair<iterator, iterator> equal_range(const K& key)` | log n | no |
| `std::map<K,V>` | `begin` | `<map>` | `iterator begin()` | 1 | no |
| `std::map<K,V>` | `end` | `<map>` | `iterator end()` | 1 | no |
| `std::map<K,V>` | `rbegin` | `<map>` | `reverse_iterator rbegin()` | 1 | no |
| `std::map<K,V>` | `rend` | `<map>` | `reverse_iterator rend()` | 1 | no |
| `std::unordered_set<T>` | `size` | `<unordered_set>` | `std::size_t size() const` | 1 | yes |
| `std::unordered_set<T>` | `empty` | `<unordered_set>` | `bool empty() const` | 1 | yes |
| `std::unordered_set<T>` | `clear` | `<unordered_set>` | `void clear()` | n | yes |
| `std::unordered_set<T>` | `count` | `<unordered_set>` | `std::size_t count(const T& key) const` | 1 average | yes |
| `std::unordered_set<T>` | `find` | `<unordered_set>` | `iterator find(const T& key)` | 1 average | yes |
| `std::unordered_set<T>` | `insert` | `<unordered_set>` | `std::pair<iterator, bool> insert(const T& key)` | 1 average | yes |
| `std::unordered_set<T>` | `insert` | `<unordered_set>` | `void insert(InputIt first, InputIt last)` | n | no |
| `std::unordered_set<T>` | `insert` | `<unordered_set>` | `void insert(std::initializer_list<T> il)` | n | no |
| `std::unordered_set<T>` | `emplace` | `<unordered_set>` | `std::pair<iterator, bool> emplace(Args... args)` | 1 average | yes |
| `std::unordered_set<T>` | `erase` | `<unordered_set>` | `std::size_t erase(const T& key)` | 1 average | yes |
| `std::unordered_set<T>` | `erase` | `<unordered_set>` | `iterator erase(iterator pos)` | 1 average | no |
| `std::unordered_set<T>` | `erase` | `<unordered_set>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::unordered_set<T>` | `begin` | `<unordered_set>` | `iterator begin()` | 1 | no |
| `std::unordered_set<T>` | `end` | `<unordered_set>` | `iterator end()` | 1 | no |
| `std::set<T>` | `size` | `<set>` | `std::size_t size() const` | 1 | yes |
| `std::set<T>` | `empty` | `<set>` | `bool empty() const` | 1 | yes |
| `std::set<T>` | `clear` | `<set>` | `void clear()` | n | yes |
| `std::set<T>` | `count` | `<set>` | `std::size_t count(const T& key) const` | log n | yes |
| `std::set<T>` | `find` | `<set>` | `iterator find(const T& key)` | log n | yes |
| `std::set<T>` | `insert` | `<set>` | `std::pair<iterator, bool> insert(const T& key)` | log n | yes |
| `std::set<T>` | `insert` | `<set>` | `void insert(InputIt first, InputIt last)` | n | no |
| `std::set<T>` | `insert` | `<set>` | `void insert(std::initializer_list<T> il)` | n | no |
| `std::set<T>` | `emplace` | `<set>` | `std::pair<iterator, bool> emplace(Args... args)` | log n | yes |
| `std::set<T>` | `erase` | `<set>` | `std::size_t erase(const T& key)` | log n | yes |
| `std::set<T>` | `erase` | `<set>` | `iterator erase(iterator pos)` | 1 amortised | no |
| `std::set<T>` | `erase` | `<set>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::set<T>` | `lower_bound` | `<set>` | `iterator lower_bound(const T& key)` | log n | yes |
| `std::set<T>` | `upper_bound` | `<set>` | `iterator upper_bound(const T& key)` | log n | yes |
| `std::set<T>` | `equal_range` | `<set>` | `std::pair<iterator, iterator> equal_range(const T& key)` | log n | no |
| `std::set<T>` | `begin` | `<set>` | `iterator begin()` | 1 | no |
| `std::set<T>` | `end` | `<set>` | `iterator end()` | 1 | no |
| `std::set<T>` | `rbegin` | `<set>` | `reverse_iterator rbegin()` | 1 | no |
| `std::set<T>` | `rend` | `<set>` | `reverse_iterator rend()` | 1 | no |
| `std::stack<T>` | `push` | `<stack>` | `void push(const T& x)` | 1 amortised | yes |
| `std::stack<T>` | `emplace` | `<stack>` | `void emplace(Args... args)` | 1 amortised | yes |
| `std::stack<T>` | `pop` | `<stack>` | `void pop()` | 1 | yes |
| `std::stack<T>` | `top` | `<stack>` | `T& top()` | 1 | yes |
| `std::stack<T>` | `size` | `<stack>` | `std::size_t size() const` | 1 | yes |
| `std::stack<T>` | `empty` | `<stack>` | `bool empty() const` | 1 | yes |
| `std::queue<T>` | `push` | `<queue>` | `void push(const T& x)` | 1 amortised | yes |
| `std::queue<T>` | `emplace` | `<queue>` | `void emplace(Args... args)` | 1 amortised | yes |
| `std::queue<T>` | `pop` | `<queue>` | `void pop()` | 1 | yes |
| `std::queue<T>` | `front` | `<queue>` | `T& front()` | 1 | yes |
| `std::queue<T>` | `back` | `<queue>` | `T& back()` | 1 | yes |
| `std::queue<T>` | `size` | `<queue>` | `std::size_t size() const` | 1 | yes |
| `std::queue<T>` | `empty` | `<queue>` | `bool empty() const` | 1 | yes |
| `std::priority_queue<T>` | `ctor` | `<queue>` | `priority_queue(const Cmp& cmp)` | 1 | no |
| `std::priority_queue<T>` | `push` | `<queue>` | `void push(const T& x)` | log n | yes |
| `std::priority_queue<T>` | `emplace` | `<queue>` | `void emplace(Args... args)` | log n | yes |
| `std::priority_queue<T>` | `pop` | `<queue>` | `void pop()` | log n | yes |
| `std::priority_queue<T>` | `top` | `<queue>` | `const T& top() const` | 1 | yes |
| `std::priority_queue<T>` | `size` | `<queue>` | `std::size_t size() const` | 1 | yes |
| `std::priority_queue<T>` | `empty` | `<queue>` | `bool empty() const` | 1 | yes |
| `std::deque<T>` | `push_front` | `<deque>` | `void push_front(const T& x)` | 1 | yes |
| `std::deque<T>` | `pop_front` | `<deque>` | `void pop_front()` | 1 | yes |
| `std::deque<T>` | `push_back` | `<deque>` | `void push_back(const T& x)` | 1 | yes |
| `std::deque<T>` | `pop_back` | `<deque>` | `void pop_back()` | 1 | yes |
| `std::deque<T>` | `emplace_back` | `<deque>` | `T& emplace_back(Args... args)` | 1 | yes |
| `std::deque<T>` | `emplace_front` | `<deque>` | `T& emplace_front(Args... args)` | 1 | yes |
| `std::deque<T>` | `front` | `<deque>` | `T& front()` | 1 | yes |
| `std::deque<T>` | `back` | `<deque>` | `T& back()` | 1 | yes |
| `std::deque<T>` | `operator[]` | `<deque>` | `T& operator[](std::size_t i)` | 1 | yes |
| `std::deque<T>` | `at` | `<deque>` | `T& at(std::size_t i)` | 1 | yes |
| `std::deque<T>` | `size` | `<deque>` | `std::size_t size() const` | 1 | yes |
| `std::deque<T>` | `empty` | `<deque>` | `bool empty() const` | 1 | yes |
| `std::deque<T>` | `clear` | `<deque>` | `void clear()` | n | yes |
| `std::deque<T>` | `insert` | `<deque>` | `iterator insert(iterator pos, const T& x)` | n | yes |
| `std::deque<T>` | `erase` | `<deque>` | `iterator erase(iterator pos)` | n | yes |
| `std::deque<T>` | `insert` | `<deque>` | `iterator insert(iterator pos, std::size_t n, const T& x)` | n | no |
| `std::deque<T>` | `insert` | `<deque>` | `iterator insert(iterator pos, InputIt first, InputIt last)` | n | no |
| `std::deque<T>` | `insert` | `<deque>` | `iterator insert(iterator pos, std::initializer_list<T> il)` | n | no |
| `std::deque<T>` | `emplace` | `<deque>` | `iterator emplace(iterator pos, Args... args)` | n | no |
| `std::deque<T>` | `assign` | `<deque>` | `void assign(std::size_t n, const T& x)` | n | no |
| `std::deque<T>` | `assign` | `<deque>` | `void assign(InputIt first, InputIt last)` | n | no |
| `std::deque<T>` | `assign` | `<deque>` | `void assign(std::initializer_list<T> il)` | n | no |
| `std::deque<T>` | `erase` | `<deque>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::deque<T>` | `resize` | `<deque>` | `void resize(std::size_t n)` | n | no |
| `std::deque<T>` | `resize` | `<deque>` | `void resize(std::size_t n, const T& x)` | n | no |
| `std::deque<T>` | `swap` | `<deque>` | `void swap(deque& other)` | 1 | no |
| `std::deque<T>` | `rbegin` | `<deque>` | `reverse_iterator rbegin()` | 1 | no |
| `std::deque<T>` | `rend` | `<deque>` | `reverse_iterator rend()` | 1 | no |
| `std::deque<T>` | `cbegin` | `<deque>` | `const_iterator cbegin() const` | 1 | no |
| `std::deque<T>` | `cend` | `<deque>` | `const_iterator cend() const` | 1 | no |
| `std::deque<T>` | `operator==` | `<deque>` | `bool operator==(const deque& other) const` | n | no |
| `std::deque<T>` | `operator<` | `<deque>` | `bool operator<(const deque& other) const` | n | no |
| `std::deque<T>` | `begin` | `<deque>` | `iterator begin()` | 1 | no |
| `std::deque<T>` | `end` | `<deque>` | `iterator end()` | 1 | no |
| `std::list<T>` | `push_front` | `<list>` | `void push_front(const T& x)` | 1 | yes |
| `std::list<T>` | `pop_front` | `<list>` | `void pop_front()` | 1 | yes |
| `std::list<T>` | `push_back` | `<list>` | `void push_back(const T& x)` | 1 | yes |
| `std::list<T>` | `pop_back` | `<list>` | `void pop_back()` | 1 | yes |
| `std::list<T>` | `emplace_back` | `<list>` | `T& emplace_back(Args... args)` | 1 | yes |
| `std::list<T>` | `emplace_front` | `<list>` | `T& emplace_front(Args... args)` | 1 | yes |
| `std::list<T>` | `front` | `<list>` | `T& front()` | 1 | yes |
| `std::list<T>` | `back` | `<list>` | `T& back()` | 1 | yes |
| `std::list<T>` | `size` | `<list>` | `std::size_t size() const` | 1 | yes |
| `std::list<T>` | `empty` | `<list>` | `bool empty() const` | 1 | yes |
| `std::list<T>` | `clear` | `<list>` | `void clear()` | n | yes |
| `std::list<T>` | `insert` | `<list>` | `iterator insert(iterator pos, const T& x)` | 1 | yes |
| `std::list<T>` | `erase` | `<list>` | `iterator erase(iterator pos)` | 1 | yes |
| `std::list<T>` | `insert` | `<list>` | `iterator insert(iterator pos, std::size_t n, const T& x)` | n | no |
| `std::list<T>` | `insert` | `<list>` | `iterator insert(iterator pos, InputIt first, InputIt last)` | n | no |
| `std::list<T>` | `insert` | `<list>` | `iterator insert(iterator pos, std::initializer_list<T> il)` | n | no |
| `std::list<T>` | `emplace` | `<list>` | `iterator emplace(iterator pos, Args... args)` | n | no |
| `std::list<T>` | `assign` | `<list>` | `void assign(std::size_t n, const T& x)` | n | no |
| `std::list<T>` | `assign` | `<list>` | `void assign(InputIt first, InputIt last)` | n | no |
| `std::list<T>` | `assign` | `<list>` | `void assign(std::initializer_list<T> il)` | n | no |
| `std::list<T>` | `erase` | `<list>` | `iterator erase(iterator first, iterator last)` | n | no |
| `std::list<T>` | `reverse` | `<list>` | `void reverse()` | n | no |
| `std::list<T>` | `sort` | `<list>` | `void sort()` | n log n | no |
| `std::list<T>` | `sort` | `<list>` | `void sort(Compare cmp)` | n log n | no |
| `std::list<T>` | `unique` | `<list>` | `std::size_t unique()` | n | no |
| `std::list<T>` | `remove` | `<list>` | `std::size_t remove(const T& x)` | n | no |
| `std::list<T>` | `remove_if` | `<list>` | `std::size_t remove_if(Pred pred)` | n | no |
| `std::list<T>` | `resize` | `<list>` | `void resize(std::size_t n)` | n | no |
| `std::list<T>` | `resize` | `<list>` | `void resize(std::size_t n, const T& x)` | n | no |
| `std::list<T>` | `swap` | `<list>` | `void swap(list& other)` | 1 | no |
| `std::list<T>` | `cbegin` | `<list>` | `const_iterator cbegin() const` | 1 | no |
| `std::list<T>` | `cend` | `<list>` | `const_iterator cend() const` | 1 | no |
| `std::list<T>` | `operator==` | `<list>` | `bool operator==(const list& other) const` | n | no |
| `std::list<T>` | `operator<` | `<list>` | `bool operator<(const list& other) const` | n | no |
| `std::list<T>` | `begin` | `<list>` | `iterator begin()` | 1 | no |
| `std::list<T>` | `end` | `<list>` | `iterator end()` | 1 | no |
| `std::list<T>` | `rbegin` | `<list>` | `reverse_iterator rbegin()` | 1 | no |
| `std::list<T>` | `rend` | `<list>` | `reverse_iterator rend()` | 1 | no |
| `free` | `std::sort` | `<algorithm>` | `void sort(RandomIt first, RandomIt last)` | n log n | yes |
| `free` | `std::sort` | `<algorithm>` | `void sort(RandomIt first, RandomIt last, Compare cmp)` | n log n | no |
| `free` | `std::stable_sort` | `<algorithm>` | `void stable_sort(RandomIt first, RandomIt last)` | n log n | yes |
| `free` | `std::stable_sort` | `<algorithm>` | `void stable_sort(RandomIt first, RandomIt last, Compare cmp)` | n log n | no |
| `free` | `std::reverse` | `<algorithm>` | `void reverse(BidirIt first, BidirIt last)` | n | yes |
| `free` | `std::min` | `<algorithm>` | `const T& min(const T& a, const T& b)` | 1 | yes |
| `free` | `std::max` | `<algorithm>` | `const T& max(const T& a, const T& b)` | 1 | yes |
| `free` | `std::min` | `<algorithm>` | `T min(std::initializer_list<T> il)` | n | no |
| `free` | `std::max` | `<algorithm>` | `T max(std::initializer_list<T> il)` | n | no |
| `free` | `std::swap` | `<utility>` | `void swap(T& a, T& b)` | 1 | yes |
| `free` | `std::find` | `<algorithm>` | `InputIt find(InputIt first, InputIt last, const T& x)` | n | yes |
| `free` | `std::find_if` | `<algorithm>` | `InputIt find_if(InputIt first, InputIt last, Pred pred)` | n | no |
| `free` | `std::count` | `<algorithm>` | `difference_type count(InputIt first, InputIt last, const T& x)` | n | yes |
| `free` | `std::count_if` | `<algorithm>` | `difference_type count_if(InputIt first, InputIt last, Pred pred)` | n | no |
| `free` | `std::binary_search` | `<algorithm>` | `bool binary_search(ForwardIt first, ForwardIt last, const T& x)` | log n | yes |
| `free` | `std::lower_bound` | `<algorithm>` | `ForwardIt lower_bound(ForwardIt first, ForwardIt last, const T& x)` | log n | yes |
| `free` | `std::upper_bound` | `<algorithm>` | `ForwardIt upper_bound(ForwardIt first, ForwardIt last, const T& x)` | log n | yes |
| `free` | `std::unique` | `<algorithm>` | `ForwardIt unique(ForwardIt first, ForwardIt last)` | n | yes |
| `free` | `std::min_element` | `<algorithm>` | `ForwardIt min_element(ForwardIt first, ForwardIt last)` | n | yes |
| `free` | `std::max_element` | `<algorithm>` | `ForwardIt max_element(ForwardIt first, ForwardIt last)` | n | yes |
| `free` | `std::fill` | `<algorithm>` | `void fill(ForwardIt first, ForwardIt last, const T& x)` | n | yes |
| `free` | `std::next_permutation` | `<algorithm>` | `bool next_permutation(BidirIt first, BidirIt last)` | n | yes |
| `free` | `std::prev_permutation` | `<algorithm>` | `bool prev_permutation(BidirIt first, BidirIt last)` | n | yes |
| `free` | `std::any_of` | `<algorithm>` | `bool any_of(InputIt first, InputIt last, Pred pred)` | n | no |
| `free` | `std::all_of` | `<algorithm>` | `bool all_of(InputIt first, InputIt last, Pred pred)` | n | no |
| `free` | `std::none_of` | `<algorithm>` | `bool none_of(InputIt first, InputIt last, Pred pred)` | n | no |
| `free` | `std::for_each` | `<algorithm>` | `void for_each(InputIt first, InputIt last, Fn fn)` | n | no |
| `free` | `std::remove` | `<algorithm>` | `ForwardIt remove(ForwardIt first, ForwardIt last, const T& x)` | n | yes |
| `free` | `std::remove_if` | `<algorithm>` | `ForwardIt remove_if(ForwardIt first, ForwardIt last, Pred pred)` | n | no |
| `free` | `std::rotate` | `<algorithm>` | `ForwardIt rotate(ForwardIt first, ForwardIt middle, ForwardIt last)` | n | no |
| `free` | `std::equal` | `<algorithm>` | `bool equal(InputIt1 first1, InputIt1 last1, InputIt2 first2)` | n | no |
| `free` | `std::copy` | `<algorithm>` | `OutputIt copy(InputIt first, InputIt last, OutputIt out)` | n | no |
| `free` | `std::lexicographical_compare` | `<algorithm>` | `bool lexicographical_compare(InputIt1 f1, InputIt1 l1, InputIt2 f2, InputIt2 l2)` | n | no |
| `free` | `std::accumulate` | `<numeric>` | `T accumulate(InputIt first, InputIt last, T init)` | n | yes |
| `free` | `std::accumulate` | `<numeric>` | `T accumulate(InputIt first, InputIt last, T init, BinaryOp op)` | n | no |
| `free` | `std::iota` | `<numeric>` | `void iota(ForwardIt first, ForwardIt last, T value)` | n | yes |
| `free` | `std::partial_sum` | `<numeric>` | `OutputIt partial_sum(InputIt first, InputIt last, OutputIt out)` | n | no |
| `free` | `std::partial_sum` | `<numeric>` | `OutputIt partial_sum(InputIt first, InputIt last, OutputIt out, BinaryOp op)` | n | no |
| `free` | `std::distance` | `<iterator>` | `difference_type distance(InputIt first, InputIt last)` | 1 | no |
| `free` | `std::advance` | `<iterator>` | `void advance(InputIt& it, difference_type n)` | 1 | no |
| `free` | `std::next` | `<iterator>` | `ForwardIt next(ForwardIt it, difference_type n = 1)` | 1 | no |
| `free` | `std::prev` | `<iterator>` | `BidirIt prev(BidirIt it, difference_type n = 1)` | 1 | no |
| `free` | `std::make_pair` | `<utility>` | `pair<A, B> make_pair(A a, B b)` | 1 | yes |
| `free` | `std::get` | `<utility>` | `T& get<I>(pair<A, B>& p)` | 1 | no |
| `free` | `std::get` | `<tuple>` | `T& get<I>(tuple<T...>& t)` | 1 | no |
| `free` | `std::make_tuple` | `<tuple>` | `tuple<T...> make_tuple(T... args)` | 1 | no |
| `free` | `std::tie` | `<tuple>` | `tuple<T&...> tie(T&... args)` | 0 | no |
| `free` | `std::nullopt` | `<optional>` | `constexpr nullopt_t nullopt` | 0 | yes |
| `free` | `std::abs` | `<cstdlib>` | `int abs(int x)` | 1 | yes |
| `free` | `std::abs` | `<cstdlib>` | `long long abs(long long x)` | 1 | yes |
| `free` | `std::to_string` | `<string>` | `std::string to_string(int x)` | n | yes |
| `free` | `std::to_string` | `<string>` | `std::string to_string(long long x)` | n | yes |
| `free` | `std::stoi` | `<string>` | `int stoi(const std::string& s)` | n | yes |
| `free` | `std::stoll` | `<string>` | `long long stoll(const std::string& s)` | n | yes |
| `free` | `std::isdigit` | `<cctype>` | `int isdigit(int c)` | 1 | yes |
| `free` | `std::isalpha` | `<cctype>` | `int isalpha(int c)` | 1 | yes |
| `free` | `std::isalnum` | `<cctype>` | `int isalnum(int c)` | 1 | yes |
| `free` | `std::isspace` | `<cctype>` | `int isspace(int c)` | 1 | yes |
| `free` | `std::isupper` | `<cctype>` | `int isupper(int c)` | 1 | yes |
| `free` | `std::islower` | `<cctype>` | `int islower(int c)` | 1 | yes |
| `free` | `std::ispunct` | `<cctype>` | `int ispunct(int c)` | 1 | yes |
| `free` | `std::tolower` | `<cctype>` | `int tolower(int c)` | 1 | yes |
| `free` | `std::toupper` | `<cctype>` | `int toupper(int c)` | 1 | yes |
| `free` | `std::numeric_limits<T>::max` | `<limits>` | `constexpr T max()` | 0 | yes |
| `free` | `std::numeric_limits<T>::min` | `<limits>` | `constexpr T min()` | 0 | yes |
| `free` | `std::numeric_limits<T>::lowest` | `<limits>` | `constexpr T lowest()` | 0 | yes |
| `free` | `INT_MAX` | `<climits>` | `int INT_MAX` | 0 | yes |
| `free` | `INT_MIN` | `<climits>` | `int INT_MIN` | 0 | yes |
| `free` | `UINT_MAX` | `<climits>` | `unsigned UINT_MAX` | 0 | yes |
| `free` | `LLONG_MAX` | `<climits>` | `long long LLONG_MAX` | 0 | yes |
| `free` | `LLONG_MIN` | `<climits>` | `long long LLONG_MIN` | 0 | yes |
| `free` | `ULLONG_MAX` | `<climits>` | `unsigned long long ULLONG_MAX` | 0 | yes |
| `free` | `CHAR_MAX` | `<climits>` | `int CHAR_MAX` | 0 | yes |
| `free` | `CHAR_MIN` | `<climits>` | `int CHAR_MIN` | 0 | yes |
| `free` | `std::function` | `<functional>` | `template<class R, class... A> class function<R(A...)>` | 0 | no |
| `free` | `std::greater` | `<functional>` | `template<class T> struct greater` | 0 | yes |
| `free` | `std::less` | `<functional>` | `template<class T> struct less` | 0 | yes |
<!-- library-table:end -->
