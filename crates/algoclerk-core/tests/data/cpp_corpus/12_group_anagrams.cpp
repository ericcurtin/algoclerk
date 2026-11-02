#include <algorithm>
#include <string>
#include <unordered_map>
#include <vector>

std::vector<std::vector<std::string>> groupAnagrams(const std::vector<std::string>& strs) {
    std::unordered_map<std::string, std::vector<std::string>> groups;
    for (const auto& s : strs) {
        std::string key = s;
        std::sort(key.begin(), key.end());
        groups[key].push_back(s);
    }
    std::vector<std::vector<std::string>> result;
    for (auto& [key, members] : groups) {
        std::sort(members.begin(), members.end());
        result.push_back(members);
    }
    std::sort(result.begin(), result.end());
    return result;
}
