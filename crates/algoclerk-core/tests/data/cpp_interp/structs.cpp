//// ptr_list_build_reverse
ListNode* test() {
    ListNode* head = nullptr;
    for (int i = 5; i >= 1; --i) head = new ListNode(i, head);
    ListNode* prev = nullptr;
    ListNode* cur = head;
    while (cur) {
        ListNode* nxt = cur->next;
        cur->next = prev;
        prev = cur;
        cur = nxt;
    }
    return prev;
}
//// ptr_list_dummy_merge
#include <vector>
ListNode* merge(ListNode* a, ListNode* b) {
    ListNode dummy(0);
    ListNode* tail = &dummy;
    while (a && b) {
        if (a->val <= b->val) {
            tail->next = a;
            a = a->next;
        } else {
            tail->next = b;
            b = b->next;
        }
        tail = tail->next;
    }
    tail->next = a ? a : b;
    return dummy.next;
}
ListNode* build(const std::vector<int>& arr) {
    ListNode* h = nullptr;
    for (int i = static_cast<int>(arr.size()) - 1; i >= 0; --i) h = new ListNode(arr[i], h);
    return h;
}
ListNode* test() { return merge(build({1, 4, 6, 9}), build({2, 3, 7, 8, 10})); }
//// ptr_list_cycle_middle
int test() {
    ListNode* head = new ListNode(1);
    ListNode* t = head;
    for (int i = 2; i <= 7; ++i) {
        t->next = new ListNode(i);
        t = t->next;
    }
    ListNode *slow = head, *fast = head;
    while (fast && fast->next) {
        slow = slow->next;
        fast = fast->next->next;
    }
    int mid = slow->val;
    t->next = head->next->next;
    ListNode *s2 = head, *f2 = head;
    bool cyc = false;
    while (f2 && f2->next) {
        s2 = s2->next;
        f2 = f2->next->next;
        if (s2 == f2) {
            cyc = true;
            break;
        }
    }
    return mid * 10 + cyc;
}
//// ptr_list_delete_nodes
int test() {
    ListNode* a = new ListNode(1);
    a->next = new ListNode(2);
    a->next->next = new ListNode(3);
    ListNode* del = a->next;
    a->next = del->next;
    delete del;
    int s = 0;
    for (ListNode* p = a; p != nullptr; p = p->next) s = s * 10 + p->val;
    delete a->next;
    delete a;
    return s;
}
//// ptr_tree_insert_inorder
#include <algorithm>
#include <vector>
TreeNode* insert(TreeNode* root, int v) {
    if (!root) return new TreeNode(v);
    if (v < root->val) root->left = insert(root->left, v);
    else root->right = insert(root->right, v);
    return root;
}
void inorder(TreeNode* n, std::vector<int>& out) {
    if (!n) return;
    inorder(n->left, out);
    out.push_back(n->val);
    inorder(n->right, out);
}
int height(TreeNode* n) { return n ? 1 + std::max(height(n->left), height(n->right)) : 0; }
std::vector<int> test() {
    TreeNode* root = nullptr;
    for (int v : {50, 30, 70, 20, 40, 60, 80, 35, 45, 65}) root = insert(root, v);
    std::vector<int> out;
    inorder(root, out);
    out.push_back(height(root));
    return out;
}
//// ptr_tree_level_order_result
#include <queue>
#include <vector>
TreeNode* test() {
    TreeNode* r = new TreeNode(1);
    r->left = new TreeNode(2);
    r->right = new TreeNode(3);
    r->left->right = new TreeNode(4);
    r->right->left = new TreeNode(5);
    r->right->right = new TreeNode(6);
    r->right->right->left = new TreeNode(7);
    return r;
}
//// ptr_tree_bfs_levels
#include <queue>
#include <vector>
std::vector<std::vector<int>> test() {
    TreeNode* r = new TreeNode(3);
    r->left = new TreeNode(9);
    r->right = new TreeNode(20);
    r->right->left = new TreeNode(15);
    r->right->right = new TreeNode(7);
    std::vector<std::vector<int>> out;
    std::queue<TreeNode*> q;
    q.push(r);
    while (!q.empty()) {
        int n = static_cast<int>(q.size());
        std::vector<int> level;
        for (int i = 0; i < n; ++i) {
            TreeNode* c = q.front();
            q.pop();
            level.push_back(c->val);
            if (c->left) q.push(c->left);
            if (c->right) q.push(c->right);
        }
        out.push_back(level);
    }
    return out;
}
//// ptr_tree_invert_depth
TreeNode* invert(TreeNode* n) {
    if (!n) return nullptr;
    TreeNode* l = invert(n->left);
    TreeNode* r = invert(n->right);
    n->left = r;
    n->right = l;
    return n;
}
TreeNode* test() {
    TreeNode* r = new TreeNode(4);
    r->left = new TreeNode(2);
    r->right = new TreeNode(7);
    r->left->left = new TreeNode(1);
    r->left->right = new TreeNode(3);
    r->right->left = new TreeNode(6);
    r->right->right = new TreeNode(9);
    return invert(r);
}
//// ptr_null_compare_and_swap
int test() {
    ListNode* a = new ListNode(1);
    ListNode* b = new ListNode(2);
    ListNode* c = a;
    int r = 0;
    if (a == c) r += 1;
    if (a != b) r += 10;
    if (a->next == nullptr) r += 100;
    if (!b->next) r += 1000;
    ListNode* t = a;
    a = b;
    b = t;
    r += a->val * 10000 + b->val * 100000;
    return r;
}
//// struct_player_defined
#include <string>
#include <vector>
struct Point {
    int x;
    int y;
    Point(int a, int b) : x(a), y(b) {}
};
struct Counter {
    int hits = 0;
    std::string name = "c";
};
struct Pair2 {
    int a;
    long long b;
};
std::vector<int> test() {
    Point p(3, 4);
    Point q = Point(5, 6);
    Counter c;
    c.hits += 2;
    Counter d{7, "dd"};
    Pair2 e{1, 2};
    Pair2 f = {3, 4};
    std::vector<Point> pts;
    pts.push_back(p);
    pts.push_back(Point(7, 8));
    pts.emplace_back(9, 10);
    pts[0].x = 100;
    int sum = 0;
    for (const auto& pt : pts) sum += pt.x + pt.y;
    return {p.x, p.y, q.x, q.y, c.hits, static_cast<int>(c.name.size()), d.hits, static_cast<int>(d.name.size()), static_cast<int>(e.b + f.b), sum};
}
//// struct_copy_semantics
struct Box {
    int v;
    Box(int x) : v(x) {}
};
int test() {
    Box a(1);
    Box b = a;
    b.v = 5;
    Box* p = &a;
    p->v += 10;
    Box c(0);
    c = a;
    c.v++;
    return a.v * 100 + b.v * 10 + c.v;
}
//// struct_ctor_body_and_this
struct Acc {
    int total;
    int n;
    Acc(int a, int b) : total(0), n(0) {
        for (int i = a; i <= b; ++i) {
            total += i;
            n++;
        }
    }
};
int test() {
    Acc a(1, 10);
    Acc b(5, 5);
    return a.total * 1000 + a.n * 100 + b.total * 10 + b.n;
}
//// ptr_new_scalar_and_ref
int test() {
    int* p = new int(5);
    *p += 3;
    int* q = p;
    *q *= 2;
    int x = 10;
    int* r = &x;
    *r += 1;
    int& ref = x;
    ref += 1;
    int result = *p + x;
    delete p;
    return result;
}
