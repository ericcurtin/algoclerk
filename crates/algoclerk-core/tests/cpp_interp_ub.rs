// SPDX-License-Identifier: Apache-2.0
//! One negative test per undefined behaviour class (and several per class): the interpreter must stop with
//! the right `RtKind` and point at the offending statement.

use algoclerk_core::cpp::interp::{Limits, RtKind, run};
mod common;
use algoclerk_core::cpp::value::Heap;

const NODES: &str = "struct ListNode {
    int val;
    ListNode* next;
    ListNode(int x) : val(x), next(nullptr) {}
    ListNode(int x, ListNode* n) : val(x), next(n) {}
};
";

fn limits() -> Limits {
    Limits {
        steps: 50_000_000,
        ..Limits::default()
    }
}

/// Run `test()` and return (kind, text of the offending statement, message).
fn fail(src: &str, lim: &Limits) -> (RtKind, String, String) {
    let full = format!("{NODES}{src}");
    let prog = common::parse_any(&full, &["ListNode"])
        .unwrap_or_else(|d| panic!("parse error {d}\n{src}"));
    let rep = run(&prog, "test", vec![], Heap::default(), lim);
    match rep.result {
        Ok(v) => panic!("expected a runtime error, got {v:?}\n{src}"),
        Err(e) => {
            let text = prog
                .span(e.node)
                .map(|s| full[s.start as usize..s.end as usize].to_string())
                .unwrap_or_default();
            (e.kind, text, e.msg)
        }
    }
}

fn check(src: &str, kind: RtKind, needle: &str) {
    let (k, text, msg) = fail(src, &limits());
    assert_eq!(k, kind, "wrong kind ({msg}) for:\n{src}");
    assert!(
        text.contains(needle),
        "error attributed to `{text}` instead of a statement containing `{needle}` ({msg}) for:\n{src}"
    );
}

macro_rules! ub {
    ($name:ident, $kind:expr, $needle:expr, $src:expr) => {
        #[test]
        fn $name() {
            check($src, $kind, $needle);
        }
    };
}

// ---- index ----
ub!(
    index_vector,
    RtKind::Index,
    "v[i]",
    "#include <vector>\nint test() {\n  std::vector<int> v(3);\n  int i = 5;\n  return v[i];\n}\n"
);
ub!(
    index_vector_neg,
    RtKind::Index,
    "v[i]",
    "#include <vector>\nint test() {\n  std::vector<int> v(3);\n  int i = -1;\n  return v[i];\n}\n"
);
ub!(
    index_vector_size,
    RtKind::Index,
    "v[v.size()]",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2};\n  v[v.size()] = 5;\n  return 0;\n}\n"
);
ub!(
    index_empty,
    RtKind::Index,
    "v[0]",
    "#include <vector>\nint test() {\n  std::vector<int> v;\n  return v[0];\n}\n"
);
ub!(
    index_at,
    RtKind::Index,
    "v.at(3)",
    "#include <vector>\nint test() {\n  std::vector<int> v(3);\n  return v.at(3);\n}\n"
);
ub!(
    index_string,
    RtKind::Index,
    "s[10]",
    "#include <string>\nchar test() {\n  std::string s = \"abc\";\n  return s[10];\n}\n"
);
ub!(
    index_string_at,
    RtKind::Index,
    "s.at(3)",
    "#include <string>\nchar test() {\n  std::string s = \"abc\";\n  return s.at(3);\n}\n"
);
ub!(
    index_2d,
    RtKind::Index,
    "g[1][5]",
    "#include <vector>\nint test() {\n  std::vector<std::vector<int>> g(3, std::vector<int>(3));\n  return g[1][5];\n}\n"
);
ub!(
    index_array,
    RtKind::Index,
    "a[3]",
    "#include <array>\nint test() {\n  std::array<int, 3> a = {1, 2, 3};\n  return a[3];\n}\n"
);
ub!(
    index_c_array,
    RtKind::Index,
    "a[5]",
    "int test() {\n  int a[5] = {1, 2, 3, 4, 5};\n  return a[5];\n}\n"
);
ub!(
    index_deque,
    RtKind::Index,
    "d[4]",
    "#include <deque>\nint test() {\n  std::deque<int> d = {1, 2};\n  return d[4];\n}\n"
);
ub!(
    index_map_at,
    RtKind::Index,
    "m.at(9)",
    "#include <map>\nint test() {\n  std::map<int, int> m = {{1, 2}};\n  return m.at(9);\n}\n"
);
ub!(
    index_umap_at,
    RtKind::Index,
    "m.at(9)",
    "#include <unordered_map>\nint test() {\n  std::unordered_map<int, int> m = {{1, 2}};\n  return m.at(9);\n}\n"
);
ub!(
    index_substr,
    RtKind::Index,
    "s.substr(9)",
    "#include <string>\nint test() {\n  std::string s = \"abc\";\n  return static_cast<int>(s.substr(9).size());\n}\n"
);
ub!(
    index_off_by_one_loop,
    RtKind::Index,
    "v[i]",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3};\n  int s = 0;\n  for (size_t i = 0; i <= v.size(); ++i) s += v[i];\n  return s;\n}\n"
);
ub!(
    index_iter_plus,
    RtKind::Index,
    "v.begin() + 9",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3};\n  auto it = v.begin() + 9;\n  return 0;\n}\n"
);

