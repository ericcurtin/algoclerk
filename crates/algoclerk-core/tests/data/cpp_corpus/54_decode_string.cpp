#include <cctype>
#include <stack>
#include <string>

std::string decodeString(const std::string& s) {
    std::stack<int> counts;
    std::stack<std::string> parts;
    std::string cur;
    int k = 0;
    for (char c : s) {
        if (std::isdigit(c)) {
            k = k * 10 + (c - '0');
        } else if (c == '[') {
            counts.push(k);
            parts.push(cur);
            k = 0;
            cur = "";
        } else if (c == ']') {
            std::string prev = parts.top();
            parts.pop();
            int times = counts.top();
            counts.pop();
            for (int i = 0; i < times; ++i) prev += cur;
            cur = prev;
        } else {
            cur.push_back(c);
        }
    }
    return cur;
}
