#include <stack>
#include <vector>

std::vector<int> dailyTemperatures(const std::vector<int>& temps) {
    std::vector<int> answer(temps.size(), 0);
    std::stack<int> pending;
    for (int i = 0; i < static_cast<int>(temps.size()); ++i) {
        while (!pending.empty() && temps[pending.top()] < temps[i]) {
            int j = pending.top();
            pending.pop();
            answer[j] = i - j;
        }
        pending.push(i);
    }
    return answer;
}

std::vector<int> nextGreater(const std::vector<int>& a) {
    std::vector<int> res(a.size(), -1);
    std::stack<int> st;
    for (int i = (int)a.size() - 1; i >= 0; --i) {
        while (!st.empty() && st.top() <= a[i]) st.pop();
        if (!st.empty()) res[i] = st.top();
        st.push(a[i]);
    }
    return res;
}
