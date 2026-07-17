//! Represents artifacts of the Solidity compiler input and output in the
//! Standard JSON format.
//!
//! See <https://docs.soliditylang.org/en/latest/using-the-compiler.html#compiler-input-and-output-json-description>.
#![allow(missing_docs)]

use std::{collections::HashMap, str::FromStr};

use auto_impl::auto_impl;
use indexmap::IndexMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use self::{
    solc::{parse_solc_compiler_metadata, parse_split_solc_compiler_metadata},
    solx::{parse_solx_compiler_metadata, parse_split_solx_compiler_metadata},
};
use crate::contracts_identifier::IdentifiedContract;

pub mod solc;
pub mod solx;

/// Per-compiler bytecode artifact.
#[auto_impl(&, Box)]
pub trait CompilerArtifact: std::fmt::Debug + 'static {
    /// Hex-encoded creation- or runtime-bytecode `object` from the
    /// Standard JSON output.
    fn object(&self) -> &str;

    /// Library link references (source → library name → positions).
    fn link_references(&self) -> &HashMap<String, HashMap<String, Vec<LinkReference>>>;

    /// Immutable-variable references emitted by the compiler, if any.
    fn immutable_references(&self) -> Option<&HashMap<String, Vec<ImmutableReference>>>;
}

/// A JSON source that can be deserialized into an owned type. Lets the
/// compiler-metadata parse functions accept raw bytes, a string, or an
/// already-parsed [`serde_json::Value`] through one signature.
pub trait JsonSource {
    /// Deserializes this source into `T`.
    fn parse_json<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error>;
}

impl JsonSource for &[u8] {
    fn parse_json<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(self)
    }
}

impl JsonSource for &str {
    fn parse_json<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::from_str(self)
    }
}

