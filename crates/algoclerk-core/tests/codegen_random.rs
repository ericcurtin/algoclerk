// SPDX-License-Identifier: Apache-2.0
//! Property tests for the code generator: random programs are translated to C, C++ and Rust,
//! compiled with the host compilers and compared with the interpreter.

mod support;

use algoclerk_core::asm::Program;
use algoclerk_core::codegen::structure::structure;
use algoclerk_core::codegen::{self, Lang, Options};
use algoclerk_core::vm::{Status, run_program};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use std::sync::Arc;

fn runner(seed: u8) -> TestRunner {
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]);
    TestRunner::new_with_rng(Config::default(), rng)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn well_structured_programs_are_recovered(p in support::structured_program()) {
        prop_assert!(structure(&p).is_some(), "not structured:\n{}", p.to_text());
    }

    #[test]
    fn generation_never_panics_and_is_deterministic(p in support::wild_program()) {
        for lang in Lang::ALL {
            let a = codegen::generate(&p, lang, None, &Options::default());
            let b = codegen::generate(&p, lang, None, &Options::default());
            prop_assert_eq!(&a.code, &b.code);
            let f = codegen::generate(&p, lang, None, &Options { force_fallback: true, ..Options::default() });
            prop_assert!(!f.structured);
        }
    }
}

#[cfg(feature = "compile-tests")]
mod compiled {
    use super::*;
    use algoclerk_core::codegen::compile::{self, Case};
    use algoclerk_core::codegen::harness::HarnessCase;
    use std::path::PathBuf;

    const BATCH: usize = 60;

    fn dir() -> PathBuf {
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("codegen-random")
    }

    /// Programs that finish without error on at least one input, with those inputs.
    fn usable(
        strategy: &impl Strategy<Value = Program>,
        seed: u8,
        want: usize,
    ) -> Vec<(Arc<Program>, Vec<Vec<i32>>)> {
        let mut r = runner(seed);
        let mut out = Vec::new();
        let mut attempts = 0;
        while out.len() < want && attempts < 20_000 {
            attempts += 1;
            let p = strategy.new_tree(&mut r).unwrap().current();
            let p = Arc::new(p);
            let mut inputs = Vec::new();
            for _ in 0..8 {
                let inbox = support::inbox().new_tree(&mut r).unwrap().current();
                let res = run_program(&p, &inbox, &support::floor(), 600);
                if res.status == Status::Finished {
                    inputs.push(inbox);
                }
                if inputs.len() == 4 {
                    break;
                }
            }
            if !inputs.is_empty() {
                out.push((p, inputs));
            }
        }
        assert!(
            out.len() >= want / 2,
            "only {} usable random programs",
            out.len()
        );
        let looping = out
            .iter()
            .filter(|(p, _)| {
                p.instrs
                    .iter()
                    .enumerate()
                    .any(|(i, ins)| ins.target().is_some_and(|t| *t <= i))
            })
            .count();
        let with_output = out
            .iter()
            .filter(|(p, ins)| {
                ins.iter()
                    .any(|i| !run_program(p, i, &support::floor(), 600).outbox.is_empty())
            })
            .count();
        let structured = out.iter().filter(|(p, _)| structure(p).is_some()).count();
        eprintln!(
            "corpus seed {seed}: {} programs ({looping} with loops, {with_output} producing output, {structured} structured)",
            out.len()
        );
        out
    }

    fn run_batch(lang: Lang, programs: &[(Arc<Program>, Vec<Vec<i32>>)], tag: &str) {
        let compilers = compile::available(lang);
        if compilers.is_empty() {
            eprintln!("SKIPPED: no compiler for {}", lang.title());
            return;
        }
        let mut items: Vec<Case<'_>> = Vec::new();
        for (idx, (p, inputs)) in programs.iter().enumerate() {
            for fallback in [false, true] {
                let cases: Vec<HarnessCase> = inputs
                    .iter()
                    .map(|i| HarnessCase {
                        inbox: i.clone(),
                        floor: support::floor(),
                    })
                    .collect();
                let expected: Vec<Vec<i32>> = inputs
                    .iter()
                    .map(|i| run_program(p, i, &support::floor(), 600).outbox)
                    .collect();
                let opts = Options {
                    function_name: Some(format!(
                        "prog_{idx}_{}",
                        if fallback { "fb" } else { "st" }
                    )),
                    floor_init: Some(support::floor().iter().map(|v| v.unwrap_or(0)).collect()),
                    force_fallback: fallback,
                    ..Options::default()
                };
                items.push(compile::prepare(p, lang, None, &opts, cases, expected));
            }
        }
        for c in &compilers {
            let stem = format!("{tag}_{}_{}", lang.key(), c.exe.replace('+', "p"));
            compile::verify(c, &items, &dir(), &stem, false).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    #[test]
    fn c_random_programs_match_the_interpreter() {
        for (seed, name, structured) in [(1u8, "st", true), (2, "wild", false)] {
            let progs = if structured {
                usable(&support::structured_program(), seed, BATCH)
            } else {
                usable(&support::wild_program(), seed, BATCH)
            };
            run_batch(Lang::C, &progs, name);
        }
    }

    #[test]
    fn cpp_random_programs_match_the_interpreter() {
        for (seed, name, structured) in [(3u8, "st", true), (4, "wild", false)] {
            let progs = if structured {
                usable(&support::structured_program(), seed, BATCH)
            } else {
                usable(&support::wild_program(), seed, BATCH)
            };
            run_batch(Lang::Cpp, &progs, name);
        }
    }

    #[test]
    fn rust_random_programs_match_the_interpreter() {
        for (seed, name, structured) in [(5u8, "st", true), (6, "wild", false)] {
            let progs = if structured {
                usable(&support::structured_program(), seed, BATCH)
            } else {
                usable(&support::wild_program(), seed, BATCH)
            };
            run_batch(Lang::Rust, &progs, name);
        }
    }

    #[test]
    fn the_harness_detects_wrong_behaviour() {
        // A deliberately wrong expectation must be reported, proving the comparison is live.
        let p = Arc::new(algoclerk_core::asm::assemble("a:\nINBOX\nOUTBOX\nJUMP a").unwrap());
        for lang in Lang::ALL {
            let Some(c) = compile::available(lang).into_iter().next() else {
                continue;
            };
            let case = compile::prepare(
                &p,
                lang,
                None,
                &Options::default(),
                vec![HarnessCase {
                    inbox: vec![1, 2],
                    floor: vec![],
                }],
                vec![vec![1, 3]],
            );
            let err = compile::verify(
                &c,
                &[case],
                &dir(),
                &format!("negative_{}", lang.key()),
                false,
            )
            .unwrap_err();
            assert!(err.contains("generated code printed"), "{err}");
        }
    }
}
