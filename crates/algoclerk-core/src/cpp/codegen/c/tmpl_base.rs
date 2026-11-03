// SPDX-License-Identifier: Apache-2.0
//! C prelude text: headers, scalar helpers, the string type and the vector template. Everything is plain C11,
//! written to be read: small functions, no tricks beyond token pasting in the container macros.

pub const HEADER: &str = r#"#include <ctype.h>
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-function"
#endif

typedef unsigned uint;
typedef unsigned char uchar;
typedef long long ll;
typedef unsigned long long ull;

#define NOT_FOUND ((size_t)-1)
#define SWAP(T, a, b) do { T tmp_ = (a); (a) = (b); (b) = tmp_; } while (0)

/* Scalars get the same helpers as containers so that containers can nest. */
#define SCALAR(T) \
    static T T##_copy(T x) { return x; } \
    static int T##_eq(T a, T b) { return a == b; } \
    static int T##_lt(T a, T b) { return a < b; } \
    static size_t T##_hash(T x) { return (size_t)x * 0x9E3779B97F4A7C15ull; } \
    static T T##_zero(void) { return (T)0; } \
    static T T##_min(T a, T b) { return b < a ? b : a; } \
    static T T##_max(T a, T b) { return a < b ? b : a; }
"#;

pub const STR: &str = r#"
/* std::string: bytes, always NUL terminated. */
typedef struct {
    char *data;
    size_t len, cap;
} Str;

static Str Str_n(const char *s, size_t n) {
    Str r;
    r.cap = n + 1;
    r.data = malloc(r.cap);
    memcpy(r.data, s, n);
    r.data[n] = 0;
    r.len = n;
    return r;
}
static Str Str_from(const char *s) { return Str_n(s, strlen(s)); }
static Str Str_copy(Str s) { return Str_n(s.data, s.len); }
static Str Str_zero(void) { return Str_from(""); }
static Str Str_repeat(size_t n, char c) {
    Str r = Str_n("", 0);
    for (size_t i = 0; i < n; ++i) {
        r.data = realloc(r.data, r.len + 2);
        r.data[r.len++] = c;
    }
    r.data[r.len] = 0;
    r.cap = r.len + 1;
    return r;
}
static void Str_cat_n(Str *s, const char *p, size_t n) {
    if (s->len + n + 1 > s->cap) {
        s->cap = (s->len + n + 1) * 2;
        s->data = realloc(s->data, s->cap);
    }
    memcpy(s->data + s->len, p, n);
    s->len += n;
    s->data[s->len] = 0;
}
static void Str_push(Str *s, char c) { Str_cat_n(s, &c, 1); }
static void Str_cat(Str *s, Str t) { Str_cat_n(s, t.data, t.len); }
static Str Str_add(Str a, Str b) {
    Str r = Str_copy(a);
    Str_cat(&r, b);
    return r;
}
static Str Str_substr(Str s, size_t pos, size_t len) {
    if (len > s.len - pos) len = s.len - pos;
    return Str_n(s.data + pos, len);
}
static size_t Str_find_n(Str s, const char *p, size_t n, size_t pos) {
    for (size_t i = pos; i + n <= s.len; ++i) {
        if (memcmp(s.data + i, p, n) == 0) return i;
    }
    return NOT_FOUND;
}
static size_t Str_find(Str s, Str t, size_t pos) { return Str_find_n(s, t.data, t.len, pos); }
static size_t Str_find_c(Str s, char c, size_t pos) { return Str_find_n(s, &c, 1, pos); }
static size_t Str_rfind_c(Str s, char c) {
    for (size_t i = s.len; i > 0; --i) {
        if (s.data[i - 1] == c) return i - 1;
    }
    return NOT_FOUND;
}
static void Str_erase(Str *s, size_t pos, size_t len) {
    if (len > s->len - pos) len = s->len - pos;
    memmove(s->data + pos, s->data + pos + len, s->len - pos - len + 1);
    s->len -= len;
}
static void Str_insert(Str *s, size_t pos, Str t) {
    Str tail = Str_n(s->data + pos, s->len - pos);
    s->len = pos;
    s->data[pos] = 0;
    Str_cat(s, t);
    Str_cat(s, tail);
}
static void Str_resize(Str *s, size_t n, char c) {
    if (n <= s->len) {
        s->len = n;
        s->data[n] = 0;
        return;
    }
    while (s->len < n) Str_push(s, c);
}
static int Str_cmp(Str a, Str b) {
    size_t n = a.len < b.len ? a.len : b.len;
    int r = memcmp(a.data, b.data, n);
    if (r != 0) return r;
    return a.len < b.len ? -1 : (a.len > b.len ? 1 : 0);
}
static int Str_eq(Str a, Str b) { return a.len == b.len && memcmp(a.data, b.data, a.len) == 0; }
static int Str_lt(Str a, Str b) { return Str_cmp(a, b) < 0; }
static int Str_eq_c(Str a, const char *b) { return strcmp(a.data, b) == 0; }
static int Str_cmp_c(Str a, const char *b) { return strcmp(a.data, b); }
static size_t Str_hash(Str s) {
    size_t h = 1469598103934665603ull;
    for (size_t i = 0; i < s.len; ++i) h = (h ^ (unsigned char)s.data[i]) * 1099511628211ull;
    return h;
}
static Str Str_from_ll(ll x) {
    char b[32];
    snprintf(b, sizeof b, "%lld", x);
    return Str_from(b);
}
static Str Str_from_ull(ull x) {
    char b[32];
    snprintf(b, sizeof b, "%llu", x);
    return Str_from(b);
}
"#;

