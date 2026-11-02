#include <algorithm>
#include <vector>

std::vector<std::vector<int>> threeSum(std::vector<int> nums) {
    std::vector<std::vector<int>> res;
    std::sort(nums.begin(), nums.end());
    const int n = static_cast<int>(nums.size());
    for (int i = 0; i + 2 < n; ++i) {
        if (i > 0 && nums[i] == nums[i - 1]) continue;
        int l = i + 1, r = n - 1;
        while (l < r) {
            int sum = nums[i] + nums[l] + nums[r];
            if (sum < 0) {
                ++l;
            } else if (sum > 0) {
                --r;
            } else {
                res.push_back({nums[i], nums[l], nums[r]});
                while (l < r && nums[l] == nums[l + 1]) ++l;
                while (l < r && nums[r] == nums[r - 1]) --r;
                ++l;
                --r;
            }
        }
    }
    return res;
}
