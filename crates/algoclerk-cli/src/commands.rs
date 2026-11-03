// SPDX-License-Identifier: Apache-2.0
//! Subcommands of `algoclerk-cli`.

use crate::args::Args;
use algoclerk_core::cpp::check::CheckOpts;
use algoclerk_core::cpp::diag::line_col;
use algoclerk_core::cpp::harness::{DiffOpts, available_compilers, diff_level, translation_unit};
use algoclerk_core::cpp::interp::Limits;
use algoclerk_core::cpp::run::compile;
use algoclerk_core::judge::{FailKind, Judge, Medal};
use algoclerk_core::level::Level;
use algoclerk_core::validate::validate_level;
use algoclerk_levels::Campaign;
use std::fmt::Write as _;
use std::path::Path;

const USAGE: &str = "algoclerk-cli: run, judge, validate and test C++ solutions

USAGE:
  algoclerk-cli list [--levels DIR]                      all packs and levels
  algoclerk-cli show LEVEL                               the level brief
  algoclerk-cli starter LEVEL                            the starter program
  algoclerk-cli reference LEVEL                          the reference solution
  algoclerk-cli check FILE [--level LEVEL]               parse and type-check a program
  algoclerk-cli run FILE FUNCTION [ARG...]               run a function on literal arguments (sandbox)
  algoclerk-cli judge LEVEL FILE [--json]                judge a solution against a level
  algoclerk-cli validate [LEVEL ...] [--levels DIR]      validate level files (all when none given)
  algoclerk-cli table                                    judge table of every reference solution (markdown)
  algoclerk-cli subset-table                             the supported C++ library members (markdown, as in docs/CPP_SUBSET.md)
  algoclerk-cli tune LEVEL|--all [--write]               compute medal targets from the reference
  algoclerk-cli codegen LEVEL|--file FILE --lang cpp [--verify] [--no-prelude]
                                                         print the formatted C++ (other languages: not yet)
  algoclerk-cli harness LEVEL [FILE]                     print the real-compiler test program for a solution
  algoclerk-cli diff [LEVEL ...] [--full] [--no-sanitize]  build reference and naive with g++ and clang++ and compare
  algoclerk-cli help
";

const VALUE_OPTS: &[&str] = &["levels", "level", "lang", "file", "steps", "cap"];

/// Run a subcommand.
pub fn dispatch(argv: &[String]) -> Result<u8, String> {
    let Some(cmd) = argv.first() else {
        print!("{USAGE}");
        return Ok(0);
    };
    let args = Args::parse(&argv[1..], VALUE_OPTS)?;
    match cmd.as_str() {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(0)
        }
        "list" => list(&args),
        "show" => show(&args),
        "starter" => starter(&args),
        "reference" => reference(&args),
        "check" => check(&args),
        "run" => run(&args),
        "judge" => judge(&args),
        "validate" => validate(&args),
        "table" => table(&args),
        "subset-table" => {
            print!("{}", algoclerk_core::cpp::stdlib::markdown_table());
            Ok(0)
        }
        "harness" => harness(&args),
        "diff" => diff(&args),
        "tune" => tune(&args),
        "codegen" => crate::codegen_cmd::codegen(&args),
        other => Err(format!(
            "unknown command \"{other}\" (try `algoclerk-cli help`)"
        )),
    }
}

/// Load the shipped campaign (plus an optional extra directory).
pub fn campaign(args: &Args) -> Campaign {
    // `--levels DIR`: files under DIR replace the embedded files of the same pack/name (new ones are added)
    let c = match args.opt("levels") {
        Some(dir) => Campaign::shipped_overlaid(Path::new(dir)),
        None => Campaign::shipped(),
    };
    for e in &c.load_errors {
        eprintln!("warning: {e}");
    }
    c
}

/// Find a level by id.
pub fn find_level<'a>(c: &'a Campaign, id: &str) -> Result<&'a Level, String> {
    let bare = id.rsplit('/').next().unwrap_or(id);
    c.find(bare)
        .ok_or_else(|| format!("no level called \"{id}\" (see `algoclerk-cli list`)"))
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn list(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    for p in &c.packs {
        println!(
            "{}  ({} levels)  {}",
            p.info.title,
            p.levels.len(),
            p.info.blurb
        );
        for l in &p.levels {
            println!("  {:<28} {}", l.id, l.title);
        }
    }
    println!("\n{} levels in {} wings", c.level_count(), c.packs.len());
    Ok(0)
}

