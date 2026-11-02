#include <vector>

int hammingWeight(unsigned n) {
    int count = 0;
    while (n != 0) {
        n &= n - 1;
        ++count;
    }
    return count;
}

unsigned reverseBits(unsigned n) {
    unsigned out = 0;
    for (int i = 0; i < 32; ++i) {
        out = (out << 1) | (n & 1u);
        n >>= 1;
    }
    return out;
}

bool isPowerOfTwo(int n) {
    return n > 0 && (n & (n - 1)) == 0;
}

int singleNumber(const std::vector<int>& nums) {
    int x = 0;
    for (int v : nums) x ^= v;
    return x;
}

int swapNibbles(int b) {
    return ((b & 0x0F) << 4) | ((b & 0xF0) >> 4);
}

std::vector<int> countBits(int n) {
    std::vector<int> bits(n + 1);
    for (int i = 1; i <= n; ++i) bits[i] = bits[i >> 1] + (i & 1);
    return bits;
}