pub const VEC: &str = r#"
/* std::vector<T>: a growable array. E names the element's helpers (E_copy, E_eq, E_lt, ...). */
#define VEC(NAME, T, E) \
    typedef struct { T *data; size_t len, cap; } NAME; \
    static NAME NAME##_zero(void) { NAME v = {0}; return v; } \
    static void NAME##_push(NAME *v, T x) { \
        if (v->len == v->cap) { \
            v->cap = v->cap ? v->cap * 2 : 4; \
            v->data = realloc(v->data, v->cap * sizeof(T)); \
        } \
        v->data[v->len++] = x; \
    } \
    static NAME NAME##_from(const T *a, size_t n) { \
        NAME v = {0}; \
        for (size_t i = 0; i < n; ++i) NAME##_push(&v, E##_copy(a[i])); \
        return v; \
    } \
    static NAME NAME##_filled(size_t n, T x) { \
        NAME v = {0}; \
        for (size_t i = 0; i < n; ++i) NAME##_push(&v, E##_copy(x)); \
        return v; \
    } \
    static NAME NAME##_copy(NAME v) { return NAME##_from(v.data, v.len); } \
    static void NAME##_pop(NAME *v) { v->len--; } \
    static void NAME##_insert(NAME *v, size_t pos, T x) { \
        NAME##_push(v, x); \
        memmove(v->data + pos + 1, v->data + pos, (v->len - 1 - pos) * sizeof(T)); \
        v->data[pos] = x; \
    } \
    static void NAME##_insert_n(NAME *v, size_t pos, const T *a, size_t n) { \
        for (size_t i = 0; i < n; ++i) NAME##_insert(v, pos + i, E##_copy(a[i])); \
    } \
    static void NAME##_erase(NAME *v, size_t pos, size_t n) { \
        memmove(v->data + pos, v->data + pos + n, (v->len - pos - n) * sizeof(T)); \
        v->len -= n; \
    } \
    static void NAME##_resize(NAME *v, size_t n, T fill) { \
        while (v->len > n) v->len--; \
        while (v->len < n) NAME##_push(v, E##_copy(fill)); \
    } \
    static int NAME##_eq(NAME a, NAME b) { \
        if (a.len != b.len) return 0; \
        for (size_t i = 0; i < a.len; ++i) if (!E##_eq(a.data[i], b.data[i])) return 0; \
        return 1; \
    } \
    static int NAME##_lt(NAME a, NAME b) { \
        for (size_t i = 0; i < a.len && i < b.len; ++i) { \
            if (E##_lt(a.data[i], b.data[i])) return 1; \
            if (E##_lt(b.data[i], a.data[i])) return 0; \
        } \
        return a.len < b.len; \
    } \
    static size_t NAME##_hash(NAME v) { \
        size_t h = 17; \
        for (size_t i = 0; i < v.len; ++i) h = h * 31 + E##_hash(v.data[i]); \
        return h; \
    } \
    static int NAME##_asc(const T *a, const T *b) { return E##_lt(*a, *b); } \
    static int NAME##_desc(const T *a, const T *b) { return E##_lt(*b, *a); } \
    /* stable merge sort of a[0..n) with the strict ordering lt */ \
    static void NAME##_sort_by(T *a, size_t n, int (*lt)(const T *, const T *)) { \
        if (n < 2) return; \
        T *tmp = malloc(n * sizeof(T)); \
        for (size_t w = 1; w < n; w *= 2) { \
            for (size_t lo = 0; lo < n; lo += 2 * w) { \
                size_t mid = lo + w < n ? lo + w : n, hi = lo + 2 * w < n ? lo + 2 * w : n; \
                size_t i = lo, j = mid, k = lo; \
                while (i < mid && j < hi) tmp[k++] = lt(&a[j], &a[i]) ? a[j++] : a[i++]; \
                while (i < mid) tmp[k++] = a[i++]; \
                while (j < hi) tmp[k++] = a[j++]; \
            } \
            memcpy(a, tmp, n * sizeof(T)); \
        } \
        free(tmp); \
    } \
    static void NAME##_sort(T *a, size_t n) { NAME##_sort_by(a, n, NAME##_asc); } \
    static void NAME##_reverse(T *a, size_t n) { \
        for (size_t i = 0; i < n / 2; ++i) SWAP(T, a[i], a[n - 1 - i]); \
    } \
    static size_t NAME##_lower(const T *a, size_t n, T x) { \
        size_t lo = 0, hi = n; \
        while (lo < hi) { \
            size_t mid = lo + (hi - lo) / 2; \
            if (E##_lt(a[mid], x)) lo = mid + 1; else hi = mid; \
        } \
        return lo; \
    } \
    static size_t NAME##_find(const T *a, size_t n, T x) { \
        for (size_t i = 0; i < n; ++i) if (E##_eq(a[i], x)) return i; \
        return NOT_FOUND; \
    } \
    static size_t NAME##_count(const T *a, size_t n, T x) { \
        size_t c = 0; \
        for (size_t i = 0; i < n; ++i) c += E##_eq(a[i], x); \
        return c; \
    } \
    static int NAME##_has(const T *a, size_t n, T x) { \
        size_t i = NAME##_lower(a, n, x); \
        return i < n && E##_eq(a[i], x); \
    } \
    static T *NAME##_min_at(T *a, size_t n) { \
        T *best = a; \
        for (size_t i = 1; i < n; ++i) if (E##_lt(a[i], *best)) best = &a[i]; \
        return best; \
    } \
    static T *NAME##_max_at(T *a, size_t n) { \
        T *best = a; \
        for (size_t i = 1; i < n; ++i) if (E##_lt(*best, a[i])) best = &a[i]; \
        return best; \
    } \
    static void NAME##_unique(NAME *v) { \
        size_t w = 0; \
        for (size_t i = 0; i < v->len; ++i) if (i == 0 || !E##_eq(v->data[i], v->data[w - 1])) v->data[w++] = v->data[i]; \
        v->len = w; \
    } \
    static size_t NAME##_upper(const T *a, size_t n, T x) { \
        size_t lo = 0, hi = n; \
        while (lo < hi) { \
            size_t mid = lo + (hi - lo) / 2; \
            if (!E##_lt(x, a[mid])) lo = mid + 1; else hi = mid; \
        } \
        return lo; \
    }
"#;
