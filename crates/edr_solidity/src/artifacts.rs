//! Represents artifacts of the Solidity compiler input and output in the
//! Standard JSON format.
//!
//! See <https://docs.soliditylang.org/en/latest/using-the-compiler.html#compiler-input-and-output-json-description>.
#![allow(missing_docs)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use alloy_json_abi::JsonAbi;
use alloy_primitives::Bytes;
use indexmap::IndexMap;
use itertools::Itertools;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Error in the build info config
#[derive(Debug, thiserror::Error)]
pub enum BuildInfoConfigError {
    /// JSON deserialization error
    #[error("Failed to parse build info: {0}")]
    Json(#[from] serde_json::Error),
    /// Invalid semver in the build info
    #[error("Invalid solc version: {0}")]
    Semver(#[from] semver::Error),
    /// Input output file mismatch
    #[error("Input output mismatch. Input id: '{input_id}'. Output id: '{output_id}'")]
    InputOutputMismatch { input_id: String, output_id: String },
}

/// Configuration for the [`crate::contract_decoder::ContractDecoder`].
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoConfig {
    /// Build information to use for decoding contracts.
    pub build_infos: Vec<BuildInfoWithOutput>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

impl BuildInfoConfig {
    /// Parse the config from bytes. This is a performance intensive operation
    /// which is why it's not a `TryFrom` implementation.
    pub fn parse_from_buffers(
        config: BuildInfoConfigWithBuffers<'_>,
    ) -> Result<Self, BuildInfoConfigError> {
        let BuildInfoConfigWithBuffers {
            build_infos,
            ignore_contracts,
        } = config;

        let build_infos = build_infos.map_or_else(|| Ok(Vec::default()), |bi| bi.parse())?;

        Ok(Self {
            build_infos,
            ignore_contracts,
        })
    }
}

/// Configuration for the [`crate::contract_decoder::ContractDecoder`] unparsed
/// build infos.
#[derive(Clone, Debug)]
pub struct BuildInfoConfigWithBuffers<'a> {
    /// Build information to use for decoding contracts.
    pub build_infos: Option<BuildInfoBuffers<'a>>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

/// Unparsed build infos.
#[derive(Clone, Debug)]
pub enum BuildInfoBuffers<'a> {
    /// Deserializes to `BuildInfoWithOutput`.
    WithOutput(Vec<&'a [u8]>),
    /// Separate build info input and output files.
    SeparateInputOutput(Vec<BuildInfoBufferSeparateOutput<'a>>),
}

impl BuildInfoBuffers<'_> {
    fn parse(&self) -> Result<Vec<BuildInfoWithOutput>, BuildInfoConfigError> {
        fn filter_on_solc_version(
            build_info: BuildInfoWithOutput,
        ) -> Result<Option<BuildInfoWithOutput>, BuildInfoConfigError> {
            let solc_version = build_info.solc_version.parse::<semver::Version>()?;

            if crate::compiler::FIRST_SOLC_VERSION_SUPPORTED <= solc_version {
                Ok(Some(build_info))
            } else {
                Ok(None)
            }
        }

        match self {
            BuildInfoBuffers::WithOutput(build_infos_with_output) => build_infos_with_output
                .iter()
                .map(|item| {
                    let build_info: BuildInfoWithOutput = serde_json::from_slice(item)?;
                    filter_on_solc_version(build_info)
                })
                .flatten_ok()
                .collect::<Result<Vec<BuildInfoWithOutput>, _>>(),
            BuildInfoBuffers::SeparateInputOutput(separate_output) => separate_output
                .iter()
                .map(|item| {
                    let input: BuildInfo = serde_json::from_slice(item.build_info)?;
                    let output: BuildInfoOutput = serde_json::from_slice(item.output)?;
                    // Make sure we get the output matching the input.
                    if input.id != output.id {
                        return Err(BuildInfoConfigError::InputOutputMismatch {
                            input_id: input.id,
                            output_id: output.id,
                        });
                    }
                    filter_on_solc_version(BuildInfoWithOutput {
                        _format: input._format,
                        id: input.id,
                        solc_version: input.solc_version,
                        solc_long_version: input.solc_long_version,
                        input: input.input,
                        output: output.output,
                    })
                })
                .flatten_ok()
                .collect::<Result<Vec<BuildInfoWithOutput>, _>>(),
        }
    }
}

/// Separate build info input and output files.
#[derive(Clone, Debug)]
pub struct BuildInfoBufferSeparateOutput<'a> {
    /// Deserializes to `BuildInfo`
    pub build_info: &'a [u8],
    /// Deserializes to `BuildInfoOutput`
    pub output: &'a [u8],
}

/// A `BuildInfoWithOutput` contains all the information of a solc run. It
/// includes all the necessary information to recreate that exact same run, and
/// the output of the run.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoWithOutput {
    #[serde(rename = "_format")]
    pub _format: String,
    pub id: String,
    pub solc_version: String,
    pub solc_long_version: String,
    pub input: CompilerInput,
    pub output: CompilerOutput,
}

/// A `BuildInfo` contains all the input information of a solc run. It
/// includes all the necessary information to recreate that exact same run.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    #[serde(rename = "_format")]
    pub _format: String,
    pub id: String,
    pub solc_version: String,
    pub solc_long_version: String,
    pub input: CompilerInput,
}

