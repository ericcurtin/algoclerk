// SPDX-License-Identifier: Apache-2.0
//! Test harness generation: wrap generated functions in a `main` that runs fixed inputs and prints
//! the results, so the generated code can be compiled and compared with the interpreter.
//!
//! Output format: one line per case, `<count> <v1> <v2> ...`, in item order then case order.

use super::meta::{Arg, split_args};
use super::{Generated, Lang, Shape};
use crate::level::{CodeMeta, Level, ReturnKind};
use std::fmt::Write as _;

/// One input to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessCase {
    /// Inbox values.
    pub inbox: Vec<i32>,
    /// Initial floor.
    pub floor: Vec<Option<i32>>,
}

/// A generated function plus the inputs to call it with.
#[derive(Debug)]
pub struct Item<'a> {
    /// The generated function.
    pub generated: &'a Generated,
    /// The level (needed for metadata-shaped functions).
    pub level: Option<&'a Level>,
    /// Inputs.
    pub cases: Vec<HarnessCase>,
}

fn c_array(v: &[i32]) -> String {
    if v.is_empty() {
        "{0}".to_string()
    } else {
        format!("{{{}}}", v.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))
    }
}

fn rust_array(v: &[i32]) -> String {
    format!("&[{}]", v.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))
}

fn floor_values(floor: &[Option<i32>], len: usize) -> Vec<i32> {
    (0..len).map(|i| floor.get(i).copied().flatten().unwrap_or(0)).collect()
}

fn meta_args(meta: &CodeMeta, level: &Level, case: &HarnessCase) -> Result<Vec<Arg>, String> {
    let template = level.floor_template().map_err(|e| e.to_string())?;
    split_args(meta, &template, &case.inbox, &case.floor)
}

