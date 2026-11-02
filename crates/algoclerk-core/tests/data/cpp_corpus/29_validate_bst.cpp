#include <climits>
#include <functional>

struct TreeNode {
    int val;
    TreeNode* left;
    TreeNode* right;
    TreeNode(int x) : val(x), left(nullptr), right(nullptr) {}
};

bool check(TreeNode* node, long long lo, long long hi) {
    if (!node) return true;
    if (node->val <= lo || node->val >= hi) return false;
    return check(node->left, lo, node->val) && check(node->right, node->val, hi);
}

bool isValidBST(TreeNode* root) {
    return check(root, LLONG_MIN, LLONG_MAX);
}

bool isValidInorder(TreeNode* root) {
    long long prev = LLONG_MIN;
    std::function<bool(TreeNode*)> walk = [&](TreeNode* n) {
        if (n == nullptr) return true;
        if (!walk(n->left)) return false;
        if (n->val <= prev) return false;
        prev = n->val;
        return walk(n->right);
    };
    return walk(root);
}
