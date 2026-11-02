// SPDX-License-Identifier: Apache-2.0
//! The judge: runs a program on every test of a level and scores it.

use crate::asm::{Problem, Program};
use crate::i18n::{Diagnostic, Strings};
use crate::level::{Level, LevelError, TestCase, TestOrigin};
use crate::vm::{RuntimeError, Status, run_program};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Medal for one metric, from lowest to highest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Medal {
    /// Solved, nothing more.
    Clip,
    /// Met the par target.
    Staple,
    /// Met the ace target.
    Seal,
}

impl Medal {
    /// Translation key for the medal name.
    pub fn key(self) -> &'static str {
        match self {
            Medal::Clip => "medal.clip",
            Medal::Staple => "medal.staple",
            Medal::Seal => "medal.seal",
        }
    }
}

/// What happened in one test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The outbox matched.
    Pass,
    /// The program finished but the outbox differed.
    Wrong {
        /// What the program produced.
        got: Vec<i32>,
    },
    /// The program crashed or ran too long.
    Error {
        /// The error.
        error: RuntimeError,
        /// What had been output before the error.
        got: Vec<i32>,
    },
}

/// Per-test result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestResult {
    /// Index in the level's test list.
    pub index: usize,
    /// Origin of the test.
    pub origin: TestOrigin,
    /// Inbox of the test.
    pub inbox: Vec<i32>,
    /// Expected outbox.
    pub expected: Vec<i32>,
    /// Steps executed.
    pub steps: u64,
    /// Outcome.
    pub outcome: Outcome,
}

/// Medals for both metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Medals {
    /// Size medal.
    pub size: Option<Medal>,
    /// Speed medal.
    pub speed: Option<Medal>,
}

impl Medals {
    /// Stars 0-3: one for solving, one for each metric at staple or better.
    pub fn stars(&self) -> u8 {
        match (self.size, self.speed) {
            (None, None) => 0,
            (s, p) => {
                1 + u8::from(s.is_some_and(|m| m >= Medal::Staple))
                    + u8::from(p.is_some_and(|m| m >= Medal::Staple))
            }
        }
    }
}

/// The judge's verdict on a program.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    /// All tests passed and the program obeys the office rules.
    pub passed: bool,
    /// Rule violations (instructions not allowed, tiles that do not exist).
    pub problems: Vec<Problem>,
    /// Per-test results (empty if the rules were broken).
    pub tests: Vec<TestResult>,
    /// Instruction count.
    pub size: usize,
    /// Maximum steps over tests.
    pub worst_steps: u64,
    /// Mean steps over tests.
    pub avg_steps: f64,
    /// Medals (only meaningful if passed).
    pub medals: Medals,
}

impl Verdict {
    /// First failing test.
    pub fn first_failure(&self) -> Option<&TestResult> {
        self.tests.iter().find(|t| t.outcome != Outcome::Pass)
    }

    /// Number of passing tests.
    pub fn passed_count(&self) -> usize {
        self.tests.iter().filter(|t| t.outcome == Outcome::Pass).count()
    }

    /// A one-line explanation for the player.
    pub fn headline(&self, s: &Strings) -> String {
        if !self.problems.is_empty() {
            return self.problems[0].message(s);
        }
        match self.first_failure() {
            None => s.t("judge.solved"),
            Some(t) => match &t.outcome {
                Outcome::Wrong { got } => s.format(
                    "judge.wrong_output",
                    &[
                        ("test", (t.index + 1).to_string()),
                        ("got", format!("{got:?}")),
                        ("expected", format!("{:?}", t.expected)),
                    ],
                ),
                Outcome::Error { error, .. } => s.format(
                    "judge.run_error",
                    &[
                        ("test", (t.index + 1).to_string()),
                        ("error", error.message(s)),
                    ],
                ),
                Outcome::Pass => String::new(),
            },
        }
    }
}

/// A level prepared for judging (tests are built once).
#[derive(Clone, Debug)]
pub struct Judge {
    level: Level,
    tests: Vec<TestCase>,
}

impl Judge {
    /// Build the tests of a level.
    pub fn new(level: &Level) -> Result<Judge, LevelError> {
        Ok(Judge {
            level: level.clone(),
            tests: level.build_tests()?,
        })
    }

    /// The level.
    pub fn level(&self) -> &Level {
        &self.level
    }

    /// The tests.
    pub fn tests(&self) -> &[TestCase] {
        &self.tests
    }

