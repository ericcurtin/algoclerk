// SPDX-License-Identifier: Apache-2.0
#![allow(missing_docs)]
//! Text templates for the C prelude: the container "classes" instantiated per element type.
//! `$S` is the struct name, `$E` the element C type, `$P` the element operation prefix
//! (`$P_eq`, `$P_cmp`, `$P_hash`, `$P_clone`), `$K`/`$V` key and value types for maps.

pub const BASE: &str = r#"#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

/* ---- scalar helpers: equality, ordering, hashing and copying for the built-in types ---- */
static inline bool int_eq(long long a, long long b) { return a == b; }
static inline int int_cmp(long long a, long long b) { return (a > b) - (a < b); }
static inline size_t int_hash(long long a) { return (size_t)((unsigned long long)a * 0x9E3779B97F4A7C15ULL >> 7); }
static inline long long int_clone(long long a) { return a; }
static inline long long int_min(long long a, long long b) { return a < b ? a : b; }
static inline long long int_max(long long a, long long b) { return a > b ? a : b; }
static inline bool bool_eq(bool a, bool b) { return a == b; }
static inline int bool_cmp(bool a, bool b) { return (int)a - (int)b; }
static inline size_t bool_hash(bool a) { return (size_t)a; }
static inline bool bool_clone(bool a) { return a; }
static inline bool char_eq(char a, char b) { return a == b; }
static inline int char_cmp(char a, char b) { return (int)(unsigned char)a - (int)(unsigned char)b; }
static inline size_t char_hash(char a) { return (size_t)(unsigned char)a * 0x9E3779B1u; }
static inline char char_clone(char a) { return a; }
static inline char char_min(char a, char b) { return char_cmp(a, b) <= 0 ? a : b; }
static inline char char_max(char a, char b) { return char_cmp(a, b) >= 0 ? a : b; }
static inline size_t hash_mix(size_t h, size_t v) { return h ^ (v + 0x9e3779b97f4a7c15ULL + (h << 6) + (h >> 2)); }
"#;

pub const LIST_NODE: &str = r#"
typedef struct ListNode { long long val; struct ListNode *next; } ListNode;
static inline ListNode *ListNode_new(long long val, ListNode *next) { ListNode *n = (ListNode *)malloc(sizeof *n); n->val = val; n->next = next; return n; }
static inline bool lnode_eq(ListNode *a, ListNode *b) { return a == b; }
static inline ListNode *lnode_clone(ListNode *a) { return a; }
"#;

pub const TREE_NODE: &str = r#"
typedef struct TreeNode { long long val; struct TreeNode *left, *right; } TreeNode;
static inline TreeNode *TreeNode_new(long long val, TreeNode *left, TreeNode *right) { TreeNode *n = (TreeNode *)malloc(sizeof *n); n->val = val; n->left = left; n->right = right; return n; }
static inline bool tnode_eq(TreeNode *a, TreeNode *b) { return a == b; }
static inline TreeNode *tnode_clone(TreeNode *a) { return a; }
"#;

