#include <climits>
#include <cstddef>
#include <string>
#include <vector>

unsigned long long factorial(unsigned n) {
    unsigned long long r = 1ULL;
    for (unsigned i = 2u; i <= n; ++i) r *= i;
    return r;
}

long long widen(int a, int b) {
    return static_cast<long long>(a) * b + (long long)a - b;
}

int narrow(long long v) {
    return static_cast<int>(v % INT_MAX);
}

char shift(char c, int k) {
    if (c >= 'a' && c <= 'z') return 'a' + (c - 'a' + k) % 26;
    if (c >= 'A' && c <= 'Z') return (char)('A' + (c - 'A' + k) % 26);
    return c;
}

std::string caesar(const std::string& s, int k) {
    std::string out = s;
    for (std::size_t i = 0; i < out.size(); ++i) out[i] = shift(out[i], k);
    return out;
}

size_t countBytes(const std::vector<int>& v) {
    size_t n = v.size();
    return n * 4u;
}

int escapes() {
    std::string s = "tab\there \"quoted\" back\\slash\n";
    char nl = '\n', q = '\'', z = '\0', bs = '\\';
    unsigned mask = 0xFFu, bin = 0b1010, oct = 0777;
    return (int)s.size() + nl + q + z + bs + (int)(mask + bin + oct);
}
