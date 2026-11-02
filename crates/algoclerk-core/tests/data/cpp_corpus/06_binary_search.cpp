#include <vector>

int search(const std::vector<int>& nums, int target) {
    int lo = 0, hi = static_cast<int>(nums.size()) - 1;
    while (lo <= hi) {
        int mid = lo + (hi - lo) / 2;
        if (nums[mid] == target) {
            return mid;
        } else if (nums[mid] < target) {
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    return -1;
}

int lowerBound(const std::vector<int>& a, int x) {
    int lo = 0;
    int hi = a.size();
    while (lo < hi) {
        int mid = (lo + hi) >> 1;
        if (a[mid] < x) lo = mid + 1;
        else hi = mid;
    }
    return lo;
}