pub const VEC: &str = r#"
typedef struct { $E *data; size_t len, cap; } $S;
static inline $S $S_new(void) { $S v = {NULL, 0, 0}; return v; }
static inline void $S_push($S *v, $E x) {
    if (v->len == v->cap) { v->cap = v->cap ? v->cap * 2 : 4; v->data = ($E *)realloc(v->data, v->cap * sizeof($E)); }
    v->data[v->len++] = x;
}
static inline $E $S_pop($S *v) { return v->data[--v->len]; }
static inline $E $S_peek($S v) { return v.data[v.len - 1]; }
static inline $S $S_from(const $E *items, size_t n) { $S v = $S_new(); for (size_t i = 0; i < n; i++) $S_push(&v, items[i]); return v; }
static inline $S $S_repeat($E x, long long n) { $S v = $S_new(); for (long long i = 0; i < n; i++) $S_push(&v, $P_clone(x)); return v; }
static inline void $S_insert($S *v, long long i, $E x) {
    $S_push(v, x);
    memmove(v->data + i + 1, v->data + i, (v->len - 1 - (size_t)i) * sizeof($E));
    v->data[i] = x;
}
static inline $E $S_remove($S *v, long long i) {
    $E x = v->data[i];
    memmove(v->data + i, v->data + i + 1, (v->len - 1 - (size_t)i) * sizeof($E));
    v->len--;
    return x;
}
static inline $S $S_clone($S v) { $S c = $S_new(); for (size_t i = 0; i < v.len; i++) $S_push(&c, $P_clone(v.data[i])); return c; }
static inline bool $S_eq($S a, $S b) {
    if (a.len != b.len) return false;
    for (size_t i = 0; i < a.len; i++) if (!$P_eq(a.data[i], b.data[i])) return false;
    return true;
}
static inline long long $S_index_of($S v, $E x) { for (size_t i = 0; i < v.len; i++) if ($P_eq(v.data[i], x)) return (long long)i; return -1; }
static inline bool $S_contains($S v, $E x) { return $S_index_of(v, x) >= 0; }
static inline long long $S_count($S v, $E x) { long long n = 0; for (size_t i = 0; i < v.len; i++) if ($P_eq(v.data[i], x)) n++; return n; }
static inline void $S_reverse($S *v) { for (size_t i = 0, j = v->len; i + 1 < j; i++, j--) { $E t = v->data[i]; v->data[i] = v->data[j - 1]; v->data[j - 1] = t; } }
static inline $S $S_reversed($S v) { $S c = $S_clone(v); $S_reverse(&c); return c; }
static inline void $S_swap($S *v, long long i, long long j) { $E t = v->data[i]; v->data[i] = v->data[j]; v->data[j] = t; }
static inline void $S_clear($S *v) { v->len = 0; }
static inline $S $S_slice($S v, long long lo, long long hi) { return $S_from(v.data + lo, (size_t)(hi - lo)); }
static inline void $S_extend($S *a, $S b) { for (size_t i = 0; i < b.len; i++) $S_push(a, $P_clone(b.data[i])); }
static inline $S $S_concat($S a, $S b) { $S c = $S_clone(a); $S_extend(&c, b); return c; }
"#;

pub const VEC_ORD: &str = r#"
static inline int $S_cmp($S a, $S b) {
    size_t n = a.len < b.len ? a.len : b.len;
    for (size_t i = 0; i < n; i++) { int c = $P_cmp(a.data[i], b.data[i]); if (c) return c; }
    return (a.len > b.len) - (a.len < b.len);
}
static inline $S $S_min($S a, $S b) { return $S_cmp(a, b) <= 0 ? a : b; }
static inline $S $S_max($S a, $S b) { return $S_cmp(a, b) >= 0 ? a : b; }
static int $S_qcmp(const void *a, const void *b) { return $P_cmp(*(const $E *)a, *(const $E *)b); }
static inline void $S_sort($S *v) { if (v->len > 1) qsort(v->data, v->len, sizeof($E), $S_qcmp); }
static inline $S $S_sorted($S v) { $S c = $S_clone(v); $S_sort(&c); return c; }
static inline $E $S_vmin($S v) { $E m = v.data[0]; for (size_t i = 1; i < v.len; i++) if ($P_cmp(v.data[i], m) < 0) m = v.data[i]; return m; }
static inline $E $S_vmax($S v) { $E m = v.data[0]; for (size_t i = 1; i < v.len; i++) if ($P_cmp(v.data[i], m) > 0) m = v.data[i]; return m; }
"#;

pub const VEC_HASH: &str = r#"
static inline size_t $S_hash($S v) { size_t h = v.len; for (size_t i = 0; i < v.len; i++) h = hash_mix(h, $P_hash(v.data[i])); return h; }
"#;

pub const VEC_INT: &str = r#"
static inline long long $S_sum($S v) { long long t = 0; for (size_t i = 0; i < v.len; i++) t += v.data[i]; return t; }
"#;

