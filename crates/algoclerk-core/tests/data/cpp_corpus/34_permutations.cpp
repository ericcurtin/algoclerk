#include <algorithm>
#include <vector>

using namespace std;

void backtrack(vector<int>& nums, vector<bool>& used, vector<int>& path,
               vector<vector<int>>& out) {
    if (path.size() == nums.size()) {
        out.push_back(path);
        return;
    }
    for (size_t i = 0; i < nums.size(); ++i) {
        if (used[i]) continue;
        used[i] = true;
        path.push_back(nums[i]);
        backtrack(nums, used, path, out);
        path.pop_back();
        used[i] = false;
    }
}

vector<vector<int>> permute(vector<int> nums) {
    vector<vector<int>> out;
    vector<bool> used(nums.size(), false);
    vector<int> path;
    backtrack(nums, used, path, out);
    return out;
}

vector<vector<int>> permuteNext(vector<int> nums) {
    vector<vector<int>> out;
    sort(nums.begin(), nums.end());
    do {
        out.push_back(nums);
    } while (next_permutation(nums.begin(), nums.end()));
    return out;
}
