#include <string>
#include <vector>

using namespace std;

string longestCommonPrefix(vector<string>& strs) {
    if (strs.empty()) return "";
    string prefix = strs[0];
    for (size_t i = 1; i < strs.size(); ++i) {
        size_t j = 0;
        while (j < prefix.size() && j < strs[i].size() && prefix[j] == strs[i][j]) {
            ++j;
        }
        prefix = prefix.substr(0, j);
        if (prefix.empty()) break;
    }
    return prefix;
}

string verticalScan(const vector<string>& strs) {
    for (size_t col = 0; col < strs[0].size(); ++col) {
        for (const string& s : strs) {
            if (col >= s.size() || s[col] != strs[0][col]) {
                return strs[0].substr(0, col);
            }
        }
    }
    return strs[0];
}
