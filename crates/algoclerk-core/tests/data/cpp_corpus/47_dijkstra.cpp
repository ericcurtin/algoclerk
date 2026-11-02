#include <climits>
#include <functional>
#include <queue>
#include <utility>
#include <vector>

using namespace std;

typedef pair<int, int> pii;

vector<int> dijkstra(int n, const vector<vector<pii>>& graph, int src) {
    vector<int> dist(n, INT_MAX);
    priority_queue<pii, vector<pii>, greater<pii>> pq;
    dist[src] = 0;
    pq.push(make_pair(0, src));
    while (!pq.empty()) {
        pii top = pq.top();
        pq.pop();
        int d = top.first, u = top.second;
        if (d > dist[u]) continue;
        for (const pii& edge : graph[u]) {
            int v = edge.first, w = edge.second;
            if (dist[u] + w < dist[v]) {
                dist[v] = dist[u] + w;
                pq.push({dist[v], v});
            }
        }
    }
    return dist;
}
