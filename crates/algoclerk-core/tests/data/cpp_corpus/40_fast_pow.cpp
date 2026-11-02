#include <vector>

const long long MOD = 1000000007LL;

long long power(long long base, long long exp) {
    long long result = 1;
    base %= MOD;
    while (exp > 0) {
        if (exp & 1) result = result * base % MOD;
        base = base * base % MOD;
        exp >>= 1;
    }
    return result;
}

long long inverse(long long a) {
    return power(a, MOD - 2);
}

long long choose(int n, int k) {
    if (k < 0 || k > n) return 0;
    std::vector<long long> fact(n + 1, 1);
    for (int i = 1; i <= n; ++i) fact[i] = fact[i - 1] * i % MOD;
    return fact[n] * inverse(fact[k]) % MOD * inverse(fact[n - k]) % MOD;
}
