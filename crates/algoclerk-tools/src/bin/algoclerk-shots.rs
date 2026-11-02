// SPDX-License-Identifier: Apache-2.0
//! Headless screenshots: renders the real game (no window, no GPU) and writes upscaled PNGs.
//!
//! `algoclerk-shots OUT_DIR [--scale N] [--only NAME[,NAME..]] [--portrait] [--palette warm|colorblind|contrast]`

use algoclerk_game::scenarios::{NAMES, Spec, render};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next() else {
        eprintln!(
            "usage: algoclerk-shots OUT_DIR [--scale N] [--only a,b] [--portrait] [--palette P] [--text-size N]"
        );
        std::process::exit(2);
    };
    let out = PathBuf::from(out);
    let mut scale = 3usize;
    let mut only: Option<Vec<String>> = None;
    let mut portrait_only = false;
    let mut palette = "warm".to_string();
    let mut text_size = 1u8;
    let mut reduce = false;
    let mut suffix_palette = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => scale = args.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            "--only" => {
                only = args
                    .next()
                    .map(|v| v.split(',').map(String::from).collect())
            }
            "--portrait" => portrait_only = true,
            "--palette" => {
                palette = args.next().unwrap_or_else(|| "warm".into());
                suffix_palette = true;
            }
            "--text-size" => text_size = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--reduce-motion" => reduce = true,
            other => eprintln!("ignoring {other}"),
        }
    }
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("cannot create {}: {e}", out.display());
        std::process::exit(1);
    }
    let mut failures = 0;
    for name in NAMES {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == name)) {
            continue;
        }
        for portrait in [false, true] {
            if portrait_only && !portrait {
                continue;
            }
            if !portrait_only
                && portrait
                && only.is_none()
                && !matches!(
                    name,
                    "play"
                        | "victory"
                        | "title"
                        | "select"
                        | "settings"
                        | "code-rust"
                        | "brief"
                        | "play-keyboard"
                )
            {
                continue;
            }
            let spec = Spec {
                name: name.to_string(),
                portrait,
                palette: palette.clone(),
                text_size,
                reduce_motion: reduce,
                touch: portrait,
            };
            match render(&spec) {
                Ok(c) => {
                    let file = format!(
                        "{name}{}{}.png",
                        if portrait { "-portrait" } else { "" },
                        if suffix_palette {
                            format!("-{palette}")
                        } else {
                            String::new()
                        }
                    );
                    let path = out.join(file);
                    if let Err(e) = std::fs::write(&path, c.upscaled(scale).to_png()) {
                        eprintln!("{}: {e}", path.display());
                        failures += 1;
                    } else {
                        println!("wrote {}", path.display());
                    }
                }
                Err(e) => {
                    eprintln!("{name}{}: {e}", if portrait { " (portrait)" } else { "" });
                    failures += 1;
                }
            }
        }
    }
    if failures > 0 {
        std::process::exit(1);
    }
}
