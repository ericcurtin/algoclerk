//// str_basic
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "hello";
    std::string t = s + " " + "world";
    s += '!';
    s += t;
    std::string u(3, 'x');
    std::string e;
    std::vector<std::string> r = {s, t, u, e, std::string("abc"), std::string(), std::to_string(-123), std::to_string(0)};
    return r;
}
//// str_index_ops
#include <string>
#include <vector>
std::vector<int> test() {
    std::string s = "Hello, World";
    std::vector<int> r;
    r.push_back(s.size());
    r.push_back(s.length());
    r.push_back(s.empty());
    r.push_back(s[0]);
    r.push_back(s.at(4));
    r.push_back(s.front());
    r.push_back(s.back());
    s[0] = 'J';
    s[s.size() - 1] = '?';
    r.push_back(s == "Jello, Worl?");
    r.push_back(s < "Kello");
    r.push_back(s > "Jello");
    r.push_back(s.compare("Jello, Worl?"));
    r.push_back(s.compare("Z") < 0);
    r.push_back(s != "x");
    return r;
}
//// str_find
#include <string>
#include <vector>
std::vector<unsigned long long> test() {
    std::string s = "abracadabra";
    std::vector<unsigned long long> r;
    r.push_back(s.find("bra"));
    r.push_back(s.find("bra", 2));
    r.push_back(s.find('c'));
    r.push_back(s.find("xyz"));
    r.push_back(s.find(""));
    r.push_back(s.rfind("bra"));
    r.push_back(s.rfind('a'));
    r.push_back(s.rfind("bra", 6));
    r.push_back(s.find_first_of("cd"));
    r.push_back(s.find_last_of("cd"));
    r.push_back(s.find_first_not_of("ab"));
    r.push_back(s.find_last_not_of("ab"));
    r.push_back(s.find("a", 100));
    r.push_back(std::string::npos);
    r.push_back(s.find_first_of("zzz"));
    r.push_back(s.find_first_of("abc", 3));
    return r;
}
//// str_substr
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "0123456789";
    std::vector<std::string> r;
    r.push_back(s.substr(3));
    r.push_back(s.substr(3, 4));
    r.push_back(s.substr(0, 0));
    r.push_back(s.substr(10));
    r.push_back(s.substr(8, 100));
    r.push_back(s.substr(0));
    return r;
}
//// str_modify
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::vector<std::string> r;
    std::string s = "hello world";
    s.insert(5, ",");
    r.push_back(s);
    s.insert(0, "> ");
    r.push_back(s);
    s.erase(0, 2);
    r.push_back(s);
    s.erase(5, 1);
    r.push_back(s);
    s.replace(0, 5, "HOWDY");
    r.push_back(s);
    s.push_back('!');
    s.pop_back();
    s.pop_back();
    r.push_back(s);
    s.append(3, '-');
    s.append("end");
    r.push_back(s);
    s.resize(5);
    r.push_back(s);
    s.resize(8, '*');
    r.push_back(s);
    s.clear();
    r.push_back(s);
    s.assign(4, 'z');
    r.push_back(s);
    s.assign("new");
    r.push_back(s);
    s.insert(s.begin(), 'A');
    s.insert(s.end(), 'Z');
    r.push_back(s);
    s.erase(s.begin());
    s.erase(s.end() - 1);
    r.push_back(s);
    s.erase(s.begin() + 1, s.end());
    r.push_back(s);
    return r;
}
//// str_iterate
#include <string>
#include <vector>
std::vector<int> test() {
    std::string s = "abcXYZ123";
    int lower = 0, upper = 0, digit = 0, other = 0;
    for (char c : s) {
        if (c >= 'a' && c <= 'z') lower++;
        else if (c >= 'A' && c <= 'Z') upper++;
        else if (c >= '0' && c <= '9') digit++;
        else other++;
    }
    int sum = 0;
    for (auto it = s.begin(); it != s.end(); ++it) sum += *it - 'a';
    int rsum = 0;
    for (auto it = s.rbegin(); it != s.rend(); ++it) rsum = rsum * 3 + (*it % 7);
    for (size_t i = 0; i < s.size(); ++i) rsum += s[i];
    return {lower, upper, digit, other, sum, rsum};
}
//// str_transform
#include <cctype>
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "Hello World 42";
    std::string up, lo, al;
    for (char c : s) {
        up += static_cast<char>(std::toupper(c));
        lo += static_cast<char>(std::tolower(c));
        if (std::isalpha(c)) al.push_back(c);
    }
    std::string rev(s.rbegin(), s.rend());
    std::string digits;
    for (char c : s)
        if (std::isdigit(c)) digits += c;
    return {up, lo, al, rev, digits};
}
//// str_ctype
#include <cctype>
#include <vector>
std::vector<int> test() {
    std::vector<int> r;
    for (char c : {'a', 'Z', '5', ' ', '!', '\n'}) {
        r.push_back(std::isalpha(c) != 0);
        r.push_back(std::isdigit(c) != 0);
        r.push_back(std::isalnum(c) != 0);
        r.push_back(std::isupper(c) != 0);
        r.push_back(std::islower(c) != 0);
        r.push_back(std::isspace(c) != 0);
    }
    return r;
}
//// str_stoi_to_string
#include <string>
#include <vector>
std::vector<long long> test() {
    std::vector<long long> r;
    r.push_back(std::stoi("123"));
    r.push_back(std::stoi("-45"));
    r.push_back(std::stoi("  77abc"));
    r.push_back(std::stoi("+8"));
    r.push_back(std::stoll("123456789012"));
    r.push_back(std::stoi("2147483647"));
    r.push_back(std::stoi("-2147483648"));
    r.push_back(std::to_string(12345).size());
    r.push_back(std::stoi(std::to_string(-9876)));
    r.push_back(std::stoll("-9223372036854775807"));
    return r;
}
//// str_compare_sort
#include <algorithm>
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::vector<std::string> v = {"pear", "apple", "fig", "banana", "Apple", "app", "", "zebra", "apple pie"};
    std::sort(v.begin(), v.end());
    return v;
}
//// str_view
#include <string>
#include <string_view>
#include <vector>
int count_a(std::string_view sv) {
    int c = 0;
    for (char ch : sv)
        if (ch == 'a') ++c;
    return c;
}
std::vector<unsigned long long> test() {
    std::string s = "banana";
    std::string_view v = s;
    std::vector<unsigned long long> r;
    r.push_back(count_a(v));
    r.push_back(v.size());
    r.push_back(v.find("nan"));
    r.push_back(v.substr(2, 3) == "nan");
    r.push_back(v[1]);
    r.push_back(count_a("a a a"));
    std::string_view w = v.substr(1);
    r.push_back(w.size());
    w.remove_prefix(2);
    r.push_back(w.size());
    w.remove_suffix(1);
    r.push_back(w.size());
    r.push_back(w == "na");
    return r;
}
//// str_palindromes
#include <string>
#include <vector>
bool pal(const std::string& s) {
    int i = 0, j = static_cast<int>(s.size()) - 1;
    while (i < j) {
        if (s[i++] != s[j--]) return false;
    }
    return true;
}
std::vector<int> test() {
    return {pal(""), pal("a"), pal("abba"), pal("abca"), pal("racecar"), pal("ab")};
}
//// str_build_numbers
#include <string>
std::string test() {
    std::string out;
    for (int i = 1; i <= 12; ++i) {
        if (!out.empty()) out += ",";
        out += std::to_string(i * i);
    }
    std::string rev;
    for (int i = static_cast<int>(out.size()) - 1; i >= 0; --i) rev.push_back(out[i]);
    return out + "|" + rev;
}
//// str_char_arith
#include <string>
std::string test() {
    std::string s = "hello";
    std::string r;
    for (char c : s) r += static_cast<char>((c - 'a' + 3) % 26 + 'a');
    int total = 0;
    for (char c : r) total += c;
    return r + std::to_string(total);
}
//// str_concat_variants
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string a = "ab", b = "cd";
    std::string c = a + b;
    std::string d = a + 'x' + b;
    std::string e = "[" + a + "]";
    std::string f = 'q' + b;
    std::string g;
    g = a;
    g += g;
    g += "!";
    g = g + g;
    return {c, d, e, f, g};
}
//// str_eq_ops
#include <string>
#include <vector>
std::vector<int> test() {
    std::string a = "abc", b = "abd", c = "abc", d = "ab";
    return {a == c, a != b, a < b, b > a, d < a, a <= c, a >= b, d == "ab", "abc" == a, a > d};
}
//// str_with_embedded
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "tab\there \"quoted\" \\ back\n";
    std::string t = "line1\nline2";
    std::string u = std::string(1, '\0') + "x";
    return {s, t, u, std::string("a\\b")};
}
//// str_split_words
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "  the quick  brown fox ";
    std::vector<std::string> words;
    std::string cur;
    for (char c : s) {
        if (c == ' ') {
            if (!cur.empty()) words.push_back(cur);
            cur.clear();
        } else {
            cur += c;
        }
    }
    if (!cur.empty()) words.push_back(cur);
    return words;
}
//// str_rotate_unique_sort_chars
#include <algorithm>
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "mississippi";
    std::string a = s;
    std::sort(a.begin(), a.end());
    std::string b = a;
    b.erase(std::unique(b.begin(), b.end()), b.end());
    std::string c = s;
    std::reverse(c.begin(), c.end());
    std::string d = s;
    std::rotate(d.begin(), d.begin() + 4, d.end());
    int cnt = static_cast<int>(std::count(s.begin(), s.end(), 's'));
    return {a, b, c, d, std::to_string(cnt)};
}
