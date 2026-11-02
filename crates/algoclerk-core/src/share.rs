// SPDX-License-Identifier: Apache-2.0
//! Plain-text solution export and import, for sharing solutions without any server.

use crate::asm::{Listing, ParseError, parse};

/// Magic first line of an exported solution.
pub const HEADER: &str = "-- algoclerk solution v1";

/// Statistics shown in the export header.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShareStats {
    /// Instruction count.
    pub size: usize,
    /// Average steps.
    pub avg_steps: f64,
    /// Worst-case steps.
    pub worst_steps: u64,
}

/// An imported solution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
    /// The level id from the header, if present.
    pub level: Option<String>,
    /// The program.
    pub listing: Listing,
}

/// Produce the shareable text.
pub fn export(level_id: &str, listing: &Listing, stats: Option<ShareStats>) -> String {
    let mut out = format!("{HEADER}\n-- level: {level_id}\n");
    if let Some(s) = stats {
        out.push_str(&format!(
            "-- size: {}  speed-avg: {:.1}  speed-worst: {}\n",
            s.size, s.avg_steps, s.worst_steps
        ));
    }
    out.push_str(&listing.to_text());
    out
}

/// Parse shared text (the header is optional; plain assembly works too).
pub fn import(text: &str) -> Result<Imported, Vec<ParseError>> {
    let mut level = None;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let Some(rest) = t.strip_prefix("--") else {
            break;
        };
        if let Some(id) = rest.trim().strip_prefix("level:") {
            level = Some(id.trim().to_string());
        }
    }
    Ok(Imported {
        level,
        listing: parse(text)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let l = parse("a:\nINBOX\nOUTBOX\nJUMP a").unwrap();
        let t = export(
            "basics/echo",
            &l,
            Some(ShareStats {
                size: 3,
                avg_steps: 7.5,
                worst_steps: 9,
            }),
        );
        assert!(t.starts_with(HEADER));
        let i = import(&t).unwrap();
        assert_eq!(i.level.as_deref(), Some("basics/echo"));
        assert_eq!(i.listing.instruction_count(), 3);
    }

    #[test]
    fn plain_assembly_imports() {
        let i = import("INBOX\nOUTBOX").unwrap();
        assert_eq!(i.level, None);
    }

    #[test]
    fn errors_are_returned() {
        assert!(import("-- level: x\nBOGUS").is_err());
    }
}
