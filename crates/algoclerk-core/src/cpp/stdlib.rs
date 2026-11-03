// SPDX-License-Identifier: Apache-2.0
//! Library metadata table: the single source of truth for the supported part of the C++ standard library.
//!
//! The checker types library calls from it, the interpreter implements exactly these names, the card editor
//! offers the rows with `card == true`, autocomplete lists [`members_of`] for the receiver's static type, and
//! the "Library" section of `docs/CPP_SUBSET.md` is [`markdown_table`].
//!
//! Conventions:
//! * `recv` is a receiver pattern: `std::vector<T>`, `std::string`, `std::map<K,V>`, `iterator` (any
//!   iterator, `header` is empty because the container's header is already required), or `free` for free
//!   functions, constants and macros (`std::sort`, `INT_MAX`, `std::nullopt`).
//! * `name` is the member name; constructors are `ctor`, operators are spelled `operator[]`, `operator==`,
//!   data members are listed too (`first`, `second`, `npos`), and `std::numeric_limits<T>::max` is a free row.
//! * `header` is the strict standard header declaring the name (valid with both libstdc++ and libc++).
//! * `cost` is the abstract cost model class (`n` is the number of elements touched).
//! * Every `example` is a complete statement list; it is placed in a function body and compiled by tests,
//!   with the member's header plus the headers of the types the example spells out ([`example_program`]).
//! * Rows with cost `0` are free in the cost model (`reserve`, `shrink_to_fit`, `data`, `std::tie`).

use super::ast::Ty;

/// One supported library member or free function.
#[derive(Clone, Copy, Debug)]
pub struct Member {
    /// Receiver type pattern such as `std::vector<T>`, or `free`.
    pub recv: &'static str,
    /// Name.
    pub name: &'static str,
    /// Header.
    pub header: &'static str,
    /// Display signature.
    pub sig: &'static str,
    /// Cost in the abstract model.
    pub cost: &'static str,
    /// The card editor can express it.
    pub card: bool,
    /// Example statements.
    pub example: &'static str,
}

/// Build a table row (kept short so the table stays one row per line).
const fn m(
    recv: &'static str,
    name: &'static str,
    header: &'static str,
    sig: &'static str,
    cost: &'static str,
    card: bool,
    example: &'static str,
) -> Member {
    Member {
        recv,
        name,
        header,
        sig,
        cost,
        card,
        example,
    }
}

const V: &str = "std::vector<T>";
const S: &str = "std::string";
const SV: &str = "std::string_view";
const AR: &str = "std::array<T,N>";
const PR: &str = "std::pair<A,B>";
const TU: &str = "std::tuple<T...>";
const OP: &str = "std::optional<T>";
const UM: &str = "std::unordered_map<K,V>";
const MP: &str = "std::map<K,V>";
const US: &str = "std::unordered_set<T>";
const ST: &str = "std::set<T>";
const SK: &str = "std::stack<T>";
const QU: &str = "std::queue<T>";
const DQ: &str = "std::deque<T>";
const LS: &str = "std::list<T>";
const PQ: &str = "std::priority_queue<T>";
const FN: &str = "std::function<R(A...)>";
const IT: &str = "iterator";
const FR: &str = "free";