pub const SET: &str = r#"
typedef struct { $E *keys; unsigned char *st; size_t cap, len, used; } $S;  /* st: 0 empty, 1 full, 2 deleted */
static inline $S $S_new(void) { $S s = {NULL, NULL, 0, 0, 0}; return s; }
static inline long long $S_slot(const $S *s, $E x) {
    if (!s->cap) return -1;
    size_t i = $P_hash(x) & (s->cap - 1);
    while (s->st[i]) { if (s->st[i] == 1 && $P_eq(s->keys[i], x)) return (long long)i; i = (i + 1) & (s->cap - 1); }
    return -1;
}
static inline bool $S_contains($S s, $E x) { return $S_slot(&s, x) >= 0; }
static inline void $S_add($S *s, $E x);
static inline void $S_rehash($S *s, size_t cap) {
    $S old = *s;
    s->keys = ($E *)calloc(cap, sizeof($E)); s->st = (unsigned char *)calloc(cap, 1); s->cap = cap; s->len = 0; s->used = 0;
    for (size_t i = 0; i < old.cap; i++) if (old.st[i] == 1) $S_add(s, old.keys[i]);
    free(old.keys); free(old.st);
}
static inline void $S_add($S *s, $E x) {
    if ((s->used + 1) * 2 > s->cap) $S_rehash(s, s->cap ? s->cap * 2 : 8);
    if ($S_slot(s, x) >= 0) return;
    size_t i = $P_hash(x) & (s->cap - 1);
    while (s->st[i] == 1) i = (i + 1) & (s->cap - 1);
    if (s->st[i] == 0) s->used++;
    s->keys[i] = x; s->st[i] = 1; s->len++;
}
static inline void $S_remove($S *s, $E x) { long long i = $S_slot(s, x); if (i >= 0) { s->st[i] = 2; s->len--; } }
static inline void $S_clear($S *s) { for (size_t i = 0; i < s->cap; i++) s->st[i] = 0; s->len = 0; s->used = 0; }
static inline $S $S_clone($S s) { $S c = $S_new(); for (size_t i = 0; i < s.cap; i++) if (s.st[i] == 1) $S_add(&c, $P_clone(s.keys[i])); return c; }
static inline bool $S_eq($S a, $S b) { if (a.len != b.len) return false; for (size_t i = 0; i < a.cap; i++) if (a.st[i] == 1 && !$S_contains(b, a.keys[i])) return false; return true; }
static inline $S $S_union($S a, $S b) { $S c = $S_clone(a); for (size_t i = 0; i < b.cap; i++) if (b.st[i] == 1) $S_add(&c, $P_clone(b.keys[i])); return c; }
static inline $S $S_intersection($S a, $S b) { $S c = $S_new(); for (size_t i = 0; i < a.cap; i++) if (a.st[i] == 1 && $S_contains(b, a.keys[i])) $S_add(&c, $P_clone(a.keys[i])); return c; }
static inline $S $S_difference($S a, $S b) { $S c = $S_new(); for (size_t i = 0; i < a.cap; i++) if (a.st[i] == 1 && !$S_contains(b, a.keys[i])) $S_add(&c, $P_clone(a.keys[i])); return c; }
"#;

pub const SET_SORTED: &str = r#"
static inline $V $S_sorted($S s) {
    $V v = $V_new();
    for (size_t i = 0; i < s.cap; i++) if (s.st[i] == 1) $V_push(&v, $P_clone(s.keys[i]));
    $V_sort(&v);
    return v;
}
"#;

