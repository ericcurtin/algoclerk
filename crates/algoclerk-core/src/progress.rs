// SPDX-License-Identifier: Apache-2.0
//! Local progress and settings, serialised as JSON by the front end.

use crate::judge::{Medals, Verdict};
use crate::level::Level;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Current on-disk format version. Version 1 saves were written when players still wrote the invented
/// ClerkScript language; version 2 is the C++ era.
pub const PROGRESS_VERSION: u32 = 2;

/// Shown once after an old save was converted (see [`Progress::from_json`]).
pub const MIGRATION_NOTE: &str = "The office switched from ClerkScript to real C++. Old solutions cannot be migrated, so drafts and saved solutions were cleared; your solved levels, stars and settings were kept.";

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
    /// Measured complexity of the latest accepted solution.
    pub complexity: Option<crate::complexity::Complexity>,
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
    /// Legacy (egui front end): `"light"`, `"dark"` or `"contrast"`. The pixel UI uses `palette`.
    pub theme: String,
    /// Legacy (egui front end): colour-blind friendly palette. The pixel UI uses `palette`.
    pub colorblind: bool,
    /// Legacy animation speed multiplier. The pixel UI uses `speed_level`.
    pub speed: f32,
    /// Legacy master switch for sound effects. The pixel UI uses the volumes.
    pub sound: bool,
    /// Language code.
    pub language: String,
    /// Legacy UI scale multiplier. The pixel UI uses `text_size`.
    pub text_scale: f32,
    /// Unlock every level.
    pub unlock_all: bool,
    /// `"c"`, `"cpp"` or `"rust"`.
    pub code_lang: String,
    /// Pixel UI palette: `"warm"`, `"colorblind"` or `"contrast"`.
    pub palette: String,
    /// Clerk outfit as a compact code (empty means the default outfit).
    pub outfit: String,
    /// Skip decorative animation (idle motion, particles, wipes).
    pub reduce_motion: bool,
    /// Music volume 0..=10.
    pub music_volume: u8,
    /// Sound effect volume 0..=10.
    pub sfx_volume: u8,
    /// Reading text size: 1 or 2 (an integer scale of the pixel font).
    pub text_size: u8,
    /// Start in a borderless full-screen window (desktop).
    pub fullscreen: bool,
    /// Animation speed step 0..=4 (0 slowest, 4 = no animation).
    pub speed_level: u8,
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
            palette: "warm".into(),
            outfit: String::new(),
            reduce_motion: false,
            music_volume: 6,
            sfx_volume: 8,
            text_size: 1,
            fullscreen: false,
            speed_level: 1,
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
    /// Non-empty when an old save was converted on load: tell the player once, then clear it.
    pub migration_note: String,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            version: PROGRESS_VERSION,
            levels: BTreeMap::new(),
            settings: Settings::default(),
            sandbox: String::new(),
            last_level: String::new(),
            migration_note: String::new(),
        }
    }
}

impl Progress {
    /// Parse JSON; falls back to defaults on corrupt data (never panics, never loses the app).
    ///
    /// Saves older than [`PROGRESS_VERSION`] (ClerkScript era) are migrated: settings, solved flags, medals
    /// and stars are kept; everything that contained ClerkScript text (drafts, saved best solutions,
    /// sandbox) and the numbers that are not comparable with the C++ size metric (best size, best speed,
    /// measured complexity) are cleared, and [`Progress::migration_note`] says so.
    pub fn from_json(s: &str) -> Progress {
        let Ok(raw) = serde_json::from_str::<serde_json::Value>(s) else {
            return Progress::default();
        };
        let version = raw
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let Ok(mut p) = serde_json::from_value::<Progress>(raw) else {
            return Progress::default();
        };
        if version < u64::from(PROGRESS_VERSION) {
            p.migrate_old();
        }
        p
    }

