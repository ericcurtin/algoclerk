#include <vector>

const int DX[4] = {1, -1, 0, 0};
const int DY[4] = {0, 0, 1, -1};

void dfs(std::vector<std::vector<char>>& grid, int x, int y);

int numIslands(std::vector<std::vector<char>>& grid) {
    int count = 0;
    for (int i = 0; i < (int)grid.size(); ++i) {
        for (int j = 0; j < (int)grid[i].size(); ++j) {
            if (grid[i][j] == '1') {
                ++count;
                dfs(grid, i, j);
            }
        }
    }
    return count;
}

void dfs(std::vector<std::vector<char>>& grid, int x, int y) {
    if (x < 0 || y < 0 || x >= (int)grid.size() || y >= (int)grid[0].size()) return;
    if (grid[x][y] != '1') return;
    grid[x][y] = '0';
    for (int d = 0; d < 4; ++d) {
        dfs(grid, x + DX[d], y + DY[d]);
    }
}
