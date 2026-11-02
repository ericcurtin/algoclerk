//// bind_pair_tuple
#include <map>
#include <string>
#include <tuple>
#include <utility>
#include <vector>
std::vector<std::string> test() {
    std::pair<int, std::string> p = {7, "seven"};
    auto [n, s] = p;
    n += 1;
    auto& [n2, s2] = p;
    n2 += 10;
    std::tuple<int, char, std::string> t = {1, 'x', "tup"};
    auto [a, b, c] = t;
    std::map<int, std::string> m = {{1, "one"}, {2, "two"}};
    std::string out;
    for (const auto& [k, v] : m) out += std::to_string(k) + v;
    return {std::to_string(n) + s, std::to_string(p.first) + s2, std::to_string(a) + b + c, out, std::to_string(std::get<0>(t)) + std::get<2>(t)};
}
//// tuple_ops
#include <string>
#include <tuple>
#include <vector>
std::vector<int> test() {
    auto t = std::make_tuple(1, 5, 'c');
    std::get<0>(t) = 100;
    std::tuple<int, int> a = {1, 2}, b = {1, 3};
    std::tuple<int, long long, std::string> c(1, 2LL, "x");
    return {std::get<0>(t), std::get<1>(t), std::get<2>(t), a < b, a == b, static_cast<int>(std::get<1>(c)), static_cast<int>(std::get<2>(c).size())};
}
//// pair_ops
#include <string>
#include <utility>
#include <vector>
std::vector<int> test() {
    std::pair<int, int> a = {1, 2}, b = {1, 3};
    auto c = std::make_pair(5, 'x');
    std::pair<std::string, int> d("hi", 3);
    auto e = a;
    e.first = 99;
    std::pair<long long, long long> f = {1, 2};
    std::pair<int, int> g;
    return {a < b, a == b, a != b, a > b, c.first, c.second, static_cast<int>(d.first.size()), d.second, a.first, e.first,
            static_cast<int>(f.first + f.second), g.first, g.second};
}
//// optional_basic
#include <optional>
#include <string>
#include <vector>
std::optional<int> find_even(const std::vector<int>& v) {
    for (int x : v)
        if (x % 2 == 0) return x;
    return std::nullopt;
}
std::vector<int> test() {
    auto a = find_even({1, 3, 4, 5});
    auto b = find_even({1, 3});
    std::optional<std::string> s;
    std::optional<int> c = 5;
    std::optional<int> d;
    d = 7;
    c.reset();
    return {a.has_value(), b.has_value(), a.value(), *a, b.value_or(-1), a.value_or(-1), s.has_value(), c.has_value(), *d, static_cast<bool>(a), !b};
}
//// optional_string_and_ops
#include <optional>
#include <string>
#include <vector>
std::optional<std::string> pick(int n) {
    if (n > 0) return std::string(n, 'z');
    return {};
}
std::vector<std::string> test() {
    auto a = pick(3);
    auto b = pick(0);
    std::string out = a ? *a : "none";
    std::string out2 = b ? *b : "none";
    return {out, out2, a.value(), b.value_or("dflt"), std::to_string(a->size())};
}
//// auto_deduction
#include <string>
#include <vector>
std::vector<std::string> test() {
    auto i = 5;
    auto u = 5u;
    auto l = 5LL;
    auto c = 'x';
    auto b = true;
    auto s = std::string("s");
    auto lit = "lit";
    auto sz = s.size();
    auto neg = i - 10;
    auto uneg = u - 10;
    const auto ci = i * 2;
    auto v = std::vector<int>{1, 2, 3};
    auto f = v.front();
    auto& bk = v.back();
    bk = 30;
    auto cp = v;
    cp[0] = 100;
    auto mix = i + l;
    auto mix2 = u + i;
    auto mix3 = sz + i;
    return {std::to_string(i), std::to_string(u), std::to_string(l), std::string(1, c), std::to_string(b), s, std::string(lit),
            std::to_string(sz), std::to_string(neg), std::to_string(uneg), std::to_string(ci), std::to_string(f),
            std::to_string(v[2]), std::to_string(cp[0]), std::to_string(mix), std::to_string(mix2), std::to_string(mix3)};
}
//// const_refs_and_copies
#include <string>
#include <vector>
std::string longest(const std::vector<std::string>& v) {
    std::string r;
    for (const std::string& s : v)
        if (s.size() > r.size()) r = s;
    return r;
}
int test_inner(std::vector<int> a, const std::vector<int>& b, std::vector<int>& c) {
    a.push_back(1);
    c.push_back(2);
    return static_cast<int>(a.size() + b.size() + c.size());
}
std::string test() {
    std::vector<int> a = {1}, b = {1, 2}, c;
    int r = test_inner(a, b, c);
    const std::vector<int>& alias = c;
    c.push_back(9);
    return longest({"aa", "bbbb", "c", "dddd"}) + std::to_string(r) + std::to_string(alias.size()) + std::to_string(a.size());
}
//// uninit_then_assign
#include <vector>
std::vector<int> test() {
    int x;
    long long y;
    char c;
    bool flag;
    x = 5;
    y = x * 2LL;
    c = 'q';
    flag = x > 3;
    int arr[3];
    arr[0] = 1;
    arr[1] = arr[0] + 1;
    arr[2] = arr[1] + 1;
    int a, b = 7, d;
    a = b;
    d = a + b;
    return {x, static_cast<int>(y), c, flag, arr[0], arr[1], arr[2], a, b, d};
}
//// global_const_arrays
#include <string>
#include <vector>
const int MOD = 1000000007;
const long long BIG = 1LL << 40;
const std::string NAME = "global";
const std::vector<std::vector<int>> GRID = {{1, 2}, {3, 4}};
int test() {
    long long r = 1;
    for (int i = 0; i < 40; ++i) r = r * 3 % MOD;
    return static_cast<int>(r) + static_cast<int>(BIG % 1000) + static_cast<int>(NAME.size()) + GRID[1][0];
}
//// mod_pow_matrix
#include <vector>
using Mat = std::vector<std::vector<long long>>;
const long long M = 1000000007LL;
Mat mul(const Mat& a, const Mat& b) {
    int n = static_cast<int>(a.size());
    Mat c(n, std::vector<long long>(n, 0));
    for (int i = 0; i < n; ++i)
        for (int j = 0; j < n; ++j)
            for (int k = 0; k < n; ++k) c[i][j] = (c[i][j] + a[i][k] * b[k][j]) % M;
    return c;
}
long long fibm(long long n) {
    Mat r = {{1, 0}, {0, 1}};
    Mat b = {{1, 1}, {1, 0}};
    while (n > 0) {
        if (n & 1) r = mul(r, b);
        b = mul(b, b);
        n >>= 1;
    }
    return r[0][1];
}
std::vector<long long> test() { return {fibm(10), fibm(90), fibm(1000000)}; }
//// using_alias_typedef
#include <map>
#include <string>
#include <vector>
using Grid = std::vector<std::vector<int>>;
using Index = std::map<std::string, int>;
typedef long long ll;
typedef std::vector<ll> VL;
ll total(const Grid& g) {
    ll t = 0;
    for (const auto& r : g)
        for (int x : r) t += x;
    return t;
}
VL test() {
    Grid g = {{1, 2}, {3, 4}, {5}};
    Index idx;
    idx["a"] = 1;
    ll big = 3000000000LL;
    return {total(g), big * 2, static_cast<ll>(idx.size())};
}
//// using_namespace_std
#include <algorithm>
#include <map>
#include <string>
#include <vector>
using namespace std;
vector<string> test() {
    vector<int> v = {5, 2, 9};
    sort(v.begin(), v.end());
    map<string, int> m;
    for (int x : v) m[to_string(x)] = x;
    string s = "x";
    s += to_string(max(v[0], v[2]));
    vector<string> out;
    for (auto& [k, val] : m) out.push_back(k + "=" + to_string(val));
    out.push_back(s);
    out.push_back(to_string(min(3, 4)));
    return out;
}
//// string_building_loops
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::vector<std::string> out;
    for (int n = 1; n <= 5; ++n) {
        std::string row;
        for (int i = 0; i < n; ++i) row += '*';
        out.push_back(row);
    }
    std::string fizz;
    for (int i = 1; i <= 15; ++i) {
        if (i % 15 == 0) fizz += "FizzBuzz,";
        else if (i % 5 == 0) fizz += "Buzz,";
        else if (i % 3 == 0) fizz += "Fizz,";
        else fizz += std::to_string(i) + ",";
    }
    out.push_back(fizz);
    return out;
}
//// bit_tricks
#include <vector>
std::vector<long long> test() {
    unsigned x = 0b101101;
    int pop = 0;
    for (unsigned t = x; t; t &= t - 1) ++pop;
    long long mask = (1LL << 33) - 1;
    int lowbit = 12 & -12;
    unsigned rev = 0;
    for (int i = 0; i < 32; ++i) rev |= ((x >> i) & 1u) << (31 - i);
    return {pop, mask, lowbit, rev, (x ^ (x >> 1)), static_cast<long long>(x << 28), ~x & 0xFFu, (5 & 3) | (8 ^ 2)};
}
//// two_pointers_sliding
#include <algorithm>
#include <string>
#include <unordered_map>
#include <vector>
std::vector<int> test() {
    std::string s = "abcabcbbxyzxyz";
    std::unordered_map<char, int> last;
    int best = 0, lo = 0;
    for (int i = 0; i < static_cast<int>(s.size()); ++i) {
        auto it = last.find(s[i]);
        if (it != last.end() && it->second >= lo) lo = it->second + 1;
        last[s[i]] = i;
        best = std::max(best, i - lo + 1);
    }
    std::vector<int> a = {2, 3, 1, 2, 4, 3};
    int target = 7, sum = 0, minlen = 1000;
    for (int l = 0, r = 0; r < static_cast<int>(a.size()); ++r) {
        sum += a[r];
        while (sum >= target) {
            minlen = std::min(minlen, r - l + 1);
            sum -= a[l++];
        }
    }
    return {best, minlen};
}
