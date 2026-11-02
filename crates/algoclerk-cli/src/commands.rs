// SPDX-License-Identifier: Apache-2.0
//! Subcommand implementations.

use crate::args::Args;
use algoclerk_core::asm::{self, Listing, Program};
use algoclerk_core::codegen::{self, Lang, Options, Stats};
use algoclerk_core::i18n::english;
use algoclerk_core::isa::Allowed;
use algoclerk_core::judge::{Judge, Medal, Outcome, Verdict};
use algoclerk_core::level::Level;
use algoclerk_core::share;
use algoclerk_core::validate::validate_level;
use algoclerk_core::vm::{Machine, Status};
use algoclerk_levels::Campaign;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

const USAGE: &str = "algoclerk-cli: headless tools for AlgoClerk

USAGE:
    algoclerk-cli <command> [options]

COMMANDS:
    list                         List packs and levels
    show <level>                 Print a level's story, statement and rules
    run                          Run a program on one input (add --level for its floor and rules)
    judge                        Judge a program against a level's tests
    validate [path...]           Validate level files, pack directories or the shipped levels
    tune                         Suggest [targets] from a level's reference solution (or --all)
    codegen                      Translate a program to C, C++ or Rust
    export                       Produce shareable solution text
    import <file>                Read shared solution text and print the canonical program
    help                         Show this help

COMMON OPTIONS:
    --levels <dir>               Also load level packs from this directory
    --json                       Machine readable output
    --solution <file|->          Program text (assembly), '-' for stdin
    --reference                  Use the level's reference solution instead of --solution
    --level <id>                 Level id (the pack prefix is optional: basics/echo or echo)

EXAMPLES:
    algoclerk-cli judge --level echo --solution my.asm
    algoclerk-cli judge --all --reference
    algoclerk-cli run --solution my.asm --input 3,4,5 --trace
    algoclerk-cli codegen --lang rust --level max-of --reference
    algoclerk-cli validate levels/
";

const VALUE_OPTS: &[&str] = &[
    "levels", "solution", "level", "input", "text", "floor", "limit", "lang", "function", "pack",
];

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
        "run" => run(&args),
        "judge" => judge(&args),
        "validate" => validate(&args),
        "tune" => tune(&args),
        "codegen" => codegen_cmd(&args),
        "export" => export(&args),
        "import" => import(&args),
        other => Err(format!(
            "unknown command \"{other}\" (try `algoclerk-cli help`)"
        )),
    }
}

fn campaign(args: &Args) -> Campaign {
    let mut c = Campaign::shipped();
    if let Some(dir) = args.opt("levels") {
        c.load_dir(Path::new(dir));
    }
    for e in &c.load_errors {
        eprintln!("warning: {e}");
    }
    c
}

fn find_level<'a>(c: &'a Campaign, id: &str) -> Result<&'a Level, String> {
    let bare = id.rsplit('/').next().unwrap_or(id);
    c.find(bare)
        .ok_or_else(|| format!("no level called \"{id}\" (see `algoclerk-cli list`)"))
}

fn read_source(args: &Args) -> Result<String, String> {
    let path = args
        .opt("solution")
        .ok_or("give --solution <file> (or --reference)")?;
    if path == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        Ok(s)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
    }
}