// ---- overflow ----
ub!(
    overflow_add,
    RtKind::Overflow,
    "x + 1",
    "#include <climits>\nint test() {\n  int x = INT_MAX;\n  return x + 1;\n}\n"
);
ub!(
    overflow_sub,
    RtKind::Overflow,
    "x - 1",
    "#include <climits>\nint test() {\n  int x = INT_MIN;\n  return x - 1;\n}\n"
);
ub!(
    overflow_mul,
    RtKind::Overflow,
    "x * x",
    "int test() {\n  int x = 100000;\n  return x * x;\n}\n"
);
ub!(
    overflow_neg,
    RtKind::Overflow,
    "-x",
    "#include <climits>\nint test() {\n  int x = INT_MIN;\n  return -x;\n}\n"
);
ub!(
    overflow_div,
    RtKind::Overflow,
    "x / -1",
    "#include <climits>\nint test() {\n  int x = INT_MIN;\n  return x / -1;\n}\n"
);
ub!(
    overflow_rem,
    RtKind::Overflow,
    "x % -1",
    "#include <climits>\nint test() {\n  int x = INT_MIN;\n  return x % -1;\n}\n"
);
ub!(
    overflow_abs,
    RtKind::Overflow,
    "std::abs(x)",
    "#include <climits>\n#include <cstdlib>\nint test() {\n  int x = INT_MIN;\n  return std::abs(x);\n}\n"
);
ub!(
    overflow_ll,
    RtKind::Overflow,
    "x + 1",
    "#include <climits>\nlong long test() {\n  long long x = LLONG_MAX;\n  return x + 1;\n}\n"
);
ub!(
    overflow_ll_mul,
    RtKind::Overflow,
    "x * 3",
    "long long test() {\n  long long x = 4000000000000000000LL;\n  return x * 3;\n}\n"
);
ub!(
    overflow_incr,
    RtKind::Overflow,
    "x++",
    "#include <climits>\nint test() {\n  int x = INT_MAX;\n  x++;\n  return x;\n}\n"
);
ub!(
    overflow_compound,
    RtKind::Overflow,
    "x += x",
    "int test() {\n  int x = 2000000000;\n  x += x;\n  return x;\n}\n"
);
ub!(
    overflow_factorial,
    RtKind::Overflow,
    "r *= i",
    "int test() {\n  int r = 1;\n  for (int i = 1; i <= 20; ++i) r *= i;\n  return r;\n}\n"
);
ub!(
    overflow_sum_accumulate_int,
    RtKind::Overflow,
    "sum += x",
    "#include <vector>\nint test() {\n  std::vector<int> v(5, 1500000000);\n  int sum = 0;\n  for (int x : v) sum += x;\n  return sum;\n}\n"
);
ub!(
    overflow_size_to_int_mul,
    RtKind::Overflow,
    "n * n",
    "#include <vector>\nint test() {\n  std::vector<int> v(70000);\n  int n = static_cast<int>(v.size());\n  return n * n;\n}\n"
);

