// SPDX-License-Identifier: Apache-2.0
//! Local progress and settings, serialised as JSON by the front end.

use crate::judge::{Medals, Verdict};
use crate::level::Level;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Current on-disk format version.
pub const PROGRESS_VERSION: u32 = 1;

/// Per-level record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelRecord {
    /// Solved at least once.
    pub solved: bool,
    /// Smallest solution size.
    pub best_size: Option<u32>,
    /// Lowest average steps.
    pub best_speed: Option<f64>,
    /// Best medals earned for each metric.
    pub medals: Medals,
    /// Stars 0-3.
    pub stars: u8,
    /// Text of the smallest solution.
    pub best_size_solution: String,
    /// Text of the fastest solution.
    pub best_speed_solution: String,
    /// Work in progress.
    pub draft: String,
    /// How many hints were revealed.
    pub hints_used: u8,
    /// The player chose to reveal the reference solution.
    pub reference_revealed: bool,
    /// The player chose to see generated code before solving.
    pub code_revealed: bool,
}

/// User settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `"light"`, `"dark"` or `"contrast"`.
    pub theme: String,
    /// Colour-blind friendly palette.
    pub colorblind: bool,
    /// Animation speed multiplier.
    pub speed: f32,
    /// Sound on.
    pub sound: bool,
    /// Language code.
    pub language: String,
    /// UI scale multiplier.
    pub text_scale: f32,
    /// Unlock every level.
    pub unlock_all: bool,
    /// `"c"`, `"cpp"` or `"rust"`.
    pub code_lang: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "dark".into(),
            colorblind: false,
            speed: 1.0,
            sound: true,
            language: "en".into(),
            text_scale: 1.0,
            unlock_all: false,
            code_lang: "rust".into(),
        }
    }
}

/// Everything saved locally.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progress {
    /// Format version.
    pub version: u32,
    /// Records by level id.
    pub levels: BTreeMap<String, LevelRecord>,
    /// Settings.
    pub settings: Settings,
    /// Sandbox program text.
    pub sandbox: String,
    /// Id of the level last played.
    pub last_level: String,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            version: PROGRESS_VERSION,
            levels: BTreeMap::new(),
            settings: Settings::default(),
            sandbox: String::new(),
            last_level: String::new(),
        }
    }
}

impl Progress {
    /// Parse JSON; falls back to defaults on corrupt data (never panics, never loses the app).
    pub fn from_json(s: &str) -> Progress {
        serde_json::from_str(s).unwrap_or_default()
    }

    /// Serialise.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Record of a level (default if never played).
    pub fn record(&self, id: &str) -> LevelRecord {
        self.levels.get(id).cloned().unwrap_or_default()
    }

    /// Mutable record.
    pub fn record_mut(&mut self, id: &str) -> &mut LevelRecord {
        self.levels.entry(id.to_string()).or_default()
    }

    /// Record a verdict. Returns true if something improved (new solve or better score).
    pub fn record_verdict(
        &mut self,
        level: &Level,
        verdict: &Verdict,
        solution_text: &str,
    ) -> bool {
        if !verdict.passed {
            return false;
        }
        let rec = self.record_mut(&level.id);
        let mut improved = !rec.solved;
        rec.solved = true;
        let size = verdict.size as u32;
        if rec.best_size.is_none_or(|b| size < b) {
            rec.best_size = Some(size);
            rec.best_size_solution = solution_text.to_string();
            improved = true;
        }
        if rec.best_speed.is_none_or(|b| verdict.avg_steps < b) {
            rec.best_speed = Some(verdict.avg_steps);
            rec.best_speed_solution = solution_text.to_string();
            improved = true;
        }
        rec.medals.size = rec.medals.size.max(verdict.medals.size);
        rec.medals.speed = rec.medals.speed.max(verdict.medals.speed);
        rec.stars = rec.medals.stars();
        improved
    }

    /// Total stars.
    pub fn total_stars(&self) -> u32 {
        self.levels.values().map(|r| u32::from(r.stars)).sum()
    }

    /// Number of solved levels.
    pub fn solved_count(&self) -> usize {
        self.levels.values().filter(|r| r.solved).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::Judge;

    #[test]
    fn corrupt_json_gives_defaults() {
        assert_eq!(Progress::from_json("{{{{"), Progress::default());
        assert_eq!(Progress::from_json("{\"levels\": 5}"), Progress::default());
    }

    #[test]
    fn partial_json_fills_defaults() {
        let p = Progress::from_json("{\"settings\": {\"theme\": \"light\"}}");
        assert_eq!(p.settings.theme, "light");
        assert!(p.settings.sound);
    }

    #[test]
    fn verdict_recording_keeps_best() {
        let level = Level::from_toml(include_str!("../tests/data/echo.toml"), "t").unwrap();
        let judge = Judge::new(&level).unwrap();
        let v = judge.judge(&level.reference_program().unwrap());
        let mut p = Progress::default();
        assert!(p.record_verdict(&level, &v, "a"));
        assert!(!p.record_verdict(&level, &v, "b"));
        assert_eq!(p.record(&level.id).best_size_solution, "a");
        assert_eq!(p.total_stars(), 3);
        let again = Progress::from_json(&p.to_json());
        assert_eq!(again, p);
    }
}
