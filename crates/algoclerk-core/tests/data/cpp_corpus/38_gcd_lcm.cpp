#include <algorithm>
#include <vector>

constexpr int LIMIT = 1000;
const long long BIG = 1LL << 40;

int gcd(int a, int b) {
    while (b != 0) {
        int t = a % b;
        a = b;
        b = t;
    }
    return a < 0 ? -a : a;
}

long long lcm(int a, int b) {
    if (a == 0 || b == 0) return 0;
    return static_cast<long long>(a / gcd(a, b)) * b;
}

int gcdAll(const std::vector<int>& v) {
    int g = 0;
    for (int x : v) g = gcd(g, x);
    return std::min(g, LIMIT);
}

long long lcmAll(const std::vector<int>& v) {
    long long l = 1;
    for (int x : v) {
        l = lcm(static_cast<int>(l % BIG), x);
    }
    return l;
}
