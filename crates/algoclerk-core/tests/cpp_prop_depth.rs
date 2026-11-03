// SPDX-License-Identifier: Apache-2.0
//! Regression test for the differential property test: the random program generator must keep every
//! expression well inside the parser's nesting limit (`parse.depth`). A generated program that the
//! parser rejects for being nested too deeply is a generator bug, not an interpreter divergence.
//!
//! CI once failed on `ALGOCLERK_PROP_SEED=9 ALGOCLERK_PROP_CASES=400`, program 374: calls nested in
//! the arguments of calls did not use up the generator's depth budget. This test generates exactly that
//! batch (and a few more seeds) and only runs the checker and the interpreter: no compiler is needed.

mod prop_common;

use prop_common::{bytes_strategy, generate, interpret};
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

fn too_deep(seed: u64, cases: usize) -> Vec<(usize, String)> {
    let mut s = [0u8; 32];
    s[..8].copy_from_slice(&seed.to_le_bytes());
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &s);
    let mut runner = TestRunner::new_with_rng(Config::default(), rng);
    let strat = bytes_strategy();
    let mut bad = Vec::new();
    for i in 0..cases {
        let tree = strat.new_tree(&mut runner).expect("tree");
        let p = generate(tree.current());
        if let Err(e) = interpret(&p)
            && e.contains("parse.depth")
        {
            bad.push((i, e));
        }
    }
    bad
}

#[test]
fn generated_programs_never_hit_the_parse_depth_limit() {
    for seed in [9u64, 1, 2, 3] {
        let bad = too_deep(seed, 400);
        assert!(
            bad.is_empty(),
            "seed {seed}: {} generated programs are nested too deeply, first: program {} {}",
            bad.len(),
            bad[0].0,
            bad[0].1
        );
    }
}
