// SPDX-License-Identifier: Apache-2.0
//! Embeds every file under `levels/` (the shipped puzzle packs) into the crate.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let rd = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read the levels directory {}: {e}", dir.display()));
    for e in rd.filter_map(Result::ok) {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "toml") {
            out.push(p);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no level files (*.toml) found under {}; refusing to build a game without levels",
        root.display()
    );
    let mut out = String::from(
        "/// `(relative path, file contents)` of every embedded level file.\npub static FILES: &[(&str, &str)] = &[\n",
    );
    for f in files {
        let rel = f
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let abs = f.canonicalize().unwrap();
        let _ = writeln!(
            out,
            "    ({rel:?}, include_str!({:?})),",
            abs.to_string_lossy()
        );
    }
    out.push_str("];\n");
    let target = Path::new(&std::env::var("OUT_DIR").unwrap()).join("embedded.rs");
    std::fs::write(target, out).unwrap();
}