/// Every supported member.
#[rustfmt::skip]
pub static MEMBERS: &[Member] = &[
    // ---- std::vector ----
    m(V, "ctor", "<vector>", "vector(std::size_t n)", "n", false, "std::vector<int> v(5);"),
    m(V, "ctor", "<vector>", "vector(std::size_t n, const T& x)", "n", true, "std::vector<int> v(5, 0);"),
    m(V, "ctor", "<vector>", "vector(InputIt first, InputIt last)", "n", false, "std::vector<int> a{1, 2}; std::vector<int> v(a.begin(), a.end());"),
    m(V, "ctor", "<vector>", "vector(std::initializer_list<T> il)", "n", true, "std::vector<int> v{1, 2, 3};"),
    m(V, "ctor", "<vector>", "vector(const vector& other)", "n", true, "std::vector<int> a{1}; std::vector<int> v(a);"),
    m(V, "operator=", "<vector>", "vector& operator=(const vector& other)", "n", true, "std::vector<int> a{1}; std::vector<int> v; v = a;"),
    m(V, "size", "<vector>", "std::size_t size() const", "1", true, "std::vector<int> v{1, 2}; std::size_t n = v.size();"),
    m(V, "empty", "<vector>", "bool empty() const", "1", true, "std::vector<int> v; bool e = v.empty();"),
    m(V, "push_back", "<vector>", "void push_back(const T& x)", "1 amortised", true, "std::vector<int> v; v.push_back(1);"),
    m(V, "emplace_back", "<vector>", "T& emplace_back(Args... args)", "1 amortised", true, "std::vector<std::pair<int, int>> v; v.emplace_back(1, 2);"),
    m(V, "pop_back", "<vector>", "void pop_back()", "1", true, "std::vector<int> v{1}; v.pop_back();"),
    m(V, "back", "<vector>", "T& back()", "1", true, "std::vector<int> v{1, 2}; int x = v.back();"),
    m(V, "front", "<vector>", "T& front()", "1", true, "std::vector<int> v{1, 2}; int x = v.front();"),
    m(V, "clear", "<vector>", "void clear()", "n", true, "std::vector<int> v{1, 2}; v.clear();"),
    m(V, "operator[]", "<vector>", "T& operator[](std::size_t i)", "1", true, "std::vector<int> v{1, 2}; v[0] = v[1];"),
    m(V, "at", "<vector>", "T& at(std::size_t i)", "1", true, "std::vector<int> v{1, 2}; int x = v.at(1);"),
    m(V, "data", "<vector>", "T* data()", "0", false, "std::vector<int> v{1, 2}; int* p = v.data(); (void)p;"),
    m(V, "reserve", "<vector>", "void reserve(std::size_t n)", "0", false, "std::vector<int> v; v.reserve(10);"),
    m(V, "shrink_to_fit", "<vector>", "void shrink_to_fit()", "0", false, "std::vector<int> v{1}; v.shrink_to_fit();"),
    m(V, "insert", "<vector>", "iterator insert(iterator pos, const T& x)", "n", true, "std::vector<int> v{1, 3}; v.insert(v.begin() + 1, 2);"),
    m(V, "insert", "<vector>", "iterator insert(iterator pos, std::size_t n, const T& x)", "n", false, "std::vector<int> v{1, 3}; v.insert(v.begin(), 2, 0);"),
    m(V, "insert", "<vector>", "iterator insert(iterator pos, InputIt first, InputIt last)", "n", false, "std::vector<int> a{4, 5}; std::vector<int> v{1}; v.insert(v.end(), a.begin(), a.end());"),
    m(V, "insert", "<vector>", "iterator insert(iterator pos, std::initializer_list<T> il)", "n", false, "std::vector<int> v{1}; v.insert(v.begin(), {2, 3});"),
    m(V, "emplace", "<vector>", "iterator emplace(iterator pos, Args... args)", "n", false, "std::vector<std::pair<int, int>> v; v.emplace(v.begin(), 1, 2);"),
    m(V, "erase", "<vector>", "iterator erase(iterator pos)", "n", true, "std::vector<int> v{1, 2, 3}; v.erase(v.begin());"),
    m(V, "erase", "<vector>", "iterator erase(iterator first, iterator last)", "n", false, "std::vector<int> v{1, 2, 3}; v.erase(v.begin(), v.begin() + 2);"),
    m(V, "resize", "<vector>", "void resize(std::size_t n)", "n", true, "std::vector<int> v; v.resize(5);"),
    m(V, "resize", "<vector>", "void resize(std::size_t n, const T& x)", "n", true, "std::vector<int> v; v.resize(5, -1);"),
    m(V, "assign", "<vector>", "void assign(std::size_t n, const T& x)", "n", true, "std::vector<int> v; v.assign(5, 0);"),
    m(V, "assign", "<vector>", "void assign(InputIt first, InputIt last)", "n", false, "std::vector<int> a{1, 2}; std::vector<int> v; v.assign(a.begin(), a.end());"),
    m(V, "assign", "<vector>", "void assign(std::initializer_list<T> il)", "n", false, "std::vector<int> v; v.assign({1, 2});"),
    m(V, "begin", "<vector>", "iterator begin()", "1", false, "std::vector<int> v{1}; auto it = v.begin(); (void)it;"),
    m(V, "end", "<vector>", "iterator end()", "1", false, "std::vector<int> v{1}; auto it = v.end(); (void)it;"),
    m(V, "rbegin", "<vector>", "reverse_iterator rbegin()", "1", false, "std::vector<int> v{1}; auto it = v.rbegin(); (void)it;"),
    m(V, "rend", "<vector>", "reverse_iterator rend()", "1", false, "std::vector<int> v{1}; auto it = v.rend(); (void)it;"),
    m(V, "cbegin", "<vector>", "const_iterator cbegin() const", "1", false, "std::vector<int> v{1}; auto it = v.cbegin(); (void)it;"),
    m(V, "cend", "<vector>", "const_iterator cend() const", "1", false, "std::vector<int> v{1}; auto it = v.cend(); (void)it;"),
    m(V, "swap", "<vector>", "void swap(vector& other)", "1", true, "std::vector<int> a{1}; std::vector<int> b{2}; a.swap(b);"),
    m(V, "operator==", "<vector>", "bool operator==(const vector& other) const", "n", true, "std::vector<int> a{1}; std::vector<int> b{1}; bool e = (a == b); (void)e;"),
    m(V, "operator!=", "<vector>", "bool operator!=(const vector& other) const", "n", true, "std::vector<int> a{1}; std::vector<int> b{2}; bool e = (a != b); (void)e;"),
    m(V, "operator<", "<vector>", "bool operator<(const vector& other) const", "n", true, "std::vector<int> a{1}; std::vector<int> b{2}; bool e = (a < b); (void)e;"),
    // ---- std::array ----
    m(AR, "size", "<array>", "std::size_t size() const", "1", true, "std::array<int, 3> a{}; std::size_t n = a.size();"),
    m(AR, "operator[]", "<array>", "T& operator[](std::size_t i)", "1", true, "std::array<int, 3> a{}; a[0] = a[1];"),
    m(AR, "at", "<array>", "T& at(std::size_t i)", "1", true, "std::array<int, 3> a{}; int x = a.at(2);"),
    m(AR, "front", "<array>", "T& front()", "1", true, "std::array<int, 3> a{}; int x = a.front();"),
    m(AR, "back", "<array>", "T& back()", "1", true, "std::array<int, 3> a{}; int x = a.back();"),
    m(AR, "fill", "<array>", "void fill(const T& x)", "n", true, "std::array<int, 3> a{}; a.fill(7);"),
    m(AR, "begin", "<array>", "iterator begin()", "1", false, "std::array<int, 3> a{}; auto it = a.begin(); (void)it;"),
    m(AR, "end", "<array>", "iterator end()", "1", false, "std::array<int, 3> a{}; auto it = a.end(); (void)it;"),
    // ---- std::string ----
    m(S, "ctor", "<string>", "string(const char* s)", "n", true, "std::string s(\"abc\");"),
    m(S, "ctor", "<string>", "string(std::size_t n, char c)", "n", true, "std::string s(3, 'a');"),
    m(S, "ctor", "<string>", "string(InputIt first, InputIt last)", "n", false, "std::string a = \"abc\"; std::string s(a.begin(), a.end());"),
    m(S, "ctor", "<string>", "string(const string& other)", "n", true, "std::string a = \"abc\"; std::string s(a);"),
    m(S, "operator=", "<string>", "string& operator=(const string& other)", "n", true, "std::string a = \"abc\"; std::string s; s = a;"),
    m(S, "npos", "<string>", "static const std::size_t npos", "0", false, "std::string s = \"abc\"; bool missing = (s.find('z') == std::string::npos); (void)missing;"),
    m(S, "size", "<string>", "std::size_t size() const", "1", true, "std::string s = \"abc\"; std::size_t n = s.size();"),
    m(S, "length", "<string>", "std::size_t length() const", "1", true, "std::string s = \"abc\"; std::size_t n = s.length();"),
    m(S, "empty", "<string>", "bool empty() const", "1", true, "std::string s; bool e = s.empty();"),
    m(S, "push_back", "<string>", "void push_back(char c)", "1 amortised", true, "std::string s; s.push_back('a');"),
    m(S, "pop_back", "<string>", "void pop_back()", "1", true, "std::string s = \"ab\"; s.pop_back();"),
    m(S, "back", "<string>", "char& back()", "1", true, "std::string s = \"ab\"; char c = s.back();"),
    m(S, "front", "<string>", "char& front()", "1", true, "std::string s = \"ab\"; char c = s.front();"),
    m(S, "clear", "<string>", "void clear()", "n", true, "std::string s = \"ab\"; s.clear();"),
    m(S, "operator[]", "<string>", "char& operator[](std::size_t i)", "1", true, "std::string s = \"ab\"; s[0] = s[1];"),
    m(S, "at", "<string>", "char& at(std::size_t i)", "1", true, "std::string s = \"ab\"; char c = s.at(1);"),
    m(S, "operator+", "<string>", "string operator+(const string& other) const", "n", true, "std::string a = \"ab\"; std::string b = a + \"cd\";"),
    m(S, "operator+=", "<string>", "string& operator+=(const string& other)", "n", true, "std::string s = \"ab\"; s += \"cd\"; s += 'e';"),
    m(S, "append", "<string>", "string& append(const string& other)", "n", true, "std::string s = \"ab\"; s.append(\"cd\");"),
    m(S, "append", "<string>", "string& append(const string& other, std::size_t pos, std::size_t len = npos)", "n", false, "std::string s = \"ab\"; std::string t = \"xyz\"; s.append(t, 1, 1); s.append(t, 1);"),
    m(S, "append", "<string>", "string& append(std::size_t n, char c)", "n", false, "std::string s = \"ab\"; s.append(3, 'x');"),
    m(S, "append", "<string>", "string& append(InputIt first, InputIt last)", "n", false, "std::string t = \"xyz\"; std::string s; s.append(t.begin(), t.end());"),
    m(S, "substr", "<string>", "string substr(std::size_t pos, std::size_t len = npos) const", "n", true, "std::string s = \"abcd\"; std::string t = s.substr(1, 2);"),
    m(S, "find", "<string>", "std::size_t find(const string& needle, std::size_t pos = 0) const", "n", true, "std::string s = \"abcd\"; std::size_t i = s.find(\"cd\");"),
    m(S, "find", "<string>", "std::size_t find(char c, std::size_t pos = 0) const", "n", true, "std::string s = \"abcd\"; std::size_t i = s.find('c', 1);"),
    m(S, "rfind", "<string>", "std::size_t rfind(const string& needle, std::size_t pos = npos) const", "n", true, "std::string s = \"abcd\"; std::size_t i = s.rfind('c'); std::size_t j = s.rfind(\"ab\", 0);"),
    m(S, "find_first_of", "<string>", "std::size_t find_first_of(const string& set, std::size_t pos = 0) const", "n", false, "std::string s = \"abcd\"; std::size_t i = s.find_first_of(\"dc\");"),
    m(S, "find_last_of", "<string>", "std::size_t find_last_of(const string& set, std::size_t pos = npos) const", "n", false, "std::string s = \"abcd\"; std::size_t i = s.find_last_of(\"ab\"); std::size_t j = s.find_last_of('a', 2);"),
    m(S, "find_first_not_of", "<string>", "std::size_t find_first_not_of(const string& set, std::size_t pos = 0) const", "n", false, "std::string s = \"abcd\"; std::size_t i = s.find_first_not_of(\"ab\"); std::size_t j = s.find_first_not_of('a', 1);"),
    m(S, "find_last_not_of", "<string>", "std::size_t find_last_not_of(const string& set, std::size_t pos = npos) const", "n", false, "std::string s = \"abcd\"; std::size_t i = s.find_last_not_of(\"cd\"); std::size_t j = s.find_last_not_of('d', 2);"),
    m(S, "compare", "<string>", "int compare(const string& other) const", "n", true, "std::string a = \"ab\"; std::string b = \"ac\"; int c = a.compare(b);"),
    m(S, "compare", "<string>", "int compare(std::size_t pos, std::size_t len, const string& other) const", "n", false, "std::string a = \"abc\"; std::string b = \"ab\"; int c = a.compare(0, 2, b);"),
    m(S, "compare", "<string>", "int compare(std::size_t pos, std::size_t len, const string& other, std::size_t pos2, std::size_t len2 = npos) const", "n", false, "std::string a = \"abc\"; std::string b = \"xbc\"; int c = a.compare(1, 2, b, 1, 2);"),
    m(S, "insert", "<string>", "string& insert(std::size_t pos, const string& s)", "n", true, "std::string s = \"ad\"; s.insert(1, \"bc\");"),
    m(S, "insert", "<string>", "string& insert(std::size_t pos, const string& s, std::size_t subpos, std::size_t sublen = npos)", "n", false, "std::string s = \"ad\"; std::string t = \"xbcx\"; s.insert(1, t, 1, 2);"),
    m(S, "insert", "<string>", "string& insert(std::size_t pos, std::size_t n, char c)", "n", false, "std::string s = \"ad\"; s.insert(1, 2, 'b');"),
    m(S, "insert", "<string>", "iterator insert(iterator pos, char c)", "n", false, "std::string s = \"ad\"; s.insert(s.begin() + 1, 'b');"),
    m(S, "insert", "<string>", "iterator insert(iterator pos, std::size_t n, char c)", "n", false, "std::string s = \"ad\"; s.insert(s.begin() + 1, 2, 'b');"),
    m(S, "insert", "<string>", "iterator insert(iterator pos, InputIt first, InputIt last)", "n", false, "std::string t = \"bc\"; std::string s = \"ad\"; s.insert(s.begin() + 1, t.begin(), t.end());"),
    m(S, "replace", "<string>", "string& replace(std::size_t pos, std::size_t len, const string& s)", "n", false, "std::string s = \"abcd\"; s.replace(1, 2, \"XYZ\");"),
    m(S, "replace", "<string>", "string& replace(std::size_t pos, std::size_t len, std::size_t n, char c)", "n", false, "std::string s = \"abcd\"; s.replace(1, 2, 3, 'x');"),
    m(S, "replace", "<string>", "string& replace(std::size_t pos, std::size_t len, const string& s, std::size_t subpos, std::size_t sublen = npos)", "n", false, "std::string s = \"abcd\"; std::string t = \"xyz\"; s.replace(1, 2, t, 1, 1);"),
    m(S, "replace", "<string>", "string& replace(iterator first, iterator last, const string& s)", "n", false, "std::string s = \"abcd\"; s.replace(s.begin() + 1, s.begin() + 3, \"XYZ\");"),
    m(S, "erase", "<string>", "string& erase(std::size_t pos, std::size_t len = npos)", "n", true, "std::string s = \"abcd\"; s.erase(1, 2);"),
    m(S, "erase", "<string>", "iterator erase(iterator pos)", "n", false, "std::string s = \"abcd\"; s.erase(s.begin());"),
    m(S, "erase", "<string>", "iterator erase(iterator first, iterator last)", "n", false, "std::string s = \"abcd\"; s.erase(s.begin(), s.begin() + 2);"),
    m(S, "resize", "<string>", "void resize(std::size_t n, char c = '\\0')", "n", true, "std::string s; s.resize(3, 'x');"),
    m(S, "assign", "<string>", "string& assign(const string& other)", "n", false, "std::string s; s.assign(\"abc\");"),
    m(S, "assign", "<string>", "string& assign(std::size_t n, char c)", "n", true, "std::string s; s.assign(3, 'x');"),
    m(S, "assign", "<string>", "void assign(InputIt first, InputIt last)", "n", false, "std::string a = \"ab\"; std::string s; s.assign(a.begin(), a.end());"),
    m(S, "reserve", "<string>", "void reserve(std::size_t n)", "0", false, "std::string s; s.reserve(10);"),
    m(S, "shrink_to_fit", "<string>", "void shrink_to_fit()", "0", false, "std::string s = \"a\"; s.shrink_to_fit();"),
    m(S, "begin", "<string>", "iterator begin()", "1", false, "std::string s = \"a\"; auto it = s.begin(); (void)it;"),
    m(S, "end", "<string>", "iterator end()", "1", false, "std::string s = \"a\"; auto it = s.end(); (void)it;"),
    m(S, "rbegin", "<string>", "reverse_iterator rbegin()", "1", false, "std::string s = \"a\"; auto it = s.rbegin(); (void)it;"),
    m(S, "rend", "<string>", "reverse_iterator rend()", "1", false, "std::string s = \"a\"; auto it = s.rend(); (void)it;"),
    m(S, "cbegin", "<string>", "const_iterator cbegin() const", "1", false, "std::string s = \"a\"; auto it = s.cbegin(); (void)it;"),
    m(S, "cend", "<string>", "const_iterator cend() const", "1", false, "std::string s = \"a\"; auto it = s.cend(); (void)it;"),
    m(S, "swap", "<string>", "void swap(string& other)", "1", true, "std::string a = \"a\"; std::string b = \"b\"; a.swap(b);"),
    m(S, "operator==", "<string>", "bool operator==(const string& other) const", "n", true, "std::string a = \"a\"; bool e = (a == \"a\"); (void)e;"),
    m(S, "operator!=", "<string>", "bool operator!=(const string& other) const", "n", true, "std::string a = \"a\"; bool e = (a != \"b\"); (void)e;"),
    m(S, "operator<", "<string>", "bool operator<(const string& other) const", "n", true, "std::string a = \"a\"; std::string b = \"b\"; bool e = (a < b); (void)e;"),
    // ---- std::string_view ----
    m(SV, "size", "<string_view>", "std::size_t size() const", "1", true, "std::string_view s = \"abc\"; std::size_t n = s.size();"),
    m(SV, "npos", "<string_view>", "static const std::size_t npos", "0", false, "std::string_view s = \"abc\"; bool missing = (s.find('z') == std::string_view::npos); (void)missing;"),
    m(SV, "empty", "<string_view>", "bool empty() const", "1", true, "std::string_view s = \"abc\"; bool e = s.empty();"),
    m(SV, "operator[]", "<string_view>", "const char& operator[](std::size_t i) const", "1", true, "std::string_view s = \"abc\"; char c = s[0];"),
    m(SV, "at", "<string_view>", "const char& at(std::size_t i) const", "1", true, "std::string_view s = \"abc\"; char c = s.at(1);"),
    m(SV, "front", "<string_view>", "const char& front() const", "1", true, "std::string_view s = \"abc\"; char c = s.front();"),
    m(SV, "back", "<string_view>", "const char& back() const", "1", true, "std::string_view s = \"abc\"; char c = s.back();"),
    m(SV, "substr", "<string_view>", "string_view substr(std::size_t pos, std::size_t len = npos) const", "1", true, "std::string_view s = \"abcd\"; std::string_view t = s.substr(1, 2); (void)t;"),
    m(SV, "find", "<string_view>", "std::size_t find(string_view needle, std::size_t pos = 0) const", "n", true, "std::string_view s = \"abcd\"; std::size_t i = s.find('c');"),
    m(SV, "length", "<string_view>", "std::size_t length() const", "1", true, "std::string_view s = \"abc\"; std::size_t n = s.length();"),
    m(SV, "rfind", "<string_view>", "std::size_t rfind(string_view needle, std::size_t pos = npos) const", "n", false, "std::string_view s = \"abcd\"; std::size_t i = s.rfind('c'); std::size_t j = s.rfind(\"ab\", 0);"),
    m(SV, "find_first_of", "<string_view>", "std::size_t find_first_of(string_view set, std::size_t pos = 0) const", "n", false, "std::string_view s = \"abcd\"; std::size_t i = s.find_first_of(\"dc\");"),
    m(SV, "find_last_of", "<string_view>", "std::size_t find_last_of(string_view set, std::size_t pos = npos) const", "n", false, "std::string_view s = \"abcd\"; std::size_t i = s.find_last_of(\"ab\");"),
    m(SV, "find_first_not_of", "<string_view>", "std::size_t find_first_not_of(string_view set, std::size_t pos = 0) const", "n", false, "std::string_view s = \"abcd\"; std::size_t i = s.find_first_not_of(\"ab\");"),
    m(SV, "find_last_not_of", "<string_view>", "std::size_t find_last_not_of(string_view set, std::size_t pos = npos) const", "n", false, "std::string_view s = \"abcd\"; std::size_t i = s.find_last_not_of(\"cd\");"),
    m(SV, "compare", "<string_view>", "int compare(string_view other) const", "n", false, "std::string_view a = \"ab\"; int c = a.compare(\"ac\");"),
    m(SV, "remove_prefix", "<string_view>", "void remove_prefix(std::size_t n)", "1", false, "std::string_view s = \"abc\"; s.remove_prefix(1);"),
    m(SV, "remove_suffix", "<string_view>", "void remove_suffix(std::size_t n)", "1", false, "std::string_view s = \"abc\"; s.remove_suffix(1);"),
    m(SV, "begin", "<string_view>", "const_iterator begin() const", "1", false, "std::string_view s = \"a\"; auto it = s.begin(); (void)it;"),
    m(SV, "end", "<string_view>", "const_iterator end() const", "1", false, "std::string_view s = \"a\"; auto it = s.end(); (void)it;"),
    m(SV, "operator==", "<string_view>", "bool operator==(string_view other) const", "n", true, "std::string_view a = \"a\"; bool e = (a == \"a\"); (void)e;"),
    // ---- std::pair, std::tuple ----
    m(PR, "ctor", "<utility>", "pair(const A& a, const B& b)", "1", true, "std::pair<int, int> p(1, 2);"),
    m(PR, "first", "<utility>", "A first", "1", true, "std::pair<int, int> p(1, 2); int a = p.first;"),
    m(PR, "second", "<utility>", "B second", "1", true, "std::pair<int, int> p(1, 2); int b = p.second;"),
    m(PR, "swap", "<utility>", "void swap(pair& other)", "1", false, "std::pair<int, int> a(1, 2); std::pair<int, int> b(3, 4); a.swap(b);"),
    m(PR, "operator==", "<utility>", "bool operator==(const pair& other) const", "1", true, "std::pair<int, int> a(1, 2); bool e = (a == std::make_pair(1, 2)); (void)e;"),
    m(PR, "operator<", "<utility>", "bool operator<(const pair& other) const", "1", true, "std::pair<int, int> a(1, 2); bool e = (a < std::make_pair(1, 3)); (void)e;"),
    m(TU, "ctor", "<tuple>", "tuple(const T&... args)", "1", false, "std::tuple<int, char, int> t(1, 'a', 2);"),
    m(TU, "operator==", "<tuple>", "bool operator==(const tuple& other) const", "1", false, "std::tuple<int, int> a(1, 2); bool e = (a == std::make_tuple(1, 2)); (void)e;"),
    m(TU, "operator<", "<tuple>", "bool operator<(const tuple& other) const", "1", false, "std::tuple<int, int> a(1, 2); bool e = (a < std::make_tuple(1, 3)); (void)e;"),
    // ---- std::optional ----
    m(OP, "has_value", "<optional>", "bool has_value() const", "1", true, "std::optional<int> o = 5; bool b = o.has_value();"),
    m(OP, "value", "<optional>", "T& value()", "1", true, "std::optional<int> o = 5; int x = o.value();"),
    m(OP, "value_or", "<optional>", "T value_or(const U& fallback) const", "1", true, "std::optional<int> o; int x = o.value_or(-1);"),
    m(OP, "operator*", "<optional>", "T& operator*()", "1", true, "std::optional<int> o = 5; int x = *o;"),
    m(OP, "operator->", "<optional>", "T* operator->()", "1", false, "std::optional<std::pair<int, int>> o = std::make_pair(1, 2); int x = o->first; (void)x;"),
    m(OP, "reset", "<optional>", "void reset()", "1", true, "std::optional<int> o = 5; o.reset();"),
    m(OP, "operator=", "<optional>", "optional& operator=(const T& x)", "1", true, "std::optional<int> o; o = 5; o = std::nullopt;"),
    // ---- iterators (any container) ----
    m(IT, "operator*", "", "T& operator*() const", "1", false, "std::vector<int> v{1}; int x = *v.begin(); (void)x;"),
    m(IT, "operator->", "", "T* operator->() const", "1", false, "std::map<int, int> m{{1, 2}}; int x = m.begin()->second; (void)x;"),
    m(IT, "operator++", "", "iterator& operator++()", "1", false, "std::vector<int> v{1, 2}; auto it = v.begin(); ++it; it++;"),
    m(IT, "operator--", "", "iterator& operator--()", "1", false, "std::vector<int> v{1, 2}; auto it = v.end(); --it; it--;"),
    m(IT, "operator+", "", "iterator operator+(std::ptrdiff_t n) const", "1", false, "std::vector<int> v{1, 2}; auto it = v.begin() + 1; (void)it;"),
    m(IT, "operator-", "", "std::ptrdiff_t operator-(iterator other) const", "1", false, "std::vector<int> v{1, 2}; auto d = v.end() - v.begin(); (void)d;"),
    m(IT, "operator==", "", "bool operator==(iterator other) const", "1", false, "std::vector<int> v{1}; bool e = (v.begin() == v.end()); (void)e;"),
    m(IT, "operator!=", "", "bool operator!=(iterator other) const", "1", false, "std::vector<int> v{1}; bool e = (v.begin() != v.end()); (void)e;"),
    m(IT, "operator<", "", "bool operator<(iterator other) const", "1", false, "std::vector<int> v{1}; bool e = (v.begin() < v.end()); (void)e;"),
    // ---- std::function ----
    m(FN, "operator()", "<functional>", "R operator()(A... args) const", "1", false, "std::function<int(int)> f = [](int x) { return x + 1; }; int y = f(1); (void)y;"),
    // ---- std::unordered_map ----
    m(UM, "size", "<unordered_map>", "std::size_t size() const", "1", true, "std::unordered_map<int, int> m; std::size_t n = m.size();"),
    m(UM, "empty", "<unordered_map>", "bool empty() const", "1", true, "std::unordered_map<int, int> m; bool e = m.empty();"),
    m(UM, "clear", "<unordered_map>", "void clear()", "n", true, "std::unordered_map<int, int> m; m.clear();"),
    m(UM, "count", "<unordered_map>", "std::size_t count(const K& key) const", "1 average", true, "std::unordered_map<int, int> m; std::size_t c = m.count(1);"),
    m(UM, "find", "<unordered_map>", "iterator find(const K& key)", "1 average", true, "std::unordered_map<int, int> m; auto it = m.find(1); bool found = (it != m.end()); (void)found;"),
    m(UM, "insert", "<unordered_map>", "std::pair<iterator, bool> insert(const std::pair<K, V>& kv)", "1 average", true, "std::unordered_map<int, int> m; m.insert({1, 2});"),
    m(UM, "insert", "<unordered_map>", "void insert(InputIt first, InputIt last)", "n", false, "std::unordered_map<int, int> a{{1, 2}}; std::unordered_map<int, int> m; m.insert(a.begin(), a.end());"),
    m(UM, "try_emplace", "<unordered_map>", "std::pair<iterator, bool> try_emplace(const K& key, Args... args)", "log n", false, "std::unordered_map<int, int> m; m.try_emplace(1, 2);"),
    m(UM, "insert_or_assign", "<unordered_map>", "std::pair<iterator, bool> insert_or_assign(const K& key, const V& v)", "log n", false, "std::unordered_map<int, int> m; m.insert_or_assign(1, 2);"),
    m(UM, "emplace", "<unordered_map>", "std::pair<iterator, bool> emplace(Args... args)", "1 average", true, "std::unordered_map<int, int> m; m.emplace(1, 2);"),
    m(UM, "erase", "<unordered_map>", "std::size_t erase(const K& key)", "1 average", true, "std::unordered_map<int, int> m; m[1] = 2; m.erase(1);"),
    m(UM, "erase", "<unordered_map>", "iterator erase(iterator pos)", "1 average", false, "std::unordered_map<int, int> m; m[1] = 2; m.erase(m.begin());"),
    m(UM, "erase", "<unordered_map>", "iterator erase(iterator first, iterator last)", "n", false, "std::unordered_map<int, int> m{{1, 2}}; m.erase(m.begin(), m.end());"),
    m(UM, "operator[]", "<unordered_map>", "V& operator[](const K& key)", "1 average", true, "std::unordered_map<int, int> m; m[1] += 2;"),
    m(UM, "at", "<unordered_map>", "V& at(const K& key)", "1 average", true, "std::unordered_map<int, int> m; m[1] = 2; int v = m.at(1);"),
    m(UM, "begin", "<unordered_map>", "iterator begin()", "1", false, "std::unordered_map<int, int> m; auto it = m.begin(); (void)it;"),
    m(UM, "end", "<unordered_map>", "iterator end()", "1", false, "std::unordered_map<int, int> m; auto it = m.end(); (void)it;"),
    // ---- std::map ----
    m(MP, "size", "<map>", "std::size_t size() const", "1", true, "std::map<int, int> m; std::size_t n = m.size();"),
    m(MP, "empty", "<map>", "bool empty() const", "1", true, "std::map<int, int> m; bool e = m.empty();"),
    m(MP, "clear", "<map>", "void clear()", "n", true, "std::map<int, int> m; m.clear();"),
    m(MP, "count", "<map>", "std::size_t count(const K& key) const", "log n", true, "std::map<int, int> m; std::size_t c = m.count(1);"),
    m(MP, "find", "<map>", "iterator find(const K& key)", "log n", true, "std::map<int, int> m; auto it = m.find(1); bool found = (it != m.end()); (void)found;"),
    m(MP, "insert", "<map>", "std::pair<iterator, bool> insert(const std::pair<K, V>& kv)", "log n", true, "std::map<int, int> m; m.insert({1, 2});"),
    m(MP, "insert", "<map>", "void insert(InputIt first, InputIt last)", "n", false, "std::map<int, int> a{{1, 2}}; std::map<int, int> m; m.insert(a.begin(), a.end());"),
    m(MP, "try_emplace", "<map>", "std::pair<iterator, bool> try_emplace(const K& key, Args... args)", "log n", false, "std::map<int, int> m; m.try_emplace(1, 2);"),
    m(MP, "insert_or_assign", "<map>", "std::pair<iterator, bool> insert_or_assign(const K& key, const V& v)", "log n", false, "std::map<int, int> m; m.insert_or_assign(1, 2);"),
    m(MP, "emplace", "<map>", "std::pair<iterator, bool> emplace(Args... args)", "log n", true, "std::map<int, int> m; m.emplace(1, 2);"),
    m(MP, "erase", "<map>", "std::size_t erase(const K& key)", "log n", true, "std::map<int, int> m; m[1] = 2; m.erase(1);"),
    m(MP, "erase", "<map>", "iterator erase(iterator pos)", "1 amortised", false, "std::map<int, int> m; m[1] = 2; m.erase(m.begin());"),
    m(MP, "erase", "<map>", "iterator erase(iterator first, iterator last)", "n", false, "std::map<int, int> m{{1, 2}}; m.erase(m.begin(), m.end());"),
    m(MP, "operator[]", "<map>", "V& operator[](const K& key)", "log n", true, "std::map<int, int> m; m[1] += 2;"),
    m(MP, "at", "<map>", "V& at(const K& key)", "log n", true, "std::map<int, int> m; m[1] = 2; int v = m.at(1);"),
    m(MP, "lower_bound", "<map>", "iterator lower_bound(const K& key)", "log n", true, "std::map<int, int> m; auto it = m.lower_bound(1); (void)it;"),
    m(MP, "upper_bound", "<map>", "iterator upper_bound(const K& key)", "log n", true, "std::map<int, int> m; auto it = m.upper_bound(1); (void)it;"),
    m(MP, "equal_range", "<map>", "std::pair<iterator, iterator> equal_range(const K& key)", "log n", false, "std::map<int, int> m; auto r = m.equal_range(1); (void)r;"),
    m(MP, "begin", "<map>", "iterator begin()", "1", false, "std::map<int, int> m; auto it = m.begin(); (void)it;"),
    m(MP, "end", "<map>", "iterator end()", "1", false, "std::map<int, int> m; auto it = m.end(); (void)it;"),
    m(MP, "rbegin", "<map>", "reverse_iterator rbegin()", "1", false, "std::map<int, int> m; auto it = m.rbegin(); (void)it;"),
    m(MP, "rend", "<map>", "reverse_iterator rend()", "1", false, "std::map<int, int> m; auto it = m.rend(); (void)it;"),
    // ---- std::unordered_set ----
    m(US, "size", "<unordered_set>", "std::size_t size() const", "1", true, "std::unordered_set<int> s; std::size_t n = s.size();"),
    m(US, "empty", "<unordered_set>", "bool empty() const", "1", true, "std::unordered_set<int> s; bool e = s.empty();"),
    m(US, "clear", "<unordered_set>", "void clear()", "n", true, "std::unordered_set<int> s; s.clear();"),
    m(US, "count", "<unordered_set>", "std::size_t count(const T& key) const", "1 average", true, "std::unordered_set<int> s; std::size_t c = s.count(1);"),
    m(US, "find", "<unordered_set>", "iterator find(const T& key)", "1 average", true, "std::unordered_set<int> s; auto it = s.find(1); bool found = (it != s.end()); (void)found;"),
    m(US, "insert", "<unordered_set>", "std::pair<iterator, bool> insert(const T& key)", "1 average", true, "std::unordered_set<int> s; s.insert(1);"),
    m(US, "insert", "<unordered_set>", "void insert(InputIt first, InputIt last)", "n", false, "std::vector<int> a{1, 2}; std::unordered_set<int> s; s.insert(a.begin(), a.end());"),
    m(US, "insert", "<unordered_set>", "void insert(std::initializer_list<T> il)", "n", false, "std::unordered_set<int> s; s.insert({1, 2});"),
    m(US, "emplace", "<unordered_set>", "std::pair<iterator, bool> emplace(Args... args)", "1 average", true, "std::unordered_set<int> s; s.emplace(1);"),
    m(US, "erase", "<unordered_set>", "std::size_t erase(const T& key)", "1 average", true, "std::unordered_set<int> s{1}; s.erase(1);"),
    m(US, "erase", "<unordered_set>", "iterator erase(iterator pos)", "1 average", false, "std::unordered_set<int> s{1}; s.erase(s.begin());"),
    m(US, "erase", "<unordered_set>", "iterator erase(iterator first, iterator last)", "n", false, "std::unordered_set<int> s{1, 2}; s.erase(s.begin(), s.end());"),
    m(US, "begin", "<unordered_set>", "iterator begin()", "1", false, "std::unordered_set<int> s; auto it = s.begin(); (void)it;"),
    m(US, "end", "<unordered_set>", "iterator end()", "1", false, "std::unordered_set<int> s; auto it = s.end(); (void)it;"),
    // ---- std::set ----
    m(ST, "size", "<set>", "std::size_t size() const", "1", true, "std::set<int> s; std::size_t n = s.size();"),
    m(ST, "empty", "<set>", "bool empty() const", "1", true, "std::set<int> s; bool e = s.empty();"),
    m(ST, "clear", "<set>", "void clear()", "n", true, "std::set<int> s; s.clear();"),
    m(ST, "count", "<set>", "std::size_t count(const T& key) const", "log n", true, "std::set<int> s; std::size_t c = s.count(1);"),
    m(ST, "find", "<set>", "iterator find(const T& key)", "log n", true, "std::set<int> s; auto it = s.find(1); bool found = (it != s.end()); (void)found;"),
    m(ST, "insert", "<set>", "std::pair<iterator, bool> insert(const T& key)", "log n", true, "std::set<int> s; s.insert(1);"),
    m(ST, "insert", "<set>", "void insert(InputIt first, InputIt last)", "n", false, "std::vector<int> a{1, 2}; std::set<int> s; s.insert(a.begin(), a.end());"),
    m(ST, "insert", "<set>", "void insert(std::initializer_list<T> il)", "n", false, "std::set<int> s; s.insert({1, 2});"),
    m(ST, "emplace", "<set>", "std::pair<iterator, bool> emplace(Args... args)", "log n", true, "std::set<int> s; s.emplace(1);"),
    m(ST, "erase", "<set>", "std::size_t erase(const T& key)", "log n", true, "std::set<int> s{1}; s.erase(1);"),
    m(ST, "erase", "<set>", "iterator erase(iterator pos)", "1 amortised", false, "std::set<int> s{1}; s.erase(s.begin());"),
    m(ST, "erase", "<set>", "iterator erase(iterator first, iterator last)", "n", false, "std::set<int> s{1, 2}; s.erase(s.begin(), s.end());"),
    m(ST, "lower_bound", "<set>", "iterator lower_bound(const T& key)", "log n", true, "std::set<int> s{1, 3}; auto it = s.lower_bound(2); (void)it;"),
    m(ST, "upper_bound", "<set>", "iterator upper_bound(const T& key)", "log n", true, "std::set<int> s{1, 3}; auto it = s.upper_bound(1); (void)it;"),
    m(ST, "equal_range", "<set>", "std::pair<iterator, iterator> equal_range(const T& key)", "log n", false, "std::set<int> s; auto r = s.equal_range(1); (void)r;"),
    m(ST, "begin", "<set>", "iterator begin()", "1", false, "std::set<int> s; auto it = s.begin(); (void)it;"),
    m(ST, "end", "<set>", "iterator end()", "1", false, "std::set<int> s; auto it = s.end(); (void)it;"),
    m(ST, "rbegin", "<set>", "reverse_iterator rbegin()", "1", false, "std::set<int> s; auto it = s.rbegin(); (void)it;"),
    m(ST, "rend", "<set>", "reverse_iterator rend()", "1", false, "std::set<int> s; auto it = s.rend(); (void)it;"),
    // ---- std::stack, std::queue ----
    m(SK, "push", "<stack>", "void push(const T& x)", "1 amortised", true, "std::stack<int> s; s.push(1);"),
    m(SK, "emplace", "<stack>", "void emplace(Args... args)", "1 amortised", true, "std::stack<std::pair<int, int>> s; s.emplace(1, 2);"),
    m(SK, "pop", "<stack>", "void pop()", "1", true, "std::stack<int> s; s.push(1); s.pop();"),
    m(SK, "top", "<stack>", "T& top()", "1", true, "std::stack<int> s; s.push(1); int x = s.top();"),
    m(SK, "size", "<stack>", "std::size_t size() const", "1", true, "std::stack<int> s; std::size_t n = s.size();"),
    m(SK, "empty", "<stack>", "bool empty() const", "1", true, "std::stack<int> s; bool e = s.empty();"),
    m(QU, "push", "<queue>", "void push(const T& x)", "1 amortised", true, "std::queue<int> q; q.push(1);"),
    m(QU, "emplace", "<queue>", "void emplace(Args... args)", "1 amortised", true, "std::queue<std::pair<int, int>> q; q.emplace(1, 2);"),
    m(QU, "pop", "<queue>", "void pop()", "1", true, "std::queue<int> q; q.push(1); q.pop();"),
    m(QU, "front", "<queue>", "T& front()", "1", true, "std::queue<int> q; q.push(1); int x = q.front();"),
    m(QU, "back", "<queue>", "T& back()", "1", true, "std::queue<int> q; q.push(1); int x = q.back();"),
    m(QU, "size", "<queue>", "std::size_t size() const", "1", true, "std::queue<int> q; std::size_t n = q.size();"),
    m(QU, "empty", "<queue>", "bool empty() const", "1", true, "std::queue<int> q; bool e = q.empty();"),
    // ---- std::priority_queue ----
    m(PQ, "ctor", "<queue>", "priority_queue(const Cmp& cmp)", "1", false, "auto cmp = [](int a, int b) { return a > b; }; std::priority_queue<int, std::vector<int>, decltype(cmp)> pq(cmp);"),
    m(PQ, "push", "<queue>", "void push(const T& x)", "log n", true, "std::priority_queue<int> pq; pq.push(1);"),
    m(PQ, "emplace", "<queue>", "void emplace(Args... args)", "log n", true, "std::priority_queue<std::pair<int, int>> pq; pq.emplace(1, 2);"),
    m(PQ, "pop", "<queue>", "void pop()", "log n", true, "std::priority_queue<int, std::vector<int>, std::greater<int>> pq; pq.push(1); pq.pop();"),
    m(PQ, "top", "<queue>", "const T& top() const", "1", true, "std::priority_queue<int> pq; pq.push(1); int x = pq.top();"),
    m(PQ, "size", "<queue>", "std::size_t size() const", "1", true, "std::priority_queue<int> pq; std::size_t n = pq.size();"),
    m(PQ, "empty", "<queue>", "bool empty() const", "1", true, "std::priority_queue<int> pq; bool e = pq.empty();"),
    // ---- std::deque ----
    m(DQ, "push_front", "<deque>", "void push_front(const T& x)", "1", true, "std::deque<int> d; d.push_front(1);"),
    m(DQ, "pop_front", "<deque>", "void pop_front()", "1", true, "std::deque<int> d{1}; d.pop_front();"),
    m(DQ, "push_back", "<deque>", "void push_back(const T& x)", "1", true, "std::deque<int> d; d.push_back(1);"),
    m(DQ, "pop_back", "<deque>", "void pop_back()", "1", true, "std::deque<int> d{1}; d.pop_back();"),
    m(DQ, "emplace_back", "<deque>", "T& emplace_back(Args... args)", "1", true, "std::deque<std::pair<int, int>> d; d.emplace_back(1, 2);"),
    m(DQ, "emplace_front", "<deque>", "T& emplace_front(Args... args)", "1", true, "std::deque<std::pair<int, int>> d; d.emplace_front(1, 2);"),
    m(DQ, "front", "<deque>", "T& front()", "1", true, "std::deque<int> d{1}; int x = d.front();"),
    m(DQ, "back", "<deque>", "T& back()", "1", true, "std::deque<int> d{1}; int x = d.back();"),
    m(DQ, "operator[]", "<deque>", "T& operator[](std::size_t i)", "1", true, "std::deque<int> d{1, 2}; d[0] = d[1];"),
    m(DQ, "at", "<deque>", "T& at(std::size_t i)", "1", true, "std::deque<int> d{1, 2}; int x = d.at(1);"),
    m(DQ, "size", "<deque>", "std::size_t size() const", "1", true, "std::deque<int> d; std::size_t n = d.size();"),
    m(DQ, "empty", "<deque>", "bool empty() const", "1", true, "std::deque<int> d; bool e = d.empty();"),
    m(DQ, "clear", "<deque>", "void clear()", "n", true, "std::deque<int> d{1}; d.clear();"),
    m(DQ, "insert", "<deque>", "iterator insert(iterator pos, const T& x)", "n", true, "std::deque<int> d{1, 3}; d.insert(d.begin() + 1, 2);"),
    m(DQ, "erase", "<deque>", "iterator erase(iterator pos)", "n", true, "std::deque<int> d{1, 2}; d.erase(d.begin());"),
    m(DQ, "insert", "<deque>", "iterator insert(iterator pos, std::size_t n, const T& x)", "n", false, "std::deque<int> d{1, 3}; d.insert(d.begin(), 2, 0);"),
    m(DQ, "insert", "<deque>", "iterator insert(iterator pos, InputIt first, InputIt last)", "n", false, "std::vector<int> a{4, 5}; std::deque<int> d{1}; d.insert(d.end(), a.begin(), a.end());"),
    m(DQ, "insert", "<deque>", "iterator insert(iterator pos, std::initializer_list<T> il)", "n", false, "std::deque<int> d{1}; d.insert(d.begin(), {2, 3});"),
    m(DQ, "emplace", "<deque>", "iterator emplace(iterator pos, Args... args)", "n", false, "std::deque<std::pair<int, int>> d; d.emplace(d.begin(), 1, 2);"),
    m(DQ, "assign", "<deque>", "void assign(std::size_t n, const T& x)", "n", false, "std::deque<int> d; d.assign(5, 0);"),
    m(DQ, "assign", "<deque>", "void assign(InputIt first, InputIt last)", "n", false, "std::vector<int> a{1, 2}; std::deque<int> d; d.assign(a.begin(), a.end());"),
    m(DQ, "assign", "<deque>", "void assign(std::initializer_list<T> il)", "n", false, "std::deque<int> d; d.assign({1, 2});"),
    m(DQ, "erase", "<deque>", "iterator erase(iterator first, iterator last)", "n", false, "std::deque<int> d{1, 2, 3}; d.erase(d.begin(), std::next(d.begin(), 2));"),
    m(DQ, "resize", "<deque>", "void resize(std::size_t n)", "n", false, "std::deque<int> d; d.resize(5);"),
    m(DQ, "resize", "<deque>", "void resize(std::size_t n, const T& x)", "n", false, "std::deque<int> d; d.resize(5, -1);"),
    m(DQ, "swap", "<deque>", "void swap(deque& other)", "1", false, "std::deque<int> a{1}; std::deque<int> b{2}; a.swap(b);"),
    m(DQ, "rbegin", "<deque>", "reverse_iterator rbegin()", "1", false, "std::deque<int> d{1}; auto it = d.rbegin(); (void)it;"),
    m(DQ, "rend", "<deque>", "reverse_iterator rend()", "1", false, "std::deque<int> d{1}; auto it = d.rend(); (void)it;"),
    m(DQ, "cbegin", "<deque>", "const_iterator cbegin() const", "1", false, "std::deque<int> d{1}; auto it = d.cbegin(); (void)it;"),
    m(DQ, "cend", "<deque>", "const_iterator cend() const", "1", false, "std::deque<int> d{1}; auto it = d.cend(); (void)it;"),
    m(DQ, "operator==", "<deque>", "bool operator==(const deque& other) const", "n", false, "std::deque<int> a{1}; std::deque<int> b{1}; bool e = (a == b); (void)e;"),
    m(DQ, "operator<", "<deque>", "bool operator<(const deque& other) const", "n", false, "std::deque<int> a{1}; std::deque<int> b{2}; bool e = (a < b); (void)e;"),
    m(DQ, "begin", "<deque>", "iterator begin()", "1", false, "std::deque<int> d; auto it = d.begin(); (void)it;"),
    m(DQ, "end", "<deque>", "iterator end()", "1", false, "std::deque<int> d; auto it = d.end(); (void)it;"),
    // ---- std::list ----
    m(LS, "push_front", "<list>", "void push_front(const T& x)", "1", true, "std::list<int> l; l.push_front(1);"),
    m(LS, "pop_front", "<list>", "void pop_front()", "1", true, "std::list<int> l{1}; l.pop_front();"),
    m(LS, "push_back", "<list>", "void push_back(const T& x)", "1", true, "std::list<int> l; l.push_back(1);"),
    m(LS, "pop_back", "<list>", "void pop_back()", "1", true, "std::list<int> l{1}; l.pop_back();"),
    m(LS, "emplace_back", "<list>", "T& emplace_back(Args... args)", "1", true, "std::list<std::pair<int, int>> l; l.emplace_back(1, 2);"),
    m(LS, "emplace_front", "<list>", "T& emplace_front(Args... args)", "1", true, "std::list<std::pair<int, int>> l; l.emplace_front(1, 2);"),
    m(LS, "front", "<list>", "T& front()", "1", true, "std::list<int> l{1}; int x = l.front();"),
    m(LS, "back", "<list>", "T& back()", "1", true, "std::list<int> l{1}; int x = l.back();"),
    m(LS, "size", "<list>", "std::size_t size() const", "1", true, "std::list<int> l; std::size_t n = l.size();"),
    m(LS, "empty", "<list>", "bool empty() const", "1", true, "std::list<int> l; bool e = l.empty();"),
    m(LS, "clear", "<list>", "void clear()", "n", true, "std::list<int> l{1}; l.clear();"),
    m(LS, "insert", "<list>", "iterator insert(iterator pos, const T& x)", "1", true, "std::list<int> l{1, 3}; l.insert(std::next(l.begin()), 2);"),
    m(LS, "erase", "<list>", "iterator erase(iterator pos)", "1", true, "std::list<int> l{1, 2}; l.erase(l.begin());"),
    m(LS, "insert", "<list>", "iterator insert(iterator pos, std::size_t n, const T& x)", "n", false, "std::list<int> l{1, 3}; l.insert(l.begin(), 2, 0);"),
    m(LS, "insert", "<list>", "iterator insert(iterator pos, InputIt first, InputIt last)", "n", false, "std::vector<int> a{4, 5}; std::list<int> l{1}; l.insert(l.end(), a.begin(), a.end());"),
    m(LS, "insert", "<list>", "iterator insert(iterator pos, std::initializer_list<T> il)", "n", false, "std::list<int> l{1}; l.insert(l.begin(), {2, 3});"),
    m(LS, "emplace", "<list>", "iterator emplace(iterator pos, Args... args)", "n", false, "std::list<std::pair<int, int>> l; l.emplace(l.begin(), 1, 2);"),
    m(LS, "assign", "<list>", "void assign(std::size_t n, const T& x)", "n", false, "std::list<int> l; l.assign(5, 0);"),
    m(LS, "assign", "<list>", "void assign(InputIt first, InputIt last)", "n", false, "std::vector<int> a{1, 2}; std::list<int> l; l.assign(a.begin(), a.end());"),
    m(LS, "assign", "<list>", "void assign(std::initializer_list<T> il)", "n", false, "std::list<int> l; l.assign({1, 2});"),
    m(LS, "erase", "<list>", "iterator erase(iterator first, iterator last)", "n", false, "std::list<int> l{1, 2, 3}; l.erase(l.begin(), std::next(l.begin(), 2));"),
    m(LS, "reverse", "<list>", "void reverse()", "n", false, "std::list<int> l{1, 2}; l.reverse();"),
    m(LS, "sort", "<list>", "void sort()", "n log n", false, "std::list<int> l{2, 1}; l.sort();"),
    m(LS, "sort", "<list>", "void sort(Compare cmp)", "n log n", false, "std::list<int> l{2, 1}; l.sort([](int a, int b) { return a > b; });"),
    m(LS, "unique", "<list>", "std::size_t unique()", "n", false, "std::list<int> l{1, 1}; l.unique();"),
    m(LS, "remove", "<list>", "std::size_t remove(const T& x)", "n", false, "std::list<int> l{1, 2}; l.remove(1);"),
    m(LS, "remove_if", "<list>", "std::size_t remove_if(Pred pred)", "n", false, "std::list<int> l{1, 2}; l.remove_if([](int x) { return x > 1; });"),
    m(LS, "resize", "<list>", "void resize(std::size_t n)", "n", false, "std::list<int> l; l.resize(5);"),
    m(LS, "resize", "<list>", "void resize(std::size_t n, const T& x)", "n", false, "std::list<int> l; l.resize(5, -1);"),
    m(LS, "swap", "<list>", "void swap(list& other)", "1", false, "std::list<int> a{1}; std::list<int> b{2}; a.swap(b);"),
    m(LS, "cbegin", "<list>", "const_iterator cbegin() const", "1", false, "std::list<int> l{1}; auto it = l.cbegin(); (void)it;"),
    m(LS, "cend", "<list>", "const_iterator cend() const", "1", false, "std::list<int> l{1}; auto it = l.cend(); (void)it;"),
    m(LS, "operator==", "<list>", "bool operator==(const list& other) const", "n", false, "std::list<int> a{1}; std::list<int> b{1}; bool e = (a == b); (void)e;"),
    m(LS, "operator<", "<list>", "bool operator<(const list& other) const", "n", false, "std::list<int> a{1}; std::list<int> b{2}; bool e = (a < b); (void)e;"),
    m(LS, "begin", "<list>", "iterator begin()", "1", false, "std::list<int> l; auto it = l.begin(); (void)it;"),
    m(LS, "end", "<list>", "iterator end()", "1", false, "std::list<int> l; auto it = l.end(); (void)it;"),
    m(LS, "rbegin", "<list>", "reverse_iterator rbegin()", "1", false, "std::list<int> l; auto it = l.rbegin(); (void)it;"),
    m(LS, "rend", "<list>", "reverse_iterator rend()", "1", false, "std::list<int> l; auto it = l.rend(); (void)it;"),
    // ---- <algorithm> ----
    m(FR, "std::sort", "<algorithm>", "void sort(RandomIt first, RandomIt last)", "n log n", true, "std::vector<int> v{3, 1, 2}; std::sort(v.begin(), v.end());"),
    m(FR, "std::sort", "<algorithm>", "void sort(RandomIt first, RandomIt last, Compare cmp)", "n log n", false, "std::vector<int> v{3, 1, 2}; std::sort(v.begin(), v.end(), [](int a, int b) { return a > b; });"),
    m(FR, "std::stable_sort", "<algorithm>", "void stable_sort(RandomIt first, RandomIt last)", "n log n", true, "std::vector<int> v{3, 1, 2}; std::stable_sort(v.begin(), v.end());"),
    m(FR, "std::stable_sort", "<algorithm>", "void stable_sort(RandomIt first, RandomIt last, Compare cmp)", "n log n", false, "std::vector<int> v{3, 1, 2}; std::stable_sort(v.begin(), v.end(), [](int a, int b) { return a > b; });"),
    m(FR, "std::reverse", "<algorithm>", "void reverse(BidirIt first, BidirIt last)", "n", true, "std::vector<int> v{1, 2, 3}; std::reverse(v.begin(), v.end());"),
    m(FR, "std::min", "<algorithm>", "const T& min(const T& a, const T& b)", "1", true, "int a = 1; int b = 2; int c = std::min(a, b); (void)c;"),
    m(FR, "std::max", "<algorithm>", "const T& max(const T& a, const T& b)", "1", true, "int a = 1; int b = 2; int c = std::max(a, b); (void)c;"),
    m(FR, "std::min", "<algorithm>", "T min(std::initializer_list<T> il)", "n", false, "int c = std::min({3, 1, 2}); (void)c;"),
    m(FR, "std::max", "<algorithm>", "T max(std::initializer_list<T> il)", "n", false, "int c = std::max({3, 1, 2}); (void)c;"),
    m(FR, "std::swap", "<utility>", "void swap(T& a, T& b)", "1", true, "int a = 1; int b = 2; std::swap(a, b);"),
    m(FR, "std::find", "<algorithm>", "InputIt find(InputIt first, InputIt last, const T& x)", "n", true, "std::vector<int> v{1, 2}; auto it = std::find(v.begin(), v.end(), 2); (void)it;"),
    m(FR, "std::find_if", "<algorithm>", "InputIt find_if(InputIt first, InputIt last, Pred pred)", "n", false, "std::vector<int> v{1, 2}; auto it = std::find_if(v.begin(), v.end(), [](int x) { return x > 1; }); (void)it;"),
    m(FR, "std::count", "<algorithm>", "difference_type count(InputIt first, InputIt last, const T& x)", "n", true, "std::vector<int> v{1, 2, 1}; auto c = std::count(v.begin(), v.end(), 1); (void)c;"),
    m(FR, "std::count_if", "<algorithm>", "difference_type count_if(InputIt first, InputIt last, Pred pred)", "n", false, "std::vector<int> v{1, 2, 1}; auto c = std::count_if(v.begin(), v.end(), [](int x) { return x == 1; }); (void)c;"),
    m(FR, "std::binary_search", "<algorithm>", "bool binary_search(ForwardIt first, ForwardIt last, const T& x)", "log n", true, "std::vector<int> v{1, 2, 3}; bool b = std::binary_search(v.begin(), v.end(), 2); (void)b;"),
    m(FR, "std::lower_bound", "<algorithm>", "ForwardIt lower_bound(ForwardIt first, ForwardIt last, const T& x)", "log n", true, "std::vector<int> v{1, 2, 3}; auto it = std::lower_bound(v.begin(), v.end(), 2); (void)it;"),
    m(FR, "std::upper_bound", "<algorithm>", "ForwardIt upper_bound(ForwardIt first, ForwardIt last, const T& x)", "log n", true, "std::vector<int> v{1, 2, 3}; auto it = std::upper_bound(v.begin(), v.end(), 2); (void)it;"),
    m(FR, "std::unique", "<algorithm>", "ForwardIt unique(ForwardIt first, ForwardIt last)", "n", true, "std::vector<int> v{1, 1, 2}; v.erase(std::unique(v.begin(), v.end()), v.end());"),
    m(FR, "std::min_element", "<algorithm>", "ForwardIt min_element(ForwardIt first, ForwardIt last)", "n", true, "std::vector<int> v{3, 1, 2}; int m = *std::min_element(v.begin(), v.end()); (void)m;"),
    m(FR, "std::max_element", "<algorithm>", "ForwardIt max_element(ForwardIt first, ForwardIt last)", "n", true, "std::vector<int> v{3, 1, 2}; int m = *std::max_element(v.begin(), v.end()); (void)m;"),
    m(FR, "std::fill", "<algorithm>", "void fill(ForwardIt first, ForwardIt last, const T& x)", "n", true, "std::vector<int> v(3); std::fill(v.begin(), v.end(), 7);"),
    m(FR, "std::next_permutation", "<algorithm>", "bool next_permutation(BidirIt first, BidirIt last)", "n", true, "std::vector<int> v{1, 2, 3}; bool b = std::next_permutation(v.begin(), v.end()); (void)b;"),
    m(FR, "std::prev_permutation", "<algorithm>", "bool prev_permutation(BidirIt first, BidirIt last)", "n", true, "std::vector<int> v{3, 2, 1}; bool b = std::prev_permutation(v.begin(), v.end()); (void)b;"),
    m(FR, "std::any_of", "<algorithm>", "bool any_of(InputIt first, InputIt last, Pred pred)", "n", false, "std::vector<int> v{1, 2}; bool b = std::any_of(v.begin(), v.end(), [](int x) { return x > 1; }); (void)b;"),
    m(FR, "std::all_of", "<algorithm>", "bool all_of(InputIt first, InputIt last, Pred pred)", "n", false, "std::vector<int> v{1, 2}; bool b = std::all_of(v.begin(), v.end(), [](int x) { return x > 0; }); (void)b;"),
    m(FR, "std::none_of", "<algorithm>", "bool none_of(InputIt first, InputIt last, Pred pred)", "n", false, "std::vector<int> v{1, 2}; bool b = std::none_of(v.begin(), v.end(), [](int x) { return x > 5; }); (void)b;"),
    m(FR, "std::for_each", "<algorithm>", "void for_each(InputIt first, InputIt last, Fn fn)", "n", false, "std::vector<int> v{1, 2}; int s = 0; std::for_each(v.begin(), v.end(), [&](int x) { s += x; });"),
    m(FR, "std::remove", "<algorithm>", "ForwardIt remove(ForwardIt first, ForwardIt last, const T& x)", "n", true, "std::vector<int> v{1, 2, 1}; v.erase(std::remove(v.begin(), v.end(), 1), v.end());"),
    m(FR, "std::remove_if", "<algorithm>", "ForwardIt remove_if(ForwardIt first, ForwardIt last, Pred pred)", "n", false, "std::vector<int> v{1, 2, 1}; v.erase(std::remove_if(v.begin(), v.end(), [](int x) { return x == 1; }), v.end());"),
    m(FR, "std::rotate", "<algorithm>", "ForwardIt rotate(ForwardIt first, ForwardIt middle, ForwardIt last)", "n", false, "std::vector<int> v{1, 2, 3}; std::rotate(v.begin(), v.begin() + 1, v.end());"),
    m(FR, "std::equal", "<algorithm>", "bool equal(InputIt1 first1, InputIt1 last1, InputIt2 first2)", "n", false, "std::vector<int> a{1, 2}; std::vector<int> b{1, 2}; bool e = std::equal(a.begin(), a.end(), b.begin()); (void)e;"),
    m(FR, "std::copy", "<algorithm>", "OutputIt copy(InputIt first, InputIt last, OutputIt out)", "n", false, "std::vector<int> a{1, 2}; std::vector<int> b(2); std::copy(a.begin(), a.end(), b.begin());"),
    m(FR, "std::lexicographical_compare", "<algorithm>", "bool lexicographical_compare(InputIt1 f1, InputIt1 l1, InputIt2 f2, InputIt2 l2)", "n", false, "std::vector<int> a{1, 2}; std::vector<int> b{1, 3}; bool r = std::lexicographical_compare(a.begin(), a.end(), b.begin(), b.end()); (void)r;"),
    // ---- <numeric> ----
    m(FR, "std::accumulate", "<numeric>", "T accumulate(InputIt first, InputIt last, T init)", "n", true, "std::vector<int> v{1, 2}; int s = std::accumulate(v.begin(), v.end(), 0); (void)s;"),
    m(FR, "std::accumulate", "<numeric>", "T accumulate(InputIt first, InputIt last, T init, BinaryOp op)", "n", false, "std::vector<int> v{1, 2}; int s = std::accumulate(v.begin(), v.end(), 0, [](int a, int b) { return a + b; }); (void)s;"),
    m(FR, "std::iota", "<numeric>", "void iota(ForwardIt first, ForwardIt last, T value)", "n", true, "std::vector<int> v(3); std::iota(v.begin(), v.end(), 0);"),
    m(FR, "std::partial_sum", "<numeric>", "OutputIt partial_sum(InputIt first, InputIt last, OutputIt out)", "n", false, "std::vector<int> v{1, 2, 3}; std::vector<int> p(3); std::partial_sum(v.begin(), v.end(), p.begin());"),
    m(FR, "std::partial_sum", "<numeric>", "OutputIt partial_sum(InputIt first, InputIt last, OutputIt out, BinaryOp op)", "n", false, "std::vector<int> v{1, 2, 3}; std::vector<int> p(3); std::partial_sum(v.begin(), v.end(), p.begin(), [](int a, int b) { return a + b; });"),
    // ---- <iterator> helpers ----
    m(FR, "std::distance", "<iterator>", "difference_type distance(InputIt first, InputIt last)", "1", false, "std::vector<int> v{1, 2}; auto d = std::distance(v.begin(), v.end()); (void)d;"),
    m(FR, "std::advance", "<iterator>", "void advance(InputIt& it, difference_type n)", "1", false, "std::vector<int> v{1, 2, 3}; auto it = v.begin(); std::advance(it, 2);"),
    m(FR, "std::next", "<iterator>", "ForwardIt next(ForwardIt it, difference_type n = 1)", "1", false, "std::vector<int> v{1, 2, 3}; auto it = std::next(v.begin()); (void)it;"),
    m(FR, "std::prev", "<iterator>", "BidirIt prev(BidirIt it, difference_type n = 1)", "1", false, "std::vector<int> v{1, 2, 3}; auto it = std::prev(v.end()); (void)it;"),
    // ---- <utility>, <tuple> ----
    m(FR, "std::make_pair", "<utility>", "pair<A, B> make_pair(A a, B b)", "1", true, "auto p = std::make_pair(1, 2); (void)p;"),
    m(FR, "std::get", "<utility>", "T& get<I>(pair<A, B>& p)", "1", false, "std::pair<int, int> p(1, 2); int a = std::get<0>(p); (void)a;"),
    m(FR, "std::get", "<tuple>", "T& get<I>(tuple<T...>& t)", "1", false, "std::tuple<int, int> t(1, 2); int a = std::get<1>(t); (void)a;"),
    m(FR, "std::make_tuple", "<tuple>", "tuple<T...> make_tuple(T... args)", "1", false, "auto t = std::make_tuple(1, 2, 3); (void)t;"),
    m(FR, "std::tie", "<tuple>", "tuple<T&...> tie(T&... args)", "0", false, "int a = 0; int b = 0; std::tie(a, b) = std::make_pair(1, 2);"),
    m(FR, "std::nullopt", "<optional>", "constexpr nullopt_t nullopt", "0", true, "std::optional<int> o = std::nullopt; (void)o;"),
    // ---- <cstdlib>, <string>, <cctype> ----
    m(FR, "std::abs", "<cstdlib>", "int abs(int x)", "1", true, "int x = std::abs(-3); (void)x;"),
    m(FR, "std::abs", "<cstdlib>", "long long abs(long long x)", "1", true, "long long x = std::abs(-3LL); (void)x;"),
    m(FR, "std::to_string", "<string>", "std::string to_string(int x)", "n", true, "std::string s = std::to_string(42);"),
    m(FR, "std::to_string", "<string>", "std::string to_string(long long x)", "n", true, "std::string s = std::to_string(42LL);"),
    m(FR, "std::stoi", "<string>", "int stoi(const std::string& s)", "n", true, "int x = std::stoi(\"42\"); (void)x;"),
    m(FR, "std::stoll", "<string>", "long long stoll(const std::string& s)", "n", true, "long long x = std::stoll(\"42\"); (void)x;"),
    m(FR, "std::isdigit", "<cctype>", "int isdigit(int c)", "1", true, "bool b = std::isdigit('7') != 0; (void)b;"),
    m(FR, "std::isalpha", "<cctype>", "int isalpha(int c)", "1", true, "bool b = std::isalpha('a') != 0; (void)b;"),
    m(FR, "std::isalnum", "<cctype>", "int isalnum(int c)", "1", true, "bool b = std::isalnum('a') != 0; (void)b;"),
    m(FR, "std::isspace", "<cctype>", "int isspace(int c)", "1", true, "bool b = std::isspace(' ') != 0; (void)b;"),
    m(FR, "std::isupper", "<cctype>", "int isupper(int c)", "1", true, "bool b = std::isupper('A') != 0; (void)b;"),
    m(FR, "std::islower", "<cctype>", "int islower(int c)", "1", true, "bool b = std::islower('a') != 0; (void)b;"),
    m(FR, "std::ispunct", "<cctype>", "int ispunct(int c)", "1", true, "bool b = std::ispunct('!') != 0; (void)b;"),
    m(FR, "std::tolower", "<cctype>", "int tolower(int c)", "1", true, "char c = static_cast<char>(std::tolower('A')); (void)c;"),
    m(FR, "std::toupper", "<cctype>", "int toupper(int c)", "1", true, "char c = static_cast<char>(std::toupper('a')); (void)c;"),
    // ---- <limits>, <climits> ----
    m(FR, "std::numeric_limits<T>::max", "<limits>", "constexpr T max()", "0", true, "int x = std::numeric_limits<int>::max(); (void)x;"),
    m(FR, "std::numeric_limits<T>::min", "<limits>", "constexpr T min()", "0", true, "int x = std::numeric_limits<int>::min(); (void)x;"),
    m(FR, "std::numeric_limits<T>::lowest", "<limits>", "constexpr T lowest()", "0", true, "long long x = std::numeric_limits<long long>::lowest(); (void)x;"),
    m(FR, "INT_MAX", "<climits>", "int INT_MAX", "0", true, "int x = INT_MAX; (void)x;"),
    m(FR, "INT_MIN", "<climits>", "int INT_MIN", "0", true, "int x = INT_MIN; (void)x;"),
    m(FR, "UINT_MAX", "<climits>", "unsigned UINT_MAX", "0", true, "unsigned x = UINT_MAX; (void)x;"),
    m(FR, "LLONG_MAX", "<climits>", "long long LLONG_MAX", "0", true, "long long x = LLONG_MAX; (void)x;"),
    m(FR, "LLONG_MIN", "<climits>", "long long LLONG_MIN", "0", true, "long long x = LLONG_MIN; (void)x;"),
    m(FR, "ULLONG_MAX", "<climits>", "unsigned long long ULLONG_MAX", "0", true, "unsigned long long x = ULLONG_MAX; (void)x;"),
    m(FR, "CHAR_MAX", "<climits>", "int CHAR_MAX", "0", true, "int x = CHAR_MAX; (void)x;"),
    m(FR, "CHAR_MIN", "<climits>", "int CHAR_MIN", "0", true, "int x = CHAR_MIN; (void)x;"),
    // ---- <functional> ----
    m(FR, "std::function", "<functional>", "template<class R, class... A> class function<R(A...)>", "0", false, "std::function<int(int)> f = [](int x) { return x; }; (void)f;"),
    m(FR, "std::greater", "<functional>", "template<class T> struct greater", "0", true, "std::vector<int> v{1, 2}; std::sort(v.begin(), v.end(), std::greater<int>());"),
    m(FR, "std::less", "<functional>", "template<class T> struct less", "0", true, "std::vector<int> v{2, 1}; std::sort(v.begin(), v.end(), std::less<int>());"),
];

