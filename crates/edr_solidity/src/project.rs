//! Loading Hardhat v3 compilation artifacts and build infos from an
//! artifacts directory on disk.
//!
//! This mirrors the file layout maintained by Hardhat v3's artifact manager:
//!
//! - `artifacts/<source name>/<ContractName>.json` — one artifact per contract
//!   in the `hh3-artifact-1` format
//! - `artifacts/build-info/<build info id>.json` — compiler input
//! - `artifacts/build-info/<build info id>.output.json` — compiler output

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

use alloy_json_abi::JsonAbi;
use alloy_primitives::hex::FromHexError;
use edr_artifact::ArtifactId;
use foundry_compilers::artifacts::{
    BytecodeObject, CompactBytecode, CompactContractBytecode, CompactDeployedBytecode, Offsets,
};
use semver::Version;
use serde::Deserialize;

use crate::artifacts::{
    BuildInfoBufferSeparateOutput, BuildInfoBuffers, BuildInfoConfig, BuildInfoConfigWithBuffers,
    SplitCompilerMetadataParseError,
};

/// Name of the build info directory inside an artifacts directory.
pub const BUILD_INFO_DIR_NAME: &str = "build-info";

/// The `_format` value of Hardhat v3 artifact files.
pub const HH3_ARTIFACT_FORMAT: &str = "hh3-artifact-1";

/// Error that occurs when loading artifacts or build infos from disk.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactLoadError {
    /// A file or directory could not be read.
    #[error("Failed to read '{path}': {error}")]
    Io {
        /// The path that failed to be read.
        path: PathBuf,
        /// The underlying IO error.
        error: std::io::Error,
    },
    /// An artifact file could not be parsed as JSON.
    #[error("Failed to parse artifact '{path}': {error}")]
    InvalidJson {
        /// The path of the artifact file.
        path: PathBuf,
        /// The underlying JSON error.
        error: serde_json::Error,
    },
    /// An artifact file has an unsupported `_format` value.
    #[error(
        "Unsupported artifact format '{format}' for artifact '{path}'. Expected '{HH3_ARTIFACT_FORMAT}'."
    )]
    UnsupportedFormat {
        /// The path of the artifact file.
        path: PathBuf,
        /// The `_format` value found in the artifact file.
        format: String,
    },
    /// An artifact file is missing the `buildInfoId` field.
    #[error("Artifact '{path}' is missing the 'buildInfoId' field")]
    MissingBuildInfoId {
        /// The path of the artifact file.
        path: PathBuf,
    },
    /// An artifact references a build info id from which no solc version can
    /// be derived.
    #[error(
        "Artifact '{path}' references build info id '{build_info_id}' which doesn't match the expected format 'solc-<major>_<minor>_<patch>[-<compilerType>]-<hex>'"
    )]
    InvalidBuildInfoId {
        /// The path of the artifact file.
        path: PathBuf,
        /// The build info id that could not be parsed.
        build_info_id: String,
    },
    /// A bytecode field of an artifact could not be hex-decoded.
    #[error("Hex decoding error while parsing bytecode of artifact '{path}': '{error}'.")]
    InvalidBytecode {
        /// The path of the artifact file.
        path: PathBuf,
        /// The underlying hex decoding error.
        error: FromHexError,
    },
}

/// Library link references as stored in Hardhat v3 artifact files: source
/// name → library name → byte offsets.
///
/// Uses ordered maps because the order can matter downstream for
/// deterministic library address generation.
pub type LinkReferences = BTreeMap<String, BTreeMap<String, Vec<Offsets>>>;

