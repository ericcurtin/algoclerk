// SPDX-License-Identifier: Apache-2.0
//! `algoclerk-cli`: run, judge, validate and translate clerk programs without a GUI.

mod args;
mod commands;

use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match commands::dispatch(&argv) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}
