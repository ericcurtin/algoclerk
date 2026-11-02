// SPDX-License-Identifier: Apache-2.0
//! Fuzz-style robustness tests: nothing the player (or a level author) types may panic the core.

mod support;

use algoclerk_core::asm::{self, parse};
use algoclerk_core::codegen::Lang;
use algoclerk_core::codegen::highlight::highlight;
use algoclerk_core::level::Level;
use algoclerk_core::progress::Progress;
use algoclerk_core::share;
use algoclerk_core::vm::{Machine, Status, run_program};
use proptest::prelude::*;
use std::sync::Arc;

fn asm_soup() -> impl Strategy<Value = String> {
    let word = prop_oneof![
        Just("INBOX".to_string()),
        Just("OUTBOX".to_string()),
        Just("COPYTO".to_string()),
        Just("COPYFROM".to_string()),
        Just("ADD".to_string()),
        Just("JUMP".to_string()),
        Just("JUMPZ".to_string()),
        Just("BUMPUP".to_string()),
        Just("[".to_string()),
        Just("]".to_string()),
        Just(":".to_string()),
        Just("--".to_string()),
        Just("#".to_string()),
        Just("\n".to_string()),
        Just(" ".to_string()),
        "[a-z]{1,4}",
        "[0-9]{1,5}",
        "\\PC{0,3}",
    ];
    prop::collection::vec(word, 0..40).prop_map(|v| v.join(" "))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn parser_never_panics_on_arbitrary_text(s in "\\PC{0,200}") {
        let _ = parse(&s);
        let _ = asm::assemble(&s);
        let _ = share::import(&s);
    }

    #[test]
    fn parser_never_panics_on_token_soup(s in asm_soup()) {
        if let Ok(l) = parse(&s) {
            let _ = l.compile();
            // Printing and re-parsing is stable.
            let again = parse(&l.to_text()).expect("canonical text parses");
            let strip = |l: &asm::Listing| l.lines.iter().map(|x| (x.kind.clone(), x.comment.clone())).collect::<Vec<_>>();
            prop_assert_eq!(strip(&l), strip(&again));
        }
    }

    #[test]
    fn other_parsers_never_panic(s in "\\PC{0,300}") {
        let _ = Level::from_toml(&s, "x");
        let _ = Progress::from_json(&s);
        let _ = algoclerk_core::i18n::Strings::from_toml(&s);
        let joined: String = highlight(Lang::C, &s).iter().map(|t| t.text).collect();
        prop_assert_eq!(joined, s);
    }

    #[test]
    fn vm_never_panics_and_is_deterministic(
        p in support::wild_program(),
        inbox in support::inbox(),
        floor_empty in any::<bool>(),
    ) {
        let floor = if floor_empty { vec![None; support::FLOOR] } else { support::floor() };
        let p = Arc::new(p);
        let a = run_program(&p, &inbox, &floor, 500);
        let b = run_program(&p, &inbox, &floor, 500);
        prop_assert_eq!(&a, &b);
        prop_assert!(a.steps <= 500);
        prop_assert!(a.status != Status::Running);
    }

    #[test]
    fn rewinding_restores_every_earlier_state(
        p in support::wild_program(),
        inbox in support::inbox(),
        steps in 0usize..60,
    ) {
        let p = Arc::new(p);
        let mut m = Machine::new(p, inbox, support::floor(), 1000).with_history();
        let mut states = vec![(m.hand, m.pc, m.floor.clone(), m.outbox.clone(), m.in_pos, m.steps)];
        for _ in 0..steps {
            if m.is_done() { break; }
            m.step();
            if matches!(m.status, Status::Error(_)) { break; }
            states.push((m.hand, m.pc, m.floor.clone(), m.outbox.clone(), m.in_pos, m.steps));
        }
        while states.len() > 1 {
            states.pop();
            prop_assert!(m.step_back());
            let want = states.last().unwrap();
            let got = (m.hand, m.pc, m.floor.clone(), m.outbox.clone(), m.in_pos, m.steps);
            prop_assert_eq!(&got, want);
        }
    }

    #[test]
    fn program_text_roundtrips(p in support::wild_program()) {
        let text = p.to_text();
        let again = asm::assemble(&text).expect("canonical text assembles");
        prop_assert_eq!(&p.instrs, &again.instrs);
    }
}
