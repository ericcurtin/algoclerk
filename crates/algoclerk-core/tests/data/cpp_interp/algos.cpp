//// algo_sort_variants
#include <algorithm>
#include <functional>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {5, 2, 8, 1, 9, 3, 7, 3};
    std::vector<int> r;
    std::sort(v.begin(), v.end());
    r.insert(r.end(), v.begin(), v.end());
    r.push_back(-1);
    std::sort(v.begin(), v.end(), std::greater<int>());
    r.insert(r.end(), v.begin(), v.end());
    r.push_back(-1);
    std::sort(v.begin(), v.end(), [](int a, int b) { return a % 3 < b % 3 || (a % 3 == b % 3 && a < b); });
    r.insert(r.end(), v.begin(), v.end());
    r.push_back(-1);
    std::sort(v.rbegin(), v.rend());
    r.insert(r.end(), v.begin(), v.end());
    r.push_back(-1);
    std::sort(v.begin() + 2, v.begin() + 6);
    r.insert(r.end(), v.begin(), v.end());
    return r;
}
//// algo_sort_pairs_structs
#include <algorithm>
#include <string>
#include <utility>
#include <vector>
std::vector<std::pair<int, std::string>> test() {
    std::vector<std::pair<int, std::string>> v = {{3, "c"}, {1, "z"}, {3, "a"}, {2, "m"}, {1, "b"}};
    std::sort(v.begin(), v.end());
    std::vector<std::pair<int, std::string>> w = v;
    std::sort(w.begin(), w.end(), [](const auto& a, const auto& b) {
        if (a.second != b.second) return a.second > b.second;
        return a.first < b.first;
    });
    v.insert(v.end(), w.begin(), w.end());
    return v;
}
//// algo_stable_sort
#include <algorithm>
#include <utility>
#include <vector>
std::vector<std::pair<int, int>> test() {
    std::vector<std::pair<int, int>> v;
    for (int i = 0; i < 40; ++i) v.push_back({(i * 7) % 5, i});
    std::stable_sort(v.begin(), v.end(), [](const std::pair<int, int>& a, const std::pair<int, int>& b) { return a.first < b.first; });
    return v;
}
//// algo_reverse_fill_iota
#include <algorithm>
#include <numeric>
#include <vector>
std::vector<int> test() {
    std::vector<int> v(8);
    std::iota(v.begin(), v.end(), 10);
    std::vector<int> r = v;
    std::reverse(v.begin() + 2, v.end() - 1);
    r.insert(r.end(), v.begin(), v.end());
    std::fill(v.begin(), v.begin() + 3, 0);
    r.insert(r.end(), v.begin(), v.end());
    std::fill(v.begin(), v.end(), -1);
    r.insert(r.end(), v.begin(), v.end());
    std::vector<long long> w(4);
    std::iota(w.begin(), w.end(), 5000000000LL);
    for (long long x : w) r.push_back(static_cast<int>(x % 1000));
    return r;
}
//// algo_accumulate
#include <numeric>
#include <string>
#include <vector>
std::vector<long long> test() {
    std::vector<int> v = {1, 2, 3, 4, 5};
    std::vector<long long> r;
    r.push_back(std::accumulate(v.begin(), v.end(), 0));
    r.push_back(std::accumulate(v.begin(), v.end(), 100));
    r.push_back(std::accumulate(v.begin(), v.end(), 1LL, [](long long a, int b) { return a * b; }));
    r.push_back(std::accumulate(v.begin(), v.end(), 0LL));
    std::vector<int> big(10, 2000000000);
    r.push_back(std::accumulate(big.begin(), big.end(), 0LL));
    std::vector<std::string> s = {"a", "b", "c"};
    r.push_back(std::accumulate(s.begin(), s.end(), std::string()).size());
    r.push_back(std::accumulate(v.begin() + 1, v.begin() + 3, 0));
    return r;
}
//// algo_find_count
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {4, 8, 15, 16, 23, 42, 8};
    std::vector<int> r;
    r.push_back(static_cast<int>(std::find(v.begin(), v.end(), 15) - v.begin()));
    r.push_back(std::find(v.begin(), v.end(), 99) == v.end());
    r.push_back(static_cast<int>(std::count(v.begin(), v.end(), 8)));
    r.push_back(static_cast<int>(std::count_if(v.begin(), v.end(), [](int x) { return x > 10; })));
    auto it = std::find_if(v.begin(), v.end(), [](int x) { return x % 2 == 1; });
    r.push_back(*it);
    r.push_back(std::any_of(v.begin(), v.end(), [](int x) { return x > 40; }));
    r.push_back(std::all_of(v.begin(), v.end(), [](int x) { return x > 3; }));
    r.push_back(std::none_of(v.begin(), v.end(), [](int x) { return x < 0; }));
    r.push_back(std::all_of(v.begin(), v.begin(), [](int x) { return false; }));
    return r;
}
//// algo_minmax_element
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {4, 9, 2, 9, 2, 7};
    auto mn = std::min_element(v.begin(), v.end());
    auto mx = std::max_element(v.begin(), v.end());
    std::vector<int> r = {static_cast<int>(mn - v.begin()), *mn, static_cast<int>(mx - v.begin()), *mx};
    auto mx2 = std::max_element(v.begin(), v.end(), [](int a, int b) { return a % 5 < b % 5; });
    r.push_back(static_cast<int>(mx2 - v.begin()));
    auto mn2 = std::min_element(v.begin() + 1, v.end());
    r.push_back(static_cast<int>(mn2 - v.begin()));
    return r;
}
//// algo_binary_search
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 3, 3, 5, 7, 7, 7, 9};
    std::vector<int> r;
    for (int x : {0, 1, 3, 4, 7, 9, 10}) {
        auto lo = std::lower_bound(v.begin(), v.end(), x);
        auto hi = std::upper_bound(v.begin(), v.end(), x);
        r.push_back(static_cast<int>(lo - v.begin()));
        r.push_back(static_cast<int>(hi - v.begin()));
        r.push_back(std::binary_search(v.begin(), v.end(), x));
    }
    std::vector<int> d = {9, 7, 5, 3, 1};
    auto it = std::lower_bound(d.begin(), d.end(), 5, std::greater<int>());
    r.push_back(static_cast<int>(it - d.begin()));
    std::vector<int> e;
    r.push_back(std::lower_bound(e.begin(), e.end(), 5) == e.end());
    return r;
}
//// algo_unique_erase
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 1, 2, 2, 2, 3, 1, 1, 4};
    std::vector<int> w = v;
    auto e = std::unique(v.begin(), v.end());
    std::vector<int> r;
    r.push_back(static_cast<int>(e - v.begin()));
    v.erase(e, v.end());
    r.insert(r.end(), v.begin(), v.end());
    std::sort(w.begin(), w.end());
    w.erase(std::unique(w.begin(), w.end()), w.end());
    r.push_back(-1);
    r.insert(r.end(), w.begin(), w.end());
    return r;
}
//// algo_next_permutation
#include <algorithm>
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "abc";
    std::vector<std::string> r;
    do {
        r.push_back(s);
    } while (std::next_permutation(s.begin(), s.end()));
    std::vector<int> v = {1, 2, 2};
    do {
        r.push_back(std::to_string(v[0]) + std::to_string(v[1]) + std::to_string(v[2]));
    } while (std::next_permutation(v.begin(), v.end()));
    std::vector<int> w = {3, 2, 1};
    bool more = std::prev_permutation(w.begin(), w.end());
    r.push_back(std::to_string(more) + std::to_string(w[0]) + std::to_string(w[1]) + std::to_string(w[2]));
    return r;
}
//// algo_rotate_remove
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3, 4, 5, 6};
    std::rotate(v.begin(), v.begin() + 2, v.end());
    std::vector<int> r = v;
    std::rotate(v.begin(), v.end() - 1, v.end());
    r.insert(r.end(), v.begin(), v.end());
    auto e = std::remove(v.begin(), v.end(), 3);
    r.push_back(static_cast<int>(e - v.begin()));
    auto e2 = std::remove_if(v.begin(), e, [](int x) { return x > 4; });
    r.push_back(static_cast<int>(e2 - v.begin()));
    v.erase(e2, v.end());
    r.insert(r.end(), v.begin(), v.end());
    return r;
}
//// algo_equal_distance_advance
#include <algorithm>
#include <iterator>
#include <vector>
std::vector<int> test() {
    std::vector<int> a = {1, 2, 3}, b = {1, 2, 3}, c = {1, 2, 4};
    std::vector<int> r;
    r.push_back(std::equal(a.begin(), a.end(), b.begin()));
    r.push_back(std::equal(a.begin(), a.end(), c.begin()));
    r.push_back(static_cast<int>(std::distance(a.begin(), a.end())));
    auto it = a.begin();
    std::advance(it, 2);
    r.push_back(*it);
    r.push_back(*std::next(a.begin()));
    r.push_back(*std::prev(a.end()));
    r.push_back(*std::next(a.begin(), 2));
    return r;
}
//// algo_swap_ops
#include <algorithm>
#include <string>
#include <utility>
#include <vector>
std::vector<std::string> test() {
    int a = 1, b = 2;
    std::swap(a, b);
    std::string s = "x", t = "yy";
    std::swap(s, t);
    std::vector<int> v = {1, 2, 3};
    std::swap(v[0], v[2]);
    std::pair<int, int> p = {5, 6};
    std::swap(p.first, p.second);
    char c1 = 'a', c2 = 'b';
    std::swap(c1, c2);
    std::string u = "hello";
    std::swap(u[0], u[4]);
    std::iter_swap(v.begin(), v.begin() + 1);
    return {std::to_string(a) + std::to_string(b), s, t, std::to_string(v[0]) + std::to_string(v[1]) + std::to_string(v[2]),
            std::to_string(p.first) + std::to_string(p.second), std::string(1, c1) + c2, u};
}
//// algo_insertion_sort_manual
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {9, 4, 7, 1, 8, 2, 2, 6};
    for (size_t i = 1; i < v.size(); ++i) {
        int key = v[i];
        int j = static_cast<int>(i) - 1;
        while (j >= 0 && v[j] > key) {
            v[j + 1] = v[j];
            --j;
        }
        v[j + 1] = key;
    }
    return v;
}
//// algo_merge_sort_recursive
#include <vector>
void msort(std::vector<int>& v, int lo, int hi, std::vector<int>& tmp) {
    if (hi - lo < 2) return;
    int mid = lo + (hi - lo) / 2;
    msort(v, lo, mid, tmp);
    msort(v, mid, hi, tmp);
    int i = lo, j = mid, k = lo;
    while (i < mid && j < hi) tmp[k++] = v[i] <= v[j] ? v[i++] : v[j++];
    while (i < mid) tmp[k++] = v[i++];
    while (j < hi) tmp[k++] = v[j++];
    for (k = lo; k < hi; ++k) v[k] = tmp[k];
}
std::vector<int> test() {
    std::vector<int> v;
    unsigned x = 12345;
    for (int i = 0; i < 200; ++i) {
        x = x * 1103515245u + 12345u;
        v.push_back(static_cast<int>((x >> 16) % 1000));
    }
    std::vector<int> tmp(v.size());
    msort(v, 0, static_cast<int>(v.size()), tmp);
    return v;
}
//// algo_binary_search_manual
#include <vector>
int bs(const std::vector<int>& v, int t) {
    int lo = 0, hi = static_cast<int>(v.size()) - 1;
    while (lo <= hi) {
        int mid = lo + (hi - lo) / 2;
        if (v[mid] == t) return mid;
        if (v[mid] < t) lo = mid + 1;
        else hi = mid - 1;
    }
    return -1;
}
std::vector<int> test() {
    std::vector<int> v;
    for (int i = 0; i < 50; ++i) v.push_back(i * 3);
    return {bs(v, 0), bs(v, 147), bs(v, 75), bs(v, 74), bs(v, 3), bs({}, 1)};
}
//// algo_matrix_dp
#include <algorithm>
#include <string>
#include <vector>
int lcs(const std::string& a, const std::string& b) {
    std::vector<std::vector<int>> dp(a.size() + 1, std::vector<int>(b.size() + 1, 0));
    for (size_t i = 1; i <= a.size(); ++i)
        for (size_t j = 1; j <= b.size(); ++j)
            dp[i][j] = a[i - 1] == b[j - 1] ? dp[i - 1][j - 1] + 1 : std::max(dp[i - 1][j], dp[i][j - 1]);
    return dp[a.size()][b.size()];
}
int test() { return lcs("AGGTAB", "GXTXAYB") * 100 + lcs("abcdef", "acf") * 10 + lcs("", "x"); }
//// algo_knapsack_coin
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> coins = {1, 5, 10, 25};
    int amt = 63;
    std::vector<int> dp(amt + 1, 1 << 20);
    dp[0] = 0;
    for (int a = 1; a <= amt; ++a)
        for (int c : coins)
            if (c <= a) dp[a] = std::min(dp[a], dp[a - c] + 1);
    std::vector<long long> ways(amt + 1, 0);
    ways[0] = 1;
    for (int c : coins)
        for (int a = c; a <= amt; ++a) ways[a] += ways[a - c];
    return {dp[amt], static_cast<int>(ways[amt])};
}
