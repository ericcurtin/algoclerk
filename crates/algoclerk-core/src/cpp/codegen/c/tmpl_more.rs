// SPDX-License-Identifier: Apache-2.0
//! C prelude text: deque (ring buffer), priority queue (binary heap), hash map / set (open addressing) and
//! ordered map / set (sorted arrays). Sets are maps whose value type is `char`.

pub const DEQ: &str = r#"
/* std::deque / std::queue / std::list: a ring buffer. */
#define DEQ(NAME, T, E) \
    typedef struct { T *data; size_t head, len, cap; } NAME; \
    static NAME NAME##_zero(void) { NAME q = {0}; return q; } \
    static T *NAME##_at(NAME *q, size_t i) { return &q->data[(q->head + i) % q->cap]; } \
    static void NAME##_grow(NAME *q) { \
        if (q->len < q->cap) return; \
        size_t cap = q->cap ? q->cap * 2 : 4; \
        T *data = malloc(cap * sizeof(T)); \
        for (size_t i = 0; i < q->len; ++i) data[i] = *NAME##_at(q, i); \
        free(q->data); \
        q->data = data; \
        q->head = 0; \
        q->cap = cap; \
    } \
    static void NAME##_push_back(NAME *q, T x) { NAME##_grow(q); q->len++; *NAME##_at(q, q->len - 1) = x; } \
    static void NAME##_push_front(NAME *q, T x) { \
        NAME##_grow(q); \
        q->head = (q->head + q->cap - 1) % q->cap; \
        q->len++; \
        q->data[q->head] = x; \
    } \
    static void NAME##_pop_front(NAME *q) { q->head = (q->head + 1) % q->cap; q->len--; } \
    static void NAME##_pop_back(NAME *q) { q->len--; } \
    static NAME NAME##_copy(NAME q) { \
        NAME r = {0}; \
        for (size_t i = 0; i < q.len; ++i) NAME##_push_back(&r, E##_copy(*NAME##_at(&q, i))); \
        return r; \
    } \
    static int NAME##_eq(NAME a, NAME b) { \
        if (a.len != b.len) return 0; \
        for (size_t i = 0; i < a.len; ++i) if (!E##_eq(*NAME##_at(&a, i), *NAME##_at(&b, i))) return 0; \
        return 1; \
    }
"#;

pub const HEAP: &str = r#"
/* std::priority_queue: a binary heap. FIRST(E, a, b) says that a must come out before b. */
#define MAX_FIRST(E, a, b) E##_lt(b, a)
#define MIN_FIRST(E, a, b) E##_lt(a, b)
#define HEAP(NAME, T, E, FIRST) \
    typedef struct { T *data; size_t len, cap; } NAME; \
    static NAME NAME##_zero(void) { NAME h = {0}; return h; } \
    static void NAME##_push(NAME *h, T x) { \
        if (h->len == h->cap) { \
            h->cap = h->cap ? h->cap * 2 : 4; \
            h->data = realloc(h->data, h->cap * sizeof(T)); \
        } \
        size_t i = h->len++; \
        while (i > 0 && FIRST(E, x, h->data[(i - 1) / 2])) { \
            h->data[i] = h->data[(i - 1) / 2]; \
            i = (i - 1) / 2; \
        } \
        h->data[i] = x; \
    } \
    static void NAME##_pop(NAME *h) { \
        T x = h->data[--h->len]; \
        size_t i = 0; \
        while (2 * i + 1 < h->len) { \
            size_t c = 2 * i + 1; \
            if (c + 1 < h->len && FIRST(E, h->data[c + 1], h->data[c])) c++; \
            if (!FIRST(E, h->data[c], x)) break; \
            h->data[i] = h->data[c]; \
            i = c; \
        } \
        if (h->len > 0) h->data[i] = x; \
    } \
    static NAME NAME##_copy(NAME h) { \
        NAME r = {0}; \
        for (size_t i = 0; i < h.len; ++i) NAME##_push(&r, E##_copy(h.data[i])); \
        return r; \
    }
"#;

pub const HMAP: &str = r#"
/* std::unordered_map / std::unordered_set: open addressing with linear probing. A slot state is
   0 (empty), 1 (full) or 2 (erased). Iterate with: for (i = 0; i < m.cap; ++i) if (m.st[i] == 1) ... */