/// Build one source file: all generated functions followed by a `main` that runs every case.
/// All items must share a language. Panics are impossible; invalid metadata yields `Err`.
pub fn build(lang: Lang, items: &[Item<'_>]) -> Result<String, String> {
    let mut out = String::new();
    for it in items {
        if it.generated.lang != lang {
            return Err("mixed languages in one harness".into());
        }
        out.push_str(&it.generated.code);
        out.push('\n');
    }
    match lang {
        Lang::C => {
            out.push_str("#include <stdio.h>\n\nstatic int out_buf[200000];\n\n");
            out.push_str("static void emit(const int *v, size_t n) {\n    printf(\"%zu\", n);\n    for (size_t i = 0; i < n; i++) printf(\" %d\", v[i]);\n    printf(\"\\n\");\n}\n\n");
            out.push_str("int main(void) {\n");
        }
        Lang::Cpp => {
            out.push_str("#include <cstdio>\n\nstatic void emit(const std::vector<int>& v) {\n    std::printf(\"%zu\", v.size());\n    for (int x : v) std::printf(\" %d\", x);\n    std::printf(\"\\n\");\n}\n\n");
            out.push_str("int main() {\n");
        }
        Lang::Rust => {
            out.push_str("fn emit(v: &[i32]) {\n    let mut s = v.len().to_string();\n    for x in v {\n        s.push(' ');\n        s.push_str(&x.to_string());\n    }\n    println!(\"{s}\");\n}\n\n");
            out.push_str("fn main() {\n");
        }
    }
    for it in items {
        for case in &it.cases {
            out.push_str(&call(lang, it, case)?);
        }
    }
    match lang {
        Lang::C => out.push_str("    return 0;\n}\n"),
        Lang::Cpp => out.push_str("    return 0;\n}\n"),
        Lang::Rust => out.push_str("}\n"),
    }
    Ok(out)
}

fn call(lang: Lang, it: &Item<'_>, case: &HarnessCase) -> Result<String, String> {
    let g = it.generated;
    let f = &g.function;
    let mut s = String::new();
    match (&g.shape, lang) {
        (Shape::Generic { floor_param }, Lang::C) => {
            let fl = floor_values(&case.floor, g.floor_len);
            let _ = write!(s, "    {{ static const int in[] = {};", c_array(&case.inbox));
            if *floor_param {
                let _ = write!(s, " static const int fl[] = {};", c_array(&fl));
            }
            let _ = write!(
                s,
                " size_t n = {f}(in, {}{}, out_buf); emit(out_buf, n); }}\n",
                case.inbox.len(),
                if *floor_param { ", fl" } else { "" }
            );
        }
        (Shape::Generic { floor_param }, Lang::Cpp) => {
            let fl = floor_values(&case.floor, g.floor_len);
            let _ = write!(s, "    {{ std::vector<int> in{};", c_array(&case.inbox).replace("{0}", "{}").replace("{", "{").replace('}', "}"));
            if *floor_param {
                let _ = write!(s, " std::vector<int> fl{};", c_array(&fl));
            }
            let _ = write!(s, " emit({f}(in{})); }}\n", if *floor_param { ", fl" } else { "" });
        }
        (Shape::Generic { floor_param }, Lang::Rust) => {
            let fl = floor_values(&case.floor, g.floor_len);
            let _ = write!(
                s,
                "    emit(&{f}({}{}));\n",
                rust_array(&case.inbox),
                if *floor_param { format!(", {}", rust_array(&fl)) } else { String::new() }
            );
        }
        (Shape::Meta(meta), _) => {
            let level = it.level.ok_or("metadata-shaped function needs its level")?;
            let args = meta_args(meta, level, case)?;
            s.push_str(&meta_call(lang, f, meta, &args));
        }
    }
    Ok(s)
}

fn meta_call(lang: Lang, f: &str, meta: &CodeMeta, args: &[Arg]) -> String {
    let scalar_ret = meta.returns == ReturnKind::Scalar;
    let mut s = String::from("    { ");
    let mut call_args: Vec<String> = Vec::new();
    for (i, (p, a)) in meta.params.iter().zip(args).enumerate() {
        let _ = p;
        match (a, lang) {
            (Arg::Scalar(v), _) => call_args.push(v.to_string()),
            (Arg::List(v), Lang::C) => {
                let _ = write!(s, "static const int a{i}[] = {}; ", c_array(v));
                call_args.push(format!("a{i}"));
                call_args.push(v.len().to_string());
            }
            (Arg::List(v), Lang::Cpp) => {
                let init = if v.is_empty() { "{}".to_string() } else { c_array(v) };
                let _ = write!(s, "std::vector<int> a{i}{init}; ");
                call_args.push(format!("a{i}"));
            }
            (Arg::List(v), Lang::Rust) => call_args.push(rust_array(v)),
        }
    }
    let list = call_args.join(", ");
    match (lang, scalar_ret) {
        (Lang::C, true) => {
            let _ = write!(s, "int r = (int){f}({list}); emit(&r, 1); }}\n");
        }
        (Lang::C, false) => {
            let sep = if list.is_empty() { "" } else { ", " };
            let _ = write!(s, "size_t n = {f}({list}{sep}out_buf); emit(out_buf, n); }}\n");
        }
        (Lang::Cpp, true) => {
            let _ = write!(s, "std::vector<int> r{{(int){f}({list})}}; emit(r); }}\n");
        }
        (Lang::Cpp, false) => {
            let _ = write!(s, "emit({f}({list})); }}\n");
        }
        (Lang::Rust, true) => {
            let _ = write!(s, "emit(&[{f}({list}) as i32]); }}\n");
        }
        (Lang::Rust, false) => {
            let _ = write!(s, "emit(&{f}({list})); }}\n");
        }
    }
    s
}

/// Parse the harness output into per-case value lists.
pub fn parse_output(text: &str) -> Result<Vec<Vec<i32>>, String> {
    text.lines()
        .map(|l| {
            let mut it = l.split_whitespace().map(|t| t.parse::<i64>());
            let n = it
                .next()
                .ok_or("empty line")?
                .map_err(|e| e.to_string())?;
            let vals: Result<Vec<i32>, String> = it
                .map(|r| r.map_err(|e| e.to_string()).map(|v| v as i32))
                .collect();
            let vals = vals?;
            if vals.len() as i64 != n {
                return Err(format!("count mismatch in line {l:?}"));
            }
            Ok(vals)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_output_roundtrip() {
        assert_eq!(parse_output("2 1 2\n0\n").unwrap(), vec![vec![1, 2], vec![]]);
        assert!(parse_output("3 1\n").is_err());
        assert!(parse_output("x\n").is_err());
    }
}