fn show(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let l = find_level(&c, args.positional.first().ok_or("usage: show LEVEL")?)?;
    println!("# {}  [{}]", l.title, l.pack);
    println!("\n{}\n\n{}\n", l.story, l.statement);
    if !l.prelude.trim().is_empty() {
        println!("{}", l.prelude.trim_end());
        println!();
    }
    println!("    {}\n", l.signature_text);
    for (i, e) in l.examples.iter().enumerate() {
        println!(
            "example {}: ({}) -> {}   {}",
            i + 1,
            e.args.join(", "),
            e.expect,
            e.note
        );
    }
    println!("\nconstraints:");
    for x in &l.constraints {
        println!("  - {x}");
    }
    println!(
        "\nunlocked headers: {}",
        if l.unlocked.is_empty() {
            "(none beyond the prelude)".into()
        } else {
            l.unlocked.join(", ")
        }
    );
    println!(
        "helpers: {}  recursion: {}  structs: {}",
        l.helpers, l.recursion, l.structs
    );
    for k in &l.locked {
        println!("locked: {} ({})", k.name, k.why);
    }
    println!("technique: {}   {}", l.technique, l.big_o);
    println!(
        "medal targets: size {:?}  speed {:?}",
        l.medals.size, l.medals.speed
    );
    Ok(0)
}

fn starter(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let l = find_level(&c, args.positional.first().ok_or("usage: starter LEVEL")?)?;
    print!("{}", l.starter_code());
    Ok(0)
}

fn reference(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let l = find_level(&c, args.positional.first().ok_or("usage: reference LEVEL")?)?;
    print!("{}", l.reference.trim_start());
    Ok(0)
}

fn print_diags(file: &str, text: &str, ds: &[algoclerk_core::cpp::Diag]) {
    for d in ds {
        let loc = d
            .span
            .map(|s| {
                let (l, c) = line_col(text, s.start);
                format!("{file}:{l}:{c}: ")
            })
            .unwrap_or_default();
        eprintln!("{loc}{}", d.msg);
    }
}

fn check(args: &Args) -> Result<u8, String> {
    let file = args
        .positional
        .first()
        .ok_or("usage: check FILE [--level LEVEL]")?;
    let text = read(file)?;
    let (prelude, opts) = match args.opt("level") {
        Some(id) => {
            let c = campaign(args);
            let l = find_level(&c, id)?;
            (l.prelude.clone(), l.check_opts())
        }
        None => (String::new(), CheckOpts::all()),
    };
    match compile(&prelude, &text, &opts) {
        Ok(c) => {
            println!(
                "ok: {} functions, size {}",
                c.program
                    .player_items()
                    .iter()
                    .filter(|i| matches!(i.kind, algoclerk_core::cpp::ast::ItemKind::Func(_)))
                    .count(),
                c.size()
            );
            Ok(0)
        }
        Err(ds) => {
            print_diags(file, &text, &ds);
            Ok(1)
        }
    }
}

fn run(args: &Args) -> Result<u8, String> {
    let [file, func, rest @ ..] = args.positional.as_slice() else {
        return Err("usage: run FILE FUNCTION [ARG...]".into());
    };
    let text = read(file)?;
    let c = match compile("", &text, &CheckOpts::all()) {
        Ok(c) => c,
        Err(ds) => {
            print_diags(file, &text, &ds);
            return Ok(1);
        }
    };
    let limits = Limits {
        steps: args
            .opt("steps")
            .and_then(|s| s.parse().ok())
            .unwrap_or(50_000_000),
        ..Limits::default()
    };
    let r = c.call(func, rest, &limits)?;
    match r.report.result {
        Ok(_) => {
            println!("{}", r.output.unwrap_or_default());
            eprintln!("cost: {}", r.report.cost);
            Ok(0)
        }
        Err(e) => {
            eprintln!("{} ({:?})", e.msg, e.kind);
            Ok(1)
        }
    }
}

