#include <deque>
#include <vector>

std::vector<int> maxSlidingWindow(const std::vector<int>& nums, int k) {
    std::deque<int> dq;  // indices, values decreasing
    std::vector<int> out;
    for (int i = 0; i < (int)nums.size(); ++i) {
        while (!dq.empty() && dq.front() <= i - k) dq.pop_front();
        while (!dq.empty() && nums[dq.back()] < nums[i]) dq.pop_back();
        dq.push_back(i);
        if (i >= k - 1) out.push_back(nums[dq.front()]);
    }
    return out;
}