// ---- division ----
ub!(
    div_zero,
    RtKind::DivZero,
    "a / b",
    "int test() {\n  int a = 5, b = 0;\n  return a / b;\n}\n"
);
ub!(
    rem_zero,
    RtKind::DivZero,
    "a % b",
    "int test() {\n  int a = 5, b = 0;\n  return a % b;\n}\n"
);
ub!(
    div_zero_unsigned,
    RtKind::DivZero,
    "a / b",
    "int test() {\n  unsigned a = 5, b = 0;\n  return static_cast<int>(a / b);\n}\n"
);
ub!(
    div_zero_compound,
    RtKind::DivZero,
    "a /= b",
    "int test() {\n  int a = 5, b = 0;\n  a /= b;\n  return a;\n}\n"
);
ub!(
    div_zero_size,
    RtKind::DivZero,
    "10 / v.size()",
    "#include <vector>\nint test() {\n  std::vector<int> v;\n  return static_cast<int>(10 / v.size());\n}\n"
);

// ---- shifts ----
ub!(
    shift_width,
    RtKind::Shift,
    "1 << s",
    "int test() {\n  int s = 32;\n  return 1 << s;\n}\n"
);
ub!(
    shift_negative_count,
    RtKind::Shift,
    "1 << s",
    "int test() {\n  int s = -1;\n  return 1 << s;\n}\n"
);
ub!(
    shift_negative_value,
    RtKind::Shift,
    "x << 1",
    "int test() {\n  int x = -1;\n  return x << 1;\n}\n"
);
ub!(
    shift_ll_width,
    RtKind::Shift,
    "1LL << s",
    "long long test() {\n  int s = 64;\n  return 1LL << s;\n}\n"
);
ub!(
    shift_right_count,
    RtKind::Shift,
    "x >> 40",
    "int test() {\n  int x = 100;\n  return x >> 40;\n}\n"
);
ub!(
    shift_unsigned_width,
    RtKind::Shift,
    "u << s",
    "unsigned test() {\n  unsigned u = 1;\n  int s = 33;\n  return u << s;\n}\n"
);
ub!(
    shift_compound,
    RtKind::Shift,
    "x <<= 70",
    "long long test() {\n  long long x = 1;\n  x <<= 70;\n  return x;\n}\n"
);
ub!(
    shift_overflowing_value,
    RtKind::Shift,
    "x << 4",
    "int test() {\n  int x = 1 << 30;\n  return x << 4;\n}\n"
);

// ---- pointers ----
ub!(
    null_arrow,
    RtKind::Null,
    "p->val",
    "int test() {\n  ListNode* p = nullptr;\n  return p->val;\n}\n"
);
ub!(
    null_deref_chain,
    RtKind::Null,
    "a->next->val",
    "int test() {\n  ListNode* a = new ListNode(1);\n  return a->next->val;\n}\n"
);
ub!(
    null_loop_end,
    RtKind::Null,
    "p->next",
    "int test() {\n  ListNode* a = new ListNode(1, new ListNode(2));\n  ListNode* p = a;\n  while (true) p = p->next;\n  return 0;\n}\n"
);
ub!(
    null_assign_field,
    RtKind::Null,
    "p->val = 1",
    "int test() {\n  ListNode* p = nullptr;\n  p->val = 1;\n  return 0;\n}\n"
);
ub!(
    dangling_after_delete,
    RtKind::Dangling,
    "p->val",
    "int test() {\n  ListNode* p = new ListNode(5);\n  delete p;\n  return p->val;\n}\n"
);
ub!(
    dangling_double_delete,
    RtKind::Dangling,
    "delete p",
    "int test() {\n  ListNode* p = new ListNode(5);\n  delete p;\n  delete p;\n  return 0;\n}\n"
);
ub!(
    dangling_alias_after_delete,
    RtKind::Dangling,
    "q->val",
    "int test() {\n  ListNode* p = new ListNode(5);\n  ListNode* q = p;\n  delete p;\n  return q->val;\n}\n"
);
ub!(
    dangling_local_address,
    RtKind::Dangling,
    "p->val",
    "ListNode* make() {\n  ListNode n(3);\n  return &n;\n}\nint test() {\n  ListNode* p = make();\n  return p->val;\n}\n"
);
ub!(
    dangling_local_ref,
    RtKind::Dangling,
    "r",
    "const int& bad() {\n  int x = 4;\n  return x;\n}\nint test() {\n  const int& r = bad();\n  return r;\n}\n"
);
ub!(
    dangling_scope_exit,
    RtKind::Dangling,
    "p->val",
    "int test() {\n  ListNode* p = nullptr;\n  {\n    ListNode n(8);\n    p = &n;\n  }\n  return p->val;\n}\n"
);

