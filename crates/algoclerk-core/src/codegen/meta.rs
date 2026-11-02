// SPDX-License-Identifier: Apache-2.0
//! The optional per-level `[code]` metadata: how test inputs map to function arguments.
//!
//! [`split_args`] turns a test's inbox and floor into arguments; [`apply_args`] does the reverse
//! (this is exactly what the generated function's prologue does). The level validator checks that
//! the two round-trip for every test of the level.

use crate::level::{CodeMeta, Level, ParamKind, ReturnKind, TestCase};

/// A concrete argument value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    /// One integer.
    Scalar(i32),
    /// A list of integers.
    List(Vec<i32>),
}

/// Split a test's inputs into function arguments according to the metadata.
pub fn split_args(
    meta: &CodeMeta,
    template: &[Option<i32>],
    inbox: &[i32],
    floor: &[Option<i32>],
) -> Result<Vec<Arg>, String> {
    let mut pos = 0usize;
    let mut args = Vec::new();
    for p in &meta.params {
        if let Some(at) = p.at {
            match p.kind {
                ParamKind::Scalar => {
                    let v = floor
                        .get(at)
                        .copied()
                        .flatten()
                        .ok_or_else(|| format!("parameter {}: floor slot {at} is empty", p.name))?;
                    args.push(Arg::Scalar(v));
                }
                _ => {
                    let limit = match p.cap {
                        Some(c) => (at + c).min(template.len()),
                        None => (at..template.len())
                            .find(|s| template[*s].is_some() && *s != at)
                            .unwrap_or(template.len()),
                    };
                    let mut vals = Vec::new();
                    for s in at..limit {
                        match floor.get(s).copied().flatten() {
                            Some(v) => vals.push(v),
                            None => break,
                        }
                    }
                    args.push(Arg::List(vals));
                }
            }
            continue;
        }
        match p.kind {
            ParamKind::Scalar => {
                let v = *inbox
                    .get(pos)
                    .ok_or_else(|| format!("parameter {}: the inbox ended too early", p.name))?;
                pos += 1;
                args.push(Arg::Scalar(v));
            }
            ParamKind::List => {
                args.push(Arg::List(inbox[pos.min(inbox.len())..].to_vec()));
                pos = inbox.len();
            }
            ParamKind::Counted => {
                let n = *inbox
                    .get(pos)
                    .ok_or_else(|| format!("parameter {}: missing length", p.name))?;
                pos += 1;
                let n = usize::try_from(n).map_err(|_| format!("parameter {}: negative length", p.name))?;
                let end = pos + n;
                if end > inbox.len() {
                    return Err(format!("parameter {}: the inbox is shorter than its length", p.name));
                }
                args.push(Arg::List(inbox[pos..end].to_vec()));
                pos = end;
            }
            ParamKind::Terminated => {
                let rel = inbox[pos.min(inbox.len())..]
                    .iter()
                    .position(|v| *v == p.terminator)
                    .ok_or_else(|| format!("parameter {}: terminator {} not found", p.name, p.terminator))?;
                args.push(Arg::List(inbox[pos..pos + rel].to_vec()));
                pos += rel + 1;
            }
        }
    }
    if pos != inbox.len() {
        return Err("the inbox has values that no parameter accounts for".into());
    }
    Ok(args)
}

/// Rebuild the inbox and floor from arguments (what the generated prologue does).
pub fn apply_args(
    meta: &CodeMeta,
    template: &[Option<i32>],
    args: &[Arg],
) -> Result<(Vec<i32>, Vec<Option<i32>>), String> {
    let mut inbox = Vec::new();
    let mut floor = template.to_vec();
    for (p, a) in meta.params.iter().zip(args) {
        if let Some(at) = p.at {
            let vals: Vec<i32> = match a {
                Arg::Scalar(v) => vec![*v],
                Arg::List(v) => v.clone(),
            };
            for (i, v) in vals.iter().enumerate() {
                *floor
                    .get_mut(at + i)
                    .ok_or_else(|| format!("parameter {} does not fit on the floor", p.name))? = Some(*v);
            }
            continue;
        }
        match (p.kind, a) {
            (ParamKind::Scalar, Arg::Scalar(v)) => inbox.push(*v),
            (ParamKind::List, Arg::List(v)) => inbox.extend_from_slice(v),
            (ParamKind::Counted, Arg::List(v)) => {
                inbox.push(v.len() as i32);
                inbox.extend_from_slice(v);
            }
            (ParamKind::Terminated, Arg::List(v)) => {
                inbox.extend_from_slice(v);
                inbox.push(p.terminator);
            }
            _ => return Err(format!("parameter {}: argument shape mismatch", p.name)),
        }
    }
    Ok((inbox, floor))
}

