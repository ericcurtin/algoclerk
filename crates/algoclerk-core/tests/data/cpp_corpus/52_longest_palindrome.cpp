#include <string>
#include <utility>

using namespace std;

pair<int, int> expand(const string& s, int l, int r) {
    while (l >= 0 && r < (int)s.size() && s[l] == s[r]) {
        --l;
        ++r;
    }
    return {l + 1, r - 1};
}

string longestPalindrome(const string& s) {
    int bestL = 0, bestR = 0;
    for (int i = 0; i < (int)s.size(); ++i) {
        for (int odd = 0; odd < 2; ++odd) {
            auto [l, r] = expand(s, i, i + odd);
            if (r - l > bestR - bestL) {
                bestL = l;
                bestR = r;
            }
        }
    }
    return s.empty() ? string() : s.substr(bestL, bestR - bestL + 1);
}