// ---- uninitialised ----
ub!(
    uninit_int,
    RtKind::Uninit,
    "x + 1",
    "int test() {\n  int x;\n  return x + 1;\n}\n"
);
ub!(
    uninit_copy,
    RtKind::Uninit,
    "int y = x",
    "int test() {\n  int x;\n  int y = x;\n  return y;\n}\n"
);
ub!(
    uninit_condition,
    RtKind::Uninit,
    "if (b)",
    "int test() {\n  bool b;\n  if (b) return 1;\n  return 0;\n}\n"
);
ub!(
    uninit_pointer,
    RtKind::Uninit,
    "p->val",
    "int test() {\n  ListNode* p;\n  return p->val;\n}\n"
);
ub!(
    uninit_array,
    RtKind::Uninit,
    "a[1]",
    "int test() {\n  int a[3];\n  a[0] = 1;\n  return a[1];\n}\n"
);
ub!(
    uninit_compound,
    RtKind::Uninit,
    "x += 1",
    "int test() {\n  int x;\n  x += 1;\n  return x;\n}\n"
);
ub!(
    uninit_return,
    RtKind::Uninit,
    "return x",
    "long long test() {\n  long long x;\n  return x;\n}\n"
);
ub!(
    uninit_loop_var,
    RtKind::Uninit,
    "sum += i",
    "int test() {\n  int sum = 0;\n  int i;\n  sum += i;\n  return sum;\n}\n"
);
ub!(
    uninit_char_vs_member,
    RtKind::Uninit,
    "c == 'a'",
    "int test() {\n  char c;\n  return c == 'a';\n}\n"
);

// ---- invalidation ----
ub!(
    invalid_iter_push_back,
    RtKind::Invalidated,
    "*it",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3};\n  auto it = v.begin();\n  v.push_back(4);\n  return *it;\n}\n"
);
ub!(
    invalid_range_for_push,
    RtKind::Invalidated,
    "for (int x : v)",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3};\n  for (int x : v) {\n    if (x == 2) v.push_back(9);\n  }\n  return 0;\n}\n"
);
ub!(
    invalid_ref_push_back,
    RtKind::Invalidated,
    "return r",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1};\n  int& r = v[0];\n  v.push_back(2);\n  return r;\n}\n"
);
ub!(
    invalid_erase_then_use,
    RtKind::Invalidated,
    "*it",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3};\n  auto it = v.begin() + 1;\n  v.erase(v.begin());\n  return *it;\n}\n"
);
ub!(
    invalid_erase_in_loop,
    RtKind::Invalidated,
    "++it",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3, 4};\n  for (auto it = v.begin(); it != v.end(); ++it)\n    if (*it == 2) v.erase(it);\n  return 0;\n}\n"
);
ub!(
    invalid_map_erase_current,
    RtKind::Invalidated,
    "for (auto& kv : m)",
    "#include <map>\nint test() {\n  std::map<int, int> m = {{1, 1}, {2, 2}, {3, 3}};\n  for (auto& kv : m)\n    if (kv.first == 2) m.erase(kv.first);\n  return 0;\n}\n"
);
ub!(
    invalid_map_erase_iter,
    RtKind::Invalidated,
    "it->second",
    "#include <map>\nint test() {\n  std::map<int, int> m = {{1, 1}, {2, 2}};\n  auto it = m.find(1);\n  m.erase(it);\n  return it->second;\n}\n"
);
ub!(
    invalid_umap_insert_iter,
    RtKind::Invalidated,
    "it->second",
    "#include <unordered_map>\nint test() {\n  std::unordered_map<int, int> m = {{1, 1}};\n  auto it = m.find(1);\n  for (int i = 10; i < 20; ++i) m[i] = i;\n  return it->second;\n}\n"
);
ub!(
    invalid_string_append_ref,
    RtKind::Invalidated,
    "return c",
    "#include <string>\nchar test() {\n  std::string s = \"abc\";\n  char& c = s[0];\n  s += \"defghijklmnop\";\n  return c;\n}\n"
);
ub!(
    invalid_resize_iter,
    RtKind::Invalidated,
    "*it",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3};\n  auto it = v.begin();\n  v.resize(100);\n  return *it;\n}\n"
);
ub!(
    invalid_clear_ref,
    RtKind::Invalidated,
    "return f",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1, 2, 3};\n  int& f = v.front();\n  v.clear();\n  return f;\n}\n"
);
ub!(
    invalid_deque_pop_ref,
    RtKind::Invalidated,
    "return f",
    "#include <deque>\nint test() {\n  std::deque<int> d = {1, 2, 3};\n  int& f = d.front();\n  d.pop_front();\n  return f;\n}\n"
);
ub!(
    invalid_list_erase_iter,
    RtKind::Invalidated,
    "*it",
    "#include <list>\nint test() {\n  std::list<int> l = {1, 2, 3};\n  auto it = l.begin();\n  l.erase(it);\n  return *it;\n}\n"
);
ub!(
    invalid_set_erase_iter,
    RtKind::Invalidated,
    "*it",
    "#include <set>\nint test() {\n  std::set<int> s = {1, 2, 3};\n  auto it = s.find(2);\n  s.erase(2);\n  return *it;\n}\n"
);

