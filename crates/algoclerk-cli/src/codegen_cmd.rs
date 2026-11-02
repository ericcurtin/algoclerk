// SPDX-License-Identifier: Apache-2.0
//! `algoclerk-cli codegen`: translate a program to C, C++ or Rust, optionally compile and compare.

use crate::args::Args;
use crate::commands::{campaign, find_level};
use algoclerk_core::codegen::compile::{available, compile_and_run};
use algoclerk_core::codegen::{Lang, generate, harness};
use algoclerk_core::judge::{Judge, canonical};
use algoclerk_core::lang::check::{Allowed, CheckOpts};
use algoclerk_core::lang::run::compile;

/// Translate a program to C, C++ or Rust.
pub fn codegen(args: &Args) -> Result<u8, String> {
    let lang =
        Lang::parse(args.opt("lang").unwrap_or("rust")).ok_or("--lang must be c, cpp or rust")?;
    let (text, entry, level) = if let Some(file) = args.opt("file") {
        let text = std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
        (text, None, None)
    } else {
        let c = campaign(args);
        let l = find_level(
            &c,
            args.positional
                .first()
                .ok_or("usage: codegen LEVEL|--file FILE --lang c|cpp|rust")?,
        )?
        .clone();
        let text = match args.positional.get(1) {
            Some(f) => std::fs::read_to_string(f).map_err(|e| format!("cannot read {f}: {e}"))?,
            None => l.reference.clone(),
        };
        let n = l.signature.name.clone();
        (text, Some(n), Some(l))
    };
    let opts = level.as_ref().map_or(
        CheckOpts {
            allowed: Allowed::all(),
            entry: None,
        },
        |l| l.check_opts(),
    );
    let sol =
        compile(&text, &opts).map_err(|d| d.first().map(|d| d.msg.clone()).unwrap_or_default())?;
    let entry_name = entry
        .clone()
        .or_else(|| sol.program.fns.last().map(|f| f.name.clone()))
        .unwrap_or_default();
    let g = generate(&sol, lang, &entry_name);
    if !args.flag("verify") {
        if !args.flag("no-prelude") {
            println!("{}", g.prelude);
        }
        print!("{}", g.code);
        return Ok(0);
    }
    let level = level.ok_or("--verify needs a level")?;
    let judge = Judge::new(&level)?;
    let cases: Vec<_> = judge.suite.iter().filter(|c| !c.large).collect();
    let argv: Vec<Vec<String>> = cases.iter().map(|c| c.args.clone()).collect();
    let src = harness::program(lang, &level.signature, &g, &argv)?;
    let comps = available(lang);
    if comps.is_empty() {
        println!("no {} compiler installed; nothing to verify", lang.name());
        return Ok(0);
    }
    let dir = std::env::temp_dir().join(format!("algoclerk-verify-{}", std::process::id()));
    let mut bad = 0;
    for comp in comps {
        match compile_and_run(&comp, &src, &dir, "verify", true) {
            Err(e) => {
                bad += 1;
                println!("{}: FAIL\n{e}", comp.exe);
            }
            Ok(out) => {
                let mism = out
                    .lines()
                    .zip(cases.iter())
                    .filter(|(l, c)| canonical(l, &level.signature.ret, level.compare) != c.expect)
                    .count();
                if mism == 0 && out.lines().count() == cases.len() {
                    println!(
                        "{}: ok ({} tests match the interpreter)",
                        comp.exe,
                        cases.len()
                    );
                } else {
                    bad += 1;
                    println!("{}: {mism} mismatches", comp.exe);
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(u8::from(bad > 0))
}
