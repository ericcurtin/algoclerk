#include <functional>
#include <queue>
#include <unordered_map>
#include <utility>
#include <vector>

using Entry = std::pair<int, int>;  // (count, value)

std::vector<int> topKFrequent(const std::vector<int>& nums, int k) {
    std::unordered_map<int, int> freq;
    for (int x : nums) ++freq[x];
    std::priority_queue<Entry, std::vector<Entry>, std::greater<Entry>> heap;
    for (const auto& kv : freq) {
        heap.push({kv.second, kv.first});
        if (static_cast<int>(heap.size()) > k) heap.pop();
    }
    std::vector<int> out;
    while (!heap.empty()) {
        out.push_back(heap.top().second);
        heap.pop();
    }
    return out;
}