// ---- missing return ----
ub!(
    missing_return,
    RtKind::MissingReturn,
    "",
    "int f(int x) {\n  if (x > 0) return 1;\n}\nint test() {\n  return f(0);\n}\n"
);
ub!(
    missing_return_loop,
    RtKind::MissingReturn,
    "",
    "int f(int x) {\n  for (int i = 0; i < x; ++i) {\n    if (i == 100) return i;\n  }\n}\nint test() {\n  return f(5);\n}\n"
);
ub!(
    missing_return_switch,
    RtKind::MissingReturn,
    "",
    "int f(int x) {\n  switch (x) {\n    case 1: return 10;\n    case 2: return 20;\n  }\n}\nint test() {\n  return f(3);\n}\n"
);

// ---- empty ----
ub!(
    empty_back,
    RtKind::Empty,
    "v.back()",
    "#include <vector>\nint test() {\n  std::vector<int> v;\n  return v.back();\n}\n"
);
ub!(
    empty_front,
    RtKind::Empty,
    "v.front()",
    "#include <vector>\nint test() {\n  std::vector<int> v;\n  return v.front();\n}\n"
);
ub!(
    empty_pop_back,
    RtKind::Empty,
    "v.pop_back()",
    "#include <vector>\nint test() {\n  std::vector<int> v;\n  v.pop_back();\n  return 0;\n}\n"
);
ub!(
    empty_stack_top,
    RtKind::Empty,
    "s.top()",
    "#include <stack>\nint test() {\n  std::stack<int> s;\n  return s.top();\n}\n"
);
ub!(
    empty_stack_pop,
    RtKind::Empty,
    "s.pop()",
    "#include <stack>\nint test() {\n  std::stack<int> s;\n  s.pop();\n  return 0;\n}\n"
);
ub!(
    empty_queue_front,
    RtKind::Empty,
    "q.front()",
    "#include <queue>\nint test() {\n  std::queue<int> q;\n  return q.front();\n}\n"
);
ub!(
    empty_queue_pop,
    RtKind::Empty,
    "q.pop()",
    "#include <queue>\nint test() {\n  std::queue<int> q;\n  q.push(1);\n  q.pop();\n  q.pop();\n  return 0;\n}\n"
);
ub!(
    empty_pq_top,
    RtKind::Empty,
    "pq.top()",
    "#include <queue>\nint test() {\n  std::priority_queue<int> pq;\n  return pq.top();\n}\n"
);
ub!(
    empty_pq_pop,
    RtKind::Empty,
    "pq.pop()",
    "#include <queue>\nint test() {\n  std::priority_queue<int> pq;\n  pq.pop();\n  return 0;\n}\n"
);
ub!(
    empty_deque_back,
    RtKind::Empty,
    "d.back()",
    "#include <deque>\nint test() {\n  std::deque<int> d;\n  return d.back();\n}\n"
);
ub!(
    empty_deque_pop_front,
    RtKind::Empty,
    "d.pop_front()",
    "#include <deque>\nint test() {\n  std::deque<int> d;\n  d.pop_front();\n  return 0;\n}\n"
);
ub!(
    empty_list_front,
    RtKind::Empty,
    "l.front()",
    "#include <list>\nint test() {\n  std::list<int> l;\n  return l.front();\n}\n"
);
ub!(
    empty_optional_value,
    RtKind::Empty,
    "o.value()",
    "#include <optional>\nint test() {\n  std::optional<int> o;\n  return o.value();\n}\n"
);
ub!(
    empty_optional_deref,
    RtKind::Empty,
    "*o",
    "#include <optional>\nint test() {\n  std::optional<int> o;\n  return *o;\n}\n"
);
ub!(
    empty_end_deref,
    RtKind::Empty,
    "*v.end()",
    "#include <vector>\nint test() {\n  std::vector<int> v = {1};\n  return *v.end();\n}\n"
);
ub!(
    empty_map_end_deref,
    RtKind::Empty,
    "m.end()",
    "#include <map>\nint test() {\n  std::map<int, int> m = {{1, 2}};\n  return m.end()->second;\n}\n"
);
ub!(
    empty_find_end_deref,
    RtKind::Empty,
    "it->second",
    "#include <unordered_map>\nint test() {\n  std::unordered_map<int, int> m;\n  auto it = m.find(3);\n  return it->second;\n}\n"
);
ub!(
    empty_string_back,
    RtKind::Empty,
    "s.back()",
    "#include <string>\nchar test() {\n  std::string s;\n  return s.back();\n}\n"
);
ub!(
    empty_string_pop,
    RtKind::Empty,
    "s.pop_back()",
    "#include <string>\nint test() {\n  std::string s;\n  s.pop_back();\n  return 0;\n}\n"
);
ub!(
    empty_set_begin_deref,
    RtKind::Empty,
    "*s.begin()",
    "#include <set>\nint test() {\n  std::set<int> s;\n  return *s.begin();\n}\n"
);

