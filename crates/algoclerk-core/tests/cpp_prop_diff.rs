// SPDX-License-Identifier: Apache-2.0
//! Differential property test: random type-correct, UB-free programs of the C++ subset must be accepted by
//! the checker, run in the interpreter without a runtime error, and print exactly what the same program
//! prints when compiled and run with `g++ -std=c++17 -Wall -Wextra -O0` (and, for a sample of the batches or
//! with `ALGOCLERK_DIFF_CLANG=1` all of them, `clang++`, with `-fsanitize=undefined,address` where the
//! sanitizer runtime exists).
//!
//! `ALGOCLERK_PROP_CASES` sets the number of programs (default 40), `ALGOCLERK_PROP_SEED` the seed (printed
//! on every run), `ALGOCLERK_PROP_REPLAY=<hex bytes>` replays one choice vector. Failing programs are shrunk
//! (with proptest's value trees) before they are reported. Constructs on which the interpreter currently
//! diverges are excluded by `prop_common::KNOWN`.

mod prop_common;

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use prop_common::{bytes_strategy, case_count, evaluate, generate, have};

const BATCH: usize = 10;

fn seed() -> u64 {
    std::env::var("ALGOCLERK_PROP_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn class(e: &str) -> String {
    e.split(']').next().unwrap_or("").to_string()
}

/// Shrink a failing choice vector; returns the smallest failing vector found and its message.
fn shrink(mut tree: impl ValueTree<Value = Vec<u8>>, first: String) -> (Vec<u8>, String) {
    let cls = class(&first);
    let batch = usize::from(!cls.contains("clang"));
    let mut best = (tree.current(), first);
    let start = std::time::Instant::now();
    for _ in 0..80 {
        if start.elapsed().as_secs() > 90 || !tree.simplify() {
            break;
        }
        loop {
            let p = generate(tree.current());
            match evaluate(&[p], batch).remove(0) {
                Err(e) if class(&e) == cls => {
                    best = (tree.current(), e);
                    break;
                }
                _ => {
                    if !tree.complicate() {
                        return best;
                    }
                }
            }
        }
    }
    best
}

#[test]
fn random_programs_agree_with_the_compilers() {
    if !have("g++") {
        eprintln!("g++ not found: differential property test skipped");
        return;
    }
    if let Ok(h) = std::env::var("ALGOCLERK_PROP_REPLAY") {
        let p = generate(unhex(&h));
        println!("{}", p.source());
        println!("args: {:?}", p.args);
        let r = evaluate(&[p], 0).remove(0);
        assert!(r.is_ok(), "{}", r.unwrap_err());
        return;
    }
    let n = case_count(40);
    let sd = seed();
    eprintln!("cpp_prop_diff: {n} cases, seed {sd}");
    let mut s = [0u8; 32];
    s[..8].copy_from_slice(&sd.to_le_bytes());
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &s);
    let mut runner = TestRunner::new_with_rng(Config::default(), rng);
    let strat = bytes_strategy();
    let mut trees = Vec::new();
    for _ in 0..n {
        trees.push(strat.new_tree(&mut runner).expect("tree"));
    }
    let progs: Vec<_> = trees.iter().map(|t| generate(t.current())).collect();
    let chunks: Vec<(usize, &[prop_common::prog::Prog])> = progs
        .chunks(BATCH)
        .enumerate()
        .map(|(i, c)| (i, c))
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::<(usize, Vec<Result<(), String>>)>::new());
    let workers = std::env::var("ALGOCLERK_PROP_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    std::thread::scope(|sc| {
        for _ in 0..workers {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some((b, c)) = chunks.get(i) else { break };
                    let r = evaluate(c, *b);
                    results.lock().expect("results").push((*b, r));
                }
            });
        }
    });
    let mut results = results.into_inner().expect("results");
    results.sort_by_key(|(b, _)| *b);
    let mut failing = Vec::new();
    for (b, r) in results {
        for (j, v) in r.into_iter().enumerate() {
            if let Err(e) = v {
                failing.push((b * BATCH + j, e));
            }
        }
    }
    eprintln!("cpp_prop_diff: {} of {n} programs differ", failing.len());
    for (i, e) in &failing {
        let errs: Vec<&str> = e.lines().filter(|l| l.contains("error") || l.starts_with('[')).take(4).collect();
        eprintln!("  program {i}: {}", errs.join(" / "));
    }
    let mut report = Vec::new();
    let shrinking = std::env::var("ALGOCLERK_PROP_NOSHRINK").is_err();
    let mut trees: Vec<Option<_>> = trees.into_iter().map(Some).collect();
    for (i, e) in failing.into_iter().take(3) {
        let tree = trees[i].take().expect("tree");
        let (bytes, msg) = if shrinking { shrink(tree, e) } else { (tree.current(), e) };
        let p = generate(bytes.clone());
        report.push(format!(
            "---- {msg}\nreplay: ALGOCLERK_PROP_REPLAY={}\nargs: {:?}\n{}",
            hex(&bytes),
            p.args,
            p.body
        ));
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}