    fn migrate_old(&mut self) {
        let had_work = !self.sandbox.trim().is_empty()
            || self.levels.values().any(|r| {
                !r.draft.trim().is_empty()
                    || !r.best_size_solution.is_empty()
                    || !r.best_speed_solution.is_empty()
            });
        for r in self.levels.values_mut() {
            r.draft.clear();
            r.best_size_solution.clear();
            r.best_speed_solution.clear();
            r.best_size = None;
            r.best_speed = None;
            r.complexity = None;
            r.hints_used = 0;
            r.reference_revealed = false;
            r.code_revealed = false;
        }
        self.sandbox.clear();
        self.version = PROGRESS_VERSION;
        if had_work {
            self.migration_note = MIGRATION_NOTE.to_string();
        }
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
        let size = verdict.size;
        if rec.best_size.is_none_or(|b| size < b) {
            rec.best_size = Some(size);
            rec.best_size_solution = solution_text.to_string();
            improved = true;
        }
        if rec.best_speed.is_none_or(|b| verdict.avg_cost < b) {
            rec.best_speed = Some(verdict.avg_cost);
            rec.best_speed_solution = solution_text.to_string();
            improved = true;
        }
        rec.medals.size = rec.medals.size.max(verdict.medals.size);
        rec.medals.speed = rec.medals.speed.max(verdict.medals.speed);
        rec.stars = rec.medals.stars();
        rec.complexity = verdict.complexity.clone();
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

    fn passed(size: u32, cost: f64) -> Verdict {
        Verdict {
            passed: true,
            size,
            avg_cost: cost,
            ..Verdict::default()
        }
    }

    fn fake_level() -> Level {
        let text = include_str!("../tests/data/tiny.toml");
        let mut level = Level::from_toml(text, "t").unwrap();
        level.id = "tiny".into();
        level
    }

    #[test]
    fn verdict_recording_keeps_best() {
        let level = fake_level();
        let mut p = Progress::default();
        assert!(p.record_verdict(&level, &passed(10, 50.0), "a"));
        assert!(!p.record_verdict(&level, &passed(12, 60.0), "b"));
        assert!(p.record_verdict(&level, &passed(8, 70.0), "c"));
        assert_eq!(p.record(&level.id).best_size_solution, "c");
        assert_eq!(p.record(&level.id).best_speed_solution, "a");
        let again = Progress::from_json(&p.to_json());
        assert_eq!(again, p);
    }

    #[test]
    fn current_saves_are_not_migrated() {
        let mut p = Progress::default();
        p.record_mut("x").draft = "int f() { return 1; }".into();
        let again = Progress::from_json(&p.to_json());
        assert_eq!(again.record("x").draft, "int f() { return 1; }");
        assert!(again.migration_note.is_empty());
    }

    #[test]
    fn clerkscript_era_saves_are_migrated_not_crashed_on() {
        let old = r#"{
            "version": 1,
            "levels": {
                "two-sum": {"solved": true, "best_size": 30, "best_speed": 42.0, "stars": 3,
                    "medals": {"size": "Gold", "speed": "Silver"},
                    "best_size_solution": "fn two_sum(nums: vec<int>) -> int { return 0; }",
                    "draft": "fn two_sum() {}", "hints_used": 2, "reference_revealed": true},
                "other": {"solved": false}
            },
            "settings": {"theme": "light", "palette": "contrast", "music_volume": 3},
            "sandbox": "fn main() -> int { return 1; }",
            "last_level": "two-sum"
        }"#;
        let p = Progress::from_json(old);
        assert_eq!(p.version, PROGRESS_VERSION);
        assert_eq!(p.settings.theme, "light");
        assert_eq!(p.settings.music_volume, 3);
        assert_eq!(p.last_level, "two-sum");
        let r = p.record("two-sum");
        assert!(r.solved && r.stars == 3);
        assert_eq!(r.medals.size, crate::judge::Medal::Gold);
        assert!(r.draft.is_empty() && r.best_size_solution.is_empty());
        assert_eq!((r.best_size, r.best_speed), (None, None));
        assert!(p.sandbox.is_empty());
        assert!(p.migration_note.contains("cannot be migrated"));
        // saving and loading again keeps the converted state and does not repeat the conversion
        let q = Progress::from_json(&p.to_json());
        assert_eq!(q, p);
    }

    #[test]
    fn saves_without_any_work_migrate_silently_and_garbage_gives_defaults() {
        let p = Progress::from_json("{\"version\": 1, \"levels\": {\"a\": {\"solved\": true}}}");
        assert!(p.record("a").solved);
        assert!(p.migration_note.is_empty());
        for junk in [
            "",
            "null",
            "[]",
            "{\"version\": \"x\"}",
            "{\"levels\": [1]}",
            "\u{0}",
        ] {
            assert_eq!(Progress::from_json(junk), Progress::default(), "{junk:?}");
        }
    }
}
