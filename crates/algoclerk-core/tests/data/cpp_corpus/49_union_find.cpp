#include <numeric>
#include <vector>

int findRoot(std::vector<int>& parent, int x) {
    while (parent[x] != x) {
        parent[x] = parent[parent[x]];  // path halving
        x = parent[x];
    }
    return x;
}

bool unite(std::vector<int>& parent, std::vector<int>& rank, int a, int b) {
    int ra = findRoot(parent, a);
    int rb = findRoot(parent, b);
    if (ra == rb) return false;
    if (rank[ra] < rank[rb]) {
        int t = ra;
        ra = rb;
        rb = t;
    }
    parent[rb] = ra;
    if (rank[ra] == rank[rb]) ++rank[ra];
    return true;
}

int countComponents(int n, const std::vector<std::vector<int>>& edges) {
    std::vector<int> parent(n);
    std::iota(parent.begin(), parent.end(), 0);
    std::vector<int> rank(n, 0);
    int comps = n;
    for (const auto& e : edges) {
        if (unite(parent, rank, e[0], e[1])) --comps;
    }
    return comps;
}
