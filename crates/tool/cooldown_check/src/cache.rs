use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    fetched_at: u64,
    value: T,
}

#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
    ttl: Duration,
}

impl Cache {
    pub fn new(ttl: Duration) -> Result<Self> {
        let mut root = PathBuf::from(edr_defaults::CACHE_DIR);
        root.push("cargo-cooldown-check");
        Self::with_root(root, ttl)
    }

    pub fn with_root(root: PathBuf, ttl: Duration) -> Result<Self> {
        fs::create_dir_all(&root)
            .with_context(|| format!("failed to create cache directory {}", root.display()))?;
        log::debug!("Cache path: {}", root.display());
        log::debug!("Cache ttl: {ttl:?}");
        Ok(Self { root, ttl })
    }

    fn path_for(&self, key: &str) -> PathBuf {
        let mut path = self.root.clone();
        for segment in key.split('/') {
            let sanitized = segment
                .chars()
                .map(|c| match c {
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '@' => c,
                    _ => '_',
                })
                .collect::<String>();
            path.push(sanitized);
        }
        path
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let path = self.path_for(key);
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read cache entry {}", path.display()))?;
        let entry: CacheEntry<T> = match serde_json::from_str(&contents) {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!("Corrupted cache entry {}: {e}, removing", path.display());
                let _ = fs::remove_file(&path);
                return Ok(None);
            }
        };
        let now = current_epoch();
        if now.saturating_sub(entry.fetched_at) >= self.ttl.as_secs() {
            let _ = fs::remove_file(&path);
            return Ok(None);
        }
        Ok(Some(entry.value))
    }

    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create cache parent {}", parent.display()))?;
        }
        let entry = CacheEntry {
            fetched_at: current_epoch(),
            value,
        };
        let serialized = serde_json::to_string(&entry)?;
        fs::write(&path, serialized)
            .with_context(|| format!("failed to write cache entry {}", path.display()))?;
        Ok(())
    }
}

fn current_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock is before the UNIX epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn cache_roundtrip_and_ttl() {
        let dir = tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf(), Duration::from_secs(3_600)).unwrap();
        cache.put("foo/bar", &"hello").unwrap();
        let value: Option<String> = cache.get("foo/bar").unwrap();
        assert_eq!(value.unwrap(), "hello");
    }

    #[test]
    fn expired_entry_is_deleted_from_disk() {
        let dir = tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf(), Duration::from_secs(3_600)).unwrap();
        cache.put("foo/bar", &"hello").unwrap();

        let file_path = cache.path_for("foo/bar");
        assert!(file_path.exists(), "cache file should exist after put");

        let expired = Cache::with_root(dir.path().to_path_buf(), Duration::from_secs(0)).unwrap();
        let value: Option<String> = expired.get("foo/bar").unwrap();
        assert!(value.is_none());
        assert!(!file_path.exists(), "expired cache file should be deleted from disk");
    }

    #[test]
    fn corrupted_entry_is_discarded_and_deleted() {
        let dir = tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf(), Duration::from_secs(3_600)).unwrap();
        let file_path = cache.path_for("foo/bar");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "not valid json{{{").unwrap();

        let value: Option<String> = cache.get("foo/bar").unwrap();
        assert!(value.is_none());
        assert!(!file_path.exists(), "corrupted cache file should be deleted from disk");
    }

    #[test]
    fn get_missing_key_returns_none() {
        let dir = tempdir().unwrap();
        let cache = Cache::with_root(dir.path().to_path_buf(), Duration::from_secs(3_600)).unwrap();
        let value: Option<String> = cache.get("nonexistent/key").unwrap();
        assert!(value.is_none());
    }
}
