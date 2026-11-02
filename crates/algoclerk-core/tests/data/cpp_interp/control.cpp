//// switch_fallthrough
#include <vector>
std::vector<int> test() {
    std::vector<int> r;
    for (int i = 0; i < 6; ++i) {
        int acc = 0;
        switch (i) {
            case 0:
                acc += 1;
            case 1:
                acc += 10;
                break;
            case 2:
                acc += 100;
            case 3:
            case 4:
                acc += 1000;
                break;
            default:
                acc = -1;
        }
        r.push_back(acc);
    }
    return r;
}
//// switch_char_default_first
#include <string>
int score(char c) {
    switch (c) {
        default:
            return 0;
        case 'a':
        case 'e':
        case 'i':
        case 'o':
        case 'u':
            return 1;
        case 'z':
            return 26;
    }
}
int test() {
    std::string s = "hello zebra!";
    int t = 0;
    for (char c : s) t += score(c);
    return t;
}
//// switch_in_loop_continue
int test() {
    int sum = 0;
    for (int i = 0; i < 10; ++i) {
        switch (i % 3) {
            case 0:
                continue;
            case 1:
                sum += i;
                break;
            default:
                sum += 100;
        }
        sum += 1;
    }
    return sum;
}
//// while_do_loops
#include <vector>
std::vector<int> test() {
    std::vector<int> r;
    int i = 0;
    while (i < 5) {
        i += 2;
    }
    r.push_back(i);
    int j = 10;
    do {
        j--;
    } while (j > 12);
    r.push_back(j);
    int k = 0;
    while (true) {
        if (++k > 7) break;
        if (k % 2) continue;
        r.push_back(k);
    }
    for (;;) {
        if (k-- < 5) break;
    }
    r.push_back(k);
    return r;
}
//// nested_loops_labels
#include <vector>
std::vector<int> test() {
    std::vector<int> r;
    for (int i = 0; i < 4; ++i) {
        for (int j = 0; j < 4; ++j) {
            if (j == 2) break;
            if (i == 1) continue;
            r.push_back(i * 10 + j);
        }
    }
    int found = -1;
    for (int i = 0; i < 5 && found < 0; ++i)
        for (int j = 0; j < 5; ++j)
            if (i * j == 6) {
                found = i * 10 + j;
                break;
            }
    r.push_back(found);
    return r;
}
//// recursion_fib_fact
#include <vector>
long long fact(int n) { return n <= 1 ? 1 : n * fact(n - 1); }
int fib(int n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}
int gcd_(int a, int b) { return b == 0 ? a : gcd_(b, a % b); }
std::vector<long long> test() {
    return {fact(15), fib(20), gcd_(48, 180), fact(1)};
}
//// recursion_mutual_forward
bool is_odd(int n);
bool is_even(int n) { return n == 0 ? true : is_odd(n - 1); }
bool is_odd(int n) { return n == 0 ? false : is_even(n - 1); }
int test() { return is_even(40) * 10 + is_odd(7) * 5 + is_even(7); }
//// helpers_refs
#include <vector>
void bump(int& x) { x += 5; }
void fill_up(std::vector<int>& v, int n) {
    for (int i = 0; i < n; ++i) v.push_back(i * i);
}
int sum(const std::vector<int>& v) {
    int s = 0;
    for (int x : v) s += x;
    return s;
}
void swap_vals(int& a, int& b) {
    int t = a;
    a = b;
    b = t;
}
int by_value(std::vector<int> v) {
    v.push_back(100);
    return static_cast<int>(v.size());
}
std::vector<int> test() {
    int a = 1, b = 2;
    bump(a);
    swap_vals(a, b);
    std::vector<int> v;
    fill_up(v, 5);
    int n = by_value(v);
    return {a, b, sum(v), static_cast<int>(v.size()), n};
}
//// default_and_overloads
#include <string>
int f(int x) { return x + 1; }
int f(int x, int y) { return x * y; }
std::string f(const std::string& s) { return s + "!"; }
long long f(long long x) { return x * 1000; }
std::string test() {
    return std::to_string(f(2)) + "," + std::to_string(f(3, 4)) + "," + f(std::string("hi")) + "," + std::to_string(f(5LL));
}
//// static_inline_globals
#include <vector>
const int N = 5;
constexpr int M = N * 2;
const std::vector<int> DIRS = {1, 0, -1, 0, 1};
static int helper(int x) { return x * M; }
inline int helper2(int x) { return helper(x) + N; }
std::vector<int> test() {
    int s = 0;
    for (int i = 0; i + 1 < static_cast<int>(DIRS.size()); ++i) s += DIRS[i] * 3 + DIRS[i + 1];
    return {s, helper(3), helper2(1), N, M};
}
//// early_return_loops
#include <vector>
int find_first_neg(const std::vector<int>& v) {
    for (size_t i = 0; i < v.size(); ++i)
        if (v[i] < 0) return static_cast<int>(i);
    return -1;
}
bool all_pos(const std::vector<int>& v) {
    for (int x : v) {
        if (x <= 0) return false;
    }
    return true;
}
std::vector<int> test() {
    std::vector<int> a = {3, 4, -1, 5};
    std::vector<int> b = {3, 4, 5};
    return {find_first_neg(a), find_first_neg(b), all_pos(a), all_pos(b)};
}
//// if_init_switch_init
#include <map>
#include <vector>
std::vector<int> test() {
    std::map<int, int> m = {{1, 10}, {2, 20}};
    std::vector<int> r;
    if (auto it = m.find(2); it != m.end()) r.push_back(it->second);
    if (auto it = m.find(3); it != m.end()) r.push_back(it->second);
    else r.push_back(-1);
    switch (int k = static_cast<int>(m.size()); k) {
        case 2:
            r.push_back(222);
            break;
        default:
            r.push_back(0);
    }
    return r;
}
//// dangling_else_blocks
int test() {
    int x = 5, r = 0;
    if (x > 3)
        if (x > 10) r = 1;
        else r = 2;
    if (x > 3) {
        r += 10;
    } else if (x > 1) {
        r += 20;
    } else {
        r += 30;
    }
    { int x = 100; r += x; }
    return r + x;
}
//// scoping_shadow
int test() {
    int x = 1;
    int r = x;
    {
        int x = 2;
        r = r * 10 + x;
        {
            int x = 3;
            r = r * 10 + x;
        }
        r = r * 10 + x;
    }
    for (int x = 7; x < 8; ++x) r = r * 10 + x;
    return r * 10 + x;
}