// ---- bad comparator ----
ub!(
    bad_compare_le,
    RtKind::BadCompare,
    "std::sort",
    "#include <algorithm>\n#include <vector>\nint test() {\n  std::vector<int> v = {3, 1, 2, 3, 3, 1, 5, 5};\n  std::sort(v.begin(), v.end(), [](int a, int b) { return a <= b; });\n  return v[0];\n}\n"
);
ub!(
    bad_compare_always_true,
    RtKind::BadCompare,
    "std::sort",
    "#include <algorithm>\n#include <vector>\nint test() {\n  std::vector<int> v = {3, 1, 2, 9, 8};\n  std::sort(v.begin(), v.end(), [](int a, int b) { return true; });\n  return v[0];\n}\n"
);
ub!(
    bad_compare_ge_pairs,
    RtKind::BadCompare,
    "std::sort",
    "#include <algorithm>\n#include <utility>\n#include <vector>\nint test() {\n  std::vector<std::pair<int, int>> v = {{1, 2}, {1, 2}, {0, 1}};\n  std::sort(v.begin(), v.end(), [](const std::pair<int, int>& a, const std::pair<int, int>& b) { return a.first >= b.first; });\n  return v[0].first;\n}\n"
);

// ---- bad arguments ----
ub!(
    badarg_stoi_text,
    RtKind::BadArg,
    "std::stoi",
    "#include <string>\nint test() {\n  return std::stoi(\"abc\");\n}\n"
);
ub!(
    badarg_stoi_overflow,
    RtKind::BadArg,
    "std::stoi",
    "#include <string>\nint test() {\n  return std::stoi(\"99999999999\");\n}\n"
);
ub!(
    badarg_stoll_overflow,
    RtKind::BadArg,
    "std::stoll",
    "#include <string>\nlong long test() {\n  return std::stoll(\"99999999999999999999\");\n}\n"
);
ub!(
    badarg_stoi_empty,
    RtKind::BadArg,
    "std::stoi",
    "#include <string>\nint test() {\n  return std::stoi(\"\");\n}\n"
);