/// The receiver pattern of a static type, or `None` for types without library members.
fn recv_of(ty: &Ty) -> Option<&'static str> {
    Some(match ty {
        Ty::Str => S,
        Ty::StrView => SV,
        Ty::Vector(_) => V,
        Ty::Array(..) => AR,
        Ty::Pair(..) => PR,
        Ty::Tuple(_) => TU,
        Ty::Optional(_) => OP,
        Ty::UnorderedMap(..) => UM,
        Ty::Map(..) => MP,
        Ty::UnorderedSet(_) => US,
        Ty::Set(_) => ST,
        Ty::Stack(_) => SK,
        Ty::Queue(_) => QU,
        Ty::Deque(_) => DQ,
        Ty::List(_) => LS,
        Ty::PriorityQueue(..) => PQ,
        Ty::Function(..) => FN,
        Ty::Iter { .. } => IT,
        _ => return None,
    })
}

/// Members whose receiver matches the static type (free functions are not included).
pub fn members_of(ty: &Ty) -> Vec<&'static Member> {
    match recv_of(ty) {
        Some(r) => MEMBERS.iter().filter(|m| m.recv == r).collect(),
        None => Vec::new(),
    }
}

/// Names a free-function row is looked up by inside example text (`std::numeric_limits<T>::max` is found
/// as `std::numeric_limits`).
fn lookup_key(name: &str) -> &str {
    name.split('<').next().unwrap_or(name)
}

