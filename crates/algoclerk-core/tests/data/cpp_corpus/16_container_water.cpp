#include <algorithm>
#include <vector>

int maxArea(const std::vector<int>& height) {
    int l = 0;
    int r = (int)height.size() - 1;
    int best = 0;
    while (l < r) {
        int h = std::min(height[l], height[r]);
        best = std::max(best, h * (r - l));
        if (height[l] < height[r])
            ++l;
        else
            --r;
    }
    return best;
}

int trap(const std::vector<int>& h) {
    int l = 0, r = (int)h.size() - 1;
    int lmax = 0, rmax = 0, water = 0;
    while (l < r) {
        if (h[l] < h[r]) {
            h[l] >= lmax ? lmax = h[l] : water += lmax - h[l];
            ++l;
        } else {
            h[r] >= rmax ? rmax = h[r] : water += rmax - h[r];
            --r;
        }
    }
    return water;
}
