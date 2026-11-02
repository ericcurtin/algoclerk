// SPDX-License-Identifier: Apache-2.0
//! Compile and run generated code with the host's C, C++ and Rust compilers.
//!
//! Every external process runs under a hard time limit (`timeout -s KILL` where available, an
//! internal kill deadline otherwise). Used by the test-suite and by `algoclerk-cli codegen --verify`.

use super::Lang;
use super::harness::{self, HarnessCase, Item};
use super::{Generated, Options};
use crate::asm::Program;
use crate::level::Level;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A compiler found on this machine.
#[derive(Clone, Debug)]
pub struct Compiler {
    /// Language it compiles.
    pub lang: Lang,
    /// Executable name.
    pub exe: String,
}

fn works(exe: &str, arg: &str) -> bool {
    Command::new(exe)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// All compilers for `lang` that exist on this machine (gcc and clang for C, g++ and clang++ for
/// C++, rustc for Rust).
pub fn available(lang: Lang) -> Vec<Compiler> {
    let candidates: &[&str] = match lang {
        Lang::C => &["gcc", "clang"],
        Lang::Cpp => &["g++", "clang++"],
        Lang::Rust => &["rustc"],
    };
    candidates
        .iter()
        .filter(|e| works(e, "--version"))
        .map(|e| Compiler {
            lang,
            exe: (*e).to_string(),
        })
        .collect()
}

fn have_timeout_cmd() -> bool {
    cfg!(unix) && works("timeout", "--version")
}

/// Run a command under a kill deadline. Returns `(success, stdout, stderr)`.
pub fn run_limited(mut cmd: Command, secs: u64) -> Result<(bool, String, String), String> {
    let mut cmd = if have_timeout_cmd() {
        let mut t = Command::new("timeout");
        t.arg("-s")
            .arg("KILL")
            .arg(secs.to_string())
            .arg(cmd.get_program());
        t.args(cmd.get_args());
        t
    } else {
        std::mem::replace(&mut cmd, Command::new("true"))
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot start {:?}: {e}", cmd.get_program()))?;
    let mut out = child.stdout.take().ok_or("no stdout")?;
    let mut err = child.stderr.take().ok_or("no stderr")?;
    let t_out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let t_err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });
    let deadline = Instant::now() + Duration::from_secs(secs + 5);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("timed out".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    Ok((
        status.success(),
        t_out.join().unwrap_or_default(),
        t_err.join().unwrap_or_default(),
    ))
}

/// Compile `source` and run the resulting program; returns its stdout.
/// With `strict`, warnings are errors (used for the shipped reference solutions).
pub fn build_and_run(
    c: &Compiler,
    source: &str,
    dir: &Path,
    stem: &str,
    strict: bool,
) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let src: PathBuf = dir.join(format!("{stem}.{}", c.lang.extension()));
    let exe: PathBuf = dir
        .join(format!("{stem}.bin"))
        .with_extension(std::env::consts::EXE_EXTENSION);
    std::fs::write(&src, source).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&c.exe);
    match c.lang {
        Lang::C => {
            cmd.args(["-std=c99", "-O1", "-Wall", "-Wextra", "-pedantic"]);
            if strict {
                cmd.arg("-Werror");
            } else {
                cmd.arg("-w");
            }
            cmd.arg("-o").arg(&exe).arg(&src);
        }
        Lang::Cpp => {
            cmd.args(["-std=c++17", "-O1", "-Wall", "-Wextra", "-pedantic"]);
            if strict {
                cmd.arg("-Werror");
            } else {
                cmd.arg("-w");
            }
            cmd.arg("-o").arg(&exe).arg(&src);
        }
        Lang::Rust => {
            cmd.args([
                "--edition",
                "2021",
                "-C",
                "opt-level=1",
                "--crate-type",
                "bin",
            ]);
            if strict {
                cmd.args(["-D", "warnings"]);
            } else {
                cmd.args(["-A", "warnings"]);
            }
            cmd.arg("-o").arg(&exe).arg(&src);
        }
    }
    let (ok, _, stderr) = run_limited(cmd, 120)?;
    if !ok {
        return Err(format!(
            "{} failed to compile {}:\n{}",
            c.exe,
            src.display(),
            stderr.lines().take(30).collect::<Vec<_>>().join("\n")
        ));
    }
    let (ok, stdout, stderr) = run_limited(Command::new(&exe), 30)?;
    let _ = std::fs::remove_file(&exe);
    if !ok {
        return Err(format!(
            "{} crashed:\n{}",
            exe.display(),
            stderr.lines().take(10).collect::<Vec<_>>().join("\n")
        ));
    }
    Ok(stdout)
}

/// One program to verify: generated code plus inputs and the interpreter's expected outputs.
#[derive(Debug)]
pub struct Case<'a> {
    /// The generated function.
    pub generated: Generated,
    /// Level (for metadata-shaped functions).
    pub level: Option<&'a Level>,
    /// Inputs.
    pub cases: Vec<HarnessCase>,
    /// Expected outputs, one per input (what the interpreter produced).
    pub expected: Vec<Vec<i32>>,
}

/// Generate code for `program`, ready to be verified.
pub fn prepare<'a>(
    program: &Arc<Program>,
    lang: Lang,
    level: Option<&'a Level>,
    opts: &Options,
    cases: Vec<HarnessCase>,
    expected: Vec<Vec<i32>>,
) -> Case<'a> {
    Case {
        generated: super::generate(program, lang, level, opts),
        level,
        cases,
        expected,
    }
}

/// Compile one source file containing every case's function and compare all outputs.
pub fn verify(
    c: &Compiler,
    items: &[Case<'_>],
    dir: &Path,
    stem: &str,
    strict: bool,
) -> Result<(), String> {
    let harness_items: Vec<Item<'_>> = items
        .iter()
        .map(|i| Item {
            generated: &i.generated,
            level: i.level,
            cases: i.cases.clone(),
        })
        .collect();
    let source = harness::build(c.lang, &harness_items)?;
    let stdout = build_and_run(c, &source, dir, stem, strict)?;
    let got = harness::parse_output(&stdout)?;
    let mut k = 0;
    for (n, item) in items.iter().enumerate() {
        for (ci, want) in item.expected.iter().enumerate() {
            let have = got
                .get(k)
                .ok_or_else(|| format!("program {n} case {ci}: no output line"))?;
            if have != want {
                return Err(format!(
                    "{} {}: program {n} case {ci} ({}): generated code printed {have:?}, the interpreter {want:?}; inbox {:?}\n--- code ---\n{}",
                    c.exe,
                    c.lang.title(),
                    item.generated.function,
                    item.cases[ci].inbox,
                    item.generated.code
                ));
            }
            k += 1;
        }
    }
    if k != got.len() {
        return Err("the generated program printed extra lines".into());
    }
    Ok(())
}
