#include <unordered_set>

struct ListNode {
    int val;
    ListNode* next;
    ListNode(int x) : val(x), next(nullptr) {}
};

bool hasCycle(ListNode* head) {
    ListNode *slow = head, *fast = head;
    while (fast != nullptr && fast->next != nullptr) {
        slow = slow->next;
        fast = fast->next->next;
        if (slow == fast) return true;
    }
    return false;
}

ListNode* detectCycle(ListNode* head) {
    std::unordered_set<ListNode*> seen;
    for (ListNode* p = head; p; p = p->next) {
        if (!seen.insert(p).second) {
            return p;
        }
    }
    return nullptr;
}
