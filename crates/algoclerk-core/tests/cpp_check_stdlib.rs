// SPDX-License-Identifier: Apache-2.0
//! Every example of the library table (`stdlib::MEMBERS`) is real C++ and must pass the checker.

use algoclerk_core::cpp::check::{CheckOpts, check};
use algoclerk_core::cpp::parse::{ParseOpts, parse};
use algoclerk_core::cpp::stdlib::{MEMBERS, example_program};

#[test]
fn every_table_example_checks() {
    let mut bad = Vec::new();
    for m in MEMBERS {
        let src = example_program(m);
        let prog = match parse(&src, &ParseOpts::default()) {
            Ok(p) => p,
            Err(d) => {
                bad.push(format!("{} {}: parse: {}", m.recv, m.name, d[0].msg));
                continue;
            }
        };
        if let Err(ds) = check(&prog, &CheckOpts::all()) {
            bad.push(format!(
                "{} {}: {}\n    {}",
                m.recv,
                m.name,
                m.example,
                ds.iter()
                    .map(|d| format!("[{}] {}", d.code, d.msg))
                    .collect::<Vec<_>>()
                    .join("\n    ")
            ));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}
