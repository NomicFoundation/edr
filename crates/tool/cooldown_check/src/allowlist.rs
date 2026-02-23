use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Allowlist {
    #[serde(default)]
    pub allow: AllowSection,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AllowSection {
    #[serde(default)]
    pub exact: Vec<AllowExact>,
    #[serde(default)]
    pub package: Vec<AllowPackage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AllowExact {
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub version: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AllowPackage {
    #[serde(rename = "crate")]
    pub crate_name: String,
    #[serde(default)]
    pub minutes: Option<u64>,
}

impl Allowlist {
    pub fn load(file_path: &Path) -> Result<Self> {
        if !file_path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(file_path)
            .with_context(|| format!("failed to read allowlist at {}", file_path.display()))?;
        let allowlist: Allowlist = toml::from_str(&contents)
            .with_context(|| format!("failed to parse allowlist at {}", file_path.display()))?;

        if !allowlist.is_empty() {
            log::info!(
                "allowlist configuration:\n\t\
allows.exact: {:?}\n\t\
allows.package {:?}",
                allowlist.allow.exact,
                allowlist.allow.package
            );
        }
        Ok(allowlist)
    }

    pub fn is_exact_allowed(&self, name: &str, version: &str) -> bool {
        self.allow
            .exact
            .iter()
            .any(|entry| entry.crate_name == name && entry.version == version)
    }

    pub fn per_crate_minutes(&self) -> HashMap<String, u64> {
        self.allow
            .package
            .iter()
            .filter_map(|pkg| pkg.effective_minutes().map(|m| (pkg.crate_name.clone(), m)))
            .collect()
    }

    fn is_empty(&self) -> bool {
        self.allow.exact.is_empty() && self.allow.package.is_empty()
    }
}

impl AllowPackage {
    pub fn effective_minutes(&self) -> Option<u64> {
        self.minutes
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn loads_allowlist_and_respects_exact() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "[[allow.exact]]\ncrate = \"foo\"\nversion = \"1.2.3\""
        )
        .unwrap();

        let allowlist = Allowlist::load(file.path()).unwrap();
        assert!(allowlist.is_exact_allowed("foo", "1.2.3"));
        assert!(!allowlist.is_exact_allowed("foo", "1.2.4"));

        let per_crate = allowlist.per_crate_minutes();
        assert!(per_crate.is_empty());
    }

    #[test]
    fn loads_allowlist_and_respects_package() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "[[allow.package]]\ncrate = \"tokio\"\nminutes = 1440").unwrap();

        let allowlist = Allowlist::load(file.path()).unwrap();
        let per_crate = allowlist.per_crate_minutes();
        assert_eq!(per_crate.get("tokio"), Some(&1440));
        assert_eq!(per_crate.get("serde"), None);
    }
}
