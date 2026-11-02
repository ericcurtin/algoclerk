#include <algorithm>
#include <stack>
#include <utility>

// Each entry stores the value and the minimum so far.
struct Entry {
    int value;
    int minSoFar;
    Entry(int v, int m) : value(v), minSoFar(m) {}
};

void pushValue(std::stack<Entry>& st, int v) {
    int m = st.empty() ? v : std::min(v, st.top().minSoFar);
    st.push(Entry(v, m));
}

int currentMin(const std::stack<Entry>& st) {
    return st.top().minSoFar;
}

int popValue(std::stack<Entry>& st) {
    int v = st.top().value;
    st.pop();
    return v;
}

int simulate() {
    std::stack<Entry> st;
    pushValue(st, 5);
    pushValue(st, 3);
    pushValue(st, 8);
    popValue(st);
    return currentMin(st);
}
