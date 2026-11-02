#include <algorithm>
#include <vector>

using Matrix = std::vector<std::vector<int>>;

void rotate(Matrix& m) {
    int n = m.size();
    for (int i = 0; i < n; ++i) {
        for (int j = i + 1; j < n; ++j) {
            std::swap(m[i][j], m[j][i]);
        }
    }
    for (auto& row : m) {
        std::reverse(row.begin(), row.end());
    }
}

Matrix transpose(const Matrix& m) {
    if (m.empty()) return {};
    Matrix t(m[0].size(), std::vector<int>(m.size()));
    for (size_t i = 0; i < m.size(); ++i)
        for (size_t j = 0; j < m[i].size(); ++j)
            t[j][i] = m[i][j];
    return t;
}
