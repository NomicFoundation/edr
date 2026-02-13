use std::{collections::HashMap, fs};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::workspace::{config_file_path, ALLOWLIST_FILE_CONFIG};

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
    pub fn load() -> Result<Self> {
        let path = config_file_path(ALLOWLIST_FILE_CONFIG)?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read allowlist at {}", path.as_path().display()))?;
        let allowlist: Allowlist = toml::from_str(&contents).with_context(|| {
            format!("failed to parse allowlist at {}", path.as_path().display())
        })?;
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
}

impl AllowPackage {
    pub fn effective_minutes(&self) -> Option<u64> {
        self.minutes
    }
}

#[cfg(test)]
mod tests {
    // use std::io::Write;

    // use tempfile::NamedTempFile;

    // use super::*;

    // #[test]
    // fn loads_allowlist_and_respects_exact() {
    //     let mut file = NamedTempFile::new().unwrap();
    //     writeln!(
    //         file,
    //         "[[allow.exact]]\ncrate = \"foo\"\nversion =
    // \"1.2.3\"\n[[allow.package]]\ncrate = \"bar\"\nminimum_release_age =
    // 3\n[allow.global]\nminutes = 5\n"     )
    //     .unwrap();

    //     let allowlist = Allowlist::load(file.path()).unwrap();
    //     assert!(allowlist.is_exact_allowed("foo", "1.2.3"));
    //     assert!(!allowlist.is_exact_allowed("foo", "1.2.4"));

    //     let per_crate = allowlist.per_crate_minutes();
    //     assert_eq!(per_crate.get("bar"), Some(&3));
    //     assert_eq!(allowlist.global_minutes(), Some(5));
    //     assert_eq!(allowlist.effective_minutes_for("bar", 7), 3);
    //     assert_eq!(allowlist.effective_minutes_for("baz", 7), 5);
    // }
}