/// An artifact file in the `hh3-artifact-1` format.
///
/// The source of truth for this format is the `Artifact` interface in
/// Hardhat's `types/artifacts.ts`; only the fields consumed by EDR are
/// declared here.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardhatV3Artifact {
    /// The format of the artifact file. Expected to be
    /// [`HH3_ARTIFACT_FORMAT`].
    #[serde(rename = "_format")]
    pub format: String,
    /// The bare name of the contract, without the source name.
    pub contract_name: String,
    /// The user-facing source name: the project-relative path of the
    /// Solidity file, or the npm module identifier (`<package>/<file>`) for
    /// contracts from npm packages.
    pub source_name: String,
    /// The source name used in the compiler input. May differ from
    /// `source_name` and is always present for artifacts compiled by Hardhat
    /// 3's build system.
    pub input_source_name: Option<String>,
    /// The contract's ABI.
    pub abi: JsonAbi,
    /// The contract's creation bytecode as a hex string. Contains `__$...$__`
    /// placeholders when the contract requires linking.
    pub bytecode: Option<String>,
    /// The link references of the creation bytecode.
    #[serde(default)]
    pub link_references: LinkReferences,
    /// The contract's runtime bytecode as a hex string. Contains `__$...$__`
    /// placeholders when the contract requires linking.
    pub deployed_bytecode: Option<String>,
    /// The link references of the runtime bytecode.
    #[serde(default)]
    pub deployed_link_references: LinkReferences,
    /// The id of the build info that produced this artifact. May be absent
    /// if the artifact wasn't generated by Hardhat's build system, in which
    /// case loading fails.
    pub build_info_id: Option<String>,
}

/// An artifact loaded from a Hardhat v3 artifacts directory.
#[derive(Clone, Debug)]
pub struct LoadedArtifact {
    /// The artifact's identifier. Its `source` is the input source name when
    /// present, falling back to the user-facing source name.
    pub id: ArtifactId,
    /// The user-facing source name: the project-relative path of the
    /// Solidity file, or the npm module identifier (`<package>/<file>`) for
    /// contracts from npm packages.
    pub user_source_name: String,
    /// The contract's ABI and bytecodes.
    pub contract: CompactContractBytecode,
}

/// Loads every artifact under `artifacts_dir`.
///
/// Any `*.json` file in a subdirectory of `artifacts_dir` is treated as an
/// artifact, excluding files directly at the top level and everything under the
/// top-level [`BUILD_INFO_DIR_NAME`] directory. A missing directory yields an
/// empty list.
pub fn load_artifacts(artifacts_dir: &Path) -> Result<Vec<LoadedArtifact>, ArtifactLoadError> {
    // Cache of build info id → solc version, as multiple artifacts usually
    // share a build info.
    let mut solc_versions: HashMap<String, Option<Version>> = HashMap::new();

    find_artifact_files(artifacts_dir)?
        .into_iter()
        .map(|path| {
            let contents = std::fs::read(&path).map_err(|error| ArtifactLoadError::Io {
                path: path.clone(),
                error,
            })?;

            let artifact: HardhatV3Artifact =
                serde_json::from_slice(&contents).map_err(|error| {
                    ArtifactLoadError::InvalidJson {
                        path: path.clone(),
                        error,
                    }
                })?;

            if artifact.format != HH3_ARTIFACT_FORMAT {
                return Err(ArtifactLoadError::UnsupportedFormat {
                    path,
                    format: artifact.format,
                });
            }

            let build_info_id = artifact
                .build_info_id
                .as_deref()
                .ok_or_else(|| ArtifactLoadError::MissingBuildInfoId { path: path.clone() })?;

            let solc_version = solc_versions
                .entry(build_info_id.to_string())
                .or_insert_with(|| solc_version_from_build_info_id(build_info_id))
                .clone()
                .ok_or_else(|| ArtifactLoadError::InvalidBuildInfoId {
                    path: path.clone(),
                    build_info_id: build_info_id.to_string(),
                })?;

            to_loaded_artifact(&path, artifact, solc_version)
        })
        .collect()
}

