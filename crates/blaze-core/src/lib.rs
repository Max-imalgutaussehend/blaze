//! Shared infrastructure: parallel file discovery and a content-hash cache.
use anyhow::Result;
use ignore::WalkBuilder;
use ignore::WalkState;
use serde::{Deserialize, Serialize};
pub use blaze_lexer as lexer;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
/// Collect all `.rs` files under `roots`, honouring `.gitignore` and skipping `target/`.
pub fn discover_rust_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let Some((first, rest)) = roots.split_first() else {
        return Vec::new();
    };
    let mut builder = WalkBuilder::new(first);
    for r in rest {
        builder.add(r);
    }
    builder.filter_entry(|e| e.file_name() != "target");
    let found = Mutex::new(Vec::new());
    builder
        .build_parallel()
        .run(|| {
            Box::new(|entry| {
                if let Ok(e) = entry && e.file_type().is_some_and(|t| t.is_file())
                    && e.path().extension().is_some_and(|x| x == "rs")
                {
                    found.lock().unwrap().push(e.into_path()); // blaze:ignore
                }
                WalkState::Continue
            })
        });
    let mut files = found.into_inner().unwrap(); // blaze:ignore
    files.sort();
    files
}
/// Read `[section] key = ["a", "b"]` from `blaze.toml` in `root`; empty when absent.
pub fn config_strings(root: &Path, section: &str, key: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(root.join("blaze.toml")) else {
        return Vec::new();
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    table
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_array())
        .map(|a| { a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect() })
        .unwrap_or_default()
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Cheap file identity: modification time and length. Taken *before* a file is read, so a
/// concurrent edit can only make the cache entry stale (re-checked), never wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stamp {
    mtime: u64,
    len: u64,
}

impl Stamp {
    pub fn of(path: &Path) -> Option<Stamp> {
        let m = std::fs::metadata(path).ok()?;
        let mtime = m
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos() as u64;
        Some(Stamp { mtime, len: m.len() })
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    m: u64,
    l: u64,
    h: String,
    t: String,
}

/// Persistent cache mapping `path -> (stamp, content hash, status tag)`.
///
/// `quick` answers from a single `stat` (no file read); `lookup` falls back to the content
/// hash, so touched-but-unchanged files are still recognised.
#[derive(Serialize, Deserialize, Default)]
pub struct Cache {
    key: String,
    entries: HashMap<String, Entry>,
    #[serde(skip)]
    path: PathBuf,
}

impl Cache {
    /// `key` fingerprints tool version + config; a changed key invalidates everything.
    pub fn load(dir: &Path, name: &str, key: &str) -> Self {
        let path = dir.join(".blaze_cache").join(format!("{name}.json"));
        let mut cache: Cache = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if cache.key != key {
            cache = Cache {
                key: key.to_string(),
                ..Default::default()
            };
        }
        cache.path = path;
        cache
    }

    /// Status tag if the file's stamp is unchanged since it was recorded.
    pub fn quick(&self, file: &Path, stamp: Stamp) -> Option<&str> {
        let e = self.entries.get(file.to_string_lossy().as_ref())?;
        (e.m == stamp.mtime && e.l == stamp.len).then_some(e.t.as_str())
    }

    /// Status tag recorded for `file` if its content still hashes to `hash`.
    pub fn lookup(&self, file: &Path, hash: &str) -> Option<&str> {
        let e = self.entries.get(file.to_string_lossy().as_ref())?;
        (e.h == hash).then_some(e.t.as_str())
    }

    /// Record a result. Without a stamp the file is stat'ed now.
    pub fn mark_stamped(
        &mut self,
        file: &Path,
        stamp: Option<Stamp>,
        hash: &str,
        tag: &str,
    ) {
        let (m, l) = stamp
            .or_else(|| Stamp::of(file))
            .map_or((0, 0), |s| (s.mtime, s.len));
        self.entries
            .insert(
                file.to_string_lossy().into_owned(),
                Entry {
                    m,
                    l,
                    h: hash.to_string(),
                    t: tag.to_string(),
                },
            );
    }

    pub fn mark(&mut self, file: &Path, hash: &str, tag: &str) {
        self.mark_stamped(file, None, hash, tag);
    }

    pub fn is_clean(&self, file: &Path, hash: &str) -> bool {
        self.lookup(file, hash) == Some("clean")
    }

    pub fn mark_clean(&mut self, file: &Path, hash: String) {
        self.mark(file, &hash, "clean");
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            let ignore = parent.join(".gitignore");
            if !ignore.exists() {
                std::fs::write(ignore, "*\n")?;
            }
        }
        std::fs::write(&self.path, serde_json::to_vec(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_config_lists() {
        let dir = std::env::temp_dir()
            .join(format!("blaze-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
                dir.join("blaze.toml"),
                "[ember]\nignore = [\"B001\", \"B006\"]\n",
            )
            .unwrap();
        assert_eq!(config_strings(&dir, "ember", "ignore"), ["B001", "B006"]);
        assert!(config_strings(&dir, "ember", "nope").is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn cache_roundtrip_and_key_invalidation() {
        let dir = std::env::temp_dir()
            .join(format!("blaze-core-{}", std::process::id()));
        let f = Path::new("a.rs");
        let mut c = Cache::load(&dir, "t", "v1");
        c.mark_clean(f, "h".into());
        c.save().unwrap();
        assert!(Cache::load(&dir, "t", "v1").is_clean(f, "h"));
        assert!(!Cache::load(&dir, "t", "v2").is_clean(f, "h"));
        std::fs::remove_dir_all(dir).ok();
    }
}
