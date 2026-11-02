#include <map>
#include <set>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

using namespace std;

vector<int> intersection(const vector<int>& a, const vector<int>& b) {
    unordered_set<int> sa(a.begin(), a.end());
    set<int> common;
    for (int x : b) {
        if (sa.count(x)) common.insert(x);
    }
    return vector<int>(common.begin(), common.end());
}

bool containsDuplicate(const vector<int>& nums) {
    unordered_set<int> seen;
    for (int x : nums) {
        if (!seen.insert(x).second) return true;
    }
    return false;
}

int longestConsecutive(const vector<int>& nums) {
    unordered_set<int> s(nums.begin(), nums.end());
    int best = 0;
    for (int x : s) {
        if (s.count(x - 1)) continue;
        int len = 1;
        while (s.count(x + len)) ++len;
        best = len > best ? len : best;
    }
    return best;
}

int majority(const vector<int>& nums) {
    unordered_map<int, int> cnt;
    int best = nums[0];
    for (int x : nums) {
        if (++cnt[x] > cnt[best]) best = x;
    }
    return best;
}

map<int, int> histogram(const vector<int>& v) {
    map<int, int> h;
    for (int x : v) h[x]++;
    for (auto it = h.begin(); it != h.end();) {
        if (it->second < 2) it = h.erase(it);
        else ++it;
    }
    return h;
}
