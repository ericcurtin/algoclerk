#include <algorithm>
#include <stack>

struct TreeNode {
    int val;
    TreeNode* left;
    TreeNode* right;
    TreeNode(int x) : val(x), left(nullptr), right(nullptr) {}
};

TreeNode* invertTree(TreeNode* root) {
    if (root == nullptr)
        return nullptr;
    TreeNode* tmp = root->left;
    root->left = invertTree(root->right);
    root->right = invertTree(tmp);
    return root;
}

TreeNode* invertIterative(TreeNode* root) {
    std::stack<TreeNode*> todo;
    if (root) todo.push(root);
    while (!todo.empty()) {
        TreeNode* n = todo.top();
        todo.pop();
        std::swap(n->left, n->right);
        if (n->left) todo.push(n->left);
        if (n->right) todo.push(n->right);
    }
    return root;
}
