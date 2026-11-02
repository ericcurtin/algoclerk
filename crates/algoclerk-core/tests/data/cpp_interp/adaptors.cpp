//// stack_basic
#include <stack>
#include <vector>
std::vector<int> test() {
    std::stack<int> s;
    for (int i = 1; i <= 5; ++i) s.push(i * i);
    std::vector<int> r;
    r.push_back(static_cast<int>(s.size()));
    r.push_back(s.top());
    s.pop();
    s.top() += 100;
    r.push_back(s.top());
    s.emplace(77);
    while (!s.empty()) {
        r.push_back(s.top());
        s.pop();
    }
    r.push_back(s.empty());
    return r;
}
//// stack_balanced_parens
#include <stack>
#include <string>
#include <vector>
bool ok(const std::string& s) {
    std::stack<char> st;
    for (char c : s) {
        if (c == '(' || c == '[' || c == '{') st.push(c);
        else {
            if (st.empty()) return false;
            char o = st.top();
            st.pop();
            if ((c == ')' && o != '(') || (c == ']' && o != '[') || (c == '}' && o != '{')) return false;
        }
    }
    return st.empty();
}
std::vector<int> test() { return {ok("()[]{}"), ok("([)]"), ok("(("), ok(""), ok("{[()]}"), ok(")(")}; }
//// queue_basic
#include <queue>
#include <vector>
std::vector<int> test() {
    std::queue<int> q;
    for (int i = 0; i < 5; ++i) q.push(i * 2);
    std::vector<int> r;
    r.push_back(q.front());
    r.push_back(q.back());
    r.push_back(static_cast<int>(q.size()));
    q.pop();
    q.pop();
    q.push(100);
    q.emplace(200);
    q.front() += 1;
    while (!q.empty()) {
        r.push_back(q.front());
        q.pop();
    }
    return r;
}
//// queue_bfs_grid
#include <queue>
#include <utility>
#include <vector>
int test() {
    std::vector<std::vector<int>> g = {
        {0, 0, 0, 0, 0}, {1, 1, 0, 1, 0}, {0, 0, 0, 1, 0}, {0, 1, 1, 1, 0}, {0, 0, 0, 0, 0}};
    int n = static_cast<int>(g.size()), m = static_cast<int>(g[0].size());
    std::vector<std::vector<int>> dist(n, std::vector<int>(m, -1));
    std::queue<std::pair<int, int>> q;
    q.push({0, 0});
    dist[0][0] = 0;
    int dr[4] = {1, -1, 0, 0}, dc[4] = {0, 0, 1, -1};
    while (!q.empty()) {
        auto [r, c] = q.front();
        q.pop();
        for (int d = 0; d < 4; ++d) {
            int nr = r + dr[d], nc = c + dc[d];
            if (nr < 0 || nc < 0 || nr >= n || nc >= m || g[nr][nc] || dist[nr][nc] >= 0) continue;
            dist[nr][nc] = dist[r][c] + 1;
            q.push({nr, nc});
        }
    }
    return dist[4][4] * 100 + dist[2][2];
}
//// deque_basic
#include <deque>
#include <vector>
std::vector<int> test() {
    std::deque<int> d;
    for (int i = 0; i < 4; ++i) {
        d.push_back(i);
        d.push_front(-i);
    }
    std::vector<int> r(d.begin(), d.end());
    r.push_back(-99);
    d.pop_front();
    d.pop_back();
    r.push_back(d.front());
    r.push_back(d.back());
    r.push_back(d[2]);
    d[2] = 55;
    r.push_back(d.at(2));
    r.push_back(static_cast<int>(d.size()));
    d.insert(d.begin() + 1, 1000);
    d.erase(d.end() - 1);
    for (int x : d) r.push_back(x);
    d.clear();
    r.push_back(d.empty());
    return r;
}
//// deque_sliding_window_max
#include <deque>
#include <vector>
std::vector<int> test() {
    std::vector<int> a = {1, 3, -1, -3, 5, 3, 6, 7};
    int k = 3;
    std::deque<int> dq;
    std::vector<int> out;
    for (int i = 0; i < static_cast<int>(a.size()); ++i) {
        while (!dq.empty() && dq.front() <= i - k) dq.pop_front();
        while (!dq.empty() && a[dq.back()] < a[i]) dq.pop_back();
        dq.push_back(i);
        if (i >= k - 1) out.push_back(a[dq.front()]);
    }
    return out;
}
//// pq_basic
#include <queue>
#include <vector>
std::vector<int> test() {
    std::priority_queue<int> pq;
    for (int x : {5, 1, 8, 3, 9, 2, 8}) pq.push(x);
    std::vector<int> r;
    r.push_back(pq.top());
    r.push_back(static_cast<int>(pq.size()));
    while (!pq.empty()) {
        r.push_back(pq.top());
        pq.pop();
    }
    return r;
}
//// pq_greater
#include <functional>
#include <queue>
#include <vector>
std::vector<int> test() {
    std::priority_queue<int, std::vector<int>, std::greater<int>> pq;
    for (int x : {5, 1, 8, 3, 9, 2, 8}) pq.push(x);
    std::vector<int> r;
    while (!pq.empty()) {
        r.push_back(pq.top());
        pq.pop();
    }
    return r;
}
//// pq_pairs
#include <functional>
#include <queue>
#include <utility>
#include <vector>
std::vector<int> test() {
    std::priority_queue<std::pair<int, int>, std::vector<std::pair<int, int>>, std::greater<std::pair<int, int>>> pq;
    pq.push({5, 1});
    pq.push({2, 9});
    pq.push({2, 3});
    pq.push({7, 0});
    pq.emplace(1, 1);
    std::vector<int> r;
    while (!pq.empty()) {
        r.push_back(pq.top().first * 10 + pq.top().second);
        pq.pop();
    }
    std::priority_queue<std::pair<int, int>> mx;
    mx.push({1, 2});
    mx.push({3, 1});
    mx.push({3, 4});
    while (!mx.empty()) {
        auto [a, b] = mx.top();
        mx.pop();
        r.push_back(a * 10 + b);
    }
    return r;
}
//// pq_lambda_cmp
#include <queue>
#include <vector>
std::vector<int> test() {
    auto cmp = [](int a, int b) { return a % 10 < b % 10 || (a % 10 == b % 10 && a < b); };
    std::priority_queue<int, std::vector<int>, decltype(cmp)> pq(cmp);
    for (int x : {15, 21, 33, 9, 40, 12, 29}) pq.push(x);
    std::vector<int> r;
    while (!pq.empty()) {
        r.push_back(pq.top());
        pq.pop();
    }
    return r;
}
//// pq_dijkstra
#include <functional>
#include <queue>
#include <utility>
#include <vector>
std::vector<int> test() {
    int n = 6;
    std::vector<std::vector<std::pair<int, int>>> adj(n);
    auto edge = [&](int a, int b, int w) {
        adj[a].push_back({b, w});
        adj[b].push_back({a, w});
    };
    edge(0, 1, 7);
    edge(0, 2, 9);
    edge(0, 5, 14);
    edge(1, 2, 10);
    edge(1, 3, 15);
    edge(2, 3, 11);
    edge(2, 5, 2);
    edge(3, 4, 6);
    edge(4, 5, 9);
    std::vector<int> dist(n, 1 << 30);
    std::priority_queue<std::pair<int, int>, std::vector<std::pair<int, int>>, std::greater<std::pair<int, int>>> pq;
    dist[0] = 0;
    pq.push({0, 0});
    while (!pq.empty()) {
        auto [d, u] = pq.top();
        pq.pop();
        if (d > dist[u]) continue;
        for (auto [v, w] : adj[u]) {
            if (dist[u] + w < dist[v]) {
                dist[v] = dist[u] + w;
                pq.push({dist[v], v});
            }
        }
    }
    return dist;
}
//// pq_top_k
#include <functional>
#include <queue>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {7, 2, 9, 4, 4, 1, 8, 3, 6, 5};
    std::priority_queue<int, std::vector<int>, std::greater<int>> pq;
    for (int x : v) {
        pq.push(x);
        if (pq.size() > 4) pq.pop();
    }
    std::vector<int> r;
    while (!pq.empty()) {
        r.push_back(pq.top());
        pq.pop();
    }
    return r;
}
//// list_basic
#include <list>
#include <vector>
std::vector<int> test() {
    std::list<int> l;
    for (int i = 0; i < 5; ++i) {
        l.push_back(i);
        l.push_front(10 + i);
    }
    std::vector<int> r(l.begin(), l.end());
    r.push_back(-1);
    l.pop_front();
    l.pop_back();
    r.push_back(l.front());
    r.push_back(l.back());
    r.push_back(static_cast<int>(l.size()));
    auto it = l.begin();
    ++it;
    ++it;
    l.insert(it, 555);
    it = l.erase(it);
    r.push_back(*it);
    l.remove(12);
    l.reverse();
    for (int x : l) r.push_back(x);
    return r;
}
//// list_sort_unique_remove_if
#include <list>
#include <vector>
std::vector<int> test() {
    std::list<int> l = {5, 3, 5, 1, 3, 3, 9, 1};
    l.sort();
    std::vector<int> r(l.begin(), l.end());
    l.unique();
    r.push_back(-1);
    r.insert(r.end(), l.begin(), l.end());
    l.remove_if([](int x) { return x > 4; });
    r.push_back(-1);
    r.insert(r.end(), l.begin(), l.end());
    l.sort([](int a, int b) { return a > b; });
    r.push_back(-1);
    r.insert(r.end(), l.begin(), l.end());
    return r;
}
//// list_erase_in_loop
#include <list>
#include <vector>
std::vector<int> test() {
    std::list<int> l;
    for (int i = 0; i < 10; ++i) l.push_back(i);
    for (auto it = l.begin(); it != l.end();) {
        if (*it % 3 == 0) it = l.erase(it);
        else ++it;
    }
    std::vector<int> r;
    for (auto it = l.rbegin(); it != l.rend(); ++it) r.push_back(*it);
    return r;
}
//// lru_cache_list_map
#include <list>
#include <unordered_map>
#include <utility>
#include <vector>
std::vector<int> test() {
    int cap = 2;
    std::list<std::pair<int, int>> order;
    std::unordered_map<int, std::list<std::pair<int, int>>::iterator> pos;
    std::vector<int> out;
    auto put = [&](int k, int v) {
        auto it = pos.find(k);
        if (it != pos.end()) {
            it->second->second = v;
            order.erase(it->second);
            order.push_front({k, v});
            it->second = order.begin();
            return;
        }
        if (static_cast<int>(order.size()) == cap) {
            pos.erase(order.back().first);
            order.pop_back();
        }
        order.push_front({k, v});
        pos[k] = order.begin();
    };
    put(1, 1);
    put(2, 2);
    put(1, 10);
    put(3, 3);
    for (auto& p : order) out.push_back(p.first * 100 + p.second);
    out.push_back(static_cast<int>(pos.size()));
    return out;
}
