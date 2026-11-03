// SPDX-License-Identifier: Apache-2.0
//! `docs/CPP_SUBSET.md` must contain the current library table (`cpp::stdlib::markdown_table()`), between
//! the `<!-- library-table:begin -->` and `<!-- library-table:end -->` markers. Regenerate with
//! `algoclerk-cli subset-table`.

use algoclerk_core::cpp::stdlib::markdown_table;
use std::path::Path;

fn doc() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CPP_SUBSET.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn library_table_in_the_document_is_current() {
    let d = doc();
    let begin = "<!-- library-table:begin -->\n";
    let end = "<!-- library-table:end -->";
    let (Some(a), Some(b)) = (d.find(begin), d.find(end)) else {
        panic!("docs/CPP_SUBSET.md has no library-table markers");
    };
    let have = &d[a + begin.len()..b];
    assert_eq!(
        have,
        markdown_table(),
        "docs/CPP_SUBSET.md is stale: run `algoclerk-cli subset-table` and paste the table between the markers"
    );
}

#[test]
fn every_library_name_the_document_promises_exists() {
    let d = doc();
    let t = markdown_table();
    for name in [
        "std::for_each",
        "std::partial_sum",
        "std::accumulate",
        "std::sort",
    ] {
        assert!(
            t.contains(&format!("`{name}`")),
            "{name} missing from the table"
        );
    }
    assert!(!d.contains('\u{2014}'), "no em dashes in the document");
}
