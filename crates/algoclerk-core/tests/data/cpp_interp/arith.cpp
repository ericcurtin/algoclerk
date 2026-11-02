//// promote_char
#include <vector>
std::vector<int> test() {
    char a = 100, b = 100;
    int s = a + b;
    char c = a + b;
    char d = 'a' + 1;
    int e = 'z' - 'a';
    std::vector<int> r;
    r.push_back(s);
    r.push_back(c);
    r.push_back(d);
    r.push_back(e);
    r.push_back('a' + 1);
    r.push_back(a * b);
    return r;
}
//// unsigned_wrap
#include <vector>
std::vector<unsigned> test() {
    unsigned u = 0;
    u -= 1;
    std::vector<unsigned> r;
    r.push_back(u);
    r.push_back(u + 1);
    r.push_back(u * 2);
    r.push_back(u / 3);
    r.push_back(0u - 5u);
    r.push_back(4000000000u * 2u);
    r.push_back(u >> 4);
    r.push_back(u << 4);
    return r;
}
//// compare_mixed
#include <vector>
std::vector<int> test() {
    int i = -1;
    unsigned u = 1;
    long long ll = -1;
    std::vector<int> v(3);
    std::vector<int> r;
    r.push_back(i < u);
    r.push_back(i < static_cast<int>(u));
    r.push_back(ll < u);
    r.push_back(i > v.size());
    r.push_back(5 > v.size());
    r.push_back(ll < v.size());
    r.push_back(-1 < 0u);
    r.push_back(-1LL < 1ULL);
    r.push_back(i == 4294967295u);
    r.push_back('a' < 200);
    return r;
}
//// shifts
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    r.push_back(1 << 10);
    r.push_back(1u << 31);
    r.push_back(-8 >> 1);
    r.push_back(1LL << 40);
    r.push_back(0x80000000u >> 31);
    r.push_back(1 << 30);
    r.push_back(-1 >> 31);
    r.push_back(255 >> 4);
    int sh = 5;
    r.push_back(3 << sh);
    r.push_back(1ULL << 63);
    r.push_back((1 << 31));
    return r;
}
//// modulo_div
#include <vector>
std::vector<int> test() {
    std::vector<int> r;
    r.push_back(-7 % 3);
    r.push_back(7 % -3);
    r.push_back(-7 / 2);
    r.push_back(-7 / -2);
    r.push_back(7 / -2);
    r.push_back(-7 % -3);
    r.push_back(0 % 5);
    r.push_back(6 % 3);
    r.push_back(-1 / 2);
    r.push_back(13 / 4 * 4 + 13 % 4);
    return r;
}
//// compound_assign
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    char c = 'a';
    c += 1;
    r.push_back(c);
    char d = 100;
    d += 100;
    r.push_back(d);
    unsigned u = 5;
    u -= 10;
    r.push_back(u);
    int x = 10;
    x /= 3;
    r.push_back(x);
    x *= 7;
    r.push_back(x);
    x <<= 2;
    r.push_back(x);
    x >>= 1;
    r.push_back(x);
    x ^= 0xFF;
    r.push_back(x);
    x |= 1;
    r.push_back(x);
    x &= ~1;
    r.push_back(x);
    x %= 13;
    r.push_back(x);
    long long big = 1;
    big <<= 40;
    big += x;
    r.push_back(big);
    unsigned short_like = 255;
    short_like += 1;
    r.push_back(short_like);
    int y = 7;
    y += 3000000000u;
    r.push_back(y);
    char e = 127;
    e++;
    r.push_back(e);
    return r;
}
//// ternary_types
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    bool t = true, f = false;
    auto a = t ? 1 : 'a';
    r.push_back(a);
    auto b = t ? -1 : 1u;
    r.push_back(b);
    auto c = t ? 1 : 2LL;
    r.push_back(c);
    auto d = f ? 1 : 2u;
    r.push_back(d);
    auto e = t ? 'x' : 'y';
    r.push_back(e);
    auto g = f ? -5 : 3000000000u;
    r.push_back(g);
    int i = -3;
    unsigned u = 2;
    auto h = i < 0 ? i : u;
    r.push_back(h);
    size_t s = 4;
    auto k = t ? i : s;
    r.push_back(k);
    r.push_back(t ? 'a' : 0);
    return r;
}
//// literals
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    auto a = 4294967295;
    r.push_back(a + 1);
    auto b = 0xFFFFFFFF;
    r.push_back(b + 1);
    auto c = 2147483647;
    r.push_back(c);
    r.push_back(0b101);
    r.push_back(010);
    r.push_back(0xFF);
    r.push_back(1000000007LL * 3);
    auto d = 0x7FFFFFFF;
    r.push_back(d);
    r.push_back(100000u * 100000u);
    r.push_back(10ULL * 1000000000ULL * 10);
    return r;
}
//// size_t_mix
#include <vector>
std::vector<unsigned long long> test() {
    std::vector<int> v(3);
    int i = 5;
    std::vector<unsigned long long> r;
    r.push_back(v.size() - i);
    r.push_back(v.size() + i);
    r.push_back(v.size() * 2);
    r.push_back(v.size() / 2);
    r.push_back(v.size() - 1);
    size_t z = 0;
    r.push_back(z - 1);
    r.push_back(static_cast<size_t>(-1) >> 60);
    r.push_back(v.size() % 2);
    int neg = -2;
    r.push_back(v.size() + neg);
    return r;
}
//// bool_arith
#include <vector>
std::vector<int> test() {
    bool b = 5;
    std::vector<int> r;
    r.push_back(true + true);
    r.push_back(b);
    r.push_back(int(b));
    r.push_back(!5);
    r.push_back(~0);
    r.push_back(-true);
    r.push_back(!!7);
    r.push_back(b && false);
    r.push_back(b || false);
    r.push_back(true ? 10 : 20);
    r.push_back(3 > 2 == 1);
    r.push_back(5 & 3);
    r.push_back(5 | 3);
    r.push_back(5 ^ 3);
    return r;
}
//// casts
#include <climits>
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    r.push_back((int)3000000000u);
    r.push_back((unsigned)-1);
    r.push_back((char)300);
    r.push_back(static_cast<size_t>(-1) == 18446744073709551615ULL);
    r.push_back((long long)(unsigned)-1);
    r.push_back(static_cast<bool>(256));
    r.push_back(static_cast<int>('A'));
    r.push_back(static_cast<char>(65 + 1));
    r.push_back(static_cast<long long>(INT_MAX) + 1);
    r.push_back(static_cast<int>(static_cast<unsigned>(4294967295u)));
    return r;
}
//// limits
#include <climits>
#include <limits>
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    r.push_back(INT_MAX / 2);
    r.push_back(INT_MIN + 1);
    r.push_back(-INT_MAX);
    r.push_back(static_cast<long long>(INT_MIN) - 1);
    r.push_back(LLONG_MAX);
    r.push_back(LLONG_MIN);
    r.push_back(UINT_MAX);
    r.push_back(std::numeric_limits<int>::max());
    r.push_back(std::numeric_limits<int>::min());
    r.push_back(std::numeric_limits<long long>::max() / 3);
    r.push_back(std::numeric_limits<unsigned>::max());
    r.push_back(std::numeric_limits<char>::max());
    r.push_back(std::numeric_limits<size_t>::max() == static_cast<size_t>(-1));
    r.push_back(UINT_MAX + 1u);
    return r;
}
//// logic_shortcircuit
#include <vector>
std::vector<int> test() {
    int c = 0;
    bool a = (c++ > 0) && (c++ > 0);
    bool b = (c++ >= 0) || (c++ > 100);
    int x = 5, y = 0;
    bool z = (y != 0) && (x / y > 1);
    return {c, a, b, z, x > 3 && y == 0, !(x > 3) || y == 0};
}
//// inc_dec
#include <vector>
std::vector<int> test() {
    int i = 5;
    int a = i++ + 1;
    int b = ++i;
    int c = i-- * 2;
    int d = --i;
    std::vector<int> r = {i, a, b, c, d};
    char ch = 'a';
    ch++;
    ++ch;
    r.push_back(ch);
    unsigned u = 0;
    u--;
    r.push_back(u == 4294967295u);
    long long ll = 9;
    ll++;
    r.push_back(ll);
    return r;
}
//// comma_op
#include <vector>
std::vector<int> test() {
    int a = (1, 2, 3);
    int b = 0;
    for (int i = 0, j = 10; i < j; i += 3, j -= 3) {
        b += i * j;
    }
    int c = (b++, b * 2);
    return {a, b, c};
}
//// unary_minus
#include <vector>
std::vector<long long> test() {
    unsigned x = 3;
    std::vector<long long> r;
    r.push_back(-1u);
    r.push_back(-(2u));
    r.push_back(-x);
    r.push_back(-(x + 1));
    r.push_back(+x);
    r.push_back(-(-5));
    r.push_back(- -5);
    r.push_back(~x);
    r.push_back(~5LL);
    r.push_back(-3LL * -3LL);
    return r;
}
//// ll_arith
#include <vector>
std::vector<long long> test() {
    long long a = 1LL << 62;
    std::vector<long long> r;
    r.push_back(a + (a - 1));
    r.push_back(3037000499LL * 3037000499LL);
    r.push_back(static_cast<long long>(2147483647) * 2);
    r.push_back(a / 1024);
    r.push_back(a % 1000003);
    r.push_back(-a / 7);
    r.push_back(-a % 7);
    int i = 123456;
    r.push_back(static_cast<long long>(i) * i);
    r.push_back(1LL * i * i);
    unsigned long long u = 18446744073709551615ULL;
    r.push_back(u % 1000);
    r.push_back(u / 7);
    r.push_back(u + 2);
    return r;
}
//// minmax_abs
#include <algorithm>
#include <cstdlib>
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    r.push_back(std::max(1, 2));
    r.push_back(std::min(1, 2));
    r.push_back(std::max<long long>(1, 2LL));
    r.push_back(std::min({3, 1, 2}));
    r.push_back(std::max({3, 1, 2, 7, 5}));
    r.push_back(std::abs(-5));
    r.push_back(std::abs(5));
    r.push_back(std::abs(-5LL));
    r.push_back(std::max('a', 'b'));
    int a = 4, b = 9;
    r.push_back(std::max(a, b) - std::min(a, b));
    unsigned x = 3, y = 4000000000u;
    r.push_back(std::max(x, y));
    r.push_back(std::min(x - y, y));
    return r;
}
//// gcd_clamp
#include <algorithm>
#include <numeric>
#include <vector>
std::vector<int> test() {
    return {std::gcd(12, 18), std::lcm(4, 6), std::clamp(15, 0, 10), std::clamp(-3, 0, 10), std::clamp(5, 0, 10)};
}
//// tricky_promotion
#include <vector>
std::vector<long long> test() {
    char a = 127;
    char b = 1;
    std::vector<long long> r;
    r.push_back(a + b);
    r.push_back(static_cast<char>(a + b));
    r.push_back(a * a);
    unsigned u = 1;
    r.push_back(u - 2 < 0);
    r.push_back(static_cast<long long>(u) - 2 < 0);
    int i = -1;
    r.push_back(i / 2u);
    r.push_back(i % 3u);
    r.push_back((i + 0u) > 0);
    r.push_back('a' + 'b');
    r.push_back(a > -1);
    return r;
}
