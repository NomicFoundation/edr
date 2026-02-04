use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result};
// use dirs::cache_dir;
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
    pub fn new(ttl_seconds: u64) -> Result<Self> {
        let mut root = PathBuf::from(edr_defaults::CACHE_DIR);
        root.push("cargo-cooldown");
        fs::create_dir_all(&root)
            .with_context(|| format!("failed to create cache directory {}", root.display()))?;
        Ok(Self {
            root,
            ttl: Duration::from_secs(ttl_seconds),
        })
    }

    pub fn with_root(root: PathBuf, ttl: Duration) -> Result<Self> {
        if !root.exists() {
            fs::create_dir_all(&root)
                .with_context(|| format!("failed to create cache directory {}", root.display()))?;
        }
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
        let entry: CacheEntry<T> = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse cache entry {}", path.display()))?;
        let now = current_epoch();
        if now.saturating_sub(entry.fetched_at) >= self.ttl.as_secs() {
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
        .unwrap_or_else(|_| Duration::from_secs(0))
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

        let expired = Cache::with_root(dir.path().to_path_buf(), Duration::from_secs(0)).unwrap();
        let value: Option<String> = expired.get("foo/bar").unwrap();
        assert!(value.is_none());
    }
}