    /// Judge a program.
    pub fn judge(&self, program: &Arc<Program>) -> Verdict {
        let size = program.size();
        let allowed = match self.level.allowed() {
            Ok(a) => a,
            Err(_) => crate::isa::Allowed::everything(),
        };
        let problems = program.check(&allowed, self.level.machine.floor);
        if !problems.is_empty() {
            return Verdict {
                passed: false,
                problems,
                tests: Vec::new(),
                size,
                worst_steps: 0,
                avg_steps: 0.0,
                medals: Medals::default(),
            };
        }
        let mut results = Vec::with_capacity(self.tests.len());
        let mut total = 0u64;
        let mut worst = 0u64;
        let mut all_pass = true;
        for (i, t) in self.tests.iter().enumerate() {
            let r = run_program(program, &t.inbox, &t.floor, self.level.tests.step_limit);
            total += r.steps;
            worst = worst.max(r.steps);
            let outcome = match r.status {
                Status::Error(error) => Outcome::Error {
                    error,
                    got: r.outbox,
                },
                _ if r.outbox == t.expected => Outcome::Pass,
                _ => Outcome::Wrong { got: r.outbox },
            };
            all_pass &= outcome == Outcome::Pass;
            results.push(TestResult {
                index: i,
                origin: t.origin,
                inbox: t.inbox.clone(),
                expected: t.expected.clone(),
                steps: r.steps,
                outcome,
            });
        }
        let avg = if results.is_empty() {
            0.0
        } else {
            total as f64 / results.len() as f64
        };
        let medals = if all_pass {
            medals_for(&self.level, size, avg)
        } else {
            Medals::default()
        };
        Verdict {
            passed: all_pass,
            problems,
            tests: results,
            size,
            worst_steps: worst,
            avg_steps: avg,
            medals,
        }
    }
}

/// Compute medals for a solved program.
pub fn medals_for(level: &Level, size: usize, avg_steps: f64) -> Medals {
    let t = &level.targets;
    let size_medal = if size <= t.size_ace as usize {
        Medal::Seal
    } else if size <= t.size_par as usize {
        Medal::Staple
    } else {
        Medal::Clip
    };
    let speed_medal = if avg_steps <= f64::from(t.speed_ace) {
        Medal::Seal
    } else if avg_steps <= f64::from(t.speed_par) {
        Medal::Staple
    } else {
        Medal::Clip
    };
    Medals {
        size: Some(size_medal),
        speed: Some(speed_medal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;

    fn level() -> Level {
        Level::from_toml(
            include_str!("../tests/data/echo.toml"),
            "test",
        )
        .unwrap()
    }

    #[test]
    fn reference_passes_and_gets_medals() {
        let l = level();
        let j = Judge::new(&l).unwrap();
        let v = j.judge(&l.reference_program().unwrap());
        assert!(v.passed, "{:?}", v.first_failure());
        assert_eq!(v.size, 3);
        assert_eq!(v.medals.size, Some(Medal::Seal));
        assert_eq!(v.medals.stars(), 3);
    }

    #[test]
    fn wrong_and_error_and_rules() {
        let l = level();
        let j = Judge::new(&l).unwrap();
        let wrong = j.judge(&Arc::new(assemble("a:\nINBOX\nOUTBOX\nOUTBOX\nJUMP a").unwrap()));
        assert!(!wrong.passed);
        assert!(matches!(
            wrong.first_failure().unwrap().outcome,
            Outcome::Error { .. }
        ));
        let hardcoded = j.judge(&Arc::new(assemble("INBOX\nOUTBOX").unwrap()));
        assert!(!hardcoded.passed);
        assert!(hardcoded.passed_count() < hardcoded.tests.len());
        let rules = j.judge(&Arc::new(assemble("INBOX\nADD 0\nOUTBOX").unwrap()));
        assert!(!rules.passed);
        assert!(!rules.problems.is_empty());
        assert!(rules.tests.is_empty());
        let looping = j.judge(&Arc::new(assemble("a:\nJUMP a").unwrap()));
        assert!(!looping.passed);
        assert!(!looping.headline(crate::i18n::english()).is_empty());
    }

    #[test]
    fn slow_but_correct_gets_clip() {
        let l = level();
        let j = Judge::new(&l).unwrap();
        let p = assemble("a:\nINBOX\nCOPYTO 0\nCOPYFROM 0\nCOPYTO 0\nCOPYFROM 0\nOUTBOX\nJUMP a").unwrap();
        let v = j.judge(&Arc::new(p));
        assert!(v.passed);
        assert_eq!(v.medals.size, Some(Medal::Clip));
        assert!(v.medals.speed < Some(Medal::Seal));
        assert!(v.medals.stars() <= 2);
    }
}
