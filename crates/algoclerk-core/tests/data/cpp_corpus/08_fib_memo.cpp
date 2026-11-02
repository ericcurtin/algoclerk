#include <functional>
#include <unordered_map>
#include <vector>

long long fibMemo(int n) {
    std::vector<long long> memo(n + 2, -1);
    std::function<long long(int)> go = [&](int k) -> long long {
        if (k < 2) return k;
        if (memo[k] != -1) return memo[k];
        return memo[k] = go(k - 1) + go(k - 2);
    };
    return go(n);
}

int fibMap(int n, std::unordered_map<int, int>& cache) {
    if (n <= 1) {
        return n;
    }
    auto found = cache.find(n);
    if (found != cache.end()) {
        return found->second;
    }
    int r = fibMap(n - 1, cache) + fibMap(n - 2, cache);
    cache[n] = r;
    return r;
}
