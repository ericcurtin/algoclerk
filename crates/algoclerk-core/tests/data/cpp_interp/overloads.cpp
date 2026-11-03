//// str_compare_pos_len
#include <string>
#include <vector>
bool startsWith(const std::string& s, const std::string& w) { return s.compare(0, w.size(), w) == 0; }
int sgn(int c) { return c < 0 ? -1 : (c > 0 ? 1 : 0); }
std::vector<int> test() {
    std::string a = "abcdef";
    std::string b = "xbcx";
    return {startsWith("abc", "ab"), startsWith("abc", "bc"), sgn(a.compare(1, 2, "bc")), sgn(a.compare(1, 2, "bd")),
            sgn(a.compare(2, 100, "cdeg")), sgn(a.compare(1, 2, b, 1, 2)), sgn(a.compare(0, 2, b, 1)),
            sgn(a.compare("abcdeg")), sgn(a.compare(a)), sgn(a.compare(6, 3, "")), sgn(a.compare("abc"))};
}
//// str_find_rfind_pos
#include <string>
#include <vector>
std::vector<int> test() {
    std::string s = "abcabcabc";
    std::string n = "bc";
    auto r = [](std::size_t i) { return i == std::string::npos ? -1 : static_cast<int>(i); };
    return {r(s.find(n)), r(s.find(n, 2)), r(s.find("bc", 8)), r(s.find('c', 3)), r(s.find('z', 1)),
            r(s.rfind(n)), r(s.rfind(n, 3)), r(s.rfind(n, 0)), r(s.rfind('a', 5)), r(s.rfind('a')),
            r(s.rfind("abc", 0)), r(s.rfind("abcabcabcabc")), r(s.find("")), r(s.rfind(""))};
}
//// str_find_sets
#include <string>
#include <vector>
std::vector<int> test() {
    std::string s = "hello, world";
    auto r = [](std::size_t i) { return i == std::string::npos ? -1 : static_cast<int>(i); };
    return {r(s.find_first_of("lo")), r(s.find_first_of("lo", 5)), r(s.find_first_of('w')), r(s.find_last_of("lo")),
            r(s.find_last_of("lo", 5)), r(s.find_last_of('h')), r(s.find_last_of('h', 0)),
            r(s.find_first_not_of("hel")), r(s.find_first_not_of("hel", 4)), r(s.find_first_not_of('h')),
            r(s.find_last_not_of("dlrow")), r(s.find_last_not_of("d")), r(s.find_last_not_of('d', 3)),
            r(s.find_first_of("xyz")), r(s.find_first_not_of(s))};
}
//// str_append_overloads
#include <string>
#include <vector>
std::string test() {
    std::string s = "ab";
    std::string t = "wxyz";
    s.append("cd");
    s.append(t, 1, 2);
    s.append(t, 2);
    s.append(3, '!');
    s.append(t.begin(), t.begin() + 2);
    s.append(std::string("Q"));
    return s;
}
//// str_insert_overloads
#include <string>
#include <vector>
std::string test() {
    std::string s = "ad";
    std::string t = "xbcx";
    s.insert(1, "bc");
    s.insert(0, t, 1, 2);
    s.insert(s.size(), 2, '*');
    s.insert(1, 3, '-');
    auto it = s.insert(s.begin(), '#');
    s.insert(it + 2, 2, '@');
    s.insert(s.end(), t.begin(), t.begin() + 2);
    s.insert(3, t, 2);
    return s;
}
//// str_erase_replace
#include <string>
#include <vector>
std::string test() {
    std::string s = "abcdefghij";
    s.erase(2, 3);
    s.erase(s.begin());
    s.erase(s.begin() + 1, s.begin() + 3);
    s.replace(1, 2, "XYZ");
    s.replace(0, 1, 2, 'q');
    s.replace(s.begin(), s.begin() + 2, "__");
    std::string t = "mid";
    s.replace(2, 1, t, 1, 1);
    s.replace(s.size() - 1, 100, "END");
    return s;
}
//// str_assign_overloads
#include <string>
#include <vector>
std::vector<std::string> test() {
    std::string s = "xyz";
    std::string t;
    t.assign("abc");
    std::string u;
    u.assign(s);
    std::string w;
    w.assign(3, 'k');
    return {t, u, w};
}
//// view_overloads
#include <string_view>
#include <vector>
int sgn(int c) { return c < 0 ? -1 : (c > 0 ? 1 : 0); }
std::vector<int> test() {
    std::string_view v = "hello, world";
    auto r = [](std::size_t i) { return i == std::string_view::npos ? -1 : static_cast<int>(i); };
    std::vector<int> out = {r(v.rfind('o')), r(v.rfind("o", 5)), r(v.find_first_of("ow")), r(v.find_last_of("ow")),
                            r(v.find_first_not_of("hel")), r(v.find_last_not_of("dl")), sgn(v.compare("hello")),
                            sgn(v.compare("hello, world")), static_cast<int>(v.length())};
    v.remove_prefix(7);
    v.remove_suffix(2);
    out.push_back(static_cast<int>(v.size()));
    out.push_back(v[0]);
    return out;
}
//// vec_emplace_insert_assign
#include <utility>
#include <vector>
std::vector<int> test() {
    std::vector<int> v{1, 2, 3};
    v.emplace(v.begin() + 1, 9);
    v.insert(v.begin(), {7, 8});
    v.insert(v.end(), 2, 5);
    std::vector<int> w{100, 200};
    v.insert(v.begin() + 2, w.begin(), w.end());
    v.erase(v.begin(), v.begin() + 2);
    std::vector<int> a;
    a.assign({4, 5, 6});
    std::vector<std::pair<int, int>> p;
    p.emplace(p.end(), 1, 2);
    p.emplace_back(3, 4);
    p.insert(p.begin(), {5, 6});
    v.insert(v.end(), a.begin(), a.end());
    v.push_back(p[0].first * 10 + p[2].second);
    return v;
}
//// set_map_overloads
#include <map>
#include <iterator>
#include <set>
#include <unordered_map>
#include <unordered_set>
#include <vector>
std::vector<int> test() {
    std::vector<int> out;
    std::set<int> s{5, 1, 9};
    std::vector<int> add{3, 7, 5};
    s.insert(add.begin(), add.end());
    s.insert({11, 0});
    s.erase(s.find(3), s.find(9));
    for (int x : s) out.push_back(x);
    out.push_back(-1);
    auto er = s.equal_range(9);
    out.push_back(*er.first);
    out.push_back(static_cast<int>(std::distance(er.first, er.second)));
    std::map<int, int> m{{1, 10}, {2, 20}};
    m.insert_or_assign(1, 11);
    m.insert_or_assign(3, 30);
    auto r = m.try_emplace(2, 99);
    out.push_back(r.second);
    r = m.try_emplace(4, 40);
    out.push_back(r.second);
    std::map<int, int> m2{{8, 80}, {9, 90}};
    m.insert(m2.begin(), m2.end());
    m.erase(m.find(2), m.find(4));
    for (auto& kv : m) out.push_back(kv.first * 1000 + kv.second);
    std::unordered_map<int, int> um;
    um.try_emplace(1, 5);
    um.insert_or_assign(1, 6);
    um.insert(m2.begin(), m2.end());
    out.push_back(static_cast<int>(um.size()) * 100 + um[1]);
    std::unordered_set<int> us{1, 2, 3, 4};
    us.insert(add.begin(), add.end());
    us.erase(us.find(1));
    out.push_back(static_cast<int>(us.size()));
    return out;
}
//// list_members
#include <iterator>
#include <list>
#include <vector>
std::vector<int> test() {
    std::list<int> l{5, 3, 5, 1, 3, 3, 9, 1};
    std::vector<int> out;
    l.sort();
    l.unique();
    l.remove(9);
    l.reverse();
    l.sort([](int a, int b) { return a > b; });
    l.remove_if([](int x) { return x == 3; });
    l.resize(4, 7);
    l.insert(l.begin(), 2, 8);
    std::vector<int> w{40, 41};
    l.insert(l.end(), w.begin(), w.end());
    l.erase(l.begin(), std::next(l.begin()));
    std::list<int> o{1};
    l.swap(o);
    for (int x : o) out.push_back(x);
    out.push_back(l.size());
    out.push_back(l == o);
    l.assign(2, 6);
    l.assign(w.begin(), w.end());
    l.assign({3, 2});
    for (auto it = l.rbegin(); it != l.rend(); ++it) out.push_back(*it);
    std::list<int> l2{3, 2};
    out.push_back(l == l2);
    out.push_back(l != l2);
    out.push_back(l < o);
    out.push_back(o < l);
    return out;
}
//// deque_members
#include <deque>
#include <vector>
std::vector<int> test() {
    std::deque<int> d{5, 3, 1};
    std::vector<int> out;
    d.insert(d.begin(), 2, 8);
    std::vector<int> w{40, 41};
    d.insert(d.end(), w.begin(), w.end());
    d.insert(d.begin() + 1, {70, 71});
    d.emplace(d.begin(), 9);
    d.erase(d.begin(), d.begin() + 2);
    d.resize(10, 7);
    d.resize(9);
    std::deque<int> o{1};
    d.swap(o);
    for (int x : o) out.push_back(x);
    out.push_back(d.size());
    d.assign(2, 6);
    d.assign(w.begin(), w.end());
    d.assign({3, 2});
    for (auto it = d.rbegin(); it != d.rend(); ++it) out.push_back(*it);
    out.push_back(d == o);
    out.push_back(d < o);
    out.push_back(*d.cbegin());
    return out;
}
