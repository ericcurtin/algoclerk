#include <algorithm>
#include <climits>
#include <vector>

using namespace std;

int maxSubArray(vector<int>& nums) {
    int cur = nums[0];
    int best = nums[0];
    for (int i = 1; i < (int)nums.size(); i++) {
        cur = max(nums[i], cur + nums[i]);
        best = max(best, cur);
    }
    return best;
}

/*
 * Divide and conquer version.
 * Returns the best sum inside [l, r].
 */
int dc(const vector<int>& a, int l, int r) {
    if (l == r) return a[l];
    int m = (l + r) / 2;
    int left = INT_MIN;
    int sum = 0;
    for (int i = m; i >= l; --i) {
        sum += a[i];
        left = max(left, sum);
    }
    int right = INT_MIN;
    sum = 0;
    for (int i = m + 1; i <= r; ++i) {
        sum += a[i];
        right = max(right, sum);
    }
    return max({dc(a, l, m), dc(a, m + 1, r), left + right});
}
