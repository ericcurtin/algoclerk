// SPDX-License-Identifier: Apache-2.0
//! `algoclerk-cli codegen LEVEL [FILE] --lang c|cpp|rust [--verify] [--no-prelude]`: print a solution as
//! formatted C++ or translated to C or Rust, and optionally compile it with the host compilers (together
//! with a generated test harness) and compare the results with the interpreter.

use crate::args::Args;
use crate::commands::{campaign, find_level};
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::codegen::harness::{Outcome, check_solution, standard_cases, toolchains};
use algoclerk_core::cpp::codegen::{Lang, generate};
use algoclerk_core::cpp::harness::{
    available_compilers, compile as cc_compile, run_binary, translation_unit,
};
use algoclerk_core::cpp::print::print_program;
use algoclerk_core::cpp::run::Compiled;
use algoclerk_core::cpp::run::compile;
use algoclerk_core::judge::{Judge, canonical};
use algoclerk_core::level::Level;

/// Print a program as formatted C++.
pub fn codegen(args: &Args) -> Result<u8, String> {
    let lang_name = args.opt("lang").unwrap_or("cpp");
    let lang = Lang::parse(lang_name)
        .ok_or_else(|| format!("unknown language {lang_name}: use c, cpp or rust"))?;
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
    if lang != Lang::Cpp {
        return translate(args, lang, &sol, &text, level.as_ref());
    }
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

/// `--lang c|rust`: print the translation; with `--verify` compile it with the host compilers and compare.
fn translate(
    args: &Args,
    lang: Lang,
    sol: &Compiled,
    text: &str,
    level: Option<&Level>,
) -> Result<u8, String> {
    let entry = level.map_or_else(
        || {
            sol.program
                .player_items()
                .iter()
                .find_map(|i| match &i.kind {
                    algoclerk_core::cpp::ast::ItemKind::Func(f) if f.body.is_some() => {
                        Some(f.name.clone())
                    }
                    _ => None,
                })
                .unwrap_or_default()
        },
        |l| l.signature.name.clone(),
    );
    let g = match generate(sol, lang, &entry) {
        Ok(g) => g,
        Err(m) => {
            println!("{m}");
            return Ok(u8::from(args.flag("verify")));
        }
    };
    if args.flag("no-prelude") {
        print!("{}", g.code);
    } else {
        print!("{}", g.full());
    }
    if !args.flag("verify") {
        return Ok(0);
    }
    let level = level.ok_or("--verify needs a level")?;
    let tcs = toolchains(lang);
    if tcs.is_empty() {
        println!("no {} compiler installed; nothing to verify", lang.name());
        return Ok(0);
    }
    let cases = standard_cases(level)?;
    let dir = std::env::temp_dir().join(format!("algoclerk-verify-{}", std::process::id()));
    let out = check_solution(level, text, lang, &tcs, &dir, "verify", &cases);
    let _ = std::fs::remove_dir_all(&dir);
    match out {
        Outcome::Ok { cases, warnings } => {
            let names: Vec<&str> = tcs.iter().map(|t| t.exe).collect();
            println!(
                "\n{}: ok with {} ({cases} tests match the interpreter)",
                lang.name(),
                names.join(" and ")
            );
            if !warnings.trim().is_empty() {
                println!("compiler warnings:\n{warnings}");
            }
            Ok(0)
        }
        Outcome::Failed(m) => {
            println!("\n{}: FAIL\n{m}", lang.name());
            Ok(1)
        }
        Outcome::Untranslatable(m) => {
            println!("{m}");
            Ok(1)
        }
        Outcome::NoCompiler => Ok(0),
    }
}
