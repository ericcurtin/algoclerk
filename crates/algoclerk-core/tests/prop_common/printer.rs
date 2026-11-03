// SPDX-License-Identifier: Apache-2.0
//! The C++ side printer: prints a value in the brace notation of `cpp::value::render` (the same text as the
//! printer of `cpp_interp_diff.rs`, without the list and tree node overloads).

pub const PRINTER: &str = r#"
#include <cstdio>
#include <deque>
#include <list>
#include <map>
#include <optional>
#include <queue>
#include <set>
#include <string>
#include <tuple>
#include <type_traits>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>
#include <array>
namespace pr {
inline void esc(std::string& o, char ch, char quote) {
    unsigned char c = static_cast<unsigned char>(ch);
    if (c == '\n') o += "\\n";
    else if (c == '\t') o += "\\t";
    else if (c == '\r') o += "\\r";
    else if (c == 0) o += "\\0";
    else if (c == '\\') o += "\\\\";
    else if (ch == quote) { o += '\\'; o += ch; }
    else if (c >= 0x20 && c < 0x7f) o += ch;
    else { char buf[8]; std::snprintf(buf, sizeof buf, "\\x%02x", c); o += buf; }
}
template <class T> void p(std::string& o, const T& v);
template <class T> struct is_opt : std::false_type {};
template <class T> struct is_opt<std::optional<T>> : std::true_type {};
template <class T> struct is_pair : std::false_type {};
template <class A, class B> struct is_pair<std::pair<A, B>> : std::true_type {};
template <class T> struct is_tuple : std::false_type {};
template <class... A> struct is_tuple<std::tuple<A...>> : std::true_type {};
template <class T, class = void> struct has_mapped : std::false_type {};
template <class T> struct has_mapped<T, std::void_t<typename T::mapped_type>> : std::true_type {};
template <class T, class = void> struct iterable : std::false_type {};
template <class T> struct iterable<T, std::void_t<decltype(std::declval<const T&>().begin())>> : std::true_type {};
template <class T> void p(std::string& o, const T& v) {
    if constexpr (std::is_same_v<T, bool>) o += v ? "true" : "false";
    else if constexpr (std::is_same_v<T, char>) { o += '\''; esc(o, v, '\''); o += '\''; }
    else if constexpr (std::is_integral_v<T>) o += std::to_string(v);
    else if constexpr (std::is_same_v<T, std::string>) { o += '"'; for (char c : v) esc(o, c, '"'); o += '"'; }
    else if constexpr (is_opt<T>::value) {
        if (!v) o += "nullopt"; else { o += '{'; p(o, *v); o += '}'; }
    }
    else if constexpr (is_pair<T>::value) { o += '{'; p(o, v.first); o += ", "; p(o, v.second); o += '}'; }
    else if constexpr (is_tuple<T>::value) {
        o += '{';
        bool first = true;
        std::apply([&](const auto&... xs) { ((o += (first ? "" : ", "), first = false, p(o, xs)), ...); }, v);
        o += '}';
    }
    else if constexpr (has_mapped<T>::value) {
        o += '{';
        bool first = true;
        for (const auto& kv : v) {
            if (!first) o += ", ";
            first = false;
            o += '{'; p(o, kv.first); o += ", "; p(o, kv.second); o += '}';
        }
        o += '}';
    }
    else if constexpr (iterable<T>::value) {
        o += '{';
        bool first = true;
        for (const auto& x : v) {
            if (!first) o += ", ";
            first = false;
            p(o, x);
        }
        o += '}';
    }
    else o += "?";
}
}
"#;
