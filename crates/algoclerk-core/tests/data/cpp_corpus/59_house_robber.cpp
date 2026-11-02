#include <algorithm>
#include <tuple>
#include <utility>
#include <vector>

int rob(const std::vector<int>& nums) {
    int take = 0, skip = 0;
    for (int x : nums) {
        int newTake = skip + x;
        skip = std::max(skip, take);
        take = newTake;
    }
    return std::max(take, skip);
}

std::pair<int, int> robPair(const std::vector<int>& nums) {
    auto best = std::make_pair(0, 0);
    for (int x : nums) {
        auto [a, b] = best;
        best = {b + x, std::max(a, b)};
    }
    return best;
}

int robCircle(const std::vector<int>& nums) {
    if (nums.size() == 1) return nums[0];
    std::vector<int> first(nums.begin(), nums.end() - 1);
    std::vector<int> second(nums.begin() + 1, nums.end());
    return std::max(rob(first), rob(second));
}

std::tuple<int, int, int> minMaxSum(const std::vector<int>& v) {
    int lo = v[0], hi = v[0], sum = 0;
    for (int x : v) {
        lo = std::min(lo, x);
        hi = std::max(hi, x);
        sum += x;
    }
    return std::make_tuple(lo, hi, sum);
}