/// Headers an example needs: the row's own header plus those of every type and free name it spells out.
pub fn example_headers(m: &Member) -> Vec<&'static str> {
    const TYPES: &[(&str, &str)] = &[
        ("std::vector", "<vector>"),
        ("std::string_view", "<string_view>"),
        ("std::string", "<string>"),
        ("std::array", "<array>"),
        ("std::pair", "<utility>"),
        ("std::tuple", "<tuple>"),
        ("std::optional", "<optional>"),
        ("std::unordered_map", "<unordered_map>"),
        ("std::map", "<map>"),
        ("std::unordered_set", "<unordered_set>"),
        ("std::set", "<set>"),
        ("std::stack", "<stack>"),
        ("std::queue", "<queue>"),
        ("std::priority_queue", "<queue>"),
        ("std::deque", "<deque>"),
        ("std::list", "<list>"),
    ];
    let mut out: Vec<&'static str> = Vec::new();
    let mut add = |h: &'static str| {
        if !h.is_empty() && !out.contains(&h) {
            out.push(h);
        }
    };
    add(m.header);
    for (needle, h) in TYPES {
        if m.example.contains(needle) {
            add(h);
        }
    }
    for f in MEMBERS.iter().filter(|f| f.recv == FR) {
        if m.example.contains(lookup_key(f.name)) {
            add(f.header);
        }
    }
    out.sort_unstable();
    out
}