#define HMAP(NAME, K, KE, V, VE) \
    typedef struct { K *keys; V *vals; unsigned char *st; size_t cap, len, used; } NAME; \
    static NAME NAME##_zero(void) { NAME m = {0}; return m; } \
    static size_t NAME##_find(const NAME *m, K k) { \
        if (m->cap == 0) return NOT_FOUND; \
        size_t i = KE##_hash(k) & (m->cap - 1); \
        while (m->st[i] != 0) { \
            if (m->st[i] == 1 && KE##_eq(m->keys[i], k)) return i; \
            i = (i + 1) & (m->cap - 1); \
        } \
        return NOT_FOUND; \
    } \
    static int NAME##_contains(const NAME *m, K k) { return NAME##_find(m, k) != NOT_FOUND; } \
    static void NAME##_rehash(NAME *m, size_t cap) { \
        NAME n = {0}; \
        n.cap = cap; \
        n.keys = malloc(cap * sizeof(K)); \
        n.vals = malloc(cap * sizeof(V)); \
        n.st = calloc(cap, 1); \
        for (size_t j = 0; j < m->cap; ++j) { \
            if (m->st[j] != 1) continue; \
            size_t i = KE##_hash(m->keys[j]) & (cap - 1); \
            while (n.st[i] != 0) i = (i + 1) & (cap - 1); \
            n.st[i] = 1; n.keys[i] = m->keys[j]; n.vals[i] = m->vals[j]; \
        } \
        n.len = n.used = m->len; \
        free(m->keys); free(m->vals); free(m->st); \
        *m = n; \
    } \
    /* the value of key k, inserting VE##_zero() first when it is missing (operator[]) */ \
    static V *NAME##_get(NAME *m, K k) { \
        size_t at = NAME##_find(m, k); \
        if (at != NOT_FOUND) return &m->vals[at]; \
        if ((m->used + 1) * 2 > m->cap) NAME##_rehash(m, m->cap ? m->cap * 2 : 8); \
        size_t i = KE##_hash(k) & (m->cap - 1); \
        while (m->st[i] == 1) i = (i + 1) & (m->cap - 1); \
        if (m->st[i] == 0) m->used++; \
        m->st[i] = 1; m->keys[i] = k; m->vals[i] = VE##_zero(); m->len++; \
        return &m->vals[i]; \
    } \
    /* insert unless present; 1 when inserted */ \
    static int NAME##_insert(NAME *m, K k, V v) { \
        if (NAME##_contains(m, k)) return 0; \
        *NAME##_get(m, k) = v; \
        return 1; \
    } \
    static int NAME##_erase(NAME *m, K k) { \
        size_t at = NAME##_find(m, k); \
        if (at == NOT_FOUND) return 0; \
        m->st[at] = 2; m->len--; \
        return 1; \
    } \
    /* a set from a list of keys */ \
    static NAME NAME##_from_keys(const K *keys, size_t n) { \
        NAME r = {0}; \
        for (size_t i = 0; i < n; ++i) *NAME##_get(&r, KE##_copy(keys[i])) = VE##_zero(); \
        return r; \
    } \
    static NAME NAME##_copy(NAME m) { \
        NAME r = {0}; \
        for (size_t i = 0; i < m.cap; ++i) if (m.st[i] == 1) *NAME##_get(&r, KE##_copy(m.keys[i])) = VE##_copy(m.vals[i]); \
        return r; \
    } \
    static int NAME##_eq(NAME a, NAME b) { \
        if (a.len != b.len) return 0; \
        for (size_t i = 0; i < a.cap; ++i) { \
            if (a.st[i] != 1) continue; \
            size_t j = NAME##_find(&b, a.keys[i]); \
            if (j == NOT_FOUND || !VE##_eq(a.vals[i], b.vals[j])) return 0; \
        } \
        return 1; \
    }
"#;

pub const SMAP: &str = r#"
/* std::map / std::set: keys kept sorted in an array. Iterate with: for (i = 0; i < m.len; ++i) ... */
#define SMAP(NAME, K, KE, V, VE) \
    typedef struct { K *keys; V *vals; size_t len, cap; } NAME; \
    static NAME NAME##_zero(void) { NAME m = {0}; return m; } \
    /* index of the first key that is not less than k */ \
    static size_t NAME##_lower(const NAME *m, K k) { \
        size_t lo = 0, hi = m->len; \
        while (lo < hi) { \
            size_t mid = lo + (hi - lo) / 2; \
            if (KE##_lt(m->keys[mid], k)) lo = mid + 1; else hi = mid; \
        } \
        return lo; \
    } \
    static size_t NAME##_find(const NAME *m, K k) { \
        size_t i = NAME##_lower(m, k); \
        return i < m->len && KE##_eq(m->keys[i], k) ? i : NOT_FOUND; \
    } \
    static int NAME##_contains(const NAME *m, K k) { return NAME##_find(m, k) != NOT_FOUND; } \
    static V *NAME##_get(NAME *m, K k) { \
        size_t i = NAME##_lower(m, k); \
        if (i < m->len && KE##_eq(m->keys[i], k)) return &m->vals[i]; \
        if (m->len == m->cap) { \
            m->cap = m->cap ? m->cap * 2 : 4; \
            m->keys = realloc(m->keys, m->cap * sizeof(K)); \
            m->vals = realloc(m->vals, m->cap * sizeof(V)); \
        } \
        memmove(m->keys + i + 1, m->keys + i, (m->len - i) * sizeof(K)); \
        memmove(m->vals + i + 1, m->vals + i, (m->len - i) * sizeof(V)); \
        m->keys[i] = k; m->vals[i] = VE##_zero(); m->len++; \
        return &m->vals[i]; \
    } \
    static int NAME##_insert(NAME *m, K k, V v) { \
        if (NAME##_contains(m, k)) return 0; \
        *NAME##_get(m, k) = v; \
        return 1; \
    } \
    static int NAME##_erase(NAME *m, K k) { \
        size_t i = NAME##_find(m, k); \
        if (i == NOT_FOUND) return 0; \
        memmove(m->keys + i, m->keys + i + 1, (m->len - i - 1) * sizeof(K)); \
        memmove(m->vals + i, m->vals + i + 1, (m->len - i - 1) * sizeof(V)); \
        m->len--; \
        return 1; \
    } \
    /* a set from a list of keys */ \
    static NAME NAME##_from_keys(const K *keys, size_t n) { \
        NAME r = {0}; \
        for (size_t i = 0; i < n; ++i) *NAME##_get(&r, KE##_copy(keys[i])) = VE##_zero(); \
        return r; \
    } \
    static NAME NAME##_copy(NAME m) { \
        NAME r = {0}; \
        for (size_t i = 0; i < m.len; ++i) *NAME##_get(&r, KE##_copy(m.keys[i])) = VE##_copy(m.vals[i]); \
        return r; \
    } \
    static int NAME##_eq(NAME a, NAME b) { \
        if (a.len != b.len) return 0; \
        for (size_t i = 0; i < a.len; ++i) \
            if (!KE##_eq(a.keys[i], b.keys[i]) || !VE##_eq(a.vals[i], b.vals[i])) return 0; \
        return 1; \
    }
"#;
