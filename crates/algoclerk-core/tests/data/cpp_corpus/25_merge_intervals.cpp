#include <algorithm>
#include <vector>

using namespace std;

vector<vector<int>> merge(vector<vector<int>>& intervals) {
    sort(intervals.begin(), intervals.end(),
         [](const vector<int>& a, const vector<int>& b) { return a[0] < b[0]; });
    vector<vector<int>> merged;
    for (const auto& cur : intervals) {
        if (merged.empty() || merged.back()[1] < cur[0]) {
            merged.push_back(cur);
        } else {
            merged.back()[1] = max(merged.back()[1], cur[1]);
        }
    }
    return merged;
}

int eraseOverlap(vector<vector<int>> intervals) {
    sort(intervals.begin(), intervals.end(), [](auto& a, auto& b) {
        return a[1] < b[1];
    });
    int kept = 0;
    int end = -1000000;
    for (auto& iv : intervals) {
        if (iv[0] >= end) {
            kept++;
            end = iv[1];
        }
    }
    return (int)intervals.size() - kept;
}