pub const MAP: &str = r#"
typedef struct { $K *keys; $V *vals; unsigned char *st; size_t cap, len, used; } $S;  /* st: 0 empty, 1 full, 2 deleted */
static inline $S $S_new(void) { $S m = {NULL, NULL, NULL, 0, 0, 0}; return m; }
static inline long long $S_slot(const $S *m, $K k) {
    if (!m->cap) return -1;
    size_t i = $KP_hash(k) & (m->cap - 1);
    while (m->st[i]) { if (m->st[i] == 1 && $KP_eq(m->keys[i], k)) return (long long)i; i = (i + 1) & (m->cap - 1); }
    return -1;
}
static inline bool $S_contains($S m, $K k) { return $S_slot(&m, k) >= 0; }
static inline void $S_set($S *m, $K k, $V v);
static inline void $S_rehash($S *m, size_t cap) {
    $S old = *m;
    m->keys = ($K *)calloc(cap, sizeof($K)); m->vals = ($V *)calloc(cap, sizeof($V)); m->st = (unsigned char *)calloc(cap, 1);
    m->cap = cap; m->len = 0; m->used = 0;
    for (size_t i = 0; i < old.cap; i++) if (old.st[i] == 1) $S_set(m, old.keys[i], old.vals[i]);
    free(old.keys); free(old.vals); free(old.st);
}
static inline void $S_set($S *m, $K k, $V v) {
    long long at = $S_slot(m, k);
    if (at >= 0) { m->vals[at] = v; return; }
    if ((m->used + 1) * 2 > m->cap) $S_rehash(m, m->cap ? m->cap * 2 : 8);
    size_t i = $KP_hash(k) & (m->cap - 1);
    while (m->st[i] == 1) i = (i + 1) & (m->cap - 1);
    if (m->st[i] == 0) m->used++;
    m->keys[i] = k; m->vals[i] = v; m->st[i] = 1; m->len++;
}
static inline $V *$S_at($S *m, $K k) { return &m->vals[$S_slot(m, k)]; }
static inline $V $S_get_or($S m, $K k, $V d) { long long i = $S_slot(&m, k); return i >= 0 ? m.vals[i] : d; }
static inline void $S_remove($S *m, $K k) { long long i = $S_slot(m, k); if (i >= 0) { m->st[i] = 2; m->len--; } }
static inline void $S_clear($S *m) { for (size_t i = 0; i < m->cap; i++) m->st[i] = 0; m->len = 0; m->used = 0; }
static inline $S $S_clone($S m) { $S c = $S_new(); for (size_t i = 0; i < m.cap; i++) if (m.st[i] == 1) $S_set(&c, $KP_clone(m.keys[i]), $VP_clone(m.vals[i])); return c; }
static inline bool $S_eq($S a, $S b) {
    if (a.len != b.len) return false;
    for (size_t i = 0; i < a.cap; i++) if (a.st[i] == 1) { long long j = $S_slot(&b, a.keys[i]); if (j < 0 || !$VP_eq(a.vals[i], b.vals[j])) return false; }
    return true;
}
"#;

pub const MAP_KEYS: &str = r#"
static inline $KV $S_keys($S m) { $KV v = $KV_new(); for (size_t i = 0; i < m.cap; i++) if (m.st[i] == 1) $KV_push(&v, $KP_clone(m.keys[i])); $KV_sort(&v); return v; }
"#;

pub const MAP_VALUES: &str = r#"
static inline $VV $S_values($S m) { $KV ks = $S_keys(m); $VV v = $VV_new(); for (size_t i = 0; i < ks.len; i++) $VV_push(&v, $VP_clone(*$S_at(&m, ks.data[i]))); return v; }
"#;

pub const MAP_ITEMS: &str = r#"
static inline $IV $S_items($S m) { $KV ks = $S_keys(m); $IV v = $IV_new(); for (size_t i = 0; i < ks.len; i++) $IV_push(&v, $T_make(ks.data[i], $VP_clone(*$S_at(&m, ks.data[i])))); return v; }
"#;

pub const TUPLE: &str = r#"
typedef struct { $FIELDS } $S;
static inline $S $S_make($PARAMS) { $S t; $INIT return t; }
static inline $S $S_clone($S t) { $S c; $CLONE return c; }
static inline bool $S_eq($S a, $S b) { return $EQ; }
"#;

pub const TUPLE_ORD: &str = r#"
static inline int $S_cmp($S a, $S b) { int c; $CMP return 0; }
static inline $S $S_min($S a, $S b) { return $S_cmp(a, b) <= 0 ? a : b; }
static inline $S $S_max($S a, $S b) { return $S_cmp(a, b) >= 0 ? a : b; }
"#;

