#include <algorithm>
#include <functional>
#include <numeric>
#include <queue>
#include <vector>

using namespace std;

int lambdas(vector<int> v) {
    int offset = 3;
    auto addOffset = [offset](int x) { return x + offset; };
    auto addRef = [&offset](int x) { offset += x; return offset; };
    auto all = [=](int x) { return x * offset; };
    auto byRef = [&](int x) { offset = x; };
    int counter = 0;
    auto tick = [counter]() mutable { return ++counter; };
    auto generic = [](auto a, auto b) { return a < b; };
    auto typed = [](int a, int b) -> int { return a - b; };
    auto none = [] {};
    none();
    byRef(2);
    sort(v.begin(), v.end(), generic);
    sort(v.begin(), v.end(), [](int a, int b) { return a > b; });
    int total = accumulate(v.begin(), v.end(), 0, [](int acc, int x) { return acc + x; });
    auto cmp = [](const pair<int, int>& a, const pair<int, int>& b) { return a.second > b.second; };
    priority_queue<pair<int, int>, vector<pair<int, int>>, decltype(cmp)> pq(cmp);
    pq.push({1, 2});
    function<int(int)> fact = [&](int n) { return n <= 1 ? 1 : n * fact(n - 1); };
    int removed = count_if(v.begin(), v.end(), [&](int x) {
        if (x < offset) {
            return true;
        }
        return false;
    });
    return addOffset(1) + addRef(2) + all(3) + tick() + typed(5, 4) + total + fact(4) + removed;
}
