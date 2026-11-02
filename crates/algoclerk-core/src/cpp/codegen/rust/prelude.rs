// SPDX-License-Identifier: Apache-2.0
//! The Rust prelude: imports, the few helpers the program uses and the node arenas.

use super::Rs;
use crate::cpp::codegen::common::R;

const SUBSTR: &str = "\
/// `std::string::substr`: the length is clamped to the end of the string.
fn substr(s: &str, pos: usize, len: usize) -> String {
    let end = pos.saturating_add(len).min(s.len());
    s[pos..end].to_string()
}
";

const STOI: &str = "\
/// `std::stoi`: optional blanks and sign, then digits; stops at the first other character.
fn stoi(s: &str) -> i32 {
    stoll(s) as i32
}
";

const STOLL: &str = "\
/// `std::stoll`: optional blanks and sign, then digits; stops at the first other character.
fn stoll(s: &str) -> i64 {
    let t = s.trim_start();
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let mut v: i64 = 0;
    for b in digits.bytes().take_while(u8::is_ascii_digit) {
        v = v * 10 + i64::from(b - b'0');
    }
    if neg { -v } else { v }
}
";

const STR_RESIZE: &str = "\
/// `std::string::resize`: shrink, or pad with `c`.
fn str_resize(s: &mut String, n: usize, c: u8) {
    if n <= s.len() {
        s.truncate(n);
    } else {
        while s.len() < n {
            s.push(c as char);
        }
    }
}
";

const INSERT_IF_ABSENT: &str = "\
/// `std::map::insert`: keeps the existing value when the key is present; true when it inserted.
fn insert_new<K: std::hash::Hash + Eq, V>(m: &mut HashMap<K, V>, k: K, v: V) -> bool {
    if m.contains_key(&k) {
        false
    } else {
        m.insert(k, v);
        true
    }
}
";

impl Rs<'_> {
    /// Imports, helpers and arenas of the generated program.
    pub(super) fn prelude_text(&mut self) -> R<String> {
        let mut o = String::from(
            "#![allow(dead_code, unused_mut, unused_parens, unused_variables, non_snake_case, non_upper_case_globals)]\n",
        );
        let structs = !self.used_structs.is_empty();
        let mut struct_blocks: Vec<String> = Vec::new();
        for s in self.used_structs.clone() {
            struct_blocks.push(self.struct_text(&s)?);
        }
        let mut coll: Vec<&str> = Vec::new();
        for u in &self.uses {
            if matches!(
                *u,
                "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet" | "VecDeque" | "BinaryHeap"
            ) {
                coll.push(u);
            }
        }
        if !coll.is_empty() {
            if coll.len() == 1 {
                o.push_str(&format!("use std::collections::{};\n", coll[0]));
            } else {
                o.push_str(&format!("use std::collections::{{{}}};\n", coll.join(", ")));
            }
        }
        if self.uses.contains("Reverse") {
            o.push_str("use std::cmp::Reverse;\n");
        }
        if self.uses.contains("Ordering") {
            o.push_str("use std::cmp::Ordering;\n");
        }
        if structs {
            o.push_str("use std::cell::RefCell;\n");
        }
        let mut blocks: Vec<String> = Vec::new();
        for h in self.helpers.clone() {
            blocks.push(
                match h {
                    "substr" => SUBSTR,
                    "stoi" => STOI,
                    "stoll" => STOLL,
                    "str_resize" => STR_RESIZE,
                    "insert_new" => INSERT_IF_ABSENT,
                    _ => "",
                }
                .to_string(),
            );
        }
        if self.helpers.contains("stoi") && !self.helpers.contains("stoll") {
            blocks.push(STOLL.to_string());
        }
        blocks.extend(struct_blocks);
        for b in blocks {
            o.push('\n');
            o.push_str(&b);
        }
        Ok(o)
    }
}
