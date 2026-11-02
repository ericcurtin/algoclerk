#include <string>
#include <vector>

using namespace std;

void place(int row, int n, vector<string>& board, vector<bool>& cols, vector<bool>& d1,
           vector<bool>& d2, vector<vector<string>>& out) {
    if (row == n) {
        out.push_back(board);
        return;
    }
    for (int c = 0; c < n; ++c) {
        if (cols[c] || d1[row + c] || d2[row - c + n - 1]) continue;
        board[row][c] = 'Q';
        cols[c] = d1[row + c] = d2[row - c + n - 1] = true;
        place(row + 1, n, board, cols, d1, d2, out);
        cols[c] = d1[row + c] = d2[row - c + n - 1] = false;
        board[row][c] = '.';
    }
}

vector<vector<string>> solveNQueens(int n) {
    vector<vector<string>> out;
    vector<string> board(n, string(n, '.'));
    vector<bool> cols(n, false), d1(2 * n, false), d2(2 * n, false);
    place(0, n, board, cols, d1, d2, out);
    return out;
}
