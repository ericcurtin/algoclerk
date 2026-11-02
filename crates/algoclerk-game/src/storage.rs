// SPDX-License-Identifier: Apache-2.0
//! Persistent storage for progress and settings. Adapted from Falabichos (Apache-2.0, same owner).

use std::collections::HashMap;
use std::path::PathBuf;

/// A tiny named-blob store.
pub trait Storage: Send + Sync {
    /// Read a blob.
    fn read(&self, name: &str) -> Option<String>;
    /// Write a blob.
    ///
    /// # Errors
    /// A readable message when the write fails.
    fn write(&mut self, name: &str, data: &str) -> Result<(), String>;
    /// Remove a blob.
    fn remove(&mut self, name: &str);
}

/// In-memory storage for tests and headless runs.
#[derive(Debug, Default, Clone)]
pub struct MemoryStorage {
    /// The blobs.
    pub files: HashMap<String, String>,
}

impl Storage for MemoryStorage {
    fn read(&self, name: &str) -> Option<String> {
        self.files.get(name).cloned()
    }
    fn write(&mut self, name: &str, data: &str) -> Result<(), String> {
        self.files.insert(name.to_string(), data.to_string());
        Ok(())
    }
    fn remove(&mut self, name: &str) {
        self.files.remove(name);
    }
}

/// Files in a directory. Writes go to a temp file and are renamed into place and the previous version is
/// kept as `<name>.bak`, so a crash mid-write cannot lose progress.
#[derive(Debug, Clone)]
pub struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    /// Store files in `dir` (created on first write).
    pub fn new(dir: PathBuf) -> FileStorage {
        FileStorage { dir }
    }
    /// The directory.
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }
}

impl Storage for FileStorage {
    fn read(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.join(name)).ok()
    }
    fn write(&mut self, name: &str, data: &str) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("create {}: {e}", self.dir.display()))?;
        let target = self.dir.join(name);
        let tmp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, data).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        if target.exists() {
            let _ = std::fs::copy(&target, self.dir.join(format!("{name}.bak")));
        }
        std::fs::rename(&tmp, &target).map_err(|e| format!("rename: {e}"))
    }
    fn remove(&mut self, name: &str) {
        let _ = std::fs::remove_file(self.dir.join(name));
    }
}

/// Per-platform default data directory, using only the standard library.
///
/// * Linux/BSD: `$XDG_DATA_HOME/algoclerk` or `~/.local/share/algoclerk`
/// * macOS and iOS: `~/Library/Application Support/AlgoClerk`
/// * Windows: `%APPDATA%\AlgoClerk`
/// * Android: the shell must pass the app's internal data path
///
/// `ALGOCLERK_DATA_DIR` overrides everything.
pub fn default_data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("ALGOCLERK_DATA_DIR") {
        return PathBuf::from(d);
    }
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if cfg!(target_os = "windows") {
        if let Some(a) = var("APPDATA") {
            return PathBuf::from(a).join("AlgoClerk");
        }
    } else if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        if let Some(h) = var("HOME") {
            return PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("AlgoClerk");
        }
    } else {
        if let Some(x) = var("XDG_DATA_HOME") {
            return PathBuf::from(x).join("algoclerk");
        }
        if let Some(h) = var("HOME") {
            return PathBuf::from(h).join(".local").join("share").join("algoclerk");
        }
    }
    PathBuf::from(".algoclerk-data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_storage_roundtrip_and_backup() {
        let dir = std::env::temp_dir().join(format!("algoclerk-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = FileStorage::new(dir.clone());
        assert!(s.read("a.json").is_none());
        s.write("a.json", "one").unwrap();
        s.write("a.json", "two").unwrap();
        assert_eq!(s.read("a.json").as_deref(), Some("two"));
        assert_eq!(s.read("a.json.bak").as_deref(), Some("one"));
        s.remove("a.json");
        assert!(s.read("a.json").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn memory_storage() {
        let mut m = MemoryStorage::default();
        m.write("x", "y").unwrap();
        assert_eq!(m.read("x").as_deref(), Some("y"));
    }
}
