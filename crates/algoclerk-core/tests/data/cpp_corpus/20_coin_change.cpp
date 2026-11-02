#include <algorithm>
#include <vector>

using namespace std;

int coinChange(vector<int>& coins, int amount) {
    const int INF = amount + 1;
    vector<int> dp(amount + 1, INF);
    dp[0] = 0;
    for (int a = 1; a <= amount; ++a) {
        for (int c : coins) {
            if (c <= a) {
                dp[a] = min(dp[a], dp[a - c] + 1);
            }
        }
    }
    return dp[amount] > amount ? -1 : dp[amount];
}

int countWays(const vector<int>& coins, int amount) {
    vector<unsigned> ways(amount + 1, 0u);
    ways[0] = 1;
    for (int c : coins)
        for (int a = c; a <= amount; ++a)
            ways[a] += ways[a - c];
    return (int)ways[amount];
}
