#include <algorithm>
#include <vector>

using namespace std;

void search(const vector<int>& cand, int start, int remain, vector<int>& cur,
            vector<vector<int>>& out) {
    if (remain == 0) {
        out.push_back(cur);
        return;
    }
    for (int i = start; i < (int)cand.size(); ++i) {
        if (cand[i] > remain) break;  // sorted, nothing further fits
        cur.push_back(cand[i]);
        search(cand, i, remain - cand[i], cur, out);
        cur.pop_back();
    }
}

vector<vector<int>> combinationSum(vector<int>& candidates, int target) {
    sort(candidates.begin(), candidates.end());
    vector<vector<int>> out;
    vector<int> cur;
    search(candidates, 0, target, cur, out);
    return out;
}
