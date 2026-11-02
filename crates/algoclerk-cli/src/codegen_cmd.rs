// SPDX-License-Identifier: Apache-2.0
//! `algoclerk-cli codegen`: print a solution as formatted C++ (the only supported language for now; the
//! translation to C and Rust will come back later) and optionally compile it with the host compilers and
//! compare it with the level's expected results.

use crate::args::Args;
use crate::commands::{campaign, find_level};
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::harness::{
    available_compilers, compile as cc_compile, run_binary, translation_unit,
};
use algoclerk_core::cpp::print::print_program;
use algoclerk_core::cpp::run::compile;
use algoclerk_core::judge::{Judge, canonical};

/// Print a program as formatted C++.
pub fn codegen(args: &Args) -> Result<u8, String> {
    let lang = args.opt("lang").unwrap_or("cpp");
    if lang != "cpp" {
        println!(
            "Translating to {lang} is not available yet: the clerks only speak C++ now. Try --lang cpp."
        );
        return Ok(0);
    }
    let (text, level) = if let Some(file) = args.opt("file") {
        let text = std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
        (text, None)
    } else {
        let c = campaign(args);
        let l = find_level(
            &c,
            args.positional
                .first()
                .ok_or("usage: codegen LEVEL|--file FILE --lang cpp")?,
        )?
        .clone();
        let text = match args.positional.get(1) {
            Some(f) => std::fs::read_to_string(f).map_err(|e| format!("cannot read {f}: {e}"))?,
            None => l.reference.clone(),
        };
        (text, Some(l))
    };
    let (prelude, opts) = level.as_ref().map_or_else(
        || (String::new(), CheckOpts::all()),
        |l| (l.prelude.clone(), l.check_opts()),
    );
    let sol = compile(&prelude, &text, &opts)
        .map_err(|d| d.first().map(|d| d.msg.clone()).unwrap_or_default())?;
    if !args.flag("verify") {
        print!("{}", print_program(&sol.program, args.flag("no-prelude")));
        return Ok(0);
    }
    let level = level.ok_or("--verify needs a level")?;
    let judge = Judge::new(&level)?;
    let cases: Vec<_> = judge.suite.iter().filter(|c| !c.large).collect();
    let argv: Vec<Vec<String>> = cases.iter().map(|c| c.args.clone()).collect();
    let src = translation_unit(&level, &text, &argv)?;
    let comps = available_compilers();
    if comps.is_empty() {
        println!("no C++ compiler installed; nothing to verify");
        return Ok(0);
    }
    let dir = std::env::temp_dir().join(format!("algoclerk-verify-{}", std::process::id()));
    let mut bad = 0;
    for comp in comps {
        let built = cc_compile(&comp, &src, &dir, "verify", false, 240);
        let out = built.and_then(|b| run_binary(&b.exe, 60, false));
        match out {
            Err(e) => {
                bad += 1;
                println!("{}: FAIL\n{e}", comp.exe);
            }
            Ok(lines) => {
                let mism = lines
                    .iter()
                    .zip(cases.iter())
                    .filter(|(l, c)| {
                        canonical(l, &level.signature.ret.ty, level.compare) != c.expect
                    })
                    .count();
                if mism == 0 && lines.len() == cases.len() {
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
