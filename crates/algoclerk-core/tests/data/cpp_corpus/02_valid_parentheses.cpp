#include <stack>
#include <string>

using namespace std;

bool isValid(string s) {
    stack<char> st;
    for (char c : s) {
        switch (c) {
            case '(':
            case '[':
            case '{':
                st.push(c);
                break;
            case ')':
                if (st.empty() || st.top() != '(') return false;
                st.pop();
                break;
            case ']':
                if (st.empty() || st.top() != '[') return false;
                st.pop();
                break;
            case '}':
                if (st.empty() || st.top() != '{') return false;
                st.pop();
                break;
            default:
                break;
        }
    }
    return st.empty();
}
