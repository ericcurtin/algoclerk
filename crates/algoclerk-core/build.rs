// SPDX-License-Identifier: Apache-2.0
//! Embeds every `lang/*.toml` translation catalogue into the crate.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=lang");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("lang");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("lang directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    entries.sort();
    let mut out = String::from("pub static LOCALES: &[(&str, &str)] = &[\n");
    for p in entries {
        let code = p.file_stem().unwrap().to_string_lossy().to_string();
        let _ = writeln!(
            out,
            "    ({code:?}, include_str!({:?})),",
            p.to_string_lossy()
        );
    }
    out.push_str("];\n");
    let target = Path::new(&std::env::var("OUT_DIR").unwrap()).join("locales.rs");
    std::fs::write(target, out).unwrap();
}
