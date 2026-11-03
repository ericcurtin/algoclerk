// SPDX-License-Identifier: Apache-2.0
//! UB cases: index, overflow, division, shifts.

use super::ub::{Case, Expect::Rt, c};
use algoclerk_core::cpp::interp::RtKind as R;

pub fn cases() -> Vec<Case> {
    vec![
        // ---- out of bounds ----
        c(
            "index_in_helper",
            Rt(R::Index),
            &[],
            true,
            r#"
int third(const std::vector<int>& v, int i) {
    return v[i + 1]; // @UB
}
int f() {
    std::vector<int> v = {4, 5, 6};
    int s = 0;
    for (int i = 0; i < 3; ++i) s += third(v, i);
    return s;
}
"#,
        ),
        c(
            "index_in_lambda",
            Rt(R::Index),
            &[],
            true,
            r#"
int f() {
    std::vector<int> v = {1, 2, 3};
    auto get = [&](int i) { return v[i]; }; // @UB
    return get(1) + get(10);
}
"#,
        ),
        c(
            "index_string_assign_at_size",
            Rt(R::Index),
            &["\"abc\""],
            false,
            r#"
int f(std::string s) {
    s[s.size()] = 'x'; // @UB
    return static_cast<int>(s.size());
}
"#,
        ),
        c(
            "index_grid_swapped",
            Rt(R::Index),
            &[],
            true,
            r#"
int f() {
    std::vector<std::vector<int>> g(2, std::vector<int>(5, 1));
    int s = 0;
    for (int c = 0; c < 5; ++c)
        for (int r = 0; r < 5; ++r) s += g[c][r]; // @UB
    return s;
}
"#,
        ),
        c(
            "index_empty_minus_one",
            Rt(R::Index),
            &[],
            true,
            r#"
int f() {
    std::deque<int> d;
    return d[d.size() - 1]; // @UB
}
"#,
        ),
        c(
            "index_size_minus_two",
            Rt(R::Index),
            &["{7}"],
            true,
            r#"
int f(const std::vector<int>& v) {
    return v[v.size() - 2]; // @UB
}
"#,
        ),
        c(
            "index_in_sort_comparator",
            Rt(R::Index),
            &[],
            true,
            r#"
int f() {
    std::vector<int> v = {3, 1, 2};
    std::vector<int> w = {0, 1};
    std::sort(v.begin(), v.end(), [&](int a, int b) { return w[a] < w[b]; }); // @UB
    return v[0];
}
"#,
        ),
        c(
            "index_inner_vector_empty",
            Rt(R::Index),
            &[],
            true,
            r#"
int f() {
    std::vector<std::vector<int>> g(3);
    g[1].push_back(5);
    return g[1][0] + g[2][0]; // @UB
}
"#,
        ),
        c(
            "index_ternary_branch",
            Rt(R::Index),
            &["5"],
            true,
            r#"
int f(int n) {
    std::array<int, 4> a = {1, 2, 3, 4};
    int i = n > 3 ? n : 0;
    return a[i]; // @UB
}
"#,
        ),
        c(
            "index_at_negative",
            Rt(R::Index),
            &["-1"],
            true,
            r#"
int f(int k) {
    std::vector<int> v = {1, 2, 3};
    return v.at(k); // @UB
}
"#,
        ),
        c(
            "index_string_at_in_loop",
            Rt(R::Index),
            &["\"hello\""],
            true,
            r#"
int f(const std::string& s) {
    int n = 0;
    for (size_t i = 0; i <= s.size(); ++i) {
        if (s.at(i) == 'l') ++n; // @UB
    }
    return n;
}
"#,
        ),
        c(
            "index_c_array_negative",
            Rt(R::Index),
            &["0"],
            false,
            r#"
int f(int k) {
    int a[4] = {1, 2, 3, 4};
    return a[k - 1]; // @UB
}
"#,
        ),
        c(
            "index_vector_after_pop",
            Rt(R::Index),
            &[],
            true,
            r#"
int f() {
    std::vector<int> v = {1, 2, 3};
    v.pop_back();
    v.pop_back();
    return v[1]; // @UB
}
"#,
        ),
        c(
            "index_range_for_extra",
            Rt(R::Index),
            &["{1, 2, 3}"],
            true,
            r#"
int f(const std::vector<int>& v) {
    int s = 0;
    for (int x : v) s += v[x]; // @UB
    return s;
}
"#,
        ),
        // ---- signed overflow ----
        c(
            "overflow_range_for_sum",
            Rt(R::Overflow),
            &["{2000000000, 2000000000}"],
            true,
            r#"
int f(const std::vector<int>& v) {
    int s = 0;
    for (int x : v) s += x; // @UB
    return s;
}
"#,
        ),
        c(
            "overflow_neg_int_min_arg",
            Rt(R::Overflow),
            &["-2147483648"],
            true,
            r#"
int f(int x) {
    return -x; // @UB
}
"#,
        ),
        c(
            "overflow_abs_arg",
            Rt(R::Overflow),
            &["-2147483648"],
            true,
            r#"
int f(int x) {
    return std::abs(x); // @UB
}
"#,
        ),
        c(
            "overflow_div_minus_one_arg",
            Rt(R::Overflow),
            &["-2147483648", "-1"],
            true,
            r#"
int f(int a, int b) {
    return a / b; // @UB
}
"#,
        ),
        c(
            "overflow_rem_minus_one_arg",
            Rt(R::Overflow),
            &["-2147483648", "-1"],
            true,
            r#"
int f(int a, int b) {
    return a % b; // @UB
}
"#,
        ),
        c(
            "overflow_ll_add_args",
            Rt(R::Overflow),
            &["9223372036854775807", "1"],
            true,
            r#"
long long f(long long a, long long b) {
    return a + b; // @UB
}
"#,
        ),
        c(
            "overflow_recursion_factorial",
            Rt(R::Overflow),
            &["13"],
            true,
            r#"
int fact(int n) {
    if (n <= 1) return 1;
    return n * fact(n - 1); // @UB
}
int f(int n) {
    return fact(n);
}
"#,
        ),
        c(
            "overflow_postincrement_max",
            Rt(R::Overflow),
            &["2147483647"],
            true,
            r#"
int f(int x) {
    x++; // @UB
    return x;
}
"#,
        ),
        c(
            "overflow_compound_mul_loop",
            Rt(R::Overflow),
            &[],
            true,
            r#"
int f() {
    int p = 1;
    for (int i = 0; i < 40; ++i) {
        p *= 3; // @UB
    }
    return p;
}
"#,
        ),
        c(
            "overflow_in_condition",
            Rt(R::Overflow),
            &["2147483647", "1"],
            true,
            r#"
int f(int a, int b) {
    if (a + b > 0) return 1; // @UB
    return 0;
}
"#,
        ),
        c(
            "overflow_in_ternary",
            Rt(R::Overflow),
            &["2147483647"],
            true,
            r#"
int f(int a) {
    int b = a > 0 ? a + a : 0; // @UB
    return b;
}
"#,
        ),
        c(
            "overflow_accumulate_int_init",
            Rt(R::Overflow),
            &["{2000000000, 2000000000}"],
            true,
            r#"
int f(const std::vector<int>& v) {
    return std::accumulate(v.begin(), v.end(), 0); // @UB
}
"#,
        ),
        c(
            "overflow_in_lambda",
            Rt(R::Overflow),
            &[],
            true,
            r#"
int f() {
    auto sq = [](int x) { return x * x; }; // @UB
    return sq(50000);
}
"#,
        ),
        c(
            "overflow_ll_shift_mul",
            Rt(R::Overflow),
            &[],
            true,
            r#"
long long f() {
    long long x = 1LL << 62;
    return x * 2; // @UB
}
"#,
        ),
        c(
            "overflow_int_product_assigned_to_ll",
            Rt(R::Overflow),
            &["100000"],
            true,
            r#"
long long f(int n) {
    long long wide = n * n; // @UB
    return wide;
}
"#,
        ),
        // ---- division and remainder by zero ----
        c(
            "div_zero_arg",
            Rt(R::DivZero),
            &["10", "0"],
            true,
            r#"
int f(int a, int b) {
    return a / b; // @UB
}
"#,
        ),
        c(
            "div_zero_computed",
            Rt(R::DivZero),
            &["5"],
            true,
            r#"
int f(int y) {
    return 100 % (y - y); // @UB
}
"#,
        ),
        c(
            "div_zero_in_lambda",
            Rt(R::DivZero),
            &[],
            true,
            r#"
int f() {
    auto avg = [](int s, int n) { return s / n; }; // @UB
    return avg(6, 3) + avg(5, 0);
}
"#,
        ),
        c(
            "div_zero_average_of_empty",
            Rt(R::DivZero),
            &["{}"],
            true,
            r#"
int f(const std::vector<int>& v) {
    int s = std::accumulate(v.begin(), v.end(), 0);
    return s / static_cast<int>(v.size()); // @UB
}
"#,
        ),
        c(
            "rem_zero_unsigned_size",
            Rt(R::DivZero),
            &["{}"],
            true,
            r#"
int f(const std::vector<int>& v) {
    size_t h = 12345;
    return static_cast<int>(h % v.size()); // @UB
}
"#,
        ),
        c(
            "div_zero_compound_ll",
            Rt(R::DivZero),
            &["0"],
            true,
            r#"
long long f(long long d) {
    long long x = 100;
    x /= d; // @UB
    return x;
}
"#,
        ),
        // ---- shifts ----
        c(
            "shift_arg_32",
            Rt(R::Shift),
            &["32"],
            true,
            r#"
int f(int n) {
    return 1 << n; // @UB
}
"#,
        ),
        c(
            "shift_loop_reaches_32",
            Rt(R::Shift),
            &[],
            true,
            r#"
int f() {
    unsigned m = 0;
    for (int i = 0; i <= 32; ++i) m |= 1u << i; // @UB
    return static_cast<int>(m);
}
"#,
        ),
        c(
            "shift_right_negative_count",
            Rt(R::Shift),
            &["-1"],
            true,
            r#"
int f(int n) {
    int x = 64;
    return x >> n; // @UB
}
"#,
        ),
        c(
            "shift_ll_64",
            Rt(R::Shift),
            &["64"],
            true,
            r#"
long long f(int n) {
    return 1LL << n; // @UB
}
"#,
        ),
        c(
            "shift_negative_value",
            Rt(R::Shift),
            &["-3"],
            true,
            r#"
int f(int x) {
    return x << 2; // @UB
}
"#,
        ),
        c(
            "shift_left_overflows_type",
            Rt(R::Shift),
            &["3"],
            true,
            r#"
int f(int x) {
    return x << 31; // @UB
}
"#,
        ),
        c(
            "shift_compound_big",
            Rt(R::Shift),
            &["40"],
            true,
            r#"
unsigned f(int n) {
    unsigned x = 1;
    x <<= n; // @UB
    return x;
}
"#,
        ),
    ]
}