fn judge(args: &Args) -> Result<u8, String> {
    let [id, file] = args.positional.as_slice() else {
        return Err("usage: judge LEVEL FILE".into());
    };
    let c = campaign(args);
    let level = find_level(&c, id)?;
    let text = read(file)?;
    let j = Judge::new(level)?;
    let v = j.judge(&text);
    if args.flag("json") {
        let mut o = serde_json::json!({
            "passed": v.passed, "size": v.size, "avg_cost": v.avg_cost, "max_cost": v.max_cost,
            "tests_passed": v.tests_passed, "tests_total": v.tests_total,
            "size_medal": v.medals.size.name(), "speed_medal": v.medals.speed.name(),
        });
        if let Some(cx) = &v.complexity {
            o["time"] = cx.time.to_string().into();
            o["space"] = cx.space.to_string().into();
        }
        if let Some(d) = v.diags.first() {
            o["error"] = d.msg.clone().into();
        }
        if let Some(f) = &v.failure {
            o["failure"] = format!("{:?} on ({})", f.kind, f.case.args.join(", ")).into();
        }
        println!("{o}");
    } else if v.passed {
        println!(
            "ACCEPTED  size {} ({})  speed {:.1} ({})",
            v.size,
            v.medals.size.name(),
            v.avg_cost,
            v.medals.speed.name()
        );
        if let Some(cx) = &v.complexity {
            println!("measured: time {}, space {}", cx.time, cx.space);
        }
    } else if let Some(d) = v.diags.first() {
        println!("DOES NOT COMPILE: {}", d.msg);
    } else if let Some(f) = &v.failure {
        let args = f.case.args.join(", ");
        match f.kind {
            FailKind::Wrong => println!(
                "WRONG ANSWER on ({args}): expected {}, got {}",
                f.case.expect,
                f.got.clone().unwrap_or_default()
            ),
            FailKind::UnorderedOrder => println!("{}", f.kind.friendly()),
            FailKind::TimeLimit => println!(
                "TIME LIMIT EXCEEDED on {} test ({}): {}",
                if f.case.large { "a large" } else { "a" },
                args.chars().take(60).collect::<String>(),
                f.error.as_ref().map_or("", |e| e.msg.as_str())
            ),
            _ => println!(
                "ERROR on ({args}): {}",
                f.error.as_ref().map_or("", |e| e.msg.as_str())
            ),
        }
    }
    Ok(u8::from(!v.passed))
}

fn validate(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let ids: Vec<&Level> = if args.positional.is_empty() {
        c.all_levels().collect()
    } else {
        args.positional
            .iter()
            .map(|i| find_level(&c, i))
            .collect::<Result<_, _>>()?
    };
    let mut bad = 0;
    for l in ids {
        let problems = validate_level(l);
        if problems.is_empty() {
            println!("ok    {}", l.id);
        } else {
            bad += 1;
            for p in problems {
                println!("FAIL  {p}");
            }
        }
    }
    Ok(u8::from(bad > 0))
}

fn table(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    println!("| Wing | Level | Size | Avg cost | Time | Space | Naive |");
    println!("| --- | --- | ---: | ---: | --- | --- | --- |");
    for p in &c.packs {
        for l in &p.levels {
            let j = Judge::new(l)?;
            let v = j.reference_verdict();
            let (t, s) = v.complexity.as_ref().map_or(("-".into(), "-".into()), |x| {
                (x.time.to_string(), x.space.to_string())
            });
            let naive = match &l.naive {
                None => "-".to_string(),
                Some(n) => compile(&l.prelude, n, &l.check_opts())
                    .ok()
                    .map_or("?".into(), |sol| {
                        let nv = j.judge_compiled(&sol, false);
                        match nv.failure {
                            Some(f) if f.kind == FailKind::TimeLimit => "TLE on large".into(),
                            Some(_) => "wrong".into(),
                            None => "passes".into(),
                        }
                    }),
            };
            println!(
                "| {} | {} | {} | {:.0} | {} | {} | {} |",
                p.info.title, l.title, v.size, v.avg_cost, t, s, naive
            );
        }
    }
    Ok(0)
}

