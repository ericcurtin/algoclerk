// SPDX-License-Identifier: Apache-2.0
//! UB cases: missing return, unsequenced modification (static), resource limits.

use super::ub::{Case, Expect::{Diag, Either, Rt}, c};
use algoclerk_core::cpp::interp::RtKind as R;

/// Programs with well defined sequencing that the checker must not flag.
pub const SEQUENCED_OK: &[(&str, &str)] = &[
    ("assign_from_postfix", "int f() {\n    int i = 1;\n    i = i++ + 1;\n    return i;\n}\n"),
    ("comma_sequences", "int f() {\n    int i = 1;\n    int j = (i++, i);\n    return j + i;\n}\n"),
    ("index_with_postfix_rhs", "int f() {\n    int a[3] = {0, 0, 0};\n    int i = 0;\n    a[i] = i++;\n    return a[0] + i;\n}\n"),
    ("shift_left_to_right", "int f() {\n    int i = 1;\n    int r = i++ << i;\n    return r;\n}\n"),
    ("compound_assign_rhs_first", "int f() {\n    int x = 1;\n    x += x++;\n    return x;\n}\n"),
    ("logical_and_sequences", "int f() {\n    int i = 0;\n    bool b = (i++ == 0) && (i == 1);\n    return b ? i : -1;\n}\n"),
    ("ternary_sequences", "int f() {\n    int i = 0;\n    int r = (i++ == 0) ? i : -1;\n    return r;\n}\n"),
    ("separate_statements", "int f() {\n    int i = 0;\n    int a = i++;\n    int b = i++;\n    return a * 10 + b;\n}\n"),
];

pub fn cases() -> Vec<Case> {
    vec![
        // ---- missing return ----
        c("missing_return_one_branch", Either(R::MissingReturn, "flow.missing-return"), &["0"], true, r#"
int f(int x) { // @UB
    if (x > 0) return 1;
}
"#),
        c("missing_return_after_loop", Either(R::MissingReturn, "flow.missing-return"), &["3"], true, r#"
int f(int n) { // @UB
    for (int i = 0; i < n; ++i) {
        if (i == 7) return i;
    }
}
"#),
        c("missing_return_switch_without_default", Either(R::MissingReturn, "flow.missing-return"), &["5"], true, r#"
int f(int x) { // @UB
    switch (x) {
    case 0: return 10;
    case 1: return 11;
    }
}
"#),
        c("missing_return_while_exit", Either(R::MissingReturn, "flow.missing-return"), &["0"], true, r#"
int f(int x) { // @UB
    while (x > 0) {
        if (x == 5) return x;
        --x;
    }
}
"#),
        // ---- unsequenced modification (static) ----
        c("unseq_plus_postfix", Diag("unseq.modify"), &[], false, r#"
int f() {
    int i = 1;
    int r = i + i++; // @UB
    return r;
}
"#),
        c("unseq_two_postfix", Diag("unseq.modify"), &[], false, r#"
int f() {
    int i = 1;
    int r = i++ + i++; // @UB
    return r;
}
"#),
        c("unseq_prefix_and_postfix", Diag("unseq.modify"), &[], false, r#"
int f() {
    int x = 1;
    x = x++ + ++x; // @UB
    return x;
}
"#),
        c("unseq_call_args", Diag("unseq.modify"), &[], false, r#"
int add(int a, int b) { return a * 10 + b; }
int f() {
    int i = 1;
    return add(i++, i); // @UB
}
"#),
        c("unseq_multiply", Diag("unseq.modify"), &[], false, r#"
int f() {
    int i = 3;
    int r = i * i--; // @UB
    return r;
}
"#),
        c("unseq_array_index_both_sides", Diag("unseq.modify"), &[], false, r#"
int f() {
    int a[4] = {1, 2, 3, 4};
    int i = 0;
    int r = a[i] + a[i++]; // @UB
    return r;
}
"#),
        c("unseq_in_condition", Diag("unseq.modify"), &[], false, r#"
int f() {
    int i = 0;
    if (i++ < i) return 1; // @UB
    return 0;
}
"#),
        c("unseq_in_for_header_body_ok_but_double_in_init", Diag("unseq.modify"), &[], false, r#"
int f() {
    int s = 0;
    for (int i = 0; i < 3; ++i) s += i++ + i++; // @UB
    return s;
}
"#),
        // ---- resource limits ----
        c("time_while_true", Rt(R::Time), &[], false, r#"
int f() {
    int x = 0;
    while (true) { x = (x + 1) % 7; } // @UB
    return x;
}
"#),
        c("time_do_while_true", Rt(R::Time), &[], false, r#"
int f() {
    int x = 0;
    do { x = (x + 3) % 5; } while (true); // @UB
    return x;
}
"#),
        c("time_for_never_advances", Rt(R::Time), &[], false, r#"
int f() {
    int s = 0;
    for (int i = 0; i < 10; ) { s = (s + 1) % 11; } // @UB
    return s;
}
"#),
        c("time_cubic_loops", Rt(R::Time), &["1000"], false, r#"
int f(int n) {
    int s = 0;
    for (int i = 0; i < n; ++i) for (int j = 0; j < n; ++j) for (int k = 0; k < n; ++k) s = (s + 1) % 1000; // @UB
    return s;
}
"#),
        c("time_collatz_like_cycle", Rt(R::Time), &["2"], false, r#"
int f(int x) {
    while (x != 1) { x = (x == 2) ? 3 : 2; } // @UB
    return x;
}
"#),
        c("depth_unbounded_recursion", Rt(R::Depth), &[], false, r#"
int deep(int n) {
    return deep(n + 1) + 1; // @UB
}
int f() {
    return deep(0);
}
"#),
        c("depth_mutual_recursion", Rt(R::Depth), &[], false, r#"
int pong(int n);
int ping(int n) {
    return pong(n + 1); // @UB
}
int pong(int n) {
    return ping(n + 1);
}
int f() {
    return ping(0);
}
"#),
        c("depth_std_function_recursion", Rt(R::Depth), &[], false, r#"
int f() {
    std::function<int(int)> go = [&](int n) { return go(n + 1); }; // @UB
    return go(0);
}
"#),
        c("depth_recursion_on_vector_arg", Rt(R::Depth), &["{1, 2, 3}"], false, r#"
int walk(const std::vector<int>& v, int i) {
    return walk(v, i + 1) + v.size(); // @UB
}
int f(const std::vector<int>& v) {
    return walk(v, 0);
}
"#),
        c("memory_huge_vector", Rt(R::Memory), &[], false, r#"
int f() {
    std::vector<int> v(1000000000); // @UB
    return static_cast<int>(v.size());
}
"#),
        c("memory_huge_string_from_arg", Rt(R::Memory), &["2000000000"], false, r#"
int f(long long n) {
    std::string s(n, 'a'); // @UB
    return static_cast<int>(s.size());
}
"#),
        c("memory_grid_product", Rt(R::Memory), &["5000"], false, r#"
int f(int n) {
    std::vector<std::vector<int>> g(n, std::vector<int>(n)); // @UB
    return static_cast<int>(g.size());
}
"#),
        c("memory_resize_from_arg", Rt(R::Memory), &["100000000"], false, r#"
int f(int n) {
    std::vector<int> v;
    v.resize(n); // @UB
    return static_cast<int>(v.size());
}
"#),
        c("memory_assign_ll_vector", Rt(R::Memory), &[], false, r#"
int f() {
    std::vector<long long> v;
    v.assign(5000000, 7LL); // @UB
    return static_cast<int>(v.size());
}
"#),
    ]
}
