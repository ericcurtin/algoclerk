//// vec_basic
#include <vector>
std::vector<int> test() {
    std::vector<int> v;
    for (int i = 0; i < 10; ++i) v.push_back(i * 3);
    v.pop_back();
    v[2] = 100;
    v.at(3) = 200;
    v.back() += 1;
    v.front() -= 1;
    std::vector<int> r = v;
    r.push_back(static_cast<int>(v.size()));
    r.push_back(v.empty());
    r.push_back(v.front());
    r.push_back(v.back());
    return r;
}
//// vec_ctors
#include <vector>
std::vector<int> test() {
    std::vector<int> a(5);
    std::vector<int> b(4, 7);
    std::vector<int> c = {1, 2, 3};
    std::vector<int> d{9, 8};
    std::vector<int> e(c.begin(), c.end());
    std::vector<int> f(b.begin() + 1, b.begin() + 3);
    std::vector<int> g(c);
    std::vector<int> h = {};
    std::vector<int> r;
    for (auto* v : {&a, &b, &c, &d, &e, &f, &g, &h}) {
        r.push_back(static_cast<int>(v->size()));
        for (int x : *v) r.push_back(x);
        r.push_back(-1);
    }
    return r;
}
//// vec_2d
#include <vector>
std::vector<std::vector<int>> test() {
    int n = 4, m = 5;
    std::vector<std::vector<int>> g(n, std::vector<int>(m, 0));
    for (int i = 0; i < n; ++i)
        for (int j = 0; j < m; ++j) g[i][j] = i * j + (i > 0 ? g[i - 1][j] : 0);
    g[2][3] += 100;
    g.push_back({1, 2, 3});
    g.emplace_back(2, 9);
    g[0].push_back(77);
    return g;
}
//// vec_2d_ragged_dp
#include <vector>
std::vector<long long> test() {
    int n = 30;
    std::vector<std::vector<long long>> tri(n);
    for (int i = 0; i < n; ++i) {
        tri[i].assign(i + 1, 1);
        for (int j = 1; j < i; ++j) tri[i][j] = tri[i - 1][j - 1] + tri[i - 1][j];
    }
    return tri[n - 1];
}
//// vec_insert_erase
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3, 4, 5};
    std::vector<int> r;
    v.insert(v.begin() + 2, 99);
    r.insert(r.end(), v.begin(), v.end());
    r.push_back(-1);
    v.insert(v.begin(), 3, 7);
    r.insert(r.end(), v.begin(), v.end());
    r.push_back(-1);
    v.erase(v.begin());
    v.erase(v.begin() + 1, v.begin() + 4);
    r.insert(r.end(), v.begin(), v.end());
    r.push_back(-1);
    std::vector<int> w = {10, 20};
    v.insert(v.end(), w.begin(), w.end());
    v.insert(v.begin() + 1, w.begin(), w.begin() + 1);
    v.insert(v.end(), {5, 6});
    r.insert(r.end(), v.begin(), v.end());
    return r;
}
//// vec_resize_assign
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3};
    std::vector<int> r;
    v.resize(5);
    r.insert(r.end(), v.begin(), v.end());
    v.resize(2);
    r.insert(r.end(), v.begin(), v.end());
    v.resize(4, 9);
    r.insert(r.end(), v.begin(), v.end());
    v.assign(3, 4);
    r.insert(r.end(), v.begin(), v.end());
    std::vector<int> w = {5, 6, 7, 8};
    v.assign(w.begin() + 1, w.end());
    r.insert(r.end(), v.begin(), v.end());
    v.clear();
    r.push_back(static_cast<int>(v.size()));
    v.assign({1, 2});
    r.push_back(static_cast<int>(v.size()));
    return r;
}
//// vec_swap_copy_assign
#include <vector>
std::vector<int> test() {
    std::vector<int> a = {1, 2, 3}, b = {9};
    a.swap(b);
    std::vector<int> c = a;
    c.push_back(5);
    b = c;
    b[0] = 100;
    std::vector<int> r;
    for (auto* v : {&a, &b, &c}) {
        for (int x : *v) r.push_back(x);
        r.push_back(-1);
    }
    std::swap(a, c);
    r.push_back(static_cast<int>(a.size()));
    r.push_back(static_cast<int>(c.size()));
    return r;
}
//// vec_compare
#include <vector>
std::vector<int> test() {
    std::vector<int> a = {1, 2, 3}, b = {1, 2, 4}, c = {1, 2}, d = {1, 2, 3};
    return {a == d, a != b, a < b, c < a, a > c, a <= d, b >= a, a == c};
}
//// vec_of_pairs
#include <utility>
#include <vector>
std::vector<std::pair<int, int>> test() {
    std::vector<std::pair<int, int>> v;
    for (int i = 0; i < 5; ++i) v.push_back({i, i * i});
    v.push_back(std::make_pair(7, 8));
    v.emplace_back(9, 10);
    v[0].second = 99;
    v[1] = {5, 5};
    return v;
}
//// vec_of_strings
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::vector<std::string> v = {"b", "a"};
    v.push_back("c");
    v.emplace_back(3, 'z');
    v[0] += "x";
    v.insert(v.begin(), "first");
    std::string joined;
    for (const auto& s : v) joined += s + ",";
    v.push_back(joined);
    return v;
}
//// vec_range_for_modify
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3, 4};
    for (int& x : v) x *= 2;
    for (auto& x : v) x += 1;
    for (int x : v) x = 0;
    int total = 0;
    for (const auto& x : v) total += x;
    v.push_back(total);
    return v;
}
//// vec_2d_range_for
#include <vector>
int test() {
    std::vector<std::vector<int>> g = {{1, 2}, {3}, {}, {4, 5, 6}};
    int s = 0;
    for (const auto& row : g)
        for (int x : row) s = s * 2 + x;
    for (auto& row : g) row.push_back(0);
    for (auto row : g) row.clear();
    int c = 0;
    for (const auto& row : g) c += static_cast<int>(row.size());
    return s * 100 + c;
}
//// vec_erase_idioms
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
    v.erase(std::remove_if(v.begin(), v.end(), [](int x) { return x % 3 == 0; }), v.end());
    std::vector<int> r = v;
    v.erase(std::remove(v.begin(), v.end(), 5), v.end());
    r.push_back(-1);
    r.insert(r.end(), v.begin(), v.end());
    for (auto it = v.begin(); it != v.end();) {
        if (*it % 2 == 0) it = v.erase(it);
        else ++it;
    }
    r.push_back(-1);
    r.insert(r.end(), v.begin(), v.end());
    return r;
}
//// vec_iter_arith
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {10, 20, 30, 40, 50};
    auto it = v.begin();
    it += 2;
    int a = *it;
    ++it;
    int b = *it;
    it--;
    int c = *(it - 1);
    auto e = v.end();
    int d = static_cast<int>(e - it);
    int f = static_cast<int>(it - v.begin());
    int g = *(v.begin() + 4);
    int h = *(v.end() - 1);
    return {a, b, c, d, f, g, h, it < e, it == v.begin() + 2, it != e};
}
//// vec_reverse_iter
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3, 4};
    std::vector<int> r;
    for (auto it = v.rbegin(); it != v.rend(); ++it) r.push_back(*it);
    for (auto it = v.crbegin(); it != v.crend(); ++it) r.push_back(*it * 10);
    return r;
}
//// vec_size_loops
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {3, 1, 4, 1, 5, 9, 2, 6};
    std::vector<int> r;
    for (size_t i = 0; i < v.size(); ++i) r.push_back(v[i] * static_cast<int>(i));
    for (int i = static_cast<int>(v.size()) - 1; i >= 0; i -= 3) r.push_back(v[i]);
    for (size_t i = v.size(); i-- > 0;) r.push_back(v[i]);
    return r;
}
//// vec_nested_3d
#include <vector>
int test() {
    std::vector<std::vector<std::vector<int>>> c(3, std::vector<std::vector<int>>(3, std::vector<int>(3, 1)));
    c[1][1][1] = 10;
    int s = 0;
    for (auto& a : c)
        for (auto& b : a)
            for (int x : b) s += x;
    return s;
}
//// vec_prefix_sums
#include <vector>
std::vector<long long> test() {
    std::vector<int> a = {5, -2, 7, 1, 0, 3};
    std::vector<long long> p(a.size() + 1, 0);
    for (size_t i = 0; i < a.size(); ++i) p[i + 1] = p[i] + a[i];
    return p;
}
//// vec_bool_like_flags
#include <vector>
std::vector<int> test() {
    int n = 50;
    std::vector<char> sieve(n + 1, 1);
    sieve[0] = sieve[1] = 0;
    for (int i = 2; i * i <= n; ++i)
        if (sieve[i])
            for (int j = i * i; j <= n; j += i) sieve[j] = 0;
    std::vector<int> primes;
    for (int i = 0; i <= n; ++i)
        if (sieve[i]) primes.push_back(i);
    return primes;
}
//// vec_front_back_refs
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3};
    int& f = v.front();
    int& b = v.back();
    f = 10;
    b = 30;
    const int& m = v[1];
    int copy = m;
    v[1] = 20;
    return {v[0], v[1], v[2], copy, m};
}
//// vec_array_std
#include <array>
#include <vector>
std::vector<int> test() {
    std::array<int, 5> a = {5, 4, 3, 2, 1};
    std::array<int, 3> z{};
    a[0] = 50;
    a.at(1) = 40;
    a.fill(0);
    a[2] = 7;
    std::array<int, 4> b = {1, 2};
    std::vector<int> r(a.begin(), a.end());
    r.push_back(static_cast<int>(z.size()));
    r.push_back(z[1]);
    for (int x : b) r.push_back(x);
    r.push_back(a.front());
    r.push_back(a.back());
    return r;
}
//// vec_c_array
#include <vector>
std::vector<int> test() {
    int a[5] = {1, 2, 3};
    int b[4] = {};
    int c[3];
    for (int i = 0; i < 3; ++i) c[i] = i * i;
    int total = 0;
    for (int x : a) total += x;
    std::vector<int> r = {total, b[0], b[3], c[2]};
    int cnt[26] = {0};
    for (char ch : {'a', 'b', 'a', 'z'}) cnt[ch - 'a']++;
    r.push_back(cnt[0]);
    r.push_back(cnt[25]);
    return r;
}
//// vec_vector_of_vectors_sort
#include <algorithm>
#include <vector>
std::vector<std::vector<int>> test() {
    std::vector<std::vector<int>> v = {{3, 1}, {1, 2, 3}, {1, 2}, {}, {2}};
    std::sort(v.begin(), v.end());
    return v;
}
//// vec_const_ref_param_big
#include <vector>
int work(const std::vector<std::vector<int>>& g, int i, int j) { return g[i][j] + g[j][i]; }
int test() {
    std::vector<std::vector<int>> g(10, std::vector<int>(10));
    for (int i = 0; i < 10; ++i)
        for (int j = 0; j < 10; ++j) g[i][j] = i * 10 + j;
    int t = 0;
    for (int i = 0; i < 10; ++i) t += work(g, i, 9 - i);
    return t;
}