fn program_for(args: &Args, level: Option<&Level>) -> Result<(Arc<Program>, Listing), String> {
    let text = if args.flag("reference") {
        let l = level.ok_or("--reference needs --level")?;
        l.solution
            .as_ref()
            .ok_or("this level has no reference solution")?
            .asm
            .clone()
    } else {
        read_source(args)?
    };
    // Accept shared text with headers.
    let imported = share::import(&text).map_err(|e| {
        e.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let program = imported.listing.compile().map_err(|e| {
        e.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok((Arc::new(program), imported.listing))
}

fn medal_name(m: Option<Medal>) -> &'static str {
    match m {
        None => "-",
        Some(Medal::Clip) => "clip",
        Some(Medal::Staple) => "staple",
        Some(Medal::Seal) => "seal",
    }
}

fn list(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    if args.flag("json") {
        let packs: Vec<Value> = c
            .packs
            .iter()
            .map(|p| {
                json!({
                    "id": p.info.id, "title": p.info.title, "blurb": p.info.blurb,
                    "levels": p.levels.iter().map(|l| json!({"id": l.id, "title": l.title, "summary": l.summary})).collect::<Vec<_>>()
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"packs": packs, "level_count": c.level_count()}))
                .unwrap_or_default()
        );
        return Ok(0);
    }
    for p in &c.packs {
        println!("{} ({}) - {}", p.info.title, p.info.id, p.info.blurb);
        for l in &p.levels {
            println!("  {:<24} {:<34} {}", l.id, l.title, l.summary);
        }
    }
    println!("\n{} levels in {} packs", c.level_count(), c.packs.len());
    Ok(0)
}

fn show(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let id = args
        .positional
        .first()
        .map(String::as_str)
        .or(args.opt("level"))
        .ok_or("show needs a level id")?;
    let l = find_level(&c, id)?;
    println!("{}\n{}\n", l.title, "=".repeat(l.title.chars().count()));
    println!("{}\n", l.story);
    println!("{}\n", l.statement);
    if !l.constraints.is_empty() {
        println!("Constraints:");
        for k in &l.constraints {
            println!("  - {k}");
        }
        println!();
    }
    for (i, e) in l.examples.iter().enumerate() {
        println!(
            "Example {}: inbox {:?} -> outbox {:?} {}",
            i + 1,
            e.input.to_vec(),
            e.output.to_vec(),
            e.note
        );
    }
    let allowed = l.allowed().map_err(|e| e.to_string())?;
    let ops: Vec<&str> = allowed.opcodes().iter().map(|o| o.mnemonic()).collect();
    println!(
        "\nInstructions: {}{}",
        ops.join(" "),
        if allowed.allows_indirect() {
            " (+ [n] pointers)"
        } else {
            ""
        }
    );
    println!("Floor tiles: {}", l.machine.floor);
    println!(
        "Targets: size {}/{} (par/ace), speed {}/{}",
        l.targets.size_par, l.targets.size_ace, l.targets.speed_par, l.targets.speed_ace
    );
    Ok(0)
}

fn parse_ints(s: &str) -> Result<Vec<i32>, String> {
    s.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            t.parse::<i32>()
                .map_err(|_| format!("\"{t}\" is not an integer"))
        })
        .collect()
}

fn run(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let level = args.opt("level").map(|id| find_level(&c, id)).transpose()?;
    let (program, _) = program_for(args, level)?;
    let inbox: Vec<i32> = match (args.opt("input"), args.opt("text")) {
        (Some(s), _) => parse_ints(s)?,
        (None, Some(t)) => t.chars().map(|c| c as i32).collect(),
        _ => Vec::new(),
    };
    let floor: Vec<Option<i32>> = match level {
        Some(l) => l.floor_template().map_err(|e| e.to_string())?,
        None => {
            let n: usize = args
                .opt("floor")
                .map(str::parse)
                .transpose()
                .map_err(|_| "--floor needs a number")?
                .unwrap_or(16);
            vec![None; n]
        }
    };
    if let Some(l) = level {
        let allowed = l.allowed().map_err(|e| e.to_string())?;
        let problems = program.check(&allowed, l.machine.floor);
        if !problems.is_empty() {
            for p in problems {
                eprintln!("{p}");
            }
            return Ok(1);
        }
    } else {
        let problems = program.check(&Allowed::everything(), floor.len());
        if let Some(p) = problems.first() {
            return Err(p.to_string());
        }
    }
    let limit: u64 = args
        .opt("limit")
        .map(str::parse)
        .transpose()
        .map_err(|_| "--limit needs a number")?
        .unwrap_or(100_000);
    let mut m = Machine::new(program.clone(), inbox.clone(), floor, limit);
    let trace = args.flag("trace");
    while !m.is_done() {
        let pc = m.pc;
        let rep = m.step();
        if trace {
            println!(
                "{:>5}  {:<18} hand={:<8} outbox={:?}",
                m.steps,
                program.instrs[pc]
                    .map_target(|t| program.labels[*t]
                        .first()
                        .cloned()
                        .unwrap_or_else(|| t.to_string()))
                    .to_string(),
                m.hand.map_or("-".to_string(), |h| h.to_string()),
                m.outbox
            );
        }
        let _ = rep;
    }
    let (ok, msg) = match &m.status {
        Status::Error(e) => (false, e.to_string()),
        _ => (true, "finished".to_string()),
    };
    if args.flag("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"inbox": inbox, "outbox": m.outbox, "steps": m.steps, "ok": ok, "message": msg})).unwrap_or_default()
        );
    } else {
        println!("outbox: {:?}\nsteps: {}\nstatus: {msg}", m.outbox, m.steps);
    }
    Ok(u8::from(!ok))
}

