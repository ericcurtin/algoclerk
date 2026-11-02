#include <algorithm>
#include <string>
#include <vector>

std::string reverseWords(const std::string& s) {
    std::string out;
    int i = (int)s.size() - 1;
    while (i >= 0) {
        while (i >= 0 && s[i] == ' ') --i;
        if (i < 0) break;
        int j = i;
        while (i >= 0 && s[i] != ' ') --i;
        if (!out.empty()) out += ' ';
        out += s.substr(i + 1, j - i);
    }
    return out;
}

int strStr(const std::string& haystack, const std::string& needle) {
    size_t pos = haystack.find(needle);
    if (pos == std::string::npos) {
        return -1;
    }
    return static_cast<int>(pos);
}

std::string longestCommonPrefix(const std::vector<std::string>& strs);

bool isRotation(const std::string& a, const std::string& b) {
    return a.size() == b.size() && (a + a).find(b) != std::string::npos;
}
