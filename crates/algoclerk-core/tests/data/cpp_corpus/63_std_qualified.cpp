#include <algorithm>
#include <functional>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

typedef std::pair<std::string, int> Entry;
using Table = std::map<std::string, std::vector<int>>;

std::optional<int> firstEven(const std::vector<int>& v) {
    for (int x : v) {
        if (x % 2 == 0) return x;
    }
    return std::nullopt;
}

std::vector<Entry> countWords(const std::vector<std::string>& words) {
    std::map<std::string, int> counts;
    for (const std::string& w : words) {
        counts[w] += 1;
    }
    std::vector<Entry> out(counts.begin(), counts.end());
    std::sort(out.begin(), out.end(), [](const Entry& a, const Entry& b) {
        if (a.second != b.second) return a.second > b.second;
        return a.first < b.first;
    });
    return out;
}

Table index(const std::vector<std::string>& words) {
    Table t;
    for (int i = 0; i < static_cast<int>(words.size()); ++i) {
        t[words[i]].push_back(i);
    }
    return t;
}

std::set<int> uniqueSorted(const std::vector<int>& v) {
    return std::set<int>(v.begin(), v.end());
}

int useTuple() {
    std::tuple<int, std::string, bool> t = std::make_tuple(1, "x", true);
    auto first = std::get<0>(t);
    return std::get<2>(t) ? first : -first;
}
