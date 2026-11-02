#include <algorithm>
#include <climits>
#include <vector>

int maxProfit(const std::vector<int>& prices) {
    int best = 0;
    int lowest = INT_MAX;
    for (int p : prices) {
        lowest = std::min(lowest, p);
        best = std::max(best, p - lowest);
    }
    return best;
}

int maxProfitMany(const std::vector<int>& prices) {
    int total = 0;
    for (size_t i = 1; i < prices.size(); ++i) {
        if (prices[i] > prices[i - 1]) {
            total += prices[i] - prices[i - 1];
        }
    }
    return total;
}
