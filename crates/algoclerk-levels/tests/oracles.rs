// SPDX-License-Identifier: Apache-2.0
//! Independent oracles: every level's expected outputs are re-computed by plain Rust code written
//! without looking at the ClerkScript reference, and compared on every test (examples, hand-written,
//! generated and large).

use algoclerk_core::judge::Judge;
use algoclerk_core::lang::ast::Ty;
use algoclerk_core::lang::value::{NodeData, Value, parse_value};
use algoclerk_core::level::Compare;
use algoclerk_levels::Campaign;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum V {
    I(i64),
    B(bool),
    C(char),
    S(String),
    L(Vec<V>),
    /// linked list values and the index the tail points back to
    Cyc(Vec<i64>, Option<usize>),
    /// binary tree in level order with holes
    T(Vec<Option<i64>>),
}
use V::*;

fn conv(v: &Value, t: &Ty, nodes: &[NodeData]) -> V {
    match (v, t) {
        (Value::Int(i), _) => I(*i),
        (Value::Bool(b), _) => B(*b),
        (Value::Char(c), _) => C(*c as char),
        (Value::Str(s), _) => S(String::from_utf8_lossy(s).into_owned()),
        (Value::Vec(xs), Ty::Vec(et)) => L(xs.iter().map(|x| conv(x, et, nodes)).collect()),
        (Value::Node(h), Ty::LNode) => {
            let mut vals = Vec::new();
            let mut seen: HashMap<u32, usize> = HashMap::new();
            let mut cur = *h;
            while cur != 0 {
                if let Some(&p) = seen.get(&cur) {
                    return Cyc(vals, Some(p));
                }
                seen.insert(cur, vals.len());
                vals.push(nodes[cur as usize].val);
                cur = nodes[cur as usize].a;
            }
            Cyc(vals, None)
        }
        (Value::Node(h), Ty::TNode) => {
            let mut items = Vec::new();
            let mut q = VecDeque::from([*h]);
            while let Some(n) = q.pop_front() {
                if n == 0 {
                    items.push(None);
                } else {
                    items.push(Some(nodes[n as usize].val));
                    q.push_back(nodes[n as usize].a);
                    q.push_back(nodes[n as usize].b);
                }
            }
            while items.last() == Some(&None) {
                items.pop();
            }
            T(items)
        }
        _ => panic!("oracle: unsupported value {v:?} of type {t}"),
    }
}

fn int(v: &V) -> i64 {
    if let I(i) = v { *i } else { panic!("int expected, got {v:?}") }
}
fn ints(v: &V) -> Vec<i64> {
    match v {
        L(xs) => xs.iter().map(int).collect(),
        Cyc(vals, _) => vals.clone(),
        _ => panic!("ints expected"),
    }
}
fn st(v: &V) -> String {
    if let S(s) = v { s.clone() } else { panic!("string expected, got {v:?}") }
}
fn strs(v: &V) -> Vec<String> {
    if let L(xs) = v { xs.iter().map(st).collect() } else { panic!("strings expected") }
}
fn grid(v: &V) -> Vec<Vec<i64>> {
    if let L(rows) = v { rows.iter().map(ints).collect() } else { panic!("grid expected") }
}
fn cgrid(v: &V) -> Vec<Vec<char>> {
    if let L(rows) = v {
        rows.iter().map(|r| if let L(cs) = r { cs.iter().map(|c| if let C(c) = c { *c } else { panic!() }).collect() } else { panic!() }).collect()
    } else {
        panic!("char grid expected")
    }
}
fn il(xs: impl IntoIterator<Item = i64>) -> V {
    L(xs.into_iter().map(I).collect())
}
fn list(xs: Vec<i64>) -> V {
    Cyc(xs, None)
}

struct Tree {
    val: Vec<i64>,
    left: Vec<Option<usize>>,
    right: Vec<Option<usize>>,
}

fn tree(v: &V) -> Tree {
    let T(items) = v else { panic!("tree expected") };
    let mut t = Tree { val: vec![], left: vec![], right: vec![] };
    if items.is_empty() || items[0].is_none() {
        return t;
    }
    t.val.push(items[0].unwrap());
    t.left.push(None);
    t.right.push(None);
    let mut q = VecDeque::from([0usize]);
    let mut i = 1;
    while let Some(n) = q.pop_front() {
        for side in 0..2 {
            if i >= items.len() {
                return t;
            }
            if let Some(x) = items[i] {
                t.val.push(x);
                t.left.push(None);
                t.right.push(None);
                let id = t.val.len() - 1;
                if side == 0 { t.left[n] = Some(id) } else { t.right[n] = Some(id) }
                q.push_back(id);
            }
            i += 1;
        }
    }
    t
}

