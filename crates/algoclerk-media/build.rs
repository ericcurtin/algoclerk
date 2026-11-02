// SPDX-License-Identifier: Apache-2.0
//! Embeds `<repo>/assets/art` and `<repo>/assets/music` into the binary, so the game needs no file
//! access to start (important on Android and iOS where assets live inside the app bundle).
//! Adapted from the sibling project Falabichos (Apache-2.0, same owner).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let p = e.path();
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if p.is_dir() {
            walk(&p, root, out);
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.push((rel.to_string_lossy().replace('\\', "/"), p));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = manifest.join("../../assets");
    let root = root.canonicalize().unwrap_or(root);
    let mut files = vec![];
    for sub in ["art", "music"] {
        println!("cargo:rerun-if-changed={}", root.join(sub).display());
        walk(&root.join(sub), &root, &mut files);
    }
    let mut src = String::from("pub static EMBEDDED: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in files {
        let _ = writeln!(src, "    ({:?}, include_bytes!({:?})),", rel, abs.to_string_lossy());
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir")).join("embedded.rs");
    std::fs::write(out, src).expect("write embedded.rs");
}