fn tune(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let levels: Vec<&Level> = if args.flag("all") {
        c.all_levels().collect()
    } else {
        vec![find_level(
            &c,
            args.positional.first().ok_or("usage: tune LEVEL|--all")?,
        )?]
    };
    let root = match args.opt("levels") {
        Some(dir) => Path::new(dir).to_path_buf(),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels"),
    };
    for l in levels {
        let j = match Judge::new(l) {
            Ok(j) => j,
            Err(e) => {
                println!(
                    "{}: cannot build the test suite: {}",
                    l.id,
                    e.chars().take(300).collect::<String>()
                );
                continue;
            }
        };
        let v = j.reference_verdict();
        if !v.passed {
            println!("{}: reference does not pass", l.id);
            continue;
        }
        let size = v.size;
        let avg = v.avg_cost;
        let sz = [
            (f64::from(size) * 1.6).ceil() as u32,
            (f64::from(size) * 1.25).ceil() as u32,
            size,
        ];
        let sp = [
            (avg * 3.0).ceil() as u64,
            (avg * 1.6).ceil() as u64,
            (avg * 1.1).ceil() as u64 + 1,
        ];
        let line = format!("size {sz:?}  speed {sp:?}");
        if args.flag("write") {
            let path = root.join(&l.pack).join(format!("{}.toml", l.id));
            let text = read(&path.to_string_lossy())?;
            let mut out = String::new();
            for ln in text.lines() {
                if ln.starts_with("size = [")
                    && text[..text.find(ln).unwrap_or(0)].contains("[medals]")
                {
                    let _ = writeln!(out, "size = [{}, {}, {}]", sz[0], sz[1], sz[2]);
                } else if ln.starts_with("speed = [")
                    && text[..text.find(ln).unwrap_or(0)].contains("[medals]")
                {
                    let _ = writeln!(out, "speed = [{}, {}, {}]", sp[0], sp[1], sp[2]);
                } else {
                    let _ = writeln!(out, "{ln}");
                }
            }
            std::fs::write(&path, out).map_err(|e| e.to_string())?;
            println!("{}: wrote {line}", l.id);
        } else {
            println!("{}: {line}", l.id);
        }
    }
    let _ = Medal::Gold;
    Ok(0)
}

/// `harness LEVEL [FILE]`: print the complete C++ test program (prelude, solution, generated main).
fn harness(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let l = find_level(
        &c,
        args.positional
            .first()
            .ok_or("usage: harness LEVEL [FILE]")?,
    )?;
    let text = match args.positional.get(1) {
        Some(f) => read(f)?,
        None => l.reference.clone(),
    };
    let cases: Vec<Vec<String>> = algoclerk_core::judge::raw_suite(l, Some(50))?
        .into_iter()
        .filter(|t| !t.large)
        .map(|t| t.args)
        .collect();
    print!("{}", translation_unit(l, &text, &cases)?);
    Ok(0)
}

/// `diff [LEVEL ...]`: reference and naive solutions with every host compiler, compared with the data.
fn diff(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let levels: Vec<&Level> = if args.positional.is_empty() {
        c.all_levels().collect()
    } else {
        args.positional
            .iter()
            .map(|i| find_level(&c, i))
            .collect::<Result<_, _>>()?
    };
    let compilers = available_compilers();
    if compilers.is_empty() {
        println!("no C++ compiler installed; nothing to compare");
        return Ok(0);
    }
    let dir = std::env::temp_dir().join(format!("algoclerk-diff-{}", std::process::id()));
    let mut bad = 0;
    for l in levels {
        let mut o = DiffOpts::new(dir.clone());
        o.compilers = compilers.clone();
        o.sanitize = !args.flag("no-sanitize");
        if args.flag("full") {
            o.large_cap = None;
        } else if let Some(n) = args.opt("cap").and_then(|n| n.parse().ok()) {
            o.large_cap = Some(n);
        }
        let rep = diff_level(l, &o);
        if rep.problems.is_empty() {
            println!(
                "ok    {} ({} cases{})",
                l.id,
                rep.cases.len(),
                if rep.notes.is_empty() {
                    String::new()
                } else {
                    format!("; {}", rep.notes.join("; "))
                }
            );
        } else {
            bad += 1;
            for p in rep.problems {
                println!("FAIL  {}: {p}", l.id);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(u8::from(bad > 0))
}