/// Returns the paths of all artifact files under `artifacts_dir`, sorted for
/// determinism.
fn find_artifact_files(artifacts_dir: &Path) -> Result<Vec<PathBuf>, ArtifactLoadError> {
    if !artifacts_dir.is_dir() {
        return Ok(Vec::new());
    }

    let build_info_dir = artifacts_dir.join(BUILD_INFO_DIR_NAME);

    let mut paths = walkdir::WalkDir::new(artifacts_dir)
        .into_iter()
        .filter_entry(|entry| entry.path() != build_info_dir)
        .map(|entry| {
            entry.map_err(|error| {
                let path = error.path().unwrap_or(artifacts_dir).to_path_buf();
                ArtifactLoadError::Io {
                    path,
                    error: error
                        .into_io_error()
                        .unwrap_or_else(|| std::io::Error::other("file system loop detected")),
                }
            })
        })
        .filter(|entry| {
            entry.as_ref().is_ok_and(|entry| {
                // Depth 1 entries are top-level files, which are ignored.
                entry.depth() >= 2
                    && entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "json")
            })
        })
        .map(|entry| entry.map(walkdir::DirEntry::into_path))
        .collect::<Result<Vec<_>, _>>()?;

    paths.sort();

    Ok(paths)
}

/// Extracts the solc version from a Hardhat v3 build info id of the form
/// `solc-<major>_<minor>_<patch>[-<compilerType>]-<hex>`.
///
/// Returns `None` when the id doesn't match this format, e.g. because the
/// build info was generated by something other than Hardhat.
pub fn solc_version_from_build_info_id(build_info_id: &str) -> Option<Version> {
    let rest = build_info_id.strip_prefix("solc-")?;

    let parts: Vec<&str> = rest.split('-').collect();
    let (version, compiler_type, hash) = match parts.as_slice() {
        [version, hash] => (version, None, hash),
        [version, compiler_type, hash] => (version, Some(compiler_type), hash),
        _ => return None,
    };

    // The compiler type must be alphanumeric and start with a letter.
    if let Some(compiler_type) = compiler_type {
        let mut chars = compiler_type.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            || !chars.all(|c| c.is_ascii_alphanumeric())
        {
            return None;
        }
    }

    // The hash may be empty but must be hexadecimal.
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let mut version_parts = version.split('_');
    let major = version_parts.next()?.parse().ok()?;
    let minor = version_parts.next()?.parse().ok()?;
    let patch = version_parts.next()?.parse().ok()?;
    if version_parts.next().is_some() {
        return None;
    }

    Some(Version::new(major, minor, patch))
}

/// Converts a parsed artifact file into a [`LoadedArtifact`].
fn to_loaded_artifact(
    path: &Path,
    artifact: HardhatV3Artifact,
    solc_version: Version,
) -> Result<LoadedArtifact, ArtifactLoadError> {
    let id = ArtifactId {
        name: artifact.contract_name,
        source: artifact
            .input_source_name
            .as_ref()
            .unwrap_or(&artifact.source_name)
            .into(),
        version: solc_version,
    };

    let bytecode = artifact
        .bytecode
        .map(|bytecode| {
            let object = to_bytecode_object(path, bytecode, !artifact.link_references.is_empty())?;
            Ok::<_, ArtifactLoadError>(CompactBytecode {
                object,
                source_map: None,
                link_references: artifact.link_references,
            })
        })
        .transpose()?;

    let deployed_bytecode = artifact
        .deployed_bytecode
        .map(|deployed_bytecode| {
            let object = to_bytecode_object(
                path,
                deployed_bytecode,
                !artifact.deployed_link_references.is_empty(),
            )?;
            Ok::<_, ArtifactLoadError>(CompactDeployedBytecode {
                bytecode: Some(CompactBytecode {
                    object,
                    source_map: None,
                    link_references: artifact.deployed_link_references,
                }),
                immutable_references: BTreeMap::default(),
            })
        })
        .transpose()?;

    Ok(LoadedArtifact {
        id,
        user_source_name: artifact.source_name,
        contract: CompactContractBytecode {
            abi: Some(artifact.abi),
            bytecode,
            deployed_bytecode,
        },
    })
}