fn depth(t: &Tree, n: Option<usize>) -> i64 {
    match n {
        None => 0,
        Some(i) => 1 + depth(t, t.left[i]).max(depth(t, t.right[i])),
    }
}

fn inorder(t: &Tree, n: Option<usize>, out: &mut Vec<i64>) {
    if let Some(i) = n {
        inorder(t, t.left[i], out);
        out.push(t.val[i]);
        inorder(t, t.right[i], out);
    }
}

fn dsu_find(p: &mut Vec<usize>, x: usize) -> usize {
    let mut r = x;
    while p[r] != r {
        r = p[r];
    }
    let mut c = x;
    while p[c] != r {
        let n = p[c];
        p[c] = r;
        c = n;
    }
    r
}

fn oracle(id: &str, a: &[V]) -> Option<V> {
    Some(match id {
        // ---- mail room ----
        "stamp-duty" => {
            let t = int(&a[0]) * int(&a[1]);
            I(if int(&a[1]) >= 10 { t - t / 10 } else { t })
        }
        "sum-the-bills" => I(ints(&a[0]).iter().sum()),
        "count-vowels" => I(st(&a[0]).chars().filter(|c| "aeiouAEIOU".contains(*c)).count() as i64),
        "biggest-parcel" => I(*ints(&a[0]).iter().max().unwrap()),
        "is-sorted" => B(ints(&a[0]).windows(2).all(|w| w[0] <= w[1])),
        "reverse-the-label" => S(st(&a[0]).chars().rev().collect()),
        "fizz-memo" => L((1..=int(&a[0])).map(|i| S(match (i % 3, i % 5) { (0, 0) => "FizzBuzz".into(), (0, _) => "Fizz".into(), (_, 0) => "Buzz".into(), _ => i.to_string() })).collect()),
        "running-total" => il(ints(&a[0]).iter().scan(0, |s, x| { *s += x; Some(*s) })),
        // ---- records office ----
        "valid-anagram" => {
            let (mut x, mut y): (Vec<char>, Vec<char>) = (st(&a[0]).chars().collect(), st(&a[1]).chars().collect());
            x.sort();
            y.sort();
            B(x == y)
        }
        "palindrome-plain" => {
            let s = st(&a[0]);
            B(s.chars().eq(s.chars().rev()))
        }
        "palindrome-clean" => {
            let f: Vec<char> = st(&a[0]).chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect();
            B(f.iter().eq(f.iter().rev()))
        }
        "reverse-words" => {
            let s = st(&a[0]);
            S(s.split(' ').rev().collect::<Vec<_>>().join(" "))
        }
        "first-unique-char" => {
            let s: Vec<char> = st(&a[0]).chars().collect();
            let mut cnt: HashMap<char, usize> = HashMap::new();
            for c in &s {
                *cnt.entry(*c).or_default() += 1;
            }
            I(s.iter().position(|c| cnt[c] == 1).map_or(-1, |p| p as i64))
        }
        "common-prefix" => {
            let w = strs(&a[0]);
            if w.is_empty() {
                return Some(S(String::new()));
            }
            let first: Vec<char> = w[0].chars().collect();
            let mut n = 0;
            'outer: while n < first.len() {
                for x in &w {
                    if x.chars().nth(n) != Some(first[n]) {
                        break 'outer;
                    }
                }
                n += 1;
            }
            S(first[..n].iter().collect())
        }
        "longest-unique-run" => {
            let s: Vec<char> = st(&a[0]).chars().collect();
            let mut best = 0;
            for i in 0..s.len() {
                let mut seen = HashSet::new();
                let mut j = i;
                while j < s.len() && seen.insert(s[j]) {
                    j += 1;
                }
                best = best.max(j - i);
            }
            I(best as i64)
        }
        "group-anagrams" => {
            let mut m: BTreeMap<Vec<char>, Vec<String>> = BTreeMap::new();
            for w in strs(&a[0]) {
                let mut k: Vec<char> = w.chars().collect();
                k.sort();
                m.entry(k).or_default().push(w);
            }
            L(m.into_values().map(|g| L(g.into_iter().map(S).collect())).collect())
        }
        "run-length" => {
            let s: Vec<char> = st(&a[0]).chars().collect();
            let mut out = String::new();
            let mut i = 0;
            while i < s.len() {
                let run = s[i..].iter().take_while(|c| **c == s[i]).count();
                out.push(s[i]);
                out.push_str(&run.to_string());
                i += run;
            }
            S(out)
        }
        "caesar-shift" => {
            let k = (int(&a[1]) % 26) as u8;
            S(st(&a[0]).chars().map(|c| match c {
                'a'..='z' => ((c as u8 - b'a' + k) % 26 + b'a') as char,
                'A'..='Z' => ((c as u8 - b'A' + k) % 26 + b'A') as char,
                o => o,
            }).collect())
        }
        // ---- filing cabinets ----
        "two-sum" => {
            let (n, t) = (ints(&a[0]), int(&a[1]));
            for j in 0..n.len() {
                for i in 0..j {
                    if n[i] + n[j] == t {
                        return Some(il([i as i64, j as i64]));
                    }
                }
            }
            L(vec![])
        }
        "has-duplicate" => {
            let mut n = ints(&a[0]);
            n.sort();
            B(n.windows(2).any(|w| w[0] == w[1]))
        }
        "missing-number" => {
            let n = ints(&a[0]);
            I((0..=n.len() as i64).sum::<i64>() - n.iter().sum::<i64>())
        }
        "first-recurring" => {
            let n = ints(&a[0]);
            let mut first: HashMap<i64, usize> = HashMap::new();
            let mut best: Option<(usize, i64)> = None;
            for (i, x) in n.iter().enumerate() {
                if first.contains_key(x) {
                    if best.is_none() {
                        best = Some((i, *x));
                    }
                } else {
                    first.insert(*x, i);
                }
            }
            I(best.map_or(-1, |b| b.1))
        }
        "top-k-frequent" => {
            let mut cnt: BTreeMap<i64, i64> = BTreeMap::new();
            for x in ints(&a[0]) {
                *cnt.entry(x).or_default() += 1;
            }
            let mut order: Vec<(i64, i64)> = cnt.into_iter().map(|(v, c)| (-c, v)).collect();
            order.sort();
            il(order.into_iter().take(int(&a[1]).max(0) as usize).map(|x| x.1))
        }
        "common-elements" => {
            let (mut x, mut y) = (ints(&a[0]), ints(&a[1]));
            x.sort();
            x.dedup();
            y.sort();
            il(x.into_iter().filter(|v| y.binary_search(v).is_ok()).collect::<Vec<_>>())
        }
        "subarray-sum-k" => {
            let (n, k) = (ints(&a[0]), int(&a[1]));
            let mut c = 0;
            for i in 0..n.len() {
                let mut s = 0;
                for x in &n[i..] {
                    s += x;
                    if s == k {
                        c += 1;
                    }
                }
            }
            I(c)
        }
        "longest-consecutive" => {
            let mut n = ints(&a[0]);
            n.sort();
            n.dedup();
            let (mut best, mut run) = (0, 0);
            for i in 0..n.len() {
                run = if i > 0 && n[i] == n[i - 1] + 1 { run + 1 } else { 1 };
                best = best.max(run);
            }
            I(best)
        }
        "ransom-note" => {
            let mut stock = [0i64; 256];
            for b in st(&a[1]).bytes() {
                stock[b as usize] += 1;
            }
            for b in st(&a[0]).bytes() {
                stock[b as usize] -= 1;
            }
            B(stock.iter().all(|c| *c >= 0) || st(&a[0]).bytes().all(|b| stock[b as usize] >= 0))
        }
        // ---- stack room ----
        "valid-brackets" => {
            let mut st_: Vec<char> = Vec::new();
            for c in st(&a[0]).chars() {
                match c {
                    '(' | '[' | '{' => st_.push(c),
                    _ => {
                        let want = match c { ')' => '(', ']' => '[', _ => '{' };
                        if st_.pop() != Some(want) {
                            return Some(B(false));
                        }
                    }
                }
            }
            B(st_.is_empty())
        }
        "daily-waits" => {
            let t = ints(&a[0]);
            il((0..t.len()).map(|i| (i + 1..t.len()).find(|&j| t[j] > t[i]).map_or(0, |j| (j - i) as i64)))
        }
        "reverse-polish" => {
            let mut s: Vec<i64> = Vec::new();
            for t in strs(&a[0]) {
                match t.as_str() {
                    "+" | "-" | "*" | "/" => {
                        let (y, x) = (s.pop().unwrap(), s.pop().unwrap());
                        s.push(match t.as_str() { "+" => x + y, "-" => x - y, "*" => x * y, _ => x / y });
                    }
                    n => s.push(n.parse().unwrap()),
                }
            }
            I(s[0])
        }
        "min-stack" => {
            let mut items: Vec<i64> = Vec::new();
            let mut out = Vec::new();
            for op in ints(&a[0]) {
                if op == 0 {
                    items.pop();
                } else {
                    items.push(op);
                }
                out.push(items.iter().copied().min().unwrap_or(-1));
            }
            il(out)
        }
        "window-maximum" => {
            let (n, k) = (ints(&a[0]), int(&a[1]) as usize);
            il(n.windows(k).map(|w| *w.iter().max().unwrap()))
        }
        "kth-largest" => {
            let mut n = ints(&a[0]);
            n.sort_by(|x, y| y.cmp(x));
            I(n[int(&a[1]) as usize - 1])
        }
        "merge-sorted-lists" => {
            let L(ls) = &a[0] else { panic!() };
            let mut all: Vec<i64> = ls.iter().flat_map(ints).collect();
            all.sort();
            il(all)
        }
        "simplify-path" => {
            let mut parts: Vec<String> = Vec::new();
            for p in st(&a[0]).split('/') {
                match p {
                    "" | "." => {}
                    ".." => {
                        parts.pop();
                    }
                    x => parts.push(x.to_string()),
                }
            }
            S(format!("/{}", parts.join("/")))
        }
        // ---- search annex ----
        "binary-search" | "search-rotated" => {
            let (n, t) = (ints(&a[0]), int(&a[1]));
            I(n.iter().position(|x| *x == t).map_or(-1, |p| p as i64))
        }
        "insert-position" => {
            let (n, t) = (ints(&a[0]), int(&a[1]));
            I(n.partition_point(|x| *x < t) as i64)
        }
        "rotate-shelf" => {
            let (mut n, k) = (ints(&a[0]), int(&a[1]) as usize);
            if !n.is_empty() {
                let len = n.len();
                n.rotate_right(k % len);
            }
            il(n)
        }
        "product-except-self" => {
            let n = ints(&a[0]);
            let mut pre = vec![1i64; n.len() + 1];
            for i in 0..n.len() {
                pre[i + 1] = pre[i] * n[i];
            }
            let mut suf = vec![1i64; n.len() + 1];
            for i in (0..n.len()).rev() {
                suf[i] = suf[i + 1] * n[i];
            }
            il((0..n.len()).map(|i| pre[i] * suf[i + 1]))
        }
        "merge-intervals" => {
            let L(rows) = &a[0] else { panic!() };
            let mut iv: Vec<(i64, i64)> = rows.iter().map(|r| { let v = ints(r); (v[0], v[1]) }).collect();
            iv.sort();
            let mut out: Vec<(i64, i64)> = Vec::new();
            for (s, e) in iv {
                match out.last_mut() {
                    Some(last) if last.1 >= s => last.1 = last.1.max(e),
                    _ => out.push((s, e)),
                }
            }
            L(out.into_iter().map(|(s, e)| il([s, e])).collect())
        }
        "max-subarray" => {
            let n = ints(&a[0]);
            let mut best = i64::MIN;
            for i in 0..n.len() {
                let mut s = 0;
                for x in &n[i..] {
                    s += x;
                    best = best.max(s);
                }
            }
            I(best)
        }
        "most-water" => {
            let h = ints(&a[0]);
            let mut best = 0;
            for i in 0..h.len() {
                for j in i + 1..h.len() {
                    best = best.max(h[i].min(h[j]) * (j - i) as i64);
                }
            }
            I(best)
        }
        "sort-colors" | "merge-sort" => {
            let mut n = ints(&a[0]);
            n.sort();
            il(n)
        }
        "move-zeroes" => {
            let n = ints(&a[0]);
            let zeros = n.iter().filter(|x| **x == 0).count();
            il(n.iter().copied().filter(|x| *x != 0).chain(std::iter::repeat_n(0, zeros)).collect::<Vec<_>>())
        }
        // ---- grid hall ----
        "transpose" => {
            let g = grid(&a[0]);
            L((0..g[0].len()).map(|c| il(g.iter().map(|r| r[c]).collect::<Vec<_>>())).collect())
        }
        "spiral-order" => {
            let g = grid(&a[0]);
            let (rows, cols) = (g.len() as i64, g[0].len() as i64);
            let mut seen = vec![vec![false; cols as usize]; rows as usize];
            let (mut r, mut c, mut d) = (0i64, 0i64, 0usize);
            let dirs = [(0, 1), (1, 0), (0, -1), (-1, 0)];
            let mut out = Vec::new();
            for _ in 0..rows * cols {
                out.push(g[r as usize][c as usize]);
                seen[r as usize][c as usize] = true;
                let (mut nr, mut nc) = (r + dirs[d].0, c + dirs[d].1);
                if nr < 0 || nr >= rows || nc < 0 || nc >= cols || seen[nr as usize][nc as usize] {
                    d = (d + 1) % 4;
                    nr = r + dirs[d].0;
                    nc = c + dirs[d].1;
                }
                r = nr;
                c = nc;
            }
            il(out)
        }
        "rotate-image" => {
            let g = grid(&a[0]);
            let n = g.len();
            let mut t: Vec<Vec<i64>> = (0..n).map(|c| (0..n).map(|r| g[r][c]).collect()).collect();
            for row in &mut t {
                row.reverse();
            }
            L(t.into_iter().map(il).collect())
        }
        "count-islands" => {
            let g = cgrid(&a[0]);
            let (rows, cols) = (g.len(), g[0].len());
            let mut p: Vec<usize> = (0..rows * cols).collect();
            for r in 0..rows {
                for c in 0..cols {
                    if g[r][c] != '1' {
                        continue;
                    }
                    for (nr, nc) in [(r + 1, c), (r, c + 1)] {
                        if nr < rows && nc < cols && g[nr][nc] == '1' {
                            let (x, y) = (dsu_find(&mut p, r * cols + c), dsu_find(&mut p, nr * cols + nc));
                            p[x] = y;
                        }
                    }
                }
            }
            let mut roots = HashSet::new();
            for r in 0..rows {
                for c in 0..cols {
                    if g[r][c] == '1' {
                        roots.insert(dsu_find(&mut p, r * cols + c));
                    }
                }
            }
            I(roots.len() as i64)
        }
        "maze-steps" => {
            let g = cgrid(&a[0]);
            let (rows, cols) = (g.len(), g[0].len());
            if g[0][0] == '#' || g[rows - 1][cols - 1] == '#' {
                return Some(I(-1));
            }
            let mut dist = vec![vec![i64::MAX; cols]; rows];
            dist[0][0] = 0;
            let mut frontier = vec![(0usize, 0usize)];
            let mut d = 0;
            while !frontier.is_empty() && dist[rows - 1][cols - 1] == i64::MAX {
                d += 1;
                let mut next = Vec::new();
                for (r, c) in frontier {
                    let mut nb = vec![(r + 1, c), (r, c + 1)];
                    if r > 0 { nb.push((r - 1, c)); }
                    if c > 0 { nb.push((r, c - 1)); }
                    for (nr, nc) in nb {
                        if nr < rows && nc < cols && g[nr][nc] == '.' && dist[nr][nc] == i64::MAX {
                            dist[nr][nc] = d;
                            next.push((nr, nc));
                        }
                    }
                }
                frontier = next;
            }
            I(if dist[rows - 1][cols - 1] == i64::MAX { -1 } else { dist[rows - 1][cols - 1] })
        }
        "shortest-route" => {
            let (n, e, s, t) = (int(&a[0]) as usize, grid(&a[1]), int(&a[2]) as usize, int(&a[3]) as usize);
            let mut adj = vec![vec![]; n];
            for p in &e {
                adj[p[0] as usize].push(p[1] as usize);
                adj[p[1] as usize].push(p[0] as usize);
            }
            let mut dist = vec![-1i64; n];
            dist[s] = 0;
            let mut q = VecDeque::from([s]);
            while let Some(u) = q.pop_front() {
                for &v in &adj[u] {
                    if dist[v] < 0 {
                        dist[v] = dist[u] + 1;
                        q.push_back(v);
                    }
                }
            }
            I(dist[t])
        }
        "course-schedule" => {
            let (n, e) = (int(&a[0]) as usize, grid(&a[1]));
            let mut adj = vec![vec![]; n];
            for p in &e {
                adj[p[1] as usize].push(p[0] as usize);
            }
            // iterative three-colour DFS
            let mut color = vec![0u8; n];
            for s in 0..n {
                if color[s] != 0 {
                    continue;
                }
                let mut stack = vec![(s, 0usize)];
                color[s] = 1;
                while let Some((u, i)) = stack.pop() {
                    if i < adj[u].len() {
                        stack.push((u, i + 1));
                        let v = adj[u][i];
                        if color[v] == 1 {
                            return Some(B(false));
                        }
                        if color[v] == 0 {
                            color[v] = 1;
                            stack.push((v, 0));
                        }
                    } else {
                        color[u] = 2;
                    }
                }
            }
            B(true)
        }
        "count-components" => {
            let (n, e) = (int(&a[0]) as usize, grid(&a[1]));
            let mut p: Vec<usize> = (0..n).collect();
            let mut comps = n as i64;
            for x in &e {
                let (u, v) = (dsu_find(&mut p, x[0] as usize), dsu_find(&mut p, x[1] as usize));
                if u != v {
                    p[u] = v;
                    comps -= 1;
                }
            }
            I(comps)
        }
        // ---- chain gallery ----
        "reverse-list" => {
            let mut v = ints(&a[0]);
            v.reverse();
            list(v)
        }
        "list-cycle" => B(matches!(&a[0], Cyc(_, Some(_)))),
        "merge-two-lists" => {
            let mut v = ints(&a[0]);
            v.extend(ints(&a[1]));
            v.sort();
            list(v)
        }
        "middle-of-list" => {
            let v = ints(&a[0]);
            list(v[v.len() / 2..].to_vec())
        }
        "remove-nth" => {
            let mut v = ints(&a[0]);
            let at = v.len() - int(&a[1]) as usize;
            v.remove(at);
            list(v)
        }
        "tree-depth" => {
            let t = tree(&a[0]);
            I(depth(&t, if t.val.is_empty() { None } else { Some(0) }))
        }
        "inorder-walk" => {
            let t = tree(&a[0]);
            let mut out = Vec::new();
            inorder(&t, if t.val.is_empty() { None } else { Some(0) }, &mut out);
            il(out)
        }
        "validate-bst" => {
            let t = tree(&a[0]);
            let mut out = Vec::new();
            inorder(&t, if t.val.is_empty() { None } else { Some(0) }, &mut out);
            B(out.windows(2).all(|w| w[0] < w[1]))
        }
        "lowest-ancestor" => {
            let t = tree(&a[0]);
            let (p, q) = (int(&a[1]), int(&a[2]));
            let n = t.val.len();
            let mut parent = vec![usize::MAX; n];
            for i in 0..n {
                for c in [t.left[i], t.right[i]].into_iter().flatten() {
                    parent[c] = i;
                }
            }
            let at = |x: i64| t.val.iter().position(|v| *v == x).unwrap();
            let chain = |mut i: usize| {
                let mut c = vec![i];
                while parent[i] != usize::MAX {
                    i = parent[i];
                    c.push(i);
                }
                c
            };
            let (cp, cq) = (chain(at(p)), chain(at(q)));
            I(t.val[*cp.iter().find(|x| cq.contains(x)).unwrap()])
        }
        "level-order" => {
            let t = tree(&a[0]);
            let mut out = Vec::new();
            let mut cur: Vec<usize> = if t.val.is_empty() { vec![] } else { vec![0] };
            while !cur.is_empty() {
                out.push(il(cur.iter().map(|&i| t.val[i]).collect::<Vec<_>>()));
                cur = cur.iter().flat_map(|&i| [t.left[i], t.right[i]]).flatten().collect();
            }
            L(out)
        }
        // ---- strategy suite ----
        "climbing-stairs" => {
            let n = int(&a[0]) as usize;
            let mut w = vec![1i64; n.max(1) + 1];
            for i in 2..=n {
                w[i] = (w[i - 1] + w[i - 2]) % 1_000_000_007;
            }
            I(w[n])
        }
        "coin-change" => {
            let (coins, amount) = (ints(&a[0]), int(&a[1]) as usize);
            let mut dist = vec![-1i64; amount + 1];
            dist[0] = 0;
            let mut q = VecDeque::from([0usize]);
            while let Some(u) = q.pop_front() {
                for &c in &coins {
                    let v = u + c as usize;
                    if v <= amount && dist[v] < 0 {
                        dist[v] = dist[u] + 1;
                        q.push_back(v);
                    }
                }
            }
            I(dist[amount])
        }
        "longest-increasing" => {
            let n = ints(&a[0]);
            let mut best = vec![1i64; n.len()];
            for i in 0..n.len() {
                for j in 0..i {
                    if n[j] < n[i] {
                        best[i] = best[i].max(best[j] + 1);
                    }
                }
            }
            I(best.into_iter().max().unwrap_or(0))
        }
        "edit-distance" => {
            let (x, y): (Vec<char>, Vec<char>) = (st(&a[0]).chars().collect(), st(&a[1]).chars().collect());
            let mut row: Vec<i64> = (0..=y.len() as i64).collect();
            for i in 1..=x.len() {
                let mut prev = row[0];
                row[0] = i as i64;
                for j in 1..=y.len() {
                    let tmp = row[j];
                    row[j] = if x[i - 1] == y[j - 1] { prev } else { 1 + prev.min(row[j]).min(row[j - 1]) };
                    prev = tmp;
                }
            }
            I(row[y.len()])
        }
        "house-robber" => {
            let n = ints(&a[0]);
            let mut best = vec![0i64; n.len() + 2];
            for i in (0..n.len()).rev() {
                best[i] = (n[i] + best[i + 2]).max(best[i + 1]);
            }
            I(best[0])
        }
        "common-subsequence" => {
            let (x, y): (Vec<char>, Vec<char>) = (st(&a[0]).chars().collect(), st(&a[1]).chars().collect());
            let mut dp = vec![vec![0i64; y.len() + 1]; x.len() + 1];
            for i in (0..x.len()).rev() {
                for j in (0..y.len()).rev() {
                    dp[i][j] = if x[i] == y[j] { 1 + dp[i + 1][j + 1] } else { dp[i + 1][j].max(dp[i][j + 1]) };
                }
            }
            I(dp[0][0])
        }
        "grid-paths" => {
            let (r, c) = (int(&a[0]) as usize, int(&a[1]) as usize);
            let m = 1_000_000_007u64;
            let mut dp = vec![vec![1u64; c]; r];
            for i in 1..r {
                for j in 1..c {
                    dp[i][j] = (dp[i - 1][j] + dp[i][j - 1]) % m;
                }
            }
            I(dp[r - 1][c - 1] as i64)
        }
        "word-break" => {
            let s = st(&a[0]);
            let words = strs(&a[1]);
            let n = s.len();
            let mut ok = vec![false; n + 1];
            ok[0] = true;
            for i in 0..n {
                if ok[i] {
                    for w in &words {
                        if s[i..].starts_with(w.as_str()) && !w.is_empty() {
                            ok[i + w.len()] = true;
                        }
                    }
                }
            }
            B(ok[n])
        }
        // ---- bits ----
        "single-number" => {
            let mut cnt: HashMap<i64, usize> = HashMap::new();
            for x in ints(&a[0]) {
                *cnt.entry(x).or_default() += 1;
            }
            I(*cnt.iter().find(|(_, c)| **c == 1).unwrap().0)
        }
        "count-bits" => I(int(&a[0]).count_ones() as i64),
        "power-of-two" => B(int(&a[0]) > 0 && int(&a[0]).count_ones() == 1),
        "add-bits" => I(int(&a[0]) + int(&a[1])),
        // ---- executive floor ----
        "trapping-water" => {
            let h = ints(&a[0]);
            let n = h.len();
            let mut lm = vec![0i64; n];
            let mut rm = vec![0i64; n];
            for i in 0..n {
                lm[i] = h[i].max(if i > 0 { lm[i - 1] } else { 0 });
            }
            for i in (0..n).rev() {
                rm[i] = h[i].max(if i + 1 < n { rm[i + 1] } else { 0 });
            }
            I((0..n).map(|i| lm[i].min(rm[i]) - h[i]).sum())
        }
        "longest-palindrome" => {
            let s: Vec<char> = st(&a[0]).chars().collect();
            let n = s.len();
            let mut pal = vec![vec![false; n]; n];
            let (mut bs, mut bl) = (0, 0);
            for len in 1..=n {
                for i in 0..=n - len {
                    let j = i + len - 1;
                    pal[i][j] = s[i] == s[j] && (len <= 2 || pal[i + 1][j - 1]);
                    if pal[i][j] && len > bl {
                        bl = len;
                        bs = i;
                    } else if pal[i][j] && len == bl && i < bs {
                        bs = i;
                    }
                }
            }
            S(s[bs..bs + bl].iter().collect())
        }
        "n-queens" => {
            fn go(row: usize, n: usize, cols: u32, d1: u32, d2: u32) -> i64 {
                if row == n {
                    return 1;
                }
                let mut total = 0;
                for c in 0..n {
                    let (a, b) = (1u32 << c, 1u32 << (row + c));
                    let d = 1u32 << (row + n - c);
                    let _ = (a, b);
                    if cols & (1 << c) == 0 && d1 & (1 << (row + c)) == 0 && d2 & d == 0 {
                        total += go(row + 1, n, cols | (1 << c), d1 | (1 << (row + c)), d2 | d);
                    }
                }
                total
            }
            I(go(0, int(&a[0]) as usize, 0, 0, 0))
        }
        "subsets" => {
            let n = ints(&a[0]);
            L((0..1u32 << n.len()).map(|m| il((0..n.len()).filter(|i| m >> i & 1 == 1).map(|i| n[i]).collect::<Vec<_>>())).collect())
        }
        _ => return None,
    })
}

