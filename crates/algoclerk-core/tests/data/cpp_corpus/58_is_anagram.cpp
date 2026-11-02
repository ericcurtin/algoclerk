#include <string>

bool isAnagram(const std::string& s, const std::string& t) {
    if (s.size() != t.size()) return false;
    int cnt[26] = {};
    for (size_t i = 0; i < s.size(); ++i) {
        cnt[s[i] - 'a']++;
        cnt[t[i] - 'a']--;
    }
    for (int c : cnt) {
        if (c != 0) return false;
    }
    return true;
}

int firstUnique(const std::string& s) {
    int freq[128];
    for (int i = 0; i < 128; ++i) freq[i] = 0;
    for (char c : s) ++freq[(int)c];
    for (int i = 0; i < (int)s.size(); ++i)
        if (freq[(int)s[i]] == 1) return i;
    return -1;
}