/// Converts a hex bytecode string into a [`BytecodeObject`]. Bytecode that
/// needs linking is kept as-is, including its `__$...$__` placeholders.
fn to_bytecode_object(
    path: &Path,
    bytecode: String,
    needs_linking: bool,
) -> Result<BytecodeObject, ArtifactLoadError> {
    if needs_linking {
        Ok(BytecodeObject::Unlinked(bytecode))
    } else {
        bytecode
            .parse()
            .map(BytecodeObject::Bytecode)
            .map_err(|error| ArtifactLoadError::InvalidBytecode {
                path: path.to_path_buf(),
                error,
            })
    }
}

/// The file paths of a single build info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildInfoFiles {
    /// The build info id, i.e. the file stem of the build info file.
    pub id: String,
    /// The path of the build info file containing the compiler input.
    pub build_info_path: PathBuf,
    /// The path of the file containing the compiler output, if it exists.
    pub output_path: Option<PathBuf>,
}

/// Returns the default build info directory for an artifacts directory.
pub fn default_build_info_dir(artifacts_dir: &Path) -> PathBuf {
    artifacts_dir.join(BUILD_INFO_DIR_NAME)
}

/// Finds all build infos in `build_info_dir`, pairing each `<id>.json` with
/// its `<id>.output.json` when present.
///
/// The directory is expected to be flat. A missing directory yields an empty
/// list. Results are sorted by id for determinism.
pub fn find_build_info_files(
    build_info_dir: &Path,
) -> Result<Vec<BuildInfoFiles>, ArtifactLoadError> {
    if !build_info_dir.is_dir() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(build_info_dir).map_err(|error| ArtifactLoadError::Io {
        path: build_info_dir.to_path_buf(),
        error,
    })?;

    let mut build_infos = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| ArtifactLoadError::Io {
            path: build_info_dir.to_path_buf(),
            error,
        })?;

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };

        let Some(id) = file_name.strip_suffix(".json") else {
            continue;
        };
        if id.ends_with(".output") {
            continue;
        }

        let output_path = build_info_dir.join(format!("{id}.output.json"));

        build_infos.push(BuildInfoFiles {
            id: id.to_string(),
            build_info_path: entry.path(),
            output_path: output_path.is_file().then_some(output_path),
        });
    }

    build_infos.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(build_infos)
}

