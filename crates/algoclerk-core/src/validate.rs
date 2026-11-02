// SPDX-License-Identifier: Apache-2.0
//! Level validation, used by `algoclerk-cli validate`, CI and the test suite.

use crate::codegen::meta;
use crate::judge::{Judge, Verdict};
use crate::level::{Level, Seq};

/// Outcome of validating a level.
#[derive(Debug)]
pub struct Report {
    /// Problems that must be fixed.
    pub errors: Vec<String>,
    /// Suggestions.
    pub warnings: Vec<String>,
    /// The reference solution's verdict, if it could be judged.
    pub verdict: Option<Verdict>,
    /// Number of tests built.
    pub test_count: usize,
}

impl Report {
    /// No errors.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// Validate a level thoroughly: static fields, tests, reference solution, targets and code metadata.
pub fn validate_level(level: &Level) -> Report {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if !valid_id(&level.id) {
        errors.push(format!(
            "id \"{}\" must be lowercase letters, digits and dashes",
            level.id
        ));
    }
    for (name, text) in [
        ("title", &level.title),
        ("summary", &level.summary),
        ("story", &level.story),
        ("statement", &level.statement),
    ] {
        if text.trim().is_empty() {
            errors.push(format!("{name} must not be empty"));
        }
    }
    if level.title.chars().count() > 40 {
        warnings.push("title is longer than 40 characters".into());
    }
    if level.summary.chars().count() > 100 {
        warnings.push("summary is longer than 100 characters".into());
    }
    if level.machine.floor == 0 || level.machine.floor > 128 {
        errors.push("machine.floor must be between 1 and 128".into());
    }
    if level.machine.names.len() > level.machine.floor {
        errors.push("machine.names has more entries than there are floor slots".into());
    }
    if let Err(e) = level.floor_template() {
        errors.push(e.to_string());
    }
    let allowed = match level.allowed() {
        Ok(a) => Some(a),
        Err(e) => {
            errors.push(e.to_string());
            None
        }
    };
    if let Some(a) = &allowed
        && (!a.allows(crate::isa::Opcode::Inbox) || !a.allows(crate::isa::Opcode::Outbox))
    {
        errors.push("INBOX and OUTBOX must be allowed".into());
    }
    if level.examples.is_empty() {
        errors.push("at least one example is required".into());
    }
    let explicit = level
        .tests
        .fixed
        .iter()
        .filter(|c| c.output.is_some())
        .count();
    if explicit < 3 {
        errors.push(format!(
            "at least 3 hand-written test cases with explicit output are required (found {explicit})"
        ));
    }
    if level.hints.is_empty() {
        errors.push("a hint ladder with at least one hint is required".into());
    }
    if level.hints.len() > 6 {
        warnings.push("more than 6 hints".into());
    }
    let t = &level.targets;
    if t.size_ace == 0 || t.speed_ace == 0 || t.size_ace > t.size_par || t.speed_ace > t.speed_par {
        errors.push("targets must be positive with ace <= par".into());
    }
    if level.constraints.is_empty() {
        warnings.push("no constraints listed".into());
    }
    if level.explain.is_none() {
        warnings.push("no [explain] section (the victory screen shows it)".into());
    }
    if level.code.is_none() {
        warnings.push(
            "no [code] metadata: generated code will use the generic inbox/outbox form".into(),
        );
    }
    for (i, ex) in level.examples.iter().enumerate() {
        if matches!(&ex.input, Seq::Ints(v) if v.len() > 40) {
            warnings.push(format!("example {} is long", i + 1));
        }
    }

    let mut verdict = None;
    let mut test_count = 0;
    match level.solution {
        None => errors.push("a reference [solution] is required".into()),
        Some(_) => match Judge::new(level) {
            Err(e) => errors.push(e.to_string()),
            Ok(judge) => {
                test_count = judge.tests().len();
                if test_count < 8 {
                    errors.push(format!(
                        "at least 8 tests are required (found {test_count})"
                    ));
                }
                match level.reference_program() {
                    Err(e) => errors.push(e.to_string()),
                    Ok(prog) => {
                        let v = judge.judge(&prog);
                        if !v.passed {
                            let msg = v.headline(crate::i18n::english());
                            errors.push(format!("the reference solution fails: {msg}"));
                        } else {
                            if v.size > t.size_par as usize {
                                errors.push(format!(
                                    "reference size {} exceeds size_par {}",
                                    v.size, t.size_par
                                ));
                            }
                            if v.avg_steps > f64::from(t.speed_par) {
                                errors.push(format!(
                                    "reference average speed {:.1} exceeds speed_par {}",
                                    v.avg_steps, t.speed_par
                                ));
                            }
                            if v.size > t.size_ace as usize {
                                warnings.push(format!(
                                    "reference size {} is above size_ace {}",
                                    v.size, t.size_ace
                                ));
                            }
                            if v.avg_steps > f64::from(t.speed_ace) {
                                warnings.push(format!(
                                    "reference average speed {:.1} is above speed_ace {}",
                                    v.avg_steps, t.speed_ace
                                ));
                            }
                        }
                        verdict = Some(v);
                    }
                }
                errors.extend(meta::check_layout(level, judge.tests()));
            }
        },
    }
    Report {
        errors,
        warnings,
        verdict,
        test_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_sample_needs_more_but_runs() {
        let l = Level::from_toml(include_str!("../tests/data/echo.toml"), "t").unwrap();
        let r = validate_level(&l);
        // The tiny sample lacks hints and enough tests; the validator must say so, not panic.
        assert!(!r.ok());
        assert!(r.errors.iter().any(|e| e.contains("hint")));
        assert!(r.verdict.as_ref().is_some_and(|v| v.passed));
    }

    #[test]
    fn id_rules() {
        assert!(valid_id("two-sum"));
        assert!(!valid_id("Two"));
        assert!(!valid_id("-x"));
        assert!(!valid_id(""));
    }
}