fn verdict_json(l: &Level, v: &Verdict) -> Value {
    let failure = v.first_failure().map(|t| {
        json!({
            "test": t.index + 1, "inbox": t.inbox, "expected": t.expected, "steps": t.steps,
            "outcome": match &t.outcome {
                Outcome::Pass => json!("pass"),
                Outcome::Wrong { got } => json!({"wrong": got}),
                Outcome::Error { error, got } => json!({"error": error.to_string(), "outbox": got}),
            }
        })
    });
    json!({
        "level": l.id, "passed": v.passed, "tests": v.tests.len(), "tests_passed": v.passed_count(),
        "size": v.size, "speed_avg": v.avg_steps, "speed_worst": v.worst_steps,
        "medal_size": medal_name(v.medals.size), "medal_speed": medal_name(v.medals.speed), "stars": v.medals.stars(),
        "problems": v.problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "first_failure": failure, "headline": v.headline(english()),
    })
}

fn judge(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    if args.flag("all") {
        return judge_all(&c, args);
    }
    let id = args
        .opt("level")
        .ok_or("judge needs --level <id> (or --all --reference)")?;
    let l = find_level(&c, id)?;
    let (program, _) = program_for(args, Some(l))?;
    let judge = Judge::new(l).map_err(|e| e.to_string())?;
    let v = judge.judge(&program);
    if args.flag("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&verdict_json(l, &v)).unwrap_or_default()
        );
    } else {
        println!("{}: {}", l.title, v.headline(english()));
        println!(
            "tests {}/{}  size {} (par {}, ace {})  speed avg {:.1} worst {} (par {}, ace {})",
            v.passed_count(),
            v.tests.len(),
            v.size,
            l.targets.size_par,
            l.targets.size_ace,
            v.avg_steps,
            v.worst_steps,
            l.targets.speed_par,
            l.targets.speed_ace
        );
        println!(
            "medals: size {}, speed {}  stars {}",
            medal_name(v.medals.size),
            medal_name(v.medals.speed),
            v.medals.stars()
        );
    }
    Ok(u8::from(!v.passed))
}

fn judge_all(c: &Campaign, args: &Args) -> Result<u8, String> {
    if !args.flag("reference") {
        return Err("--all only works together with --reference".into());
    }
    let mut rows = Vec::new();
    let mut failed = 0;
    for l in c.all_levels() {
        let judge = Judge::new(l).map_err(|e| e.to_string())?;
        let prog = l.reference_program().map_err(|e| e.to_string())?;
        let v = judge.judge(&prog);
        if !v.passed {
            failed += 1;
        }
        rows.push((l, v));
    }
    if args.flag("json") {
        let vals: Vec<Value> = rows.iter().map(|(l, v)| verdict_json(l, v)).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"results": vals, "failed": failed}))
                .unwrap_or_default()
        );
    } else {
        println!(
            "{:<4} {:<12} {:<26} {:>5} {:>9} {:>8} {:>9} {:>8} {:>7}",
            "#", "pack", "level", "tests", "size", "size t", "avg", "worst", "medals"
        );
        for (i, (l, v)) in rows.iter().enumerate() {
            println!(
                "{:<4} {:<12} {:<26} {:>2}/{:<2} {:>9} {:>8} {:>9.1} {:>8} {:>3}/{:<3}",
                i + 1,
                l.pack,
                l.id,
                v.passed_count(),
                v.tests.len(),
                v.size,
                format!("{}/{}", l.targets.size_par, l.targets.size_ace),
                v.avg_steps,
                v.worst_steps,
                medal_name(v.medals.size).chars().next().unwrap_or('-'),
                medal_name(v.medals.speed).chars().next().unwrap_or('-'),
            );
        }
        println!("\n{} levels judged, {} failed", rows.len(), failed);
    }
    Ok(u8::from(failed > 0))
}

fn collect_level_files(path: &Path, out: &mut Vec<(String, String)>) -> Result<(), String> {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        entries.sort();
        for e in entries {
            collect_level_files(&e, out)?;
        }
    } else if path.extension().is_some_and(|e| e == "toml") {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let pack = path
            .parent()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let file = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push((format!("{pack}/{file}"), text));
    }
    Ok(())
}