/// A complete translation unit for the example of `m`: its headers and a function body holding the example.
pub fn example_program(m: &Member) -> String {
    let mut s = String::new();
    for h in example_headers(m) {
        s.push_str("#include ");
        s.push_str(h);
        s.push('\n');
    }
    s.push_str("void example() {\n    ");
    s.push_str(m.example);
    s.push_str("\n}\n");
    s
}

/// Escape text for a markdown table cell.
fn cell(s: &str) -> String {
    s.replace('|', "\\|")
}

/// The library table as markdown (the "Library" section of `docs/CPP_SUBSET.md`).
pub fn markdown_table() -> String {
    let mut s = String::new();
    s.push_str("| Receiver | Name | Header | Signature | Cost | Card |\n");
    s.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for m in MEMBERS {
        let header = if m.header.is_empty() {
            "(container's)".to_string()
        } else {
            format!("`{}`", cell(m.header))
        };
        s.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` | {} | {} |\n",
            cell(m.recv),
            cell(m.name),
            header,
            cell(m.sig),
            cell(m.cost),
            if m.card { "yes" } else { "no" }
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(recv: &str, name: &str) -> bool {
        MEMBERS.iter().any(|m| m.recv == recv && m.name == name)
    }

    #[test]
    fn every_member_has_an_example() {
        assert!(MEMBERS.len() > 250);
        for m in MEMBERS {
            assert!(!m.example.trim().is_empty(), "{} {}", m.recv, m.name);
            assert!(!m.name.is_empty() && !m.sig.is_empty() && !m.cost.is_empty());
            assert!(m.recv == IT || m.header.starts_with('<'), "{}", m.name);
            assert!(m.example.trim_end().ends_with([';', '}']), "{}", m.name);
            assert!(!m.example.contains('\u{2014}') && !m.sig.contains('\u{2014}'));
        }
    }

    #[test]
    fn rows_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for m in MEMBERS {
            assert!(
                seen.insert((m.recv, m.name, m.sig)),
                "{} {}",
                m.recv,
                m.name
            );
        }
    }

    #[test]
    fn vector_members() {
        let names: Vec<&str> = members_of(&Ty::vector(Ty::Int))
            .iter()
            .map(|m| m.name)
            .collect();
        assert!(names.contains(&"push_back"));
        assert!(names.contains(&"size"));
        assert!(!names.contains(&"push_front"));
        assert!(!names.contains(&"std::sort"));
        assert!(members_of(&Ty::Int).is_empty());
    }

    #[test]
    fn spec_surface_is_covered() {
        for n in [
            "size",
            "empty",
            "push_back",
            "pop_back",
            "back",
            "front",
            "clear",
            "insert",
            "erase",
            "resize",
            "assign",
            "begin",
            "end",
            "rbegin",
            "rend",
            "cbegin",
            "cend",
            "emplace_back",
            "operator[]",
            "at",
            "data",
            "reserve",
            "shrink_to_fit",
            "swap",
            "ctor",
            "operator==",
            "operator!=",
            "operator<",
        ] {
            assert!(has(V, n), "vector {n}");
        }
        for n in [
            "substr",
            "find",
            "npos",
            "append",
            "compare",
            "push_back",
            "operator+",
        ] {
            assert!(has(S, n), "string {n}");
        }
        for n in [
            "size",
            "operator[]",
            "at",
            "front",
            "back",
            "fill",
            "begin",
            "end",
        ] {
            assert!(has(AR, n), "array {n}");
        }
        assert!(
            has(PR, "first") && has(PR, "second") && has(OP, "value_or") && has(OP, "has_value")
        );
        for r in [UM, MP, US, ST] {
            for n in [
                "count", "find", "insert", "erase", "emplace", "begin", "end", "size", "empty",
                "clear",
            ] {
                assert!(has(r, n), "{r} {n}");
            }
        }
        assert!(
            has(UM, "operator[]")
                && has(MP, "at")
                && has(ST, "lower_bound")
                && has(MP, "upper_bound")
        );
        for n in ["push", "pop", "top", "size", "empty"] {
            assert!(has(SK, n) && has(PQ, n), "{n}");
        }
        for n in ["push", "pop", "front", "back", "size", "empty"] {
            assert!(has(QU, n), "queue {n}");
        }
        for n in [
            "push_front",
            "pop_front",
            "push_back",
            "pop_back",
            "front",
            "back",
            "operator[]",
            "at",
            "size",
            "empty",
            "begin",
            "end",
            "insert",
            "erase",
            "clear",
        ] {
            assert!(has(DQ, n), "deque {n}");
        }
        assert!(has(LS, "push_front") && has(LS, "erase"));
        for n in [
            "sort",
            "stable_sort",
            "reverse",
            "min",
            "max",
            "accumulate",
            "find",
            "find_if",
            "count",
            "count_if",
            "binary_search",
            "lower_bound",
            "upper_bound",
            "unique",
            "iota",
            "min_element",
            "max_element",
            "fill",
            "next_permutation",
            "prev_permutation",
            "any_of",
            "all_of",
            "none_of",
            "for_each",
            "remove",
            "remove_if",
            "rotate",
            "copy",
            "equal",
            "lexicographical_compare",
            "partial_sum",
            "distance",
            "advance",
            "next",
            "prev",
            "make_pair",
            "get",
            "tie",
            "abs",
            "to_string",
            "stoi",
            "stoll",
            "isdigit",
            "tolower",
            "toupper",
            "swap",
            "make_tuple",
            "function",
            "greater",
            "less",
            "numeric_limits<T>::max",
        ] {
            assert!(has(FR, &format!("std::{n}")), "std::{n}");
        }
        for n in [
            "INT_MAX",
            "INT_MIN",
            "LLONG_MAX",
            "LLONG_MIN",
            "UINT_MAX",
            "ULLONG_MAX",
            "std::nullopt",
        ] {
            assert!(has(FR, n), "{n}");
        }
    }

    #[test]
    fn markdown_has_a_row_per_member() {
        let t = markdown_table();
        assert_eq!(t.lines().count(), MEMBERS.len() + 2);
        assert!(t.contains("| `std::vector<T>` | `push_back` | `<vector>` |"));
    }

    #[test]
    fn example_programs_name_their_headers() {
        let m = MEMBERS
            .iter()
            .find(|m| m.name == "std::sort")
            .expect("sort row");
        let p = example_program(m);
        assert!(p.contains("#include <algorithm>") && p.contains("#include <vector>"));
    }

    /// Compile every example with the host `g++` (one translation unit per distinct include set).
    #[cfg(feature = "compile-tests")]
    #[test]
    fn examples_compile_with_gpp() {
        use std::collections::BTreeMap;
        use std::process::Command;
        if Command::new("g++").arg("--version").output().is_err() {
            return;
        }
        let mut groups: BTreeMap<String, Vec<&Member>> = BTreeMap::new();
        for m in MEMBERS {
            let incs: String = example_headers(m)
                .iter()
                .map(|h| format!("#include {h}\n"))
                .collect();
            groups.entry(incs).or_default().push(m);
        }
        let dir = std::env::temp_dir().join(format!("algoclerk-stdlib-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut failures = Vec::new();
        for (i, (incs, ms)) in groups.iter().enumerate() {
            let mut src = incs.clone();
            for (j, m) in ms.iter().enumerate() {
                src.push_str(&format!("void example_{j}() {{\n    {}\n}}\n", m.example));
            }
            let path = dir.join(format!("g{i}.cpp"));
            std::fs::write(&path, src).expect("write source");
            let out = Command::new("g++")
                .args(["-std=c++17", "-fsyntax-only"])
                .arg(&path)
                .output()
                .expect("run g++");
            if !out.status.success() {
                failures.push(String::from_utf8_lossy(&out.stderr).into_owned());
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
