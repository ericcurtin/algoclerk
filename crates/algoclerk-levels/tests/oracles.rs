// SPDX-License-Identifier: Apache-2.0
//! Independent Rust implementations of every shipped puzzle ("oracles").
//!
//! The expected outputs of generated tests come from each level's reference solution run in the
//! VM. These oracles are separately written, plain Rust solutions of the problem statements, used
//! to prove that the reference solutions (and therefore the generated expectations) are right.

use algoclerk_core::level::TestCase;
use std::collections::{HashSet, VecDeque};

type Oracle = fn(&TestCase) -> Vec<i32>;

/// Values up to (not including) the first 0 marker.
fn upto_zero(v: &[i32]) -> &[i32] {
    let end = v.iter().position(|x| *x == 0).unwrap_or(v.len());
    &v[..end]
}

fn floor_vals(t: &TestCase, from: usize, len: usize) -> Vec<i32> {
    (from..from + len)
        .map(|i| t.floor[i].unwrap_or(0))
        .collect()
}

fn is_prime(n: i32) -> bool {
    n >= 2 && (2..n).all(|d| n % d != 0)
}

fn sorted(mut v: Vec<i32>) -> Vec<i32> {
    v.sort_unstable();
    v
}

fn oracle_table() -> Vec<(&'static str, Oracle)> {
    vec![
        ("echo", |t| t.inbox.clone()),
        ("swap-pairs", |t| {
            t.inbox.chunks(2).flat_map(|c| [c[1], c[0]]).collect()
        }),
        ("double-it", |t| t.inbox.iter().map(|x| x * 2).collect()),
        ("sum-pairs", |t| {
            t.inbox.chunks(2).map(|c| c[0] + c[1]).collect()
        }),
        ("negate", |t| t.inbox.iter().map(|x| -x).collect()),
        ("absolute-value", |t| {
            t.inbox.iter().map(|x| x.abs()).collect()
        }),
        ("sign-of", |t| t.inbox.iter().map(|x| x.signum()).collect()),
        ("countdown", |t| (1..=t.inbox[0]).rev().collect()),
        ("sum-list", |t| vec![upto_zero(&t.inbox).iter().sum()]),
        ("running-sum", |t| {
            t.inbox
                .iter()
                .scan(0, |acc, x| {
                    *acc += x;
                    Some(*acc)
                })
                .collect()
        }),
        ("max-of", |t| {
            vec![*upto_zero(&t.inbox).iter().max().unwrap()]
        }),
        ("min-and-max", |t| {
            let l = upto_zero(&t.inbox);
            vec![*l.iter().min().unwrap(), *l.iter().max().unwrap()]
        }),
        ("count-matches", |t| {
            vec![
                upto_zero(&t.inbox[1..])
                    .iter()
                    .filter(|x| **x == t.inbox[0])
                    .count() as i32,
            ]
        }),
        ("longest-run", |t| {
            let l = upto_zero(&t.inbox);
            let (mut best, mut run) = (1, 1);
            for w in l.windows(2) {
                run = if w[0] == w[1] { run + 1 } else { 1 };
                best = best.max(run);
            }
            vec![best]
        }),
        ("majority-vote", |t| {
            let l = upto_zero(&t.inbox);
            let m = l
                .iter()
                .find(|x| l.iter().filter(|y| y == x).count() * 2 > l.len());
            vec![*m.unwrap()]
        }),
        ("contains-value", |t| {
            vec![i32::from(upto_zero(&t.inbox[1..]).contains(&t.inbox[0]))]
        }),
        ("factorial", |t| vec![(1..=t.inbox[0]).product()]),
        ("power", |t| vec![t.inbox[0].pow(t.inbox[1] as u32)]),
        ("fibonacci", |t| {
            let (mut a, mut b) = (0i32, 1i32);
            for _ in 0..t.inbox[0] {
                (a, b) = (b, a + b);
            }
            vec![a]
        }),
        ("gcd", |t| {
            let (mut a, mut b) = (t.inbox[0], t.inbox[1]);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            vec![a]
        }),
        ("is-prime", |t| vec![i32::from(is_prime(t.inbox[0]))]),
        ("fizz-buzz", |t| {
            (1..=t.inbox[0])
                .map(|i| match (i % 3 == 0, i % 5 == 0) {
                    (true, true) => -3,
                    (true, false) => -1,
                    (false, true) => -2,
                    _ => i,
                })
                .collect()
        }),
        ("digit-sum", |t| {
            vec![
                t.inbox[0]
                    .to_string()
                    .chars()
                    .map(|c| c.to_digit(10).unwrap() as i32)
                    .sum(),
            ]
        }),
        ("bit-count", |t| {
            let c = t.inbox[0].count_ones() as i32;
            vec![c, c % 2]
        }),
        ("collatz-steps", |t| {
            let (mut n, mut s) = (t.inbox[0], 0);
            while n != 1 {
                n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
                s += 1;
            }
            vec![s]
        }),
        ("reverse-list", |t| {
            upto_zero(&t.inbox).iter().rev().copied().collect()
        }),
        ("find-duplicate", |t| {
            let l = upto_zero(&t.inbox);
            vec![i32::from(l.iter().collect::<HashSet<_>>().len() != l.len())]
        }),
        ("histogram", |t| {
            let l = upto_zero(&t.inbox);
            (1..=5)
                .map(|k| l.iter().filter(|x| **x == k).count() as i32)
                .collect()
        }),
        ("palindrome-check", |t| {
            let l = upto_zero(&t.inbox);
            vec![i32::from(l.iter().eq(l.iter().rev()))]
        }),
        ("remove-duplicates", |t| {
            let mut v = upto_zero(&t.inbox).to_vec();
            v.dedup();
            v
        }),
        ("missing-number", |t| {
            let l = upto_zero(&t.inbox);
            let n = l.len() as i32 + 1;
            vec![n * (n + 1) / 2 - l.iter().sum::<i32>()]
        }),
        ("merge-sorted", |t| {
            let a = upto_zero(&t.inbox);
            let b = upto_zero(&t.inbox[a.len() + 1..]);
            sorted([a, b].concat())
        }),
        ("valid-brackets", |t| {
            let mut stack = Vec::new();
            for &x in upto_zero(&t.inbox) {
                if x > 0 {
                    stack.push(x);
                } else if stack.pop() != Some(-x) {
                    return vec![0];
                }
            }
            vec![i32::from(stack.is_empty())]
        }),
        ("insertion-sort", |t| sorted(upto_zero(&t.inbox).to_vec())),
        ("selection-sort", |t| sorted(upto_zero(&t.inbox).to_vec())),
        ("bubble-sort", |t| {
            sorted(floor_vals(t, 0, t.inbox[0] as usize))
        }),
        ("binary-search", |t| {
            let a = floor_vals(t, 0, t.inbox[0] as usize);
            vec![
                a.iter()
                    .position(|x| *x == t.inbox[1])
                    .map_or(-1, |p| p as i32),
            ]
        }),
        ("pair-sum", |t| {
            let a = floor_vals(t, 0, t.inbox[0] as usize);
            let target = t.inbox[1];
            let found = (0..a.len()).any(|i| (i + 1..a.len()).any(|j| a[i] + a[j] == target));
            vec![i32::from(found)]
        }),
        ("climbing-stairs", |t| {
            let n = t.inbox[0] as usize;
            let mut w = vec![0i32; n + 4];
            w[0] = 1;
            for k in 1..=n {
                w[k] = w[k - 1]
                    + if k >= 2 { w[k - 2] } else { 0 }
                    + if k >= 3 { w[k - 3] } else { 0 };
            }
            vec![w[n]]
        }),
        ("coin-change", |t| {
            // Breadth-first search over amounts.
            let target = t.inbox[0];
            let mut seen = HashSet::from([0]);
            let mut frontier = vec![0];
            let mut coins = 0;
            loop {
                if frontier.contains(&target) {
                    return vec![coins];
                }
                let mut next = Vec::new();
                for a in frontier {
                    for c in [1, 3, 4] {
                        if a + c <= target && seen.insert(a + c) {
                            next.push(a + c);
                        }
                    }
                }
                frontier = next;
                coins += 1;
            }
        }),
        ("kadane", |t| {
            let l = upto_zero(&t.inbox);
            let mut best = i32::MIN;
            for i in 0..l.len() {
                for j in i..l.len() {
                    best = best.max(l[i..=j].iter().sum());
                }
            }
            vec![best]
        }),
        ("ring-buffer", |t| {
            let mut q: VecDeque<i32> = VecDeque::new();
            let mut out = Vec::new();
            for &c in upto_zero(&t.inbox) {
                if c > 0 {
                    if q.len() < 3 {
                        q.push_back(c);
                    }
                } else {
                    out.push(q.pop_front().unwrap_or(0));
                }
            }
            out
        }),
        ("list-walk", |t| {
            let mut out = Vec::new();
            let mut p = t.inbox[0];
            while p != -1 {
                out.push(t.floor[p as usize].unwrap());
                p = t.floor[p as usize + 1].unwrap();
            }
            out
        }),
        ("list-reverse", |t| {
            let mut out = Vec::new();
            let mut p = t.inbox[0];
            while p != -1 {
                out.push(t.floor[p as usize].unwrap());
                p = t.floor[p as usize + 1].unwrap();
            }
            out.reverse();
            out
        }),
        ("knapsack-tiny", |t| {
            let cap = t.inbox[0];
            let items: Vec<(i32, i32)> = upto_zero(&t.inbox[1..])
                .chunks(2)
                .map(|c| (c[0], c[1]))
                .collect();
            (0..1u32 << items.len())
                .filter_map(|mask| {
                    let (w, v) = items
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| mask >> i & 1 == 1)
                        .fold((0, 0), |a, (_, it)| (a.0 + it.0, a.1 + it.1));
                    (w <= cap).then_some(v)
                })
                .max()
                .into_iter()
                .collect()
        }),
        ("sieve", |t| {
            (2..=t.inbox[0]).filter(|n| is_prime(*n)).collect()
        }),
        ("run-length-encode", |t| {
            let l = upto_zero(&t.inbox);
            let mut out = Vec::new();
            let mut i = 0;
            while i < l.len() {
                let j = (i..l.len()).find(|j| l[*j] != l[i]).unwrap_or(l.len());
                out.extend([l[i], (j - i) as i32]);
                i = j;
            }
            out
        }),
        ("rotate-list", |t| {
            let l = upto_zero(&t.inbox[1..]);
            if l.is_empty() {
                return Vec::new();
            }
            let k = t.inbox[0] as usize % l.len();
            [&l[k..], &l[..k]].concat()
        }),
    ]
}

#[test]
fn oracles_agree_with_expected_outputs_of_every_test() {
    let campaign = algoclerk_levels::Campaign::shipped();
    let table = oracle_table();
    let mut checked = 0;
    for (id, oracle) in &table {
        let level = campaign
            .find(id)
            .unwrap_or_else(|| panic!("oracle for unknown level {id}"));
        let tests = level.build_tests().unwrap();
        for (i, t) in tests.iter().enumerate() {
            assert_eq!(
                oracle(t),
                t.expected,
                "level {id}, test {}: oracle and expected output differ for inbox {:?}",
                i + 1,
                t.inbox
            );
        }
        checked += tests.len();
    }
    assert!(checked > 500, "only {checked} tests were cross-checked");
}

#[test]
fn every_shipped_level_has_an_oracle() {
    let campaign = algoclerk_levels::Campaign::shipped();
    let table = oracle_table();
    let ids: HashSet<&str> = table.iter().map(|(id, _)| *id).collect();
    let missing: Vec<&str> = campaign
        .all_levels()
        .map(|l| l.id.as_str())
        .filter(|id| !ids.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "levels without an oracle in tests/oracles.rs: {missing:?}"
    );
}
