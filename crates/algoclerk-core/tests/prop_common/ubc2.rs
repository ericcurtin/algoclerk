// SPDX-License-Identifier: Apache-2.0
//! UB cases: pointers, uninitialised reads, invalidation, empty containers.

use super::ub::{Case, Expect::Rt, c};
use algoclerk_core::cpp::interp::RtKind as R;

const NODE: &str = "";

pub fn cases() -> Vec<Case> {
    let _ = NODE;
    vec![
        // ---- null and dangling pointers ----
        c("null_last_next_next", Rt(R::Null), &[], true, r#"
struct ListNode {
    int val;
    ListNode* next;
    ListNode(int x) : val(x), next(nullptr) {}
};
int f() {
    ListNode* a = new ListNode(1);
    a->next = new ListNode(2);
    return a->next->next->val; // @UB
}
"#),
        c("null_from_function", Rt(R::Null), &[], true, r#"
struct ListNode {
    int val;
    ListNode* next;
    ListNode(int x) : val(x), next(nullptr) {}
};
ListNode* find(ListNode* h, int v) {
    while (h && h->val != v) h = h->next;
    return h;
}
int f() {
    ListNode* a = new ListNode(1);
    return find(a, 7)->val; // @UB
}
"#),
        c("null_star_deref", Rt(R::Null), &[], true, r#"
int f() {
    int* p = nullptr;
    return *p; // @UB
}
"#),
        c("null_write_through", Rt(R::Null), &[], true, r#"
int f() {
    int* p = nullptr;
    *p = 5; // @UB
    return 0;
}
"#),
        c("null_tree_child", Rt(R::Null), &[], true, r#"
struct TreeNode {
    int val;
    TreeNode* left;
    TreeNode* right;
    TreeNode(int x) : val(x), left(nullptr), right(nullptr) {}
};
int depth(TreeNode* t) {
    return 1 + std::max(depth(t->left), depth(t->right)); // @UB
}
int f() {
    TreeNode* r = new TreeNode(1);
    return depth(r);
}
"#),
        c("dangling_returned_local_pointer", Rt(R::Dangling), &[], true, r#"
int* make() {
    int x = 3;
    return &x;
}
int f() {
    int* p = make();
    return *p; // @UB
}
"#),
        c("dangling_use_after_delete_int", Rt(R::Dangling), &[], true, r#"
int f() {
    int* p = new int(5);
    delete p;
    return *p; // @UB
}
"#),
        c("dangling_double_delete_node", Rt(R::Dangling), &[], false, r#"
struct ListNode {
    int val;
    ListNode* next;
    ListNode(int x) : val(x), next(nullptr) {}
};
int f() {
    ListNode* a = new ListNode(1);
    delete a;
    delete a; // @UB
    return 0;
}
"#),
        c("dangling_block_scoped_pointer", Rt(R::Dangling), &[], true, r#"
int f() {
    int* p = nullptr;
    {
        int inner = 9;
        p = &inner;
    }
    return *p; // @UB
}
"#),
        c("dangling_field_after_delete", Rt(R::Dangling), &[], true, r#"
struct ListNode {
    int val;
    ListNode* next;
    ListNode(int x) : val(x), next(nullptr) {}
};
int f() {
    ListNode* a = new ListNode(1);
    ListNode* b = a;
    delete a;
    return b->val; // @UB
}
"#),
        // ---- uninitialised reads ----
        c("uninit_classic_sum", Rt(R::Uninit), &[], false, r#"
int f() {
    int sum;
    for (int i = 0; i < 4; ++i) sum += i; // @UB
    return sum;
}
"#),
        c("uninit_one_branch", Rt(R::Uninit), &["0"], false, r#"
int f(int c) {
    int r;
    if (c > 0) r = 1;
    return r; // @UB
}
"#),
        c("uninit_bool_condition", Rt(R::Uninit), &[], false, r#"
int f() {
    bool seen;
    if (seen) return 1; // @UB
    return 0;
}
"#),
        c("uninit_char_return", Rt(R::Uninit), &[], false, r#"
char f() {
    char c;
    return c; // @UB
}
"#),
        c("uninit_array_element", Rt(R::Uninit), &[], false, r#"
int f() {
    int a[3];
    a[0] = 1;
    return a[0] + a[2]; // @UB
}
"#),
        c("uninit_pushed_into_vector", Rt(R::Uninit), &[], false, r#"
int f() {
    int x;
    std::vector<int> v;
    v.push_back(x); // @UB
    return static_cast<int>(v.size());
}
"#),
        c("uninit_increment", Rt(R::Uninit), &[], false, r#"
int f() {
    int x;
    x++; // @UB
    return 0;
}
"#),
        c("uninit_by_reference_param", Rt(R::Uninit), &[], false, r#"
void touch(int& out, bool really) {
    if (really) out = 1;
}
int f() {
    int r;
    touch(r, false);
    return r + 1; // @UB
}
"#),
        c("uninit_loop_bound", Rt(R::Uninit), &[], false, r#"
int f() {
    int n;
    int s = 0;
    for (int i = 0; i < n; ++i) s += i; // @UB
    return s;
}
"#),
        c("uninit_pointer_compare", Rt(R::Uninit), &[], false, r#"
int f() {
    int* p;
    if (p == nullptr) return 1; // @UB
    return 0;
}
"#),
        // ---- iterator and reference invalidation ----
        c("invalid_erase_in_loop", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::vector<int> v = {1, 2, 3, 2, 4};
    for (auto it = v.begin(); it != v.end(); ++it) { // @UB
        if (*it == 2) v.erase(it);
    }
    return static_cast<int>(v.size());
}
"#),
        c("invalid_push_back_while_holding_ref", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::vector<int> v = {1, 2, 3};
    int& first = v[0];
    for (int i = 0; i < 100; ++i) v.push_back(i);
    return first; // @UB
}
"#),
        c("invalid_insert_then_old_iterator", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::vector<int> v = {1, 2, 3};
    auto it = v.begin() + 1;
    v.insert(v.begin(), 0);
    return *it; // @UB
}
"#),
        c("invalid_map_erase_then_increment", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::map<int, int> m = {{1, 1}, {2, 2}, {3, 3}};
    int s = 0;
    for (auto it = m.begin(); it != m.end(); ++it) { // @UB
        s += it->second;
        if (it->first == 2) m.erase(it);
    }
    return s;
}
"#),
        c("invalid_range_for_insert_into_map", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::unordered_map<int, int> m = {{1, 1}};
    int s = 0;
    for (auto& kv : m) { // @UB
        s += kv.second;
        for (int k = 10; k < 100; ++k) m[k] = k;
    }
    return s;
}
"#),
        c("invalid_reference_to_popped", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::vector<int> v = {1, 2, 3};
    int& last = v.back();
    v.pop_back();
    return last; // @UB
}
"#),
        c("invalid_after_clear", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::vector<int> v = {5, 6};
    auto it = v.begin();
    v.clear();
    return *it; // @UB
}
"#),
        c("invalid_deque_push_front_ref", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::deque<int> d = {1, 2, 3};
    int& r = d[1];
    d.push_front(0);
    return r; // @UB
}
"#),
        c("invalid_string_iterator_after_append", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::string s = "ab";
    auto it = s.begin();
    s += "cdefghijklmnopqrstuvwxyz";
    return *it; // @UB
}
"#),
        c("invalid_set_erase_current_then_use", Rt(R::Invalidated), &[], false, r#"
int f() {
    std::set<int> s = {1, 2, 3};
    auto it = s.find(2);
    s.erase(2);
    return *it; // @UB
}
"#),
        // ---- empty containers ----
        c("empty_front_after_drain", Rt(R::Empty), &[], true, r#"
int f() {
    std::vector<int> v = {1, 2};
    while (!v.empty()) v.pop_back();
    return v.front(); // @UB
}
"#),
        c("empty_stack_top_after_pops", Rt(R::Empty), &[], true, r#"
int f() {
    std::stack<int> s;
    s.push(1);
    s.pop();
    return s.top(); // @UB
}
"#),
        c("empty_queue_pop_twice", Rt(R::Empty), &[], true, r#"
int f() {
    std::queue<int> q;
    q.push(1);
    q.pop();
    q.pop(); // @UB
    return 0;
}
"#),
        c("empty_max_element_deref", Rt(R::Empty), &["{}"], true, r#"
int f(const std::vector<int>& v) {
    return *std::max_element(v.begin(), v.end()); // @UB
}
"#),
        c("empty_optional_value", Rt(R::Empty), &[], true, r#"
int f() {
    std::optional<int> o;
    return o.value(); // @UB
}
"#),
        c("empty_deque_pop_front", Rt(R::Empty), &[], true, r#"
int f() {
    std::deque<int> d;
    d.pop_front(); // @UB
    return 0;
}
"#),
        c("empty_set_rbegin_deref", Rt(R::Empty), &[], true, r#"
int f() {
    std::set<int> s;
    return *s.rbegin(); // @UB
}
"#),
        c("empty_pq_top_after_pop", Rt(R::Empty), &[], true, r#"
int f() {
    std::priority_queue<int> pq;
    pq.push(3);
    pq.pop();
    return pq.top(); // @UB
}
"#),
        c("empty_string_back", Rt(R::Empty), &["\"\""], true, r#"
char f(const std::string& s) {
    return s.back(); // @UB
}
"#),
        c("empty_find_end_deref", Rt(R::Empty), &[], true, r#"
int f() {
    std::vector<int> v = {1, 2, 3};
    return *std::find(v.begin(), v.end(), 9); // @UB
}
"#),
    ]
}
