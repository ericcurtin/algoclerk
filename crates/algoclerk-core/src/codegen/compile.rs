// SPDX-License-Identifier: Apache-2.0
//! Drive the host C, C++ and Rust compilers (used by tests and `algoclerk-cli codegen --verify`).

use super::Lang;
use std::path::Path;
use std::process::{Command, Stdio};

/// A compiler found on this machine.
#[derive(Clone, Debug)]
pub struct Compiler {
    /// Executable name, such as `g++`.
    pub exe: &'static str,
    /// Language it compiles.
    pub lang: Lang,
    /// Arguments before the source file.
    pub args: &'static [&'static str],
}

const ALL: &[Compiler] = &[
    Compiler {
        exe: "gcc",
        lang: Lang::C,
        args: &[
            "-std=c11",
            "-O1",
            "-Wall",
            "-Wextra",
            "-Wno-unused-function",
            "-Wno-unused-parameter",
        ],
    },
    Compiler {
        exe: "clang",
        lang: Lang::C,
        args: &[
            "-std=c11",
            "-O1",
            "-Wall",
            "-Wextra",
            "-Wno-unused-function",
            "-Wno-unused-parameter",
        ],
    },
    Compiler {
        exe: "g++",
        lang: Lang::Cpp,
        args: &[
            "-std=c++17",
            "-O1",
            "-Wall",
            "-Wextra",
            "-Wno-unused-parameter",
        ],
    },
    Compiler {
        exe: "clang++",
        lang: Lang::Cpp,
        args: &[
            "-std=c++17",
            "-O1",
            "-Wall",
            "-Wextra",
            "-Wno-unused-parameter",
        ],
    },
    Compiler {
        exe: "rustc",
        lang: Lang::Rust,
        args: &[
            "--edition",
            "2021",
            "-C",
            "opt-level=1",
            "-A",
            "dead_code",
            "-A",
            "unused",
        ],
    },
];

fn exists(exe: &str) -> bool {
    Command::new(exe)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Compilers for `lang` that are installed (missing ones are skipped, never an error).
pub fn available(lang: Lang) -> Vec<Compiler> {
    ALL.iter()
        .filter(|c| c.lang == lang && exists(c.exe))
        .cloned()
        .collect()
}

/// Is `timeout` available to bound runs?
fn bounded(secs: u32, program: &str) -> Command {
    let mut c = Command::new("timeout");
    c.args(["-s", "KILL", &secs.to_string(), program]);
    c
}

/// Compile `source` (written into `dir/<tag>.<ext>`), run it and return stdout. Errors carry compiler output.
pub fn compile_and_run(
    comp: &Compiler,
    source: &str,
    dir: &Path,
    tag: &str,
    warnings_are_errors: bool,
) -> Result<String, String> {
    let ext = match comp.lang {
        Lang::C => "c",
        Lang::Cpp => "cpp",
        Lang::Rust => "rs",
    };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let src = dir.join(format!("{tag}.{ext}"));
    let exe = dir.join(format!("{tag}-{}", comp.exe.replace('+', "p")));
    std::fs::write(&src, source).map_err(|e| e.to_string())?;
    let mut cmd = bounded(120, comp.exe);
    cmd.args(comp.args);
    if warnings_are_errors && comp.lang != Lang::Rust {
        cmd.arg("-Werror");
    }
    cmd.arg("-o").arg(&exe).arg(&src);
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {}: {e}", comp.exe))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        let head: String = msg.lines().take(30).collect::<Vec<_>>().join("\n");
        return Err(format!("{} failed on {}:\n{head}", comp.exe, src.display()));
    }
    let run = bounded(30, exe.to_str().unwrap_or_default())
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&exe);
    if !run.status.success() {
        return Err(format!(
            "{} binary failed ({:?}): {}",
            comp.exe,
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
                .lines()
                .take(5)
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).into_owned())
}
