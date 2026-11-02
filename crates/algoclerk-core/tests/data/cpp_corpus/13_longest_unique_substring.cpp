#include <algorithm>
#include <string>
#include <unordered_set>
#include <vector>

int lengthOfLongestSubstring(const std::string& s) {
    int last[128];
    for (int i = 0; i < 128; ++i) last[i] = -1;
    int best = 0, start = 0;
    for (int i = 0; i < (int)s.size(); ++i) {
        int c = s[i];
        if (last[c] >= start) {
            start = last[c] + 1;
        }
        last[c] = i;
        best = std::max(best, i - start + 1);
    }
    return best;
}

int withSet(const std::string& s) {
    std::unordered_set<char> window;
    int l = 0, best = 0;
    for (int r = 0; r < static_cast<int>(s.size()); ++r) {
        while (window.count(s[r])) {
            window.erase(s[l++]);
        }
        window.insert(s[r]);
        best = std::max(best, r - l + 1);
    }
    return best;
}
