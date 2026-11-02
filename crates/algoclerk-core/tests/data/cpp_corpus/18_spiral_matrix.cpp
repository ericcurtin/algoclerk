#include <vector>

using namespace std;

vector<int> spiralOrder(vector<vector<int>>& matrix) {
    vector<int> out;
    if (matrix.empty()) return out;
    int top = 0, bottom = matrix.size() - 1;
    int left = 0, right = matrix[0].size() - 1;
    while (top <= bottom && left <= right) {
        for (int j = left; j <= right; ++j) out.push_back(matrix[top][j]);
        top++;
        for (int i = top; i <= bottom; ++i) out.push_back(matrix[i][right]);
        right--;
        if (top <= bottom) {
            for (int j = right; j >= left; --j) out.push_back(matrix[bottom][j]);
            bottom--;
        }
        if (left <= right) {
            for (int i = bottom; i >= top; --i) out.push_back(matrix[i][left]);
            left++;
        }
    }
    return out;
}
