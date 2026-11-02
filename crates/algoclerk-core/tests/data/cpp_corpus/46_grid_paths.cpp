#include <queue>
#include <utility>
#include <vector>

using namespace std;

const int DR[4] = {1, -1, 0, 0};
const int DC[4] = {0, 0, 1, -1};

int shortestPath(vector<vector<int>>& grid) {
    int n = grid.size();
    if (n == 0 || grid[0][0] == 1 || grid[n - 1][n - 1] == 1) return -1;
    vector<vector<int>> dist(n, vector<int>(n, -1));
    queue<pair<int, int>> q;
    q.push({0, 0});
    dist[0][0] = 1;
    while (!q.empty()) {
        auto [x, y] = q.front();
        q.pop();
        if (x == n - 1 && y == n - 1) return dist[x][y];
        for (int k = 0; k < 4; ++k) {
            int nx = x + DR[k], ny = y + DC[k];
            if (nx < 0 || ny < 0 || nx >= n || ny >= n) continue;
            if (grid[nx][ny] == 1 || dist[nx][ny] != -1) continue;
            dist[nx][ny] = dist[x][y] + 1;
            q.push({nx, ny});
        }
    }
    return -1;
}