/// True if any test of the level sets floor values (so the function needs floor input).
pub fn level_has_floor_input(level: &Level) -> bool {
    level.examples.iter().any(|e| !e.floor.is_empty())
        || level.tests.fixed.iter().any(|c| !c.floor.is_empty())
        || level.tests.generators.iter().any(|g| !g.floor.is_empty())
}

/// Check the `[code]` metadata of a level against its tests. Returns human-readable problems.
pub fn check_layout(level: &Level, tests: &[TestCase]) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(meta) = &level.code else {
        return problems;
    };
    let ident_ok = |s: &str| {
        !s.is_empty()
            && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && !s.starts_with(|c: char| c.is_ascii_digit())
    };
    if !ident_ok(&meta.function) {
        problems.push(format!("code.function \"{}\" must be snake_case ascii", meta.function));
    }
    let mut seen = std::collections::HashSet::new();
    for p in &meta.params {
        if !ident_ok(&p.name) {
            problems.push(format!("code parameter name \"{}\" must be snake_case ascii", p.name));
        }
        if !seen.insert(p.name.clone()) {
            problems.push(format!("duplicate code parameter \"{}\"", p.name));
        }
    }
    let inbox_params: Vec<_> = meta.params.iter().filter(|p| p.at.is_none()).collect();
    if let Some(pos) = inbox_params.iter().position(|p| p.kind == ParamKind::List)
        && pos + 1 != inbox_params.len()
    {
        problems.push("an inbox parameter of kind \"list\" must be the last inbox parameter".into());
    }
    let template = match level.floor_template() {
        Ok(t) => t,
        Err(e) => {
            problems.push(e.to_string());
            return problems;
        }
    };
    for (i, t) in tests.iter().enumerate() {
        match split_args(meta, &template, &t.inbox, &t.floor) {
            Err(e) => problems.push(format!("test {}: cannot split into arguments: {e}", i + 1)),
            Ok(args) => match apply_args(meta, &template, &args) {
                Err(e) => problems.push(format!("test {}: {e}", i + 1)),
                Ok((inbox, floor)) => {
                    if inbox != t.inbox || floor != t.floor {
                        problems.push(format!("test {}: arguments do not rebuild the original input", i + 1));
                    }
                }
            },
        }
        if meta.returns == ReturnKind::Scalar && t.expected.len() != 1 {
            problems.push(format!(
                "test {}: returns = \"scalar\" needs exactly one output value, found {}",
                i + 1,
                t.expected.len()
            ));
        }
        if problems.len() > 8 {
            problems.push("... more problems omitted".into());
            break;
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{ParamSpec, RetType};

    fn meta(params: Vec<ParamSpec>) -> CodeMeta {
        CodeMeta {
            function: "f".into(),
            doc: String::new(),
            params,
            returns: ReturnKind::Scalar,
            return_type: RetType::Int,
        }
    }

    fn p(name: &str, kind: ParamKind, at: Option<usize>) -> ParamSpec {
        ParamSpec {
            name: name.into(),
            kind,
            terminator: 0,
            at,
            cap: None,
        }
    }

    #[test]
    fn inbox_layouts_roundtrip() {
        let m = meta(vec![
            p("target", ParamKind::Scalar, None),
            p("a", ParamKind::Counted, None),
            p("b", ParamKind::Terminated, None),
            p("rest", ParamKind::List, None),
        ]);
        let inbox = vec![5, 2, 7, 8, 1, 2, 0, 9, 9];
        let args = split_args(&m, &[], &inbox, &[]).unwrap();
        assert_eq!(
            args,
            vec![
                Arg::Scalar(5),
                Arg::List(vec![7, 8]),
                Arg::List(vec![1, 2]),
                Arg::List(vec![9, 9])
            ]
        );
        let (i2, _) = apply_args(&m, &[], &args).unwrap();
        assert_eq!(i2, inbox);
    }

    #[test]
    fn floor_params() {
        let m = meta(vec![
            p("n", ParamKind::Scalar, Some(5)),
            p("xs", ParamKind::List, Some(0)),
        ]);
        let template = vec![None, None, None, None, None, None];
        let floor = vec![Some(1), Some(2), Some(3), None, None, Some(3)];
        let args = split_args(&m, &template, &[], &floor).unwrap();
        assert_eq!(args[0], Arg::Scalar(3));
        let (_, f2) = apply_args(&m, &template, &args).unwrap();
        assert_eq!(f2, floor);
    }

    #[test]
    fn bad_layouts_are_errors() {
        let m = meta(vec![p("a", ParamKind::Counted, None)]);
        assert!(split_args(&m, &[], &[5, 1], &[]).is_err());
        assert!(split_args(&m, &[], &[-1], &[]).is_err());
        assert!(split_args(&m, &[], &[1, 1, 7], &[]).is_err());
        let m = meta(vec![p("a", ParamKind::Terminated, None)]);
        assert!(split_args(&m, &[], &[1, 2], &[]).is_err());
    }
}
