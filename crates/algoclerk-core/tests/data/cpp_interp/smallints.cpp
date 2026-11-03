//// uchar_cctype
#include <cctype>
#include <string>
int test() {
    std::string s = "ab1 C#";
    int n = 0;
    for (char c : s) {
        if (std::isalpha(static_cast<unsigned char>(c))) { ++n; }
        if (std::isupper(static_cast<unsigned char>(c))) { n += 10; }
    }
    return n;
}
//// uchar_arith_wrap
#include <vector>
std::vector<int> test() {
    unsigned char a = 250;
    unsigned char b = 10;
    std::vector<int> r;
    r.push_back(a + b);
    unsigned char c = a + b;
    r.push_back(c);
    c = static_cast<unsigned char>(-1);
    r.push_back(c);
    c += 3;
    r.push_back(c);
    ++c;
    r.push_back(c);
    --c;
    --c;
    r.push_back(c);
    r.push_back(a * b);
    r.push_back(a / 7);
    r.push_back(a % 7);
    r.push_back(a >> 2);
    r.push_back(a << 2);
    r.push_back(~a);
    r.push_back(a > b);
    r.push_back(a == 250);
    r.push_back(-a);
    unsigned u = a;
    r.push_back(static_cast<int>(u + 10u));
    unsigned char x = 200;
    x = x * 2;
    r.push_back(x);
    x ^= 0xFF;
    r.push_back(x);
    x |= 1;
    x &= 0x7F;
    r.push_back(x);
    return r;
}
//// uchar_vectors
#include <string>
#include <vector>
int test() {
    std::string s = "Hello";
    std::vector<unsigned char> v(s.begin(), s.end());
    v.push_back(200);
    v.push_back(static_cast<unsigned char>(s[0] + 200));
    std::vector<unsigned char> w(4, 255);
    int total = 0;
    for (unsigned char b : v) { total += b; }
    for (auto b : w) { total += b; }
    w[0] = 7;
    w[1]++;
    total += w[0] + w[1];
    std::string back;
    for (unsigned char b : v) {
        if (b < 128) { back.push_back(static_cast<char>(b)); }
    }
    total += static_cast<int>(back.size());
    unsigned char arr[3] = {1, 2, 3};
    arr[1] = 255;
    total += arr[0] + arr[1] + arr[2];
    unsigned char cnt[256] = {};
    for (char ch : s) { cnt[static_cast<unsigned char>(ch)]++; }
    total += cnt['l'];
    return total;
}
//// uchar_ctor_cast_conv
#include <vector>
std::vector<int> test() {
    int big = 300;
    unsigned char a = static_cast<unsigned char>(big);
    unsigned char b = (unsigned char)(big + 1);
    unsigned char c = 'A';
    unsigned char d(66);
    unsigned char e{67};
    long long ll = a;
    auto f = static_cast<unsigned char>(a + b);
    bool t = c;
    unsigned char z = 0;
    auto g = c + 1;
    auto h = static_cast<unsigned char>(c + 1);
    return {a, b, c, d, e, static_cast<int>(ll), f, t, z, g, h};
}
//// signed_char_alias
#include <vector>
std::vector<int> test() {
    signed char a = 100;
    a = static_cast<signed char>(a + 100);
    signed char b = -5;
    unsigned char u = 250;
    char sc = static_cast<char>(u);
    return {a, b, a + b, sc, static_cast<unsigned char>(sc)};
}
