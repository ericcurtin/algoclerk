//// lambda_basic
#include <vector>
std::vector<int> test() {
    auto add = [](int a, int b) { return a + b; };
    auto sq = [](int x) -> long long { return static_cast<long long>(x) * x; };
    auto neg = [](bool b) { return !b; };
    int base = 10;
    auto plus_base = [base](int x) { return x + base; };
    auto plus_ref = [&base](int x) { return x + base; };
    base = 100;
    return {add(1, 2), static_cast<int>(sq(46341)), neg(false), plus_base(1), plus_ref(1)};
}
//// lambda_capture_modes
#include <vector>
std::vector<int> test() {
    int a = 1, b = 2, c = 3;
    auto f = [=]() { return a + b + c; };
    auto g = [&]() { a += 10; b += 20; return a + b; };
    auto h = [a, &b]() { b += 1; return a + b; };
    a = 100;
    int r1 = f();
    int r2 = g();
    int r3 = h();
    return {r1, r2, r3, a, b, c};
}
//// lambda_mutable
#include <vector>
std::vector<int> test() {
    int n = 0;
    auto counter = [n]() mutable { return ++n; };
    std::vector<int> r;
    r.push_back(counter());
    r.push_back(counter());
    r.push_back(counter());
    r.push_back(n);
    int total = 0;
    auto acc = [&total](int x) { total += x; };
    for (int i = 1; i <= 5; ++i) acc(i);
    r.push_back(total);
    return r;
}
//// lambda_generic
#include <string>
#include <vector>
std::vector<std::string> test() {
    auto twice = [](auto x) { return x + x; };
    auto first = [](const auto& v) { return v[0]; };
    auto larger = [](auto a, auto b) { return a < b ? b : a; };
    std::vector<int> vi = {7, 8};
    std::string s = "xy";
    return {std::to_string(twice(21)), twice(std::string("ab")), std::to_string(first(vi)), std::string(1, first(s)),
            std::to_string(larger(3, 9)), larger(std::string("pear"), std::string("apple"))};
}
//// lambda_recursive_function
#include <functional>
#include <vector>
std::vector<long long> test() {
    std::function<long long(int)> fib = [&](int n) -> long long { return n < 2 ? n : fib(n - 1) + fib(n - 2); };
    std::function<int(int, int)> gcd = [&gcd](int a, int b) { return b ? gcd(b, a % b) : a; };
    std::vector<long long> memo(60, -1);
    std::function<long long(int)> fm = [&](int n) -> long long {
        if (n < 2) return n;
        if (memo[n] >= 0) return memo[n];
        return memo[n] = fm(n - 1) + fm(n - 2);
    };
    return {fib(15), gcd(84, 36), fm(50)};
}
//// lambda_in_algos
#include <algorithm>
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::vector<std::string> v = {"ccc", "a", "bb", "dddd", "ee"};
    std::sort(v.begin(), v.end(), [](const std::string& x, const std::string& y) {
        return x.size() != y.size() ? x.size() < y.size() : x < y;
    });
    int thresh = 2;
    auto cnt = std::count_if(v.begin(), v.end(), [thresh](const std::string& s) { return static_cast<int>(s.size()) > thresh; });
    v.push_back(std::to_string(cnt));
    return v;
}
//// lambda_returning_lambda_state
#include <vector>
std::vector<int> test() {
    std::vector<int> log;
    auto push = [&log](int x) { log.push_back(x * 2); };
    for (int i = 0; i < 4; ++i) push(i);
    auto sum = [&]() {
        int s = 0;
        for (int x : log) s += x;
        return s;
    };
    log.push_back(sum());
    return log;
}
//// lambda_capture_vector_by_value
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {1, 2, 3};
    auto f = [v]() { return static_cast<int>(v.size()); };
    auto g = [&v]() { return static_cast<int>(v.size()); };
    v.push_back(4);
    return {f(), g()};
}
//// lambda_cmp_index_sort
#include <algorithm>
#include <vector>
std::vector<int> test() {
    std::vector<int> vals = {30, 10, 20, 40};
    std::vector<int> idx = {0, 1, 2, 3};
    std::sort(idx.begin(), idx.end(), [&](int a, int b) { return vals[a] < vals[b]; });
    return idx;
}
//// lambda_immediate_nested
#include <vector>
int test() {
    int x = [](int a) { return a * 3; }(7);
    auto outer = [](int a) {
        auto inner = [a](int b) { return a * b; };
        return inner(a + 1);
    };
    return x + outer(4);
}
//// function_pointer_like
#include <functional>
#include <vector>
int dbl(int x) { return 2 * x; }
int apply_twice(const std::function<int(int)>& f, int v) { return f(f(v)); }
std::vector<int> test() {
    std::function<int(int)> f = dbl;
    std::function<int(int)> g = [](int x) { return x + 1; };
    return {apply_twice(f, 3), apply_twice(g, 3), f(10), g(10)};
}
