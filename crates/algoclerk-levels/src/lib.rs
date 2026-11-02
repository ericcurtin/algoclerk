// SPDX-License-Identifier: Apache-2.0
//! The shipped AlgoClerk campaign: level packs embedded into the binary, plus loading of extra
//! packs from a directory (desktop).
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use algoclerk_core::level::{Level, PackInfo};
use algoclerk_core::progress::Progress;
use std::path::Path;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// A wing of the office: a titled group of levels.
#[derive(Clone, Debug)]
pub struct Pack {
    /// Pack metadata.
    pub info: PackInfo,
    /// Levels in play order.
    pub levels: Vec<Level>,
}

/// All packs.
#[derive(Clone, Debug, Default)]
pub struct Campaign {
    /// Packs in play order.
    pub packs: Vec<Pack>,
    /// Problems found while loading (bad files are skipped, never fatal).
    pub load_errors: Vec<String>,
}

/// Fraction of the previous pack that must be solved to open the next one.
pub const PACK_UNLOCK_PERCENT: usize = 60;

/// The raw `(relative path, contents)` of every level file embedded in the binary.
pub fn embedded_files() -> Vec<(String, String)> {
    embedded::FILES
        .iter()
        .map(|(p, c)| ((*p).to_string(), (*c).to_string()))
        .collect()
}

impl Campaign {
    /// Load the levels shipped inside the binary.
    pub fn shipped() -> Campaign {
        Campaign::from_files(&embedded_files())
    }

    /// Build a campaign from `(relative path, contents)` pairs such as `basics/echo.toml`.
    pub fn from_files(files: &[(String, String)]) -> Campaign {
        let mut c = Campaign::default();
        c.add_files(files);
        c
    }

    /// Add files to the campaign (new packs are created as needed).
    pub fn add_files(&mut self, files: &[(String, String)]) {
        let mut infos: Vec<PackInfo> = Vec::new();
        let mut levels: Vec<Level> = Vec::new();
        for (path, text) in files {
            let mut parts = path.rsplitn(2, '/');
            let file = parts.next().unwrap_or_default();
            let dir = parts.next().unwrap_or_default();
            let stem = file.strip_suffix(".toml").unwrap_or(file);
            if dir.is_empty() {
                self.load_errors
                    .push(format!("{path}: files must live in a pack directory"));
                continue;
            }
            if stem == "pack" {
                match PackInfo::from_toml(text, dir) {
                    Ok(p) => infos.push(p),
                    Err(e) => self.load_errors.push(format!("{path}: {e}")),
                }
            } else {
                match Level::from_toml(text, dir) {
                    Ok(mut l) => {
                        l.id = stem.to_string();
                        levels.push(l);
                    }
                    Err(e) => self.load_errors.push(format!("{path}: {e}")),
                }
            }
        }
        for info in infos {
            if self.packs.iter().any(|p| p.info.id == info.id) {
                self.load_errors
                    .push(format!("pack \"{}\" defined twice", info.id));
                continue;
            }
            self.packs.push(Pack {
                info,
                levels: Vec::new(),
            });
        }
        for l in levels {
            if self.find(&l.id).is_some() {
                self.load_errors
                    .push(format!("level id \"{}\" is not unique", l.id));
                continue;
            }
            match self.packs.iter_mut().find(|p| p.info.id == l.pack) {
                Some(p) => p.levels.push(l),
                None => self.load_errors.push(format!(
                    "level \"{}\": pack \"{}\" has no pack.toml",
                    l.id, l.pack
                )),
            }
        }
        self.packs
            .sort_by(|a, b| (a.info.order, &a.info.id).cmp(&(b.info.order, &b.info.id)));
        for p in &mut self.packs {
            p.levels
                .sort_by(|a, b| (a.order, &a.id).cmp(&(b.order, &b.id)));
        }
    }

    /// Load extra packs from a user directory laid out like `levels/` (desktop only; errors are
    /// recorded in `load_errors`). Returns the number of files read.
    pub fn load_dir(&mut self, root: &Path) -> usize {
        let mut files = Vec::new();
        let Ok(packs) = std::fs::read_dir(root) else {
            return 0;
        };
        for pack in packs.filter_map(Result::ok) {
            let pdir = pack.path();
            if !pdir.is_dir() {
                continue;
            }
            let pname = pack.file_name().to_string_lossy().to_string();
            let Ok(rd) = std::fs::read_dir(&pdir) else {
                continue;
            };
            for f in rd.filter_map(Result::ok) {
                let fp = f.path();
                if fp.extension().is_some_and(|e| e == "toml") {
                    match std::fs::read_to_string(&fp) {
                        Ok(text) => files
                            .push((format!("{pname}/{}", f.file_name().to_string_lossy()), text)),
                        Err(e) => self.load_errors.push(format!("{}: {e}", fp.display())),
                    }
                }
            }
        }
        let n = files.len();
        self.add_files(&files);
        n
    }

    /// Every level in play order.
    pub fn all_levels(&self) -> impl Iterator<Item = &Level> {
        self.packs.iter().flat_map(|p| p.levels.iter())
    }

    /// Total number of levels.
    pub fn level_count(&self) -> usize {
        self.packs.iter().map(|p| p.levels.len()).sum()
    }

    /// Find a level by id.
    pub fn find(&self, id: &str) -> Option<&Level> {
        self.all_levels().find(|l| l.id == id)
    }

    /// The level after `id` in play order.
    pub fn next_after(&self, id: &str) -> Option<&Level> {
        let mut it = self.all_levels();
        it.find(|l| l.id == id)?;
        it.next()
    }