// ---- resources ----
#[test]
fn time_infinite_loop() {
    let (k, text, _) = fail(
        "int test() {\n  int i = 0;\n  while (true) { i = (i + 1) % 7; }\n  return i;\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Time);
    assert!(!text.is_empty());
}

#[test]
fn time_empty_infinite_loop() {
    let (k, _, _) = fail("int test() {\n  for (;;) {}\n  return 0;\n}\n", &limits());
    assert_eq!(k, RtKind::Time);
}

#[test]
fn time_quadratic() {
    let (k, _, _) = fail(
        "int test() {\n  long long s = 0;\n  for (int i = 0; i < 100000; ++i)\n    for (int j = 0; j < 100000; ++j) s += i ^ j;\n  return static_cast<int>(s);\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Time);
}

#[test]
fn time_budget_is_tight() {
    let lim = Limits {
        steps: 100,
        ..Limits::default()
    };
    let (k, _, _) = fail(
        "int test() {\n  int s = 0;\n  for (int i = 0; i < 1000; ++i) s += i;\n  return s;\n}\n",
        &lim,
    );
    assert_eq!(k, RtKind::Time);
}

#[test]
fn memory_huge_vector() {
    let (k, text, _) = fail(
        "#include <vector>\nint test() {\n  std::vector<int> v(1000000000);\n  return static_cast<int>(v.size());\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Memory);
    assert!(text.contains("vector"));
}

#[test]
fn memory_huge_string() {
    let (k, _, _) = fail(
        "#include <string>\nint test() {\n  std::string s(2000000000, 'a');\n  return static_cast<int>(s.size());\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Memory);
}

#[test]
fn memory_huge_resize_and_2d() {
    let (k, _, _) = fail(
        "#include <vector>\nint test() {\n  std::vector<int> v;\n  v.resize(500000000);\n  return 0;\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Memory);
    let (k, _, _) = fail(
        "#include <vector>\nint test() {\n  std::vector<std::vector<int>> g(100000, std::vector<int>(100000, 1));\n  return 0;\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Memory);
    let (k, _, _) = fail(
        "#include <vector>\nint test() {\n  std::vector<int> v(-5);\n  return 0;\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Memory);
}

#[test]
fn memory_growing_forever() {
    let lim = Limits {
        steps: 4_000_000_000,
        ..Limits::default()
    };
    let (k, _, _) = fail(
        "#include <vector>\nint test() {\n  std::vector<int> v;\n  while (true) v.push_back(1);\n  return 0;\n}\n",
        &lim,
    );
    assert_eq!(k, RtKind::Memory);
    let (k, _, _) = fail(
        "#include <string>\nint test() {\n  std::string s = \"ab\";\n  while (true) s += s;\n  return 0;\n}\n",
        &lim,
    );
    assert!(matches!(k, RtKind::Memory | RtKind::Time), "{k:?}");
    let (k, _, _) = fail(
        "#include <map>\nint test() {\n  std::map<int, int> m;\n  for (int i = 0;; ++i) m[i] = i;\n  return 0;\n}\n",
        &lim,
    );
    assert_eq!(k, RtKind::Memory);
    let (k, _, _) = fail(
        "int test() {\n  for (;;) { ListNode* p = new ListNode(1); }\n  return 0;\n}\n",
        &lim,
    );
    assert_eq!(k, RtKind::Memory);
}

#[test]
fn depth_infinite_recursion() {
    let (k, text, _) = fail(
        "int f(int n) {\n  return f(n + 1) + 1;\n}\nint test() {\n  return f(0);\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Depth);
    assert!(text.contains("f(n + 1)"));
}

#[test]
fn depth_mutual_and_lambda_recursion() {
    let (k, _, _) = fail(
        "bool b(int n);\nbool a(int n) { return b(n + 1); }\nbool b(int n) { return a(n + 1); }\nint test() {\n  return a(0);\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Depth);
    let (k, _, _) = fail(
        "#include <functional>\nint test() {\n  std::function<int(int)> f = [&](int n) { return f(n + 1); };\n  return f(0);\n}\n",
        &limits(),
    );
    assert_eq!(k, RtKind::Depth);
}

#[test]
fn depth_limit_is_exact() {
    let src = "int f(int n) {\n  return n == 0 ? 0 : 1 + f(n - 1);\n}\nint test() {\n  return f(198);\n}\n";
    let full = src.to_string();
    let prog = common::parse_any(&full, &[]).expect("parse");
    let rep = run(&prog, "test", vec![], Heap::default(), &limits());
    assert_eq!(rep.result.unwrap().as_int(), Some(198));
    // 199 calls to f plus test is 200 frames: still fine; one more is not
    let src2 = "int f(int n) {\n  return n == 0 ? 0 : 1 + f(n - 1);\n}\nint test() {\n  return f(199);\n}\n";
    let (k, _, _) = fail(src2, &limits());
    assert_eq!(k, RtKind::Depth);
}

#[test]
fn depth_with_huge_limit_does_not_overflow_the_rust_stack() {
    let lim = Limits {
        depth: u32::MAX,
        steps: 4_000_000_000,
        ..Limits::default()
    };
    let (k, _, _) = fail(
        "int f(int n) {\n  return f(n + 1) + 1;\n}\nint test() {\n  return f(0);\n}\n",
        &lim,
    );
    assert!(
        matches!(k, RtKind::Depth | RtKind::Memory | RtKind::Time),
        "{k:?}"
    );
}
