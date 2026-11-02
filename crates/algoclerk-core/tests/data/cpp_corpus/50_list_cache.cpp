#include <list>
#include <unordered_map>
#include <utility>

// A tiny LRU simulation driven by a sequence of keys. Returns the number of misses.
int countMisses(const std::list<int>& accesses, int capacity) {
    std::list<int> order;  // front = most recent
    std::unordered_map<int, std::list<int>::iterator> where;
    int misses = 0;
    for (int key : accesses) {
        auto found = where.find(key);
        if (found != where.end()) {
            order.erase(found->second);
        } else {
            ++misses;
            if ((int)order.size() == capacity) {
                where.erase(order.back());
                order.pop_back();
            }
        }
        order.push_front(key);
        where[key] = order.begin();
    }
    return misses;
}