fn validate(args: &Args) -> Result<u8, String> {
    let mut files: Vec<(String, String)> = Vec::new();
    if args.positional.is_empty() {
        files = algoclerk_levels::embedded_files();
    } else {
        for p in &args.positional {
            collect_level_files(Path::new(p), &mut files)?;
        }
    }
    let campaign = Campaign::from_files(&files);
    let mut results = Vec::new();
    let mut bad = campaign.load_errors.len();
    for e in &campaign.load_errors {
        results.push(json!({"level": null, "errors": [e], "warnings": []}));
        if !args.flag("json") {
            println!("FAIL  {e}");
        }
    }
    for l in campaign.all_levels() {
        let r = validate_level(l);
        if !r.ok() {
            bad += 1;
        }
        if !args.flag("json") {
            println!(
                "{}  {}/{}  ({} tests)",
                if r.ok() { "ok  " } else { "FAIL" },
                l.pack,
                l.id,
                r.test_count
            );
            for e in &r.errors {
                println!("      error: {e}");
            }
            for w in &r.warnings {
                println!("      warning: {w}");
            }
        }
        results.push(json!({"level": l.id, "pack": l.pack, "ok": r.ok(), "tests": r.test_count, "errors": r.errors, "warnings": r.warnings}));
    }
    if args.flag("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"results": results, "failed": bad}))
                .unwrap_or_default()
        );
    } else {
        println!(
            "\n{} levels checked, {} with problems",
            campaign.level_count(),
            bad
        );
    }
    Ok(u8::from(bad > 0))
}

fn codegen_cmd(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let level = args.opt("level").map(|id| find_level(&c, id)).transpose()?;
    let lang = Lang::from_key(args.opt("lang").unwrap_or("rust"))
        .ok_or("--lang must be c, cpp or rust")?;
    let (program, _) = program_for(args, level)?;
    let mut opts = Options {
        force_generic: args.flag("generic"),
        function_name: args.opt("function").map(ToString::to_string),
        floor_len: args.opt("floor").and_then(|s| s.parse().ok()),
        ..Options::default()
    };
    if let Some(l) = level {
        let judge = Judge::new(l).map_err(|e| e.to_string())?;
        let v = judge.judge(&program);
        if !v.passed && !args.flag("force") {
            return Err(format!(
                "the program does not solve \"{}\" yet ({}). Code is a reward for solving; use --force to translate anyway.",
                l.title,
                v.headline(english())
            ));
        }
        if v.passed {
            opts.stats = Some(Stats {
                size: v.size,
                avg_steps: v.avg_steps,
                worst_steps: v.worst_steps,
            });
        }
    }
    let g = codegen::generate(&program, lang, level, &opts);
    if args.flag("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"lang": lang.key(), "function": g.function, "structured": g.structured, "code": g.code})).unwrap_or_default()
        );
    } else {
        print!("{}", g.code);
    }
    Ok(0)
}

fn export(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let id = args.opt("level").ok_or("export needs --level")?;
    let l = find_level(&c, id)?;
    let (program, listing) = program_for(args, Some(l))?;
    let judge = Judge::new(l).map_err(|e| e.to_string())?;
    let v = judge.judge(&program);
    let stats = v.passed.then_some(share::ShareStats {
        size: v.size,
        avg_steps: v.avg_steps,
        worst_steps: v.worst_steps,
    });
    print!(
        "{}",
        share::export(&format!("{}/{}", l.pack, l.id), &listing, stats)
    );
    Ok(0)
}

fn import(args: &Args) -> Result<u8, String> {
    let path = args.positional.first().ok_or("import needs a file")?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let imp = share::import(&text).map_err(|e| {
        e.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    imp.listing.compile().map_err(|e| {
        e.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    if let Some(l) = &imp.level {
        eprintln!("level: {l}");
    }
    print!("{}", imp.listing.to_text());
    let _ = asm::parse;
    Ok(0)
}

fn tune(args: &Args) -> Result<u8, String> {
    let c = campaign(args);
    let levels: Vec<&Level> = if args.flag("all") {
        c.all_levels().collect()
    } else {
        let id = args
            .opt("level")
            .ok_or("tune needs --level <id> or --all")?;
        vec![find_level(&c, id)?]
    };
    let mut out = Vec::new();
    for l in levels {
        let judge = Judge::new(l).map_err(|e| e.to_string())?;
        let v = judge.judge(&l.reference_program().map_err(|e| e.to_string())?);
        if !v.passed {
            return Err(format!("the reference solution of {} does not pass", l.id));
        }
        let t = algoclerk_core::judge::suggest_targets(v.size, v.avg_steps);
        if args.flag("json") {
            out.push(json!({"level": l.id, "size_par": t.size_par, "size_ace": t.size_ace, "speed_par": t.speed_par, "speed_ace": t.speed_ace}));
        } else {
            println!(
                "# {}\n[targets]\nsize_par = {}\nsize_ace = {}\nspeed_par = {}\nspeed_ace = {}\n",
                l.id, t.size_par, t.size_ace, t.speed_par, t.speed_ace
            );
        }
    }
    if args.flag("json") {
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    }
    Ok(0)
}