fn canon(v: V, mode: Compare) -> V {
    match (mode, v) {
        (Compare::Exact, v) => v,
        (Compare::SortOuter, L(mut xs)) => {
            xs.sort();
            L(xs)
        }
        (Compare::SortAll, L(xs)) => {
            let mut ys: Vec<V> = xs.into_iter().map(|x| canon(x, Compare::SortAll)).collect();
            ys.sort();
            L(ys)
        }
        (_, v) => v,
    }
}

#[test]
fn every_level_matches_its_independent_oracle() {
    let campaign = Campaign::shipped();
    let filter = std::env::var("ALGOCLERK_LEVEL").ok();
    let mut problems = Vec::new();
    let mut checked = 0;
    for level in campaign.all_levels() {
        if filter.as_ref().is_some_and(|f| f != &level.id) {
            continue;
        }
        let judge = Judge::new(level).unwrap_or_else(|e| panic!("{}: {e}", level.id));
        let mut any = false;
        for case in &judge.suite {
            let mut nodes = vec![NodeData::default()];
            let args: Vec<V> = level
                .signature
                .params
                .iter()
                .zip(&case.args)
                .map(|(p, lit)| conv(&parse_value(lit, &p.ty, &mut nodes).unwrap_or_else(|e| panic!("{}: arg: {e}", level.id)), &p.ty, &nodes))
                .collect();
            let Some(expect) = oracle(&level.id, &args) else { continue };
            any = true;
            let mut enodes = vec![NodeData::default()];
            let want = conv(&parse_value(&case.expect, &level.signature.ret, &mut enodes).unwrap_or_else(|e| panic!("{}: expect: {e}", level.id)), &level.signature.ret, &enodes);
            if canon(expect.clone(), level.compare) != canon(want.clone(), level.compare) {
                problems.push(format!("{}: args {:?}: oracle says {:?}, expected {:?}", level.id, case.args.iter().map(|a| a.chars().take(60).collect::<String>()).collect::<Vec<_>>(), expect, want));
                break;
            }
        }
        if !any {
            problems.push(format!("{}: no oracle", level.id));
        }
        checked += 1;
    }
    assert!(problems.is_empty(), "{} of {checked} levels disagree:\n{}", problems.len(), problems.iter().map(|p| p.chars().take(400).collect::<String>()).collect::<Vec<_>>().join("\n"));
    if filter.is_none() {
        assert!(checked >= 60, "campaign must have at least 60 levels, has {checked}");
    }
}