pub const TUPLE_HASH: &str = r#"
static inline size_t $S_hash($S t) { size_t h = 0; $HASH return h; }
"#;

pub const OPT: &str = r#"
typedef struct { bool has; $E v; } $S;
static inline $S $S_some($E x) { $S o = {true, x}; return o; }
static inline $S $S_none(void) { $S o; memset(&o, 0, sizeof o); return o; }
static inline $E $S_unwrap_or($S o, $E d) { return o.has ? o.v : d; }
static inline $S $S_clone($S o) { $S c = o; if (o.has) c.v = $P_clone(o.v); return c; }
static inline bool $S_eq($S a, $S b) { return a.has == b.has && (!a.has || $P_eq(a.v, b.v)); }
"#;

pub const DEQ: &str = r#"
typedef struct { $E *data; size_t head, len, cap; } $S;
static inline $S $S_new(void) { $S d = {NULL, 0, 0, 0}; return d; }
static inline void $S_grow($S *d) {
    size_t ncap = d->cap ? d->cap * 2 : 8;
    $E *nd = ($E *)malloc(ncap * sizeof($E));
    for (size_t i = 0; i < d->len; i++) nd[i] = d->data[(d->head + i) % d->cap];
    free(d->data); d->data = nd; d->head = 0; d->cap = ncap;
}
static inline void $S_push_back($S *d, $E x) { if (d->len == d->cap) $S_grow(d); d->data[(d->head + d->len) % d->cap] = x; d->len++; }
static inline void $S_push_front($S *d, $E x) { if (d->len == d->cap) $S_grow(d); d->head = (d->head + d->cap - 1) % d->cap; d->data[d->head] = x; d->len++; }
static inline $E $S_pop_front($S *d) { $E x = d->data[d->head]; d->head = (d->head + 1) % d->cap; d->len--; return x; }
static inline $E $S_pop_back($S *d) { d->len--; return d->data[(d->head + d->len) % d->cap]; }
static inline $E $S_front($S d) { return d.data[d.head]; }
static inline $E $S_back($S d) { return d.data[(d.head + d.len - 1) % d.cap]; }
static inline void $S_clear($S *d) { d->head = 0; d->len = 0; }
static inline $S $S_clone($S d) { $S c = $S_new(); for (size_t i = 0; i < d.len; i++) $S_push_back(&c, $P_clone(d.data[(d.head + i) % d.cap])); return c; }
"#;

pub const HEAP: &str = r#"
typedef struct { $E *data; size_t len, cap; } $S;  /* max-heap */
static inline $S $S_new(void) { $S h = {NULL, 0, 0}; return h; }
static inline void $S_push($S *h, $E x) {
    if (h->len == h->cap) { h->cap = h->cap ? h->cap * 2 : 8; h->data = ($E *)realloc(h->data, h->cap * sizeof($E)); }
    size_t i = h->len++;
    h->data[i] = x;
    while (i > 0 && $P_cmp(h->data[i], h->data[(i - 1) / 2]) > 0) { $E t = h->data[i]; h->data[i] = h->data[(i - 1) / 2]; h->data[(i - 1) / 2] = t; i = (i - 1) / 2; }
}
static inline $E $S_pop($S *h) {
    $E top = h->data[0];
    h->data[0] = h->data[--h->len];
    size_t i = 0;
    for (;;) {
        size_t l = 2 * i + 1, r = l + 1, m = i;
        if (l < h->len && $P_cmp(h->data[l], h->data[m]) > 0) m = l;
        if (r < h->len && $P_cmp(h->data[r], h->data[m]) > 0) m = r;
        if (m == i) break;
        $E t = h->data[i]; h->data[i] = h->data[m]; h->data[m] = t; i = m;
    }
    return top;
}
static inline $E $S_peek($S h) { return h.data[0]; }
static inline void $S_clear($S *h) { h->len = 0; }
static inline $S $S_clone($S h) { $S c = $S_new(); for (size_t i = 0; i < h.len; i++) $S_push(&c, $P_clone(h.data[i])); return c; }
"#;

