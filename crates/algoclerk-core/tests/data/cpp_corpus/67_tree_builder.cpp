#include <queue>
#include <vector>

struct TreeNode {
    int val;
    TreeNode* left;
    TreeNode* right;
    TreeNode(int x) : val(x), left(nullptr), right(nullptr) {}
    TreeNode(int x, TreeNode* l, TreeNode* r) : val(x), left(l), right(r) {}
};

TreeNode* sortedArrayToBST(const std::vector<int>& nums, int lo, int hi) {
    if (lo > hi) return nullptr;
    int mid = lo + (hi - lo) / 2;
    TreeNode* node = new TreeNode(nums[mid]);
    node->left = sortedArrayToBST(nums, lo, mid - 1);
    node->right = sortedArrayToBST(nums, mid + 1, hi);
    return node;
}

TreeNode* build(const std::vector<int>& nums) {
    return sortedArrayToBST(nums, 0, (int)nums.size() - 1);
}

void destroy(TreeNode* root) {
    if (!root) return;
    destroy(root->left);
    destroy(root->right);
    delete root;
}

int sumTree(TreeNode* root) {
    if (root == nullptr) return 0;
    return root->val + sumTree(root->left) + sumTree(root->right);
}

TreeNode* small() {
    return new TreeNode(2, new TreeNode(1), new TreeNode(3));
}
