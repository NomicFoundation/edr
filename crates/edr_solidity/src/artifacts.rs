//! Represents artifacts of the Solidity compiler input and output in the
//! Standard JSON format.
//!
//! See <https://docs.soliditylang.org/en/latest/using-the-compiler.html#compiler-input-and-output-json-description>.
#![allow(missing_docs)]

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Error in the build info config
#[derive(Debug, thiserror::Error)]
pub enum BuildInfoConfigError {
    /// JSON deserialization error
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Invalid semver in the build info
    #[error(transparent)]
    Semver(#[from] semver::Error),
}

/// Configuration for the [`crate::contract_decoder::ContractDecoder`].
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoConfig {
    /// Build information to use for decoding contracts.
    pub build_infos: Vec<BuildInfo>,
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

        let build_infos = build_infos
            .into_iter()
            .filter_map(|array| parse_build_info(array).transpose())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            build_infos,
            ignore_contracts,
        })
    }
}

fn parse_build_info(array: &[u8]) -> Result<Option<BuildInfo>, BuildInfoConfigError> {
    let build_info = serde_json::from_slice::<BuildInfo>(array)?;
    let solc_version = build_info.solc_version.parse::<semver::Version>()?;

    if crate::compiler::FIRST_SOLC_VERSION_SUPPORTED <= solc_version {
        Ok(Some(build_info))
    } else {
        Ok(None)
    }
}

/// Configuration for the [`crate::contract_decoder::ContractDecoder`] unparsed
/// build infos.
#[derive(Clone, Debug)]
pub struct BuildInfoConfigWithBuffers<'a> {
    /// Build information to use for decoding contracts.
    pub build_infos: Vec<&'a [u8]>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

/// A `BuildInfo` is a file that contains all the information of a solc run. It
/// includes all the necessary information to recreate that exact same run, and
/// all of its output.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    #[serde(rename = "_format")]
    pub _format: String,
    pub id: String,
    pub solc_version: String,
    pub solc_long_version: String,
    pub input: CompilerInput,
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
