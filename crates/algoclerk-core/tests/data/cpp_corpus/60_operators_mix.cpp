#include <vector>

int mix(int a, int b, int c) {
    int r = a + b * c - (a - b) * (c + 1) / 2 % 7;
    r += (a << 2) | (b >> 1) & c ^ 5;
    r -= -a - -b + +c;
    r = (r > 0) ? r : -r;
    bool flag = a < b && b < c || !(a == c) && a != 0;
    r += flag ? 1 : 0;
    r = r > 100 ? 100 : r < -100 ? -100 : r;
    int q = ~r & 0xFF;
    r = (q++, r + q);
    --r;
    r *= 2;
    r /= 3;
    r %= 1000;
    r <<= 1;
    r >>= 1;
    r &= 0x7FFF;
    r |= 1;
    r ^= 3;
    return r - (-(-r));
}

int commaLoop(int n) {
    int total = 0;
    for (int i = 0, j = n; i < j; ++i, --j) {
        total += i * j;
    }
    return total;
}

int pointerMath(const std::vector<int>& v) {
    int acc = 0;
    int idx = 0;
    acc += v[idx++];
    acc += v[idx++];
    acc -= v[v.size() - 1];
    return acc;
}
