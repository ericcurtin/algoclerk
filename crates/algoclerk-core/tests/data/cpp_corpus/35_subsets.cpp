#include <functional>
#include <vector>

std::vector<std::vector<int>> subsets(const std::vector<int>& nums) {
    std::vector<std::vector<int>> result;
    std::vector<int> cur;
    std::function<void(int)> go = [&](int start) {
        result.push_back(cur);
        for (int i = start; i < (int)nums.size(); ++i) {
            cur.push_back(nums[i]);
            go(i + 1);
            cur.pop_back();
        }
    };
    go(0);
    return result;
}

std::vector<std::vector<int>> subsetsMask(const std::vector<int>& nums) {
    int n = nums.size();
    std::vector<std::vector<int>> result;
    for (int mask = 0; mask < (1 << n); ++mask) {
        std::vector<int> pick;
        for (int b = 0; b < n; ++b) {
            if (mask & (1 << b)) pick.push_back(nums[b]);
        }
        result.push_back(pick);
    }
    return result;
}
