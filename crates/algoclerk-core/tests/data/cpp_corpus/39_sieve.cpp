#include <array>
#include <vector>

const int MAXN = 100;

std::vector<int> primesUpTo(int n) {
    std::vector<bool> composite(n + 1, false);
    std::vector<int> primes;
    for (int i = 2; i <= n; ++i) {
        if (!composite[i]) {
            primes.push_back(i);
            for (long long j = (long long)i * i; j <= n; j += i) {
                composite[j] = true;
            }
        }
    }
    return primes;
}

int smallPrimeCount() {
    std::array<bool, MAXN + 1> isComposite{};
    int count = 0;
    for (int i = 2; i <= MAXN; ++i) {
        if (isComposite[i]) continue;
        ++count;
        for (int j = 2 * i; j <= MAXN; j += i) isComposite[j] = true;
    }
    return count;
}
