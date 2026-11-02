#include <string>

const int VALUES[13] = {1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1};
const std::string SYMBOLS[13] = {"M", "CM", "D", "CD", "C", "XC", "L",
                                 "XL", "X", "IX", "V", "IV", "I"};

std::string intToRoman(int num) {
    std::string out;
    for (int i = 0; i < 13; ++i) {
        while (num >= VALUES[i]) {
            out += SYMBOLS[i];
            num -= VALUES[i];
        }
    }
    return out;
}

std::string toBinary(int n) {
    if (n == 0) return "0";
    std::string s;
    while (n > 0) {
        s.push_back(char('0' + n % 2));
        n /= 2;
    }
    return std::string(s.rbegin(), s.rend());
}