/// A `BuildInfoOutput` contains all the output of a solc run.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoOutput {
    #[serde(rename = "_format")]
    pub _format: String,
    pub id: String,
    pub output: CompilerOutput,
}

/// References: of source name -> library name -> link references.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LinkReferences(HashMap<String, HashMap<String, Vec<LinkReference>>>);

/// The source code of a contract.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Source {
    pub content: String,
}

/// The main input to the Solidity compiler.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompilerInput {
    pub language: String,
    pub sources: HashMap<String, Source>,
    pub settings: Option<CompilerSettings>,
}

/// Additional settings like the optimizer, metadata, etc.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerSettings {
    #[serde(rename = "viaIR")]
    via_ir: Option<bool>,
    optimizer: Option<OptimizerSettings>,
    metadata: Option<MetadataSettings>,
    output_selection: HashMap<String, HashMap<String, Vec<String>>>,
    evm_version: Option<String>,
    libraries: Option<HashMap<String, HashMap<String, String>>>,
    remappings: Option<Vec<String>>,
}

/// Specifies the optimizer settings.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct OptimizerSettings {
    runs: Option<u32>,
    enabled: Option<bool>,
    details: Option<OptimizerDetails>,
}

/// Specifies the optimizer details.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizerDetails {
    yul_details: Option<YulDetails>,
}

/// Yul-specific optimizer details.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YulDetails {
    optimizer_steps: Option<String>,
}

/// Specifies the metadata settings.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSettings {
    use_literal_content: Option<bool>,
}

/// The main output of the Solidity compiler.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompilerOutput {
    // Retain the order of the sources as emitted by the compiler.
    // Our post processing relies on this order to build the codebase model.
    pub sources: IndexMap<String, CompilerOutputSource>,
    pub contracts: HashMap<String, HashMap<String, CompilerOutputContract>>,
}

/// The output of a contract compilation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompilerOutputContract {
    pub abi: Vec<ContractAbiEntry>,
    pub evm: CompilerOutputEvm,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ContractAbiEntry {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub inputs: Option<Vec<serde_json::Value>>,
}

/// The EVM-specific output of a contract compilation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerOutputEvm {
    pub bytecode: CompilerOutputBytecode,
    pub deployed_bytecode: CompilerOutputBytecode,
    pub method_identifiers: HashMap<String, String>,
}

/// The ID and the AST of the compiled sources.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompilerOutputSource {
    pub id: u32,
    pub ast: serde_json::Value,
}

/// The bytecode output for a given compiled contract.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerOutputBytecode {
    pub object: String,
    pub opcodes: String,
    pub source_map: String,
    pub link_references: HashMap<String, HashMap<String, Vec<LinkReference>>>,
    pub immutable_references: Option<HashMap<String, Vec<ImmutableReference>>>,
}

/// A reference to a library.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LinkReference {
    pub start: u32,
    pub length: u32,
}

/// A reference to an immutable value.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct ImmutableReference {
    pub start: u32,
    pub length: u32,
}

// Adapted from <https://github.com/foundry-rs/compilers/blob/ea346377deaf18dc1f972a06fad76df3d9aed8d9/crates/compilers/src/artifact_output/mod.rs#L45>
/// Compilation artifact identifier
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
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
