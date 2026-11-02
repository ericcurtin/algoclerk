#include <climits>
#include <string>

int myAtoi(const std::string& s) {
    size_t i = 0;
    while (i < s.size() && s[i] == ' ') ++i;
    int sign = 1;
    if (i < s.size() && (s[i] == '+' || s[i] == '-')) {
        sign = s[i] == '-' ? -1 : 1;
        ++i;
    }
    long long value = 0;
    for (; i < s.size() && s[i] >= '0' && s[i] <= '9'; ++i) {
        value = value * 10 + (s[i] - '0');
        if (value * sign > INT_MAX) return INT_MAX;
        if (value * sign < INT_MIN) return INT_MIN;
    }
    return static_cast<int>(value * sign);
}

int digitValue(char c) {
    switch (c) {
        case '0': return 0;
        case '1': return 1;
        case '2': return 2;
        default: return -1;
    }
}