impl JsonSource for serde_json::Value {
    fn parse_json<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid solc version: {0}")]
pub struct InvalidSolcVersionError(#[from] semver::Error);

#[derive(Debug, thiserror::Error)]
#[error("The solc version {actual} is not supported. The minimum supported version is {minimum}.")]
pub struct UnsupportedSolcVersionError {
    pub minimum: semver::Version,
    pub actual: semver::Version,
}

#[derive(Debug, thiserror::Error)]
pub enum CompilerMetadataParseError {
    #[error("Failed to parse build info: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error(transparent)]
    InvalidSolcVersion(#[from] InvalidSolcVersionError),
    #[error(transparent)]
    Misc(#[from] anyhow::Error),
    #[error(transparent)]
    UnsupportedSolcVersion(#[from] UnsupportedSolcVersionError),
}

impl From<ContractMetadataExtractionError> for CompilerMetadataParseError {
    fn from(error: ContractMetadataExtractionError) -> Self {
        match error {
            ContractMetadataExtractionError::InvalidSolcVersion(error) => error.into(),
            ContractMetadataExtractionError::UnsupportedSolcVersion(error) => error.into(),
            ContractMetadataExtractionError::Misc(error) => error.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SplitCompilerMetadataParseError {
    #[error("The compiler input and output IDs do not match: input ID = {input_id}, output ID = {output_id}")]
    IdMismatch { input_id: String, output_id: String },
    #[error("Failed to parse build info: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error(transparent)]
    InvalidSolcVersion(#[from] InvalidSolcVersionError),
    #[error(transparent)]
    Misc(#[from] anyhow::Error),
    #[error(transparent)]
    UnsupportedSolcVersion(#[from] UnsupportedSolcVersionError),
}

impl From<ContractMetadataExtractionError> for SplitCompilerMetadataParseError {
    fn from(error: ContractMetadataExtractionError) -> Self {
        match error {
            ContractMetadataExtractionError::InvalidSolcVersion(error) => error.into(),
            ContractMetadataExtractionError::UnsupportedSolcVersion(error) => error.into(),
            ContractMetadataExtractionError::Misc(error) => error.into(),
        }
    }
}

impl From<CompilerMetadataParseError> for SplitCompilerMetadataParseError {
    fn from(error: CompilerMetadataParseError) -> Self {
        match error {
            CompilerMetadataParseError::InvalidJson(error) => error.into(),
            CompilerMetadataParseError::InvalidSolcVersion(error) => error.into(),
            CompilerMetadataParseError::Misc(error) => error.into(),
            CompilerMetadataParseError::UnsupportedSolcVersion(error) => error.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContractMetadataExtractionError {
    #[error(transparent)]
    InvalidSolcVersion(#[from] InvalidSolcVersionError),
    #[error(transparent)]
    UnsupportedSolcVersion(#[from] UnsupportedSolcVersionError),
    // TODO: Split these into more detailed errors
    #[error(transparent)]
    Misc(#[from] anyhow::Error),
}

/// Producing compiler for a Hardhat build-info. Absent or unknown values fall
/// back to `Solc`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum CompilerType {
    /// Reference Solidity compiler; uses `evm.{deployed,}Bytecode.sourceMap`.
    #[default]
    Solc,
    /// solx compiler; uses `evm.{deployed,}Bytecode.debugInfo`.
    Solx,
}

/// Configuration for the [`crate::contract_decoder::ContractDecoder`].
#[derive(Clone, Debug, Default)]
pub struct BuildInfoConfig {
    /// The identified contracts extracted from the build info.
    pub identified_contracts: Vec<IdentifiedContract>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

impl BuildInfoConfig {
    /// Parse the config from bytes. This is a performance intensive operation
    /// which is why it's not a `TryFrom` implementation.
    pub fn parse_from_buffers(
        config: BuildInfoConfigWithBuffers<'_>,
    ) -> Result<Self, SplitCompilerMetadataParseError> {
        let BuildInfoConfigWithBuffers {
            build_infos,
            ignore_contracts,
        } = config;

        let identified_contracts =
            build_infos.map_or_else(|| Ok(Vec::default()), |bi| bi.parse())?;

        Ok(Self {
            identified_contracts,
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
    /// Deserializes to [`BuildInfoWithOutput`].
    WithOutput(Vec<&'a [u8]>),
    /// Separate build info input and output files.
    SeparateInputOutput(Vec<BuildInfoBufferSeparateOutput<'a>>),
}

/// Peeks at `compilerType` from a build-info JSON, borrowing the field as
/// a `&str`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeekableCompilerType<'a> {
    #[serde(default, borrow)]
    compiler_type: Option<&'a str>,
}

/// Converts an optional compiler type string to a [`CompilerType`]. Unknown
/// values are treated as `Solc` with a warning.
pub fn to_compiler_type(compiler_type_str: Option<&str>) -> CompilerType {
    let Some(raw) = compiler_type_str else {
        return CompilerType::Solc;
    };
    match CompilerType::from_str(raw) {
        Ok(compiler_type) => compiler_type,
        Err(strum::ParseError::VariantNotFound) => {
            log::warn!("Unknown build-info compilerType {raw}; treating as \"solc\".");
            CompilerType::Solc
        }
    }
}

impl BuildInfoBuffers<'_> {
    fn parse(&self) -> Result<Vec<IdentifiedContract>, SplitCompilerMetadataParseError> {
        match self {
            BuildInfoBuffers::WithOutput(build_infos_with_output) => build_infos_with_output
                .iter()
                .map(|item| {
                    let peek: PeekableCompilerType<'_> = serde_json::from_slice(item)?;

                    match to_compiler_type(peek.compiler_type) {
                        CompilerType::Solc => parse_solc_compiler_metadata(*item),
                        CompilerType::Solx => parse_solx_compiler_metadata(*item),
                    }
                    // Silently ignore unsupported solc versions
                    .or_else(|error| {
                        if matches!(error, CompilerMetadataParseError::UnsupportedSolcVersion(_)) {
                            Ok(Vec::new())
                        } else {
                            Err(error)
                        }
                    })
                    .map_err(SplitCompilerMetadataParseError::from)
                })
                .flatten_ok()
                .collect(),
            BuildInfoBuffers::SeparateInputOutput(separate_output) => separate_output
                .iter()
                .map(|item| {
                    let peek: PeekableCompilerType<'_> = serde_json::from_slice(item.build_info)?;
                    match to_compiler_type(peek.compiler_type) {
                        CompilerType::Solc => {
                            parse_split_solc_compiler_metadata(item.build_info, item.output)
                        }
                        CompilerType::Solx => {
                            parse_split_solx_compiler_metadata(item.build_info, item.output)
                        }
                    }
                    // Silently ignore unsupported solc versions
                    .or_else(|error| {
                        if matches!(
                            error,
                            SplitCompilerMetadataParseError::UnsupportedSolcVersion(_)
                        ) {
                            Ok(Vec::new())
                        } else {
                            Err(error)
                        }
                    })
                })
                .flatten_ok()
                .collect(),
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

/// A `BuildInfoWithOutput` contains all the information of a compiler run. It
/// includes all the necessary information to recreate that exact same run, and
/// the output of the run.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoWithOutput<ArtifactT: CompilerArtifact> {
    #[serde(rename = "_format")]
    pub _format: String,
    pub id: String,
    pub solc_version: String,
    pub solc_long_version: String,
    pub input: CompilerInput,
    pub output: CompilerOutput<ArtifactT>,
}

/// A `BuildInfo` contains all the input information of a compiler run. It
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

/// A `BuildInfoOutput` contains all the output of a compiler run.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoOutput<ArtifactT: CompilerArtifact> {
    #[serde(rename = "_format")]
    pub _format: String,
    pub id: String,
    pub output: CompilerOutput<ArtifactT>,
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
#[derive(Clone, Debug, Deserialize)]
pub struct CompilerOutput<ArtifactT: CompilerArtifact> {
    // Retain the order of the sources as emitted by the compiler.
    // Our post processing relies on this order to build the codebase model.
    pub sources: IndexMap<String, CompilerOutputSource>,
    pub contracts: HashMap<String, HashMap<String, CompilerOutputContract<ArtifactT>>>,
}

/// The output of a contract compilation.
#[derive(Clone, Debug, Deserialize)]
pub struct CompilerOutputContract<ArtifactT: CompilerArtifact> {
    pub abi: Vec<ContractAbiEntry>,
    pub evm: CompilerOutputEvm<ArtifactT>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ContractAbiEntry {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub inputs: Option<Vec<serde_json::Value>>,
}

/// The EVM-specific output of a contract compilation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerOutputEvm<ArtifactT: CompilerArtifact> {
    pub bytecode: ArtifactT,
    pub deployed_bytecode: ArtifactT,
    pub method_identifiers: HashMap<String, String>,
}

/// The ID and the AST of the compiled sources.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompilerOutputSource {
    pub id: u32,
    pub ast: serde_json::Value,
}

/// Solc-emitted bytecode.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolcBytecode {
    pub object: String,
    pub opcodes: String,
    pub source_map: String,
    pub link_references: HashMap<String, HashMap<String, Vec<LinkReference>>>,
    pub immutable_references: Option<HashMap<String, Vec<ImmutableReference>>>,
}

/// Solx-emitted bytecode. `debug_info` is hex-encoded ELF (DWARF v5).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolxBytecode {
    pub object: String,
    pub opcodes: String,
    pub debug_info: String,
    pub link_references: HashMap<String, HashMap<String, Vec<LinkReference>>>,
    pub immutable_references: Option<HashMap<String, Vec<ImmutableReference>>>,
}

/// Collects the AST spans from the compiler output's sources. The DWARF
/// parser uses this to derive `SourceLocation.length` from a `(file, line,
/// column)` triple.
pub fn collect_ast_spans<'a>(
    sources: impl Iterator<Item = &'a CompilerOutputSource>,
) -> HashMap<u32, Vec<(u32, u32)>> {
    let mut spans: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    for source in sources {
        collect_node_spans(&source.ast, &mut spans);
    }
    // Sorted so `BuildModel::smallest_enclosing_span` can scan in order
    // and break early.
    for file_spans in spans.values_mut() {
        file_spans.sort_unstable();
        file_spans.dedup();
    }
    spans
}

/// Walk an AST subtree and append every node's `src` span keyed by file ID.
fn collect_node_spans(node: &serde_json::Value, out: &mut HashMap<u32, Vec<(u32, u32)>>) {
    if let Some(src) = node.get("src").and_then(serde_json::Value::as_str)
        && let Some((offset, length, file_id)) = parse_src(src)
    {
        out.entry(file_id).or_default().push((offset, length));
    }

    if let Some(obj) = node.as_object() {
        for value in obj.values() {
            collect_node_spans(value, out);
        }
    } else if let Some(arr) = node.as_array() {
        for value in arr {
            collect_node_spans(value, out);
        }
    }
}

/// Parse `"offset:length:fileIndex"` into `(offset, length, file_id)`.
fn parse_src(src: &str) -> Option<(u32, u32, u32)> {
    let mut parts = src.splitn(3, ':');
    let offset = parts.next()?.parse::<u32>().ok()?;
    let length = parts.next()?.parse::<u32>().ok()?;
    let file_id = parts.next()?.parse::<u32>().ok()?;
    Some((offset, length, file_id))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_source_impls_are_equivalent() {
        let input: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/compiler_input.json")).unwrap();
        let output: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/compiler_output.json")).unwrap();

        let build_info = serde_json::json!({
            "_format": "hh-sol-build-info-1",
            "id": "json-source-test",
            "solcVersion": "0.8.0",
            "solcLongVersion": "0.8.0+commit.c7dfd78e",
            "input": input,
            "output": output,
        });
        let as_string = build_info.to_string();

        let from_bytes = parse_solc_compiler_metadata(as_string.as_bytes()).unwrap();
        let from_str = parse_solc_compiler_metadata(as_string.as_str()).unwrap();
        let from_value = parse_solc_compiler_metadata(build_info).unwrap();

        assert!(!from_bytes.is_empty());
        assert_eq!(from_bytes.len(), from_str.len());
        assert_eq!(from_bytes.len(), from_value.len());
    }

    #[test]
    fn serde_compiler_input() {
        let compiler_input_json = include_str!("../fixtures/compiler_input.json");
        let _compiler_input: CompilerInput = serde_json::from_str(compiler_input_json).unwrap();
    }

    #[test]
    fn serde_solc_output() {
        let compiler_output_json = include_str!("../fixtures/compiler_output.json");
        // Solc artifacts deserialize as CompilerOutput<SolcBytecode>.
        let _output: CompilerOutput<SolcBytecode> =
            serde_json::from_str(compiler_output_json).unwrap();
    }

    #[test]
    fn solx_compiler_output_carries_debug_info() {
        let compiler_output_json = include_str!("../fixtures/solx_compiler_output.json");
        let output: CompilerOutput<SolxBytecode> =
            serde_json::from_str(compiler_output_json).unwrap();
        let contract = output
            .contracts
            .get("Counter.sol")
            .and_then(|m| m.get("Counter"))
            .expect("Counter.sol::Counter should be in the solx fixture");
        assert!(contract.evm.bytecode.debug_info.starts_with("7f454c46"));
        assert!(contract
            .evm
            .deployed_bytecode
            .debug_info
            .starts_with("7f454c46"));
        assert!(contract.evm.bytecode.debug_info.len() >= 200);
        assert!(contract.evm.deployed_bytecode.debug_info.len() >= 200);
    }
}
