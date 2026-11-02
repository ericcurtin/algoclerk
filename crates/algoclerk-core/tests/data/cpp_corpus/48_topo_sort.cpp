#include <queue>
#include <vector>

std::vector<int> findOrder(int numCourses, const std::vector<std::vector<int>>& prerequisites) {
    std::vector<std::vector<int>> adj(numCourses);
    std::vector<int> indegree(numCourses, 0);
    for (const auto& edge : prerequisites) {
        adj[edge[1]].push_back(edge[0]);
        ++indegree[edge[0]];
    }
    std::queue<int> ready;
    for (int i = 0; i < numCourses; ++i) {
        if (indegree[i] == 0) ready.push(i);
    }
    std::vector<int> order;
    while (!ready.empty()) {
        int u = ready.front();
        ready.pop();
        order.push_back(u);
        for (int v : adj[u]) {
            if (--indegree[v] == 0) {
                ready.push(v);
            }
        }
    }
    if ((int)order.size() != numCourses) {
        order.clear();
    }
    return order;
}
