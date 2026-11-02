//// map_basic
#include <map>
#include <string>
std::map<std::string, int> test() {
    std::map<std::string, int> m;
    m["pear"] = 3;
    m["apple"] += 5;
    m["fig"]++;
    m.insert({"kiwi", 7});
    m.insert(std::make_pair(std::string("apple"), 100));
    m.emplace("lime", 1);
    m["pear"] *= 2;
    m.erase("fig");
    return m;
}
//// map_find_count_at
#include <map>
#include <vector>
std::vector<int> test() {
    std::map<int, int> m = {{5, 50}, {1, 10}, {3, 30}};
    std::vector<int> r;
    r.push_back(m.count(3));
    r.push_back(m.count(4));
    r.push_back(m.find(3)->second);
    r.push_back(m.find(4) == m.end());
    r.push_back(m.at(5));
    r.push_back(static_cast<int>(m.size()));
    r.push_back(m.begin()->first);
    r.push_back(m.rbegin()->first);
    r.push_back(m[9]);
    r.push_back(static_cast<int>(m.size()));
    m.at(1) = 11;
    r.push_back(m[1]);
    return r;
}
//// map_iterate_order
#include <map>
#include <vector>
std::vector<int> test() {
    std::map<int, int> m;
    for (int i : {9, 2, 7, 4, 5, 1}) m[i] = i * i;
    std::vector<int> r;
    for (const auto& kv : m) {
        r.push_back(kv.first);
        r.push_back(kv.second);
    }
    for (auto& [k, v] : m) v += k;
    for (auto it = m.begin(); it != m.end(); ++it) r.push_back(it->second);
    for (auto it = m.rbegin(); it != m.rend(); ++it) r.push_back(it->first);
    return r;
}
//// map_bounds
#include <map>
#include <vector>
std::vector<int> test() {
    std::map<int, char> m = {{10, 'a'}, {20, 'b'}, {30, 'c'}, {40, 'd'}};
    std::vector<int> r;
    auto lb = m.lower_bound(20);
    auto ub = m.upper_bound(20);
    r.push_back(lb->first);
    r.push_back(ub->first);
    r.push_back(m.lower_bound(25)->first);
    r.push_back(m.lower_bound(5)->first);
    r.push_back(m.upper_bound(40) == m.end());
    r.push_back(m.lower_bound(41) == m.end());
    auto it = m.upper_bound(25);
    --it;
    r.push_back(it->first);
    auto er = m.equal_range(30);
    r.push_back(er.first->first);
    r.push_back(er.second->first);
    return r;
}
//// map_erase_forms
#include <map>
#include <vector>
std::vector<int> test() {
    std::map<int, int> m;
    for (int i = 0; i < 10; ++i) m[i] = i;
    std::vector<int> r;
    r.push_back(static_cast<int>(m.erase(3)));
    r.push_back(static_cast<int>(m.erase(33)));
    auto it = m.find(5);
    it = m.erase(it);
    r.push_back(it->first);
    m.erase(m.begin());
    m.erase(m.find(7), m.end());
    for (auto& [k, v] : m) r.push_back(k);
    for (auto i = m.begin(); i != m.end();) {
        if (i->first % 2 == 0) i = m.erase(i);
        else ++i;
    }
    for (auto& kv : m) r.push_back(kv.first * 100);
    return r;
}
//// map_nested_vec_values
#include <map>
#include <string>
#include <vector>
std::map<std::string, std::vector<int>> test() {
    std::map<std::string, std::vector<int>> m;
    std::vector<std::string> keys = {"b", "a", "b", "c", "a", "b"};
    int i = 0;
    for (const auto& k : keys) m[k].push_back(i++);
    return m;
}
//// map_pair_keys
#include <map>
#include <utility>
#include <vector>
std::map<std::pair<int, int>, int> test() {
    std::map<std::pair<int, int>, int> m;
    for (int i = 2; i >= 0; --i)
        for (int j = 0; j < 2; ++j) m[{i, j}] = i * 10 + j;
    m[std::make_pair(1, 5)] = 99;
    return m;
}
//// map_try_emplace_insert_or_assign
#include <map>
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::map<int, std::string> m;
    std::vector<std::string> r;
    auto a = m.try_emplace(1, "one");
    auto b = m.try_emplace(1, "uno");
    auto c = m.insert_or_assign(1, "ein");
    auto d = m.insert_or_assign(2, "two");
    auto e = m.emplace(3, "three");
    auto f = m.emplace(3, "drei");
    auto g = m.insert({4, "four"});
    r.push_back(std::to_string(a.second) + std::to_string(b.second) + std::to_string(c.second) +
                std::to_string(d.second) + std::to_string(e.second) + std::to_string(f.second) +
                std::to_string(g.second));
    for (auto& [k, v] : m) r.push_back(std::to_string(k) + v);
    r.push_back(a.first->second);
    return r;
}
//// map_word_count
#include <map>
#include <string>
#include <vector>
std::vector<std::pair<std::string, int>> test() {
    std::string text = "the cat and the dog and the bird";
    std::map<std::string, int> cnt;
    std::string w;
    for (char c : text + " ") {
        if (c == ' ') {
            if (!w.empty()) cnt[w]++;
            w.clear();
        } else w += c;
    }
    return std::vector<std::pair<std::string, int>>(cnt.begin(), cnt.end());
}
//// set_basic
#include <set>
#include <vector>
std::vector<int> test() {
    std::set<int> s;
    for (int x : {5, 3, 8, 3, 1, 9, 5}) s.insert(x);
    std::vector<int> r;
    r.push_back(static_cast<int>(s.size()));
    r.push_back(s.count(3));
    r.push_back(s.count(4));
    r.push_back(*s.begin());
    r.push_back(*s.rbegin());
    r.push_back(*s.find(8));
    r.push_back(s.find(7) == s.end());
    auto p = s.insert(8);
    r.push_back(p.second);
    auto q = s.insert(2);
    r.push_back(q.second);
    r.push_back(*q.first);
    s.erase(5);
    s.erase(s.begin());
    for (int x : s) r.push_back(x);
    return r;
}
//// set_bounds_prev
#include <set>
#include <vector>
std::vector<int> test() {
    std::set<int> s = {10, 20, 30, 40};
    std::vector<int> r;
    r.push_back(*s.lower_bound(15));
    r.push_back(*s.upper_bound(20));
    r.push_back(s.lower_bound(41) == s.end());
    auto it = s.lower_bound(30);
    r.push_back(*std::prev(it));
    r.push_back(*std::next(it));
    r.push_back(static_cast<int>(std::distance(s.begin(), it)));
    return r;
}
//// set_of_pairs_strings
#include <set>
#include <string>
#include <utility>
std::set<std::pair<int, std::string>> test() {
    std::set<std::pair<int, std::string>> s;
    s.insert({3, "c"});
    s.insert({1, "z"});
    s.insert({3, "a"});
    s.insert({1, "z"});
    s.emplace(2, "m");
    return s;
}
//// set_from_vector_ops
#include <set>
#include <vector>
std::vector<int> test() {
    std::vector<int> v = {4, 2, 4, 1, 2, 9};
    std::set<int> s(v.begin(), v.end());
    std::vector<int> r(s.begin(), s.end());
    std::set<int> t = {1, 2};
    t.insert(v.begin(), v.end());
    r.push_back(static_cast<int>(t.size()));
    t.clear();
    r.push_back(t.empty());
    return r;
}
//// set_erase_while_iterating
#include <set>
#include <vector>
std::vector<int> test() {
    std::set<int> s;
    for (int i = 1; i <= 12; ++i) s.insert(i);
    for (auto it = s.begin(); it != s.end();) {
        if (*it % 3 == 0) it = s.erase(it);
        else ++it;
    }
    return std::vector<int>(s.begin(), s.end());
}
//// uset_basic
#include <unordered_set>
#include <vector>
std::vector<int> test() {
    std::unordered_set<int> s;
    for (int x : {5, 3, 8, 3, 1, 9, 5}) s.insert(x);
    std::vector<int> r;
    r.push_back(static_cast<int>(s.size()));
    r.push_back(s.count(3));
    r.push_back(s.count(4));
    r.push_back(s.find(8) != s.end());
    r.push_back(s.find(7) == s.end());
    r.push_back(static_cast<int>(s.erase(5)));
    r.push_back(static_cast<int>(s.erase(55)));
    auto p = s.insert(8);
    r.push_back(p.second);
    r.push_back(static_cast<int>(s.size()));
    long long sum = 0;
    for (int x : s) sum += x;
    r.push_back(static_cast<int>(sum));
    s.clear();
    r.push_back(s.empty());
    return r;
}
//// umap_basic
#include <string>
#include <unordered_map>
#include <vector>
std::vector<int> test() {
    std::unordered_map<std::string, int> m;
    m["a"] = 1;
    m["b"] += 5;
    m["a"]++;
    m.insert({"c", 7});
    m.emplace("d", 8);
    m.try_emplace("a", 100);
    std::vector<int> r;
    r.push_back(m["a"]);
    r.push_back(m["b"]);
    r.push_back(m.at("c"));
    r.push_back(m.count("d"));
    r.push_back(m.count("zz"));
    r.push_back(m.find("zz") == m.end());
    r.push_back(m.find("d")->second);
    r.push_back(static_cast<int>(m.size()));
    m.erase("a");
    r.push_back(static_cast<int>(m.size()));
    int total = 0;
    for (const auto& [k, v] : m) total += v;
    r.push_back(total);
    for (auto& kv : m) kv.second *= 2;
    int t2 = 0;
    for (auto& kv : m) t2 += kv.second;
    r.push_back(t2);
    return r;
}
//// umap_two_sum
#include <unordered_map>
#include <vector>
std::vector<int> two_sum(const std::vector<int>& nums, int target) {
    std::unordered_map<int, int> seen;
    for (int i = 0; i < static_cast<int>(nums.size()); ++i) {
        if (auto it = seen.find(target - nums[i]); it != seen.end()) {
            return {it->second, i};
        }
        seen[nums[i]] = i;
    }
    return {};
}
std::vector<int> test() {
    std::vector<int> a = two_sum({2, 7, 11, 15}, 9);
    std::vector<int> b = two_sum({3, 2, 4}, 6);
    std::vector<int> c = two_sum({1, 2}, 100);
    a.insert(a.end(), b.begin(), b.end());
    a.push_back(static_cast<int>(c.size()));
    return a;
}
//// umap_group_sorted
#include <algorithm>
#include <string>
#include <unordered_map>
#include <vector>
std::vector<std::vector<std::string>> test() {
    std::vector<std::string> words = {"eat", "tea", "tan", "ate", "nat", "bat"};
    std::unordered_map<std::string, std::vector<std::string>> groups;
    for (const auto& w : words) {
        std::string key = w;
        std::sort(key.begin(), key.end());
        groups[key].push_back(w);
    }
    std::vector<std::vector<std::string>> out;
    for (auto& kv : groups) out.push_back(kv.second);
    std::sort(out.begin(), out.end());
    return out;
}
//// umap_count_freq_max
#include <unordered_map>
#include <vector>
int test() {
    std::vector<int> v = {1, 3, 2, 3, 3, 1, 2, 2, 2, 9};
    std::unordered_map<int, int> cnt;
    for (int x : v) ++cnt[x];
    int best = -1, bestc = 0;
    for (const auto& [k, c] : cnt) {
        if (c > bestc || (c == bestc && k < best)) {
            best = k;
            bestc = c;
        }
    }
    return best * 100 + bestc;
}
//// umap_erase_iter_loop
#include <unordered_map>
#include <vector>
int test() {
    std::unordered_map<int, int> m;
    for (int i = 0; i < 20; ++i) m[i] = i * i;
    for (auto it = m.begin(); it != m.end();) {
        if (it->first % 2) it = m.erase(it);
        else ++it;
    }
    int s = 0;
    for (auto& kv : m) s += kv.second;
    return s * 100 + static_cast<int>(m.size());
}
//// map_vs_umap_agree
#include <map>
#include <unordered_map>
#include <vector>
std::vector<int> test() {
    std::map<int, int> a;
    std::unordered_map<int, int> b;
    for (int i = 0; i < 200; ++i) {
        int k = (i * 37) % 53;
        a[k] += i;
        b[k] += i;
    }
    int bad = 0;
    for (auto& kv : a)
        if (b[kv.first] != kv.second) ++bad;
    return {bad, static_cast<int>(a.size()), static_cast<int>(b.size())};
}
//// map_clear_swap_empty
#include <map>
#include <vector>
std::vector<int> test() {
    std::map<int, int> a = {{1, 1}, {2, 2}};
    std::map<int, int> b = {{3, 3}};
    a.swap(b);
    std::vector<int> r = {static_cast<int>(a.size()), static_cast<int>(b.size())};
    b.clear();
    r.push_back(b.empty());
    r.push_back(a.empty());
    std::map<int, int> c = a;
    c[100] = 1;
    r.push_back(static_cast<int>(a.size()));
    r.push_back(static_cast<int>(c.size()));
    return r;
}
//// map_char_keys_struct_binding
#include <map>
#include <string>
std::string test() {
    std::map<char, int> m;
    for (char c : std::string("hello world")) m[c]++;
    std::string out;
    for (auto [c, n] : m) {
        out += c;
        out += std::to_string(n);
    }
    return out;
}
