//! Shared infrastructure: parallel file discovery and a content-hash cache.
use anyhow::Result;
use ignore::WalkBuilder;
use ignore::WalkState;
use serde::{Deserialize, Serialize};
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
    builder.build_parallel().run(|| {
        Box::new(|entry| {
            if let Ok(e) = entry
                && e.file_type().is_some_and(|t| t.is_file())
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
pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
/// Persistent cache mapping `path -> content hash` for a given tool + config fingerprint.
///
/// A file whose hash matches the stored one was already processed cleanly and is skipped.
#[derive(Serialize, Deserialize, Default)]
pub struct Cache {
    key: String,
    entries: HashMap<String, String>,
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
    pub fn is_clean(&self, file: &Path, hash: &str) -> bool {
        self.entries
            .get(&file.to_string_lossy().into_owned())
            .is_some_and(|h| h == hash)
    }
    pub fn mark_clean(&mut self, file: &Path, hash: String) {
        self.entries
            .insert(file.to_string_lossy().into_owned(), hash);
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
    fn cache_roundtrip_and_key_invalidation() {
        let dir = std::env::temp_dir().join(format!("blaze-core-{}", std::process::id()));
        let f = Path::new("a.rs");
        let mut c = Cache::load(&dir, "t", "v1");
        c.mark_clean(f, "h".into());
        c.save().unwrap();
        assert!(Cache::load(&dir, "t", "v1").is_clean(f, "h"));
        assert!(!Cache::load(&dir, "t", "v2").is_clean(f, "h"));
        std::fs::remove_dir_all(dir).ok();
    }
}
