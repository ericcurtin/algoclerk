//// ull_multiply_wraps
#include <vector>
std::vector<unsigned long long> test() {
    unsigned long long a = ~0ull;
    unsigned long long b = ~0ull;
    unsigned long long w = a;
    w *= b;
    unsigned long long c = 0xFFFFFFFFull * 0xFFFFFFFFFFull;
    return {a * b, w, c, a * 2ull, (a - 1ull) * (a - 2ull)};
}
//// for_each_mutates_through_reference
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v{123, 456, 7};
    std::for_each(v.begin(), v.end(), [](int& a) { a = a % 100; });
    int sum = 0;
    std::for_each(v.begin(), v.end(), [&](int a) { sum += a; });
    v.push_back(sum);
    return v;
}
//// partial_sum_in_place_and_op
#include <numeric>
#include <vector>
std::vector<int> test() {
    std::vector<int> v{1, 2, 3, 4};
    std::vector<int> p(4);
    std::partial_sum(v.begin(), v.end(), p.begin());
    std::partial_sum(v.begin(), v.end(), v.begin());
    std::vector<int> q(4);
    std::partial_sum(p.begin(), p.end(), q.begin(), [](int a, int b) { return a * 2 + b; });
    v.insert(v.end(), p.begin(), p.end());
    v.insert(v.end(), q.begin(), q.end());
    return v;
}
//// copy_and_lexicographical_compare
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> a{1, 2, 3};
    std::vector<int> b(5);
    auto e = std::copy(a.begin(), a.end(), b.begin() + 1);
    std::vector<int> c{1, 2};
    std::vector<int> d{1, 3};
    std::vector<int> r(b.begin(), b.end());
    r.push_back(static_cast<int>(e - b.begin()));
    r.push_back(std::lexicographical_compare(c.begin(), c.end(), d.begin(), d.end()));
    r.push_back(std::lexicographical_compare(d.begin(), d.end(), c.begin(), c.end()));
    r.push_back(std::lexicographical_compare(c.begin(), c.end(), a.begin(), a.end()));
    r.push_back(std::lexicographical_compare(c.begin(), c.end(), c.begin(), c.end()));
    return r;
}
//// tie_assigns_pair_and_tuple
#include <tuple>
#include <utility>
#include <vector>
std::vector<long long> test() {
    int a = 0;
    long long b = 0;
    std::tie(a, b) = std::make_pair(5, 7);
    int c = 0, d = 0, e = 0;
    std::tie(c, d, e) = std::make_tuple(1, 2, 3);
    return {a, b, c, d, e};
}
//// rotate_evaluates_arguments_once
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> v{1, 2, 3, 4, 5};
    auto m = v.begin();
    std::rotate(v.begin(), m++, v.end());
    std::rotate(v.begin(), v.begin() + 2, v.end());
    std::vector<int> r(v.begin(), v.end());
    r.push_back(static_cast<int>(m - v.begin()));
    return r;
}
//// string_plus_char_reference
#include <string>
std::string test() {
    std::string a = "abc";
    std::string out;
    for (auto& e : a) {
        out = out + e;
        out = e + out;
    }
    char& first = a[0];
    out += (a + first);
    for (const auto& e : a) out = a + e;
    return out;
}
//// self_initialised_scalar_shadow
int test() {
    int x = 3;
    {
        int y = x + 1;
        x = y;
    }
    return x;
}
