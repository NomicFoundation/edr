//! Defines types for Solidity compilation artifacts.
#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use alloy_json_abi::JsonAbi;
use alloy_primitives::Bytes;
use semver::Version;

// Adapted from <https://github.com/foundry-rs/compilers/blob/ea346377deaf18dc1f972a06fad76df3d9aed8d9/crates/compilers/src/artifact_output/mod.rs#L45>
/// Compilation artifact identifier
#[derive(
    Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct ArtifactId {
    /// The name of the contract
    pub name: String,
    /// Original source file path
    pub source: PathBuf,
    /// `solc` version that produced this artifact
    pub version: Version,
}

// Copied from <https://github.com/foundry-rs/compilers/blob/ea346377deaf18dc1f972a06fad76df3d9aed8d9/crates/compilers/src/artifact_output/mod.rs#L45>
impl ArtifactId {
    /// Returns a `<source path>:<name>` slug that uniquely identifies an
    /// artifact
    pub fn identifier(&self) -> String {
        format!("{}:{}", self.source.to_string_lossy(), self.name)
    }

    /// Removes `base` from the source's path.
    pub fn strip_file_prefixes(&mut self, base: &Path) {
        if let Ok(stripped) = self.source.strip_prefix(base) {
            self.source = stripped.to_path_buf();
        }
    }

    /// Convenience function for [`Self::strip_file_prefixes()`]
    pub fn with_stripped_file_prefixes(mut self, base: &Path) -> Self {
        self.strip_file_prefixes(base);
        self
    }
}

impl From<foundry_compilers::ArtifactId> for ArtifactId {
    fn from(value: foundry_compilers::ArtifactId) -> Self {
        let foundry_compilers::ArtifactId {
            path: _,
            name,
            source,
            version,
            build_id: _,
            profile: _,
        } = value;

        Self {
            name,
            source,
            version,
        }
    }
}

/// Container for commonly used contract data.
#[derive(Debug, Clone)]
pub struct ContractData {
    /// Contract ABI.
    pub abi: JsonAbi,
    /// Contract creation code.
    pub bytecode: Option<Bytes>,
    /// Contract runtime code.
    pub deployed_bytecode: Option<Bytes>,
}