/// Error that occurs when loading a [`BuildInfoConfig`] from disk.
#[derive(Debug, thiserror::Error)]
pub enum BuildInfoConfigLoadError {
    /// The build info files could not be read.
    #[error(transparent)]
    Load(#[from] ArtifactLoadError),
    /// The build info files could not be parsed.
    #[error(transparent)]
    Parse(#[from] SplitCompilerMetadataParseError),
}

/// Reads and parses all build infos in `build_info_dir` into a
/// [`BuildInfoConfig`].
///
/// Build infos without a matching output file are skipped. A missing directory
/// yields an empty config.
pub fn load_build_info_config(
    build_info_dir: &Path,
    ignore_contracts: Option<bool>,
) -> Result<BuildInfoConfig, BuildInfoConfigLoadError> {
    let buffers = find_build_info_files(build_info_dir)?
        .into_iter()
        .filter_map(|build_info| {
            let output_path = build_info.output_path?;

            let result = std::fs::read(&build_info.build_info_path)
                .map_err(|error| ArtifactLoadError::Io {
                    path: build_info.build_info_path,
                    error,
                })
                .and_then(|build_info_buffer| {
                    let output_buffer =
                        std::fs::read(&output_path).map_err(|error| ArtifactLoadError::Io {
                            path: output_path,
                            error,
                        })?;

                    Ok((build_info_buffer, output_buffer))
                });

            Some(result)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let build_infos = buffers
        .iter()
        .map(|(build_info, output)| BuildInfoBufferSeparateOutput { build_info, output })
        .collect();

    let config = BuildInfoConfig::parse_from_buffers(BuildInfoConfigWithBuffers {
        build_infos: Some(BuildInfoBuffers::SeparateInputOutput(build_infos)),
        ignore_contracts,
    })?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_artifact(
        dir: &Path,
        source_name: &str,
        contract_name: &str,
        json: &serde_json::Value,
    ) {
        let contract_dir = dir.join(source_name);
        std::fs::create_dir_all(&contract_dir).unwrap();
        std::fs::write(
            contract_dir.join(format!("{contract_name}.json")),
            serde_json::to_vec(json).unwrap(),
        )
        .unwrap();
    }

    fn minimal_artifact(contract_name: &str, source_name: &str) -> serde_json::Value {
        serde_json::json!({
            "_format": HH3_ARTIFACT_FORMAT,
            "contractName": contract_name,
            "sourceName": source_name,
            "inputSourceName": format!("project/{source_name}"),
            "abi": [],
            "bytecode": "0x6080",
            "deployedBytecode": "0x6001",
            "linkReferences": {},
            "deployedLinkReferences": {},
            // Present in `hh3-artifact-1` files but not consumed by the
            // loader; included to make sure that parsing tolerates it.
            "immutableReferences": {},
            "buildInfoId": "solc-0_8_24-945510fc2baa1a6f4138887ff6bcceb6d37ed839",
        })
    }

    #[test]
    fn solc_version_from_build_info_id_parses_valid_ids() {
        assert_eq!(
            solc_version_from_build_info_id("solc-0_8_24-945510fc2baa1a6f4138887ff6bcceb6d37ed839"),
            Some(Version::new(0, 8, 24))
        );
        // Optional compiler type segment.
        assert_eq!(
            solc_version_from_build_info_id("solc-0_8_28-solx-ABC123"),
            Some(Version::new(0, 8, 28))
        );
        // The hash may be empty.
        assert_eq!(
            solc_version_from_build_info_id("solc-1_2_3-"),
            Some(Version::new(1, 2, 3))
        );
    }

    #[test]
    fn solc_version_from_build_info_id_rejects_invalid_ids() {
        // Not produced by Hardhat + solc.
        assert_eq!(solc_version_from_build_info_id("vyper-0_4_0-abc123"), None);
        // Missing hash segment.
        assert_eq!(solc_version_from_build_info_id("solc-0_8_24"), None);
        // Incomplete version.
        assert_eq!(solc_version_from_build_info_id("solc-0_8-abc123"), None);
        // Non-hexadecimal hash.
        assert_eq!(solc_version_from_build_info_id("solc-0_8_24-xyz"), None);
        // Compiler type must start with a letter.
        assert_eq!(
            solc_version_from_build_info_id("solc-0_8_24-1solx-abc123"),
            None
        );
        // Too many segments.
        assert_eq!(
            solc_version_from_build_info_id("solc-0_8_24-solx-extra-abc123"),
            None
        );
    }

    #[test]
    fn load_artifacts_reads_hh3_artifacts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let artifacts_dir = temp_dir.path();

        write_artifact(
            artifacts_dir,
            "contracts/Foo.sol",
            "Foo",
            &minimal_artifact("Foo", "contracts/Foo.sol"),
        );

        // Top-level JSON files and the build-info directory are ignored.
        std::fs::write(artifacts_dir.join("artifacts.json"), "{}").unwrap();
        let build_info_dir = artifacts_dir.join(BUILD_INFO_DIR_NAME);
        std::fs::create_dir_all(&build_info_dir).unwrap();
        std::fs::write(build_info_dir.join("ignored.json"), "{}").unwrap();

        let artifacts = load_artifacts(artifacts_dir).unwrap();

        assert_eq!(artifacts.len(), 1);
        let artifact = &artifacts[0];
        assert_eq!(artifact.id.name, "Foo");
        // The id's source uses the input source name.
        assert_eq!(
            artifact.id.source,
            PathBuf::from("project/contracts/Foo.sol")
        );
        assert_eq!(artifact.user_source_name, "contracts/Foo.sol");
        assert_eq!(artifact.id.version, Version::new(0, 8, 24));

        let contract = &artifact.contract;
        assert!(contract.abi.is_some());
        assert_eq!(
            contract.bytecode.as_ref().unwrap().object,
            BytecodeObject::Bytecode("0x6080".parse().unwrap())
        );
        assert_eq!(
            contract
                .deployed_bytecode
                .as_ref()
                .unwrap()
                .bytecode
                .as_ref()
                .unwrap()
                .object,
            BytecodeObject::Bytecode("0x6001".parse().unwrap())
        );
    }

    #[test]
    fn load_artifacts_keeps_unlinked_bytecode_as_string() {
        let temp_dir = tempfile::tempdir().unwrap();
        let artifacts_dir = temp_dir.path();

        let placeholder_bytecode = "0x73__$fb58accembedded1234567890123456789012$__63";
        let mut artifact = minimal_artifact("Bar", "contracts/Bar.sol");
        artifact["bytecode"] = placeholder_bytecode.into();
        artifact["linkReferences"] = serde_json::json!({
            "contracts/Lib.sol": {
                "Lib": [{ "start": 1, "length": 20 }]
            }
        });

        write_artifact(artifacts_dir, "contracts/Bar.sol", "Bar", &artifact);

        let artifacts = load_artifacts(artifacts_dir).unwrap();

        assert_eq!(artifacts.len(), 1);
        let bytecode = artifacts[0].contract.bytecode.as_ref().unwrap();
        assert_eq!(
            bytecode.object,
            BytecodeObject::Unlinked(placeholder_bytecode.to_string())
        );
        assert_eq!(
            bytecode.link_references["contracts/Lib.sol"]["Lib"],
            vec![Offsets {
                start: 1,
                length: 20
            }]
        );
    }

    #[test]
    fn load_artifacts_rejects_unsupported_formats() {
        let temp_dir = tempfile::tempdir().unwrap();
        let artifacts_dir = temp_dir.path();

        let mut artifact = minimal_artifact("Foo", "contracts/Foo.sol");
        artifact["_format"] = "hh-sol-artifact-1".into();
        write_artifact(artifacts_dir, "contracts/Foo.sol", "Foo", &artifact);

        let error = load_artifacts(artifacts_dir).unwrap_err();
        assert!(matches!(
            error,
            ArtifactLoadError::UnsupportedFormat { format, .. } if format == "hh-sol-artifact-1"
        ));
    }

    #[test]
    fn load_artifacts_rejects_missing_and_invalid_build_info_ids() {
        let temp_dir = tempfile::tempdir().unwrap();
        let artifacts_dir = temp_dir.path();

        let mut artifact = minimal_artifact("Foo", "contracts/Foo.sol");
        artifact.as_object_mut().unwrap().remove("buildInfoId");
        write_artifact(artifacts_dir, "contracts/Foo.sol", "Foo", &artifact);

        let error = load_artifacts(artifacts_dir).unwrap_err();
        assert!(matches!(
            error,
            ArtifactLoadError::MissingBuildInfoId { .. }
        ));

        let mut artifact = minimal_artifact("Foo", "contracts/Foo.sol");
        artifact["buildInfoId"] = "vyper-0_4_0-abc123".into();
        write_artifact(artifacts_dir, "contracts/Foo.sol", "Foo", &artifact);

        let error = load_artifacts(artifacts_dir).unwrap_err();
        assert!(matches!(
            error,
            ArtifactLoadError::InvalidBuildInfoId { build_info_id, .. }
                if build_info_id == "vyper-0_4_0-abc123"
        ));
    }

    #[test]
    fn load_artifacts_returns_empty_for_missing_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing_dir = temp_dir.path().join("does-not-exist");

        assert!(load_artifacts(&missing_dir).unwrap().is_empty());
    }

    #[test]
    fn find_build_info_files_pairs_inputs_with_outputs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let build_info_dir = temp_dir.path();

        std::fs::write(build_info_dir.join("solc-0_8_24-aa.json"), "{}").unwrap();
        std::fs::write(build_info_dir.join("solc-0_8_24-aa.output.json"), "{}").unwrap();
        std::fs::write(build_info_dir.join("solc-0_7_6-bb.json"), "{}").unwrap();
        // Non-JSON files are ignored.
        std::fs::write(build_info_dir.join("README.md"), "").unwrap();

        let build_infos = find_build_info_files(build_info_dir).unwrap();

        assert_eq!(
            build_infos,
            vec![
                BuildInfoFiles {
                    id: "solc-0_7_6-bb".to_string(),
                    build_info_path: build_info_dir.join("solc-0_7_6-bb.json"),
                    output_path: None,
                },
                BuildInfoFiles {
                    id: "solc-0_8_24-aa".to_string(),
                    build_info_path: build_info_dir.join("solc-0_8_24-aa.json"),
                    output_path: Some(build_info_dir.join("solc-0_8_24-aa.output.json")),
                },
            ]
        );
    }

    #[test]
    fn load_artifacts_parses_integration_test_fixtures() {
        // Real-world artifact set generated by Hardhat v3. Skipped when the
        // fixtures are not present, e.g. outside the repository checkout.
        let artifacts_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../js/integration-tests/solidity-tests/artifacts");
        if !artifacts_dir.is_dir() {
            return;
        }

        let artifacts = load_artifacts(&artifacts_dir).unwrap();
        assert!(!artifacts.is_empty());

        let build_infos = find_build_info_files(&default_build_info_dir(&artifacts_dir)).unwrap();
        assert!(!build_infos.is_empty());
        assert!(build_infos
            .iter()
            .all(|build_info| build_info.output_path.is_some()));

        // Every artifact references a build info that exists on disk.
        for artifact in &artifacts {
            assert!(
                !artifact.user_source_name.is_empty(),
                "artifact {} has an empty source name",
                artifact.id.identifier()
            );
        }
    }

    #[test]
    fn load_build_info_config_parses_split_build_infos() {
        let temp_dir = tempfile::tempdir().unwrap();
        let build_info_dir = temp_dir.path();

        let input: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/compiler_input.json")).unwrap();
        let output: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/compiler_output.json")).unwrap();

        let id = "solc-0_8_0-aabb";
        let build_info = serde_json::json!({
            "_format": "hh3-sol-build-info-1",
            "id": id,
            "solcVersion": "0.8.0",
            "solcLongVersion": "0.8.0+commit.c7dfd78e",
            "input": input,
        });
        let build_info_output = serde_json::json!({
            "_format": "hh3-sol-build-info-output-1",
            "id": id,
            "output": output,
        });

        std::fs::write(
            build_info_dir.join(format!("{id}.json")),
            build_info.to_string(),
        )
        .unwrap();
        std::fs::write(
            build_info_dir.join(format!("{id}.output.json")),
            build_info_output.to_string(),
        )
        .unwrap();

        // Build infos without an output file are skipped without being read.
        std::fs::write(build_info_dir.join("solc-0_8_0-nooutput.json"), "not json").unwrap();

        let config = load_build_info_config(build_info_dir, None).unwrap();

        assert!(!config.identified_contracts.is_empty());
    }

    #[test]
    fn load_build_info_config_returns_empty_for_missing_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing_dir = temp_dir.path().join("does-not-exist");

        let config = load_build_info_config(&missing_dir, None).unwrap();

        assert!(config.identified_contracts.is_empty());
    }

    #[test]
    fn find_build_info_files_returns_empty_for_missing_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing_dir = temp_dir.path().join("does-not-exist");

        assert!(find_build_info_files(&missing_dir).unwrap().is_empty());
    }
}