    /// Whether a pack is open for the player.
    pub fn pack_unlocked(&self, progress: &Progress, pack_idx: usize) -> bool {
        if progress.settings.unlock_all || pack_idx == 0 {
            return true;
        }
        let Some(prev) = self.packs.get(pack_idx - 1) else {
            return false;
        };
        let solved = prev
            .levels
            .iter()
            .filter(|l| progress.record(&l.id).solved)
            .count();
        solved * 100 >= prev.levels.len() * PACK_UNLOCK_PERCENT
    }

    /// Whether a level is open for the player.
    pub fn level_unlocked(&self, progress: &Progress, pack_idx: usize, level_idx: usize) -> bool {
        if progress.settings.unlock_all {
            return true;
        }
        if !self.pack_unlocked(progress, pack_idx) {
            return false;
        }
        level_idx == 0
            || self.packs[pack_idx]
                .levels
                .get(level_idx - 1)
                .is_some_and(|prev| progress.record(&prev.id).solved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(_id: &str, order: u32) -> String {
        include_str!("../tests/data/tiny.toml").replace("order = 1", &format!("order = {order}"))
    }

    #[test]
    fn loads_and_orders() {
        let files = vec![
            (
                "b/pack.toml".to_string(),
                "title=\"B\"\nblurb=\"b\"\norder=2".to_string(),
            ),
            (
                "a/pack.toml".to_string(),
                "title=\"A\"\nblurb=\"a\"\norder=1".to_string(),
            ),
            ("a/two.toml".to_string(), lvl("two", 2)),
            ("a/one.toml".to_string(), lvl("one", 1)),
            ("b/three.toml".to_string(), lvl("three", 1)),
        ];
        let c = Campaign::from_files(&files);
        assert!(c.load_errors.is_empty(), "{:?}", c.load_errors);
        let ids: Vec<_> = c.all_levels().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["one", "two", "three"]);
        assert_eq!(c.next_after("two").unwrap().id, "three");
        assert!(c.next_after("three").is_none());
    }

    #[test]
    fn reports_bad_files_without_failing() {
        let files = vec![
            (
                "a/pack.toml".to_string(),
                "title=\"A\"\nblurb=\"a\"\norder=1".to_string(),
            ),
            ("a/z.toml".to_string(), "garbage".to_string()),
            ("lonely.toml".to_string(), "x".to_string()),
            ("nopack/q.toml".to_string(), lvl("q", 1)),
        ];
        let c = Campaign::from_files(&files);
        assert_eq!(c.load_errors.len(), 3, "{:?}", c.load_errors);
        assert_eq!(c.level_count(), 0);
    }

    #[test]
    fn unlock_rules() {
        let files = vec![
            (
                "a/pack.toml".to_string(),
                "title=\"A\"\nblurb=\"a\"\norder=1".to_string(),
            ),
            (
                "b/pack.toml".to_string(),
                "title=\"B\"\nblurb=\"b\"\norder=2".to_string(),
            ),
            ("a/one.toml".to_string(), lvl("one", 1)),
            ("a/two.toml".to_string(), lvl("two", 2)),
            ("b/three.toml".to_string(), lvl("three", 1)),
        ];
        let c = Campaign::from_files(&files);
        let mut p = Progress::default();
        assert!(c.level_unlocked(&p, 0, 0));
        assert!(!c.level_unlocked(&p, 0, 1));
        assert!(!c.pack_unlocked(&p, 1));
        p.record_mut("one").solved = true;
        assert!(c.level_unlocked(&p, 0, 1));
        assert!(
            !c.pack_unlocked(&p, 1),
            "one of two levels is below the 60% threshold"
        );
        p.record_mut("two").solved = true;
        assert!(c.pack_unlocked(&p, 1));
        p.settings.unlock_all = true;
        assert!(c.level_unlocked(&p, 1, 0));
    }

    #[test]
    fn extra_packs_load_from_a_directory() {
        let dir =
            std::env::temp_dir().join(format!("algoclerk-extra-packs-{}", std::process::id()));
        let pack = dir.join("community");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(
            pack.join("pack.toml"),
            "title=\"Community\"\nblurb=\"x\"\norder=99",
        )
        .unwrap();
        std::fs::write(pack.join("extra-one.toml"), lvl("extra-one", 1)).unwrap();
        std::fs::write(pack.join("broken.toml"), "this is not toml").unwrap();
        let mut c = Campaign::shipped();
        let before = c.level_count();
        assert_eq!(c.load_dir(&dir), 3);
        assert_eq!(c.level_count(), before + 1);
        assert!(c.find("extra-one").is_some());
        assert!(c.load_errors.iter().any(|e| e.contains("broken.toml")));
        assert_eq!(c.packs.last().unwrap().info.id, "community");
        // A missing directory is not an error.
        assert_eq!(Campaign::shipped().load_dir(&dir.join("nope")), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Number of levels the finished campaign has (the old ClerkScript files are skipped by the loader
    /// until each is converted to the C++ level format).
    const FULL_CAMPAIGN: usize = 80;

    #[test]
    fn shipped_campaign_loads_cleanly() {
        let c = Campaign::shipped();
        if c.level_count() >= FULL_CAMPAIGN {
            assert!(c.load_errors.is_empty(), "{:?}", c.load_errors);
            return;
        }
        // Partially converted: the only acceptable errors are old-format files (`builtins` / `types`).
        let unexpected: Vec<String> = c
            .load_errors
            .iter()
            .filter(|e| {
                !(e.contains("unknown field `builtins`") || e.contains("unknown field `types`"))
            })
            .map(|e| e.chars().take(300).collect())
            .collect();
        assert!(unexpected.is_empty(), "{unexpected:#?}");
        eprintln!(
            "note: {} of {FULL_CAMPAIGN} levels converted, {} old-format files skipped",
            c.level_count(),
            c.load_errors.len()
        );
    }
}