pub const STR_HELPERS: &str = r#"
/* ---- strings are vectors of char with a few extra helpers ---- */
typedef Vec_char Str;
static inline Str str_from(const char *s, size_t n) { return Vec_char_from(s, n); }
static inline Str str_lit(const char *s) { return Vec_char_from(s, strlen(s)); }
static inline void str_push_str(Str *a, Str b) { Vec_char_extend(a, b); }
static inline Str str_concat(Str a, Str b) { return Vec_char_concat(a, b); }
static inline Str str_from_int(long long x) { char buf[32]; int n = snprintf(buf, sizeof buf, "%lld", x); return Vec_char_from(buf, (size_t)n); }
static inline Str str_from_char(char c) { return Vec_char_from(&c, 1); }
static inline long long str_to_int(Str s) { char *t = (char *)malloc(s.len + 1); memcpy(t, s.data, s.len); t[s.len] = 0; long long v = strtoll(t, NULL, 10); free(t); return v; }
static inline long long str_find(Str s, Str p) {
    if (p.len == 0) return 0;
    for (size_t i = 0; i + p.len <= s.len; i++) if (memcmp(s.data + i, p.data, p.len) == 0) return (long long)i;
    return -1;
}
static inline long long str_find_char(Str s, char c) { return Vec_char_index_of(s, c); }
static inline bool str_contains(Str s, Str p) { return str_find(s, p) >= 0; }
static inline bool str_starts_with(Str s, Str p) { return s.len >= p.len && memcmp(s.data, p.data, p.len) == 0; }
static inline bool str_ends_with(Str s, Str p) { return s.len >= p.len && memcmp(s.data + s.len - p.len, p.data, p.len) == 0; }
static inline Str str_upper(Str s) { Str c = Vec_char_clone(s); for (size_t i = 0; i < c.len; i++) c.data[i] = (char)toupper((unsigned char)c.data[i]); return c; }
static inline Str str_lower(Str s) { Str c = Vec_char_clone(s); for (size_t i = 0; i < c.len; i++) c.data[i] = (char)tolower((unsigned char)c.data[i]); return c; }
static inline Str str_trim(Str s) {
    size_t a = 0, b = s.len;
    while (a < b && isspace((unsigned char)s.data[a])) a++;
    while (b > a && isspace((unsigned char)s.data[b - 1])) b--;
    return Vec_char_from(s.data + a, b - a);
}
static inline Str str_replace(Str s, Str from, Str to) {
    Str out = Vec_char_new();
    for (size_t i = 0; i < s.len;) {
        if (from.len && i + from.len <= s.len && memcmp(s.data + i, from.data, from.len) == 0) { Vec_char_extend(&out, to); i += from.len; }
        else Vec_char_push(&out, s.data[i++]);
    }
    return out;
}
static inline Str str_repeat(Str s, long long k) { Str out = Vec_char_new(); for (long long i = 0; i < k; i++) Vec_char_extend(&out, s); return out; }
"#;

pub const STR_SPLIT_JOIN: &str = r#"
static inline Vec_str str_split(Str s, Str sep) {
    Vec_str out = Vec_str_new();
    size_t start = 0;
    for (size_t i = 0; sep.len && i + sep.len <= s.len;) {
        if (memcmp(s.data + i, sep.data, sep.len) == 0) { Vec_str_push(&out, Vec_char_from(s.data + start, i - start)); i += sep.len; start = i; }
        else i++;
    }
    Vec_str_push(&out, Vec_char_from(s.data + start, s.len - start));
    return out;
}
static inline Str str_join(Vec_str v, Str sep) {
    Str out = Vec_char_new();
    for (size_t i = 0; i < v.len; i++) { if (i) Vec_char_extend(&out, sep); Vec_char_extend(&out, v.data[i]); }
    return out;
}
"#;

pub const MAP_GET: &str = r#"
static inline $O $S_get($S m, $K k) { long long i = $S_slot(&m, k); return i >= 0 ? $O_some(m.vals[i]) : $O_none(); }
"#;
