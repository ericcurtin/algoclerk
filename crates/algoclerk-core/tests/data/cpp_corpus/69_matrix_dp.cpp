#include <algorithm>
#include <array>
#include <climits>
#include <vector>

using namespace std;

const int ROWS = 3;
const int COLS = 4;

int minPathSum(vector<vector<int>>& grid) {
    int m = grid.size(), n = grid[0].size();
    vector<vector<int>> dp(m, vector<int>(n, INT_MAX));
    dp[0][0] = grid[0][0];
    for (int i = 0; i < m; ++i) {
        for (int j = 0; j < n; ++j) {
            if (i > 0) dp[i][j] = min(dp[i][j], dp[i - 1][j] + grid[i][j]);
            if (j > 0) dp[i][j] = min(dp[i][j], dp[i][j - 1] + grid[i][j]);
        }
    }
    return dp[m - 1][n - 1];
}

int uniquePaths(int m, int n) {
    vector<int> row(n, 1);
    for (int i = 1; i < m; ++i) {
        for (int j = 1; j < n; ++j) {
            row[j] += row[j - 1];
        }
    }
    return row[n - 1];
}

int fixedGrid() {
    array<array<int, COLS>, ROWS> g{};
    for (int i = 0; i < ROWS; ++i)
        for (int j = 0; j < COLS; ++j)
            g[i][j] = i * COLS + j;
    int total = 0;
    for (const auto& row : g)
        for (int v : row) total += v;
    return total;
}

int knapsack(const vector<int>& w, const vector<int>& val, int cap) {
    vector<int> dp(cap + 1, 0);
    for (size_t i = 0; i < w.size(); ++i) {
        for (int c = cap; c >= w[i]; --c) {
            dp[c] = max(dp[c], dp[c - w[i]] + val[i]);
        }
    }
    return dp[cap];
}
