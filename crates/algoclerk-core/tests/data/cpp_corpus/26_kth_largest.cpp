#include <functional>
#include <queue>
#include <vector>

int findKthLargest(const std::vector<int>& nums, int k) {
    std::priority_queue<int, std::vector<int>, std::greater<int>> pq;
    for (int x : nums) {
        pq.push(x);
        if ((int)pq.size() > k) pq.pop();
    }
    return pq.top();
}

int kthMax(const std::vector<int>& nums, int k) {
    std::priority_queue<int> pq(nums.begin(), nums.end());
    while (--k > 0) {
        pq.pop();
    }
    return pq.top();
}
