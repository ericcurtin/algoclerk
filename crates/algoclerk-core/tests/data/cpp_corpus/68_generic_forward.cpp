#include <algorithm>
#include <string>
#include <vector>

static int helperA(int x);
inline bool helperB(const std::string& s);
int helperC(int, const std::vector<int>&);

static int helperA(int x) {
    return x <= 1 ? 1 : helperA(x - 1) + helperA(x - 2);
}

inline bool helperB(const std::string& s) {
    return !s.empty() && s.front() == s.back();
}

int helperC(int k, const std::vector<int>& v) {
    int c = 0;
    for (int x : v)
        if (x == k) ++c;
    return c;
}

int driver(const std::vector<int>& v, const std::string& s) {
    int r = helperA(5) + helperC(3, v);
    if (helperB(s)) r += 100;
    return r;
}

void swapInPlace(int& a, int& b) {
    int t = a;
    a = b;
    b = t;
}

const std::string& pick(const std::string& a, const std::string& b) {
    return a.size() >= b.size() ? a : b;
}

void noop() {}

void noopVoid(void) {
    return;
}
