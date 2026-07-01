//! Represents artifacts of the Solidity compiler input and output in the
//! Standard JSON format.
//!
//! See <https://docs.soliditylang.org/en/latest/using-the-compiler.html#compiler-input-and-output-json-description>.
#![allow(missing_docs)]

use std::{collections::HashMap, str::FromStr};

use indexmap::IndexMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::debug_info::CompilerArtifact;

/// Producing compiler for a Hardhat build-info. Used ONLY inside
/// [`BuildInfoBuffers::parse`] — the single factory that consumes it to
/// dispatch to the correct [`CompilerArtifact`] impl. Absent or unknown
/// values fall back to `Solc`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub(crate) enum CompilerType {
    /// Reference Solidity compiler; uses `evm.{deployed,}Bytecode.sourceMap`.
    #[default]
    Solc,
    /// solx compiler; uses `evm.{deployed,}Bytecode.debugInfo`.
    Solx,
}

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
#[derive(Debug, Default)]
pub struct BuildInfoConfig {
    /// Build information to use for decoding contracts.
    pub build_infos: Vec<BuildInfoWithOutput<Box<dyn CompilerArtifact>>>,
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
    /// Deserializes to [`BuildInfoWithOutput`].
    WithOutput(Vec<&'a [u8]>),
    /// Separate build info input and output files.
    SeparateInputOutput(Vec<BuildInfoBufferSeparateOutput<'a>>),
}

/// Peeks at `compilerType` from a build-info JSON, borrowing the field as
/// a `&str`. `#[serde(default)]` makes the field optional so older
/// build-infos (Hardhat 2 flow, EDR in-process) still parse.
///
/// Used ONLY inside [`BuildInfoBuffers::parse`] — the single factory site
/// permitted to inspect the compiler tag. Every other consumer sees the
/// erased `Box<dyn CompilerArtifact>` output.
#[derive(Deserialize)]
struct PeekableCompilerType<'a> {
    #[serde(rename = "compilerType", default, borrow)]
    compiler_type: Option<&'a str>,
}

fn to_compiler_type(compiler_type_str: Option<&str>) -> CompilerType {
    let Some(raw) = compiler_type_str else {
        return CompilerType::Solc;
    };
    match CompilerType::from_str(raw) {
        Ok(compiler_type) => compiler_type,
        Err(strum::ParseError::VariantNotFound) => {
            log::warn!("Unknown build-info compilerType {raw:?}; treating as \"solc\".");
            CompilerType::Solc
        }
    }
}

impl BuildInfoBuffers<'_> {
    fn parse(
        &self,
    ) -> Result<Vec<BuildInfoWithOutput<Box<dyn CompilerArtifact>>>, BuildInfoConfigError> {
        fn filter_on_solc_version(
            build_info: BuildInfoWithOutput<Box<dyn CompilerArtifact>>,
        ) -> Result<Option<BuildInfoWithOutput<Box<dyn CompilerArtifact>>>, BuildInfoConfigError>
        {
            let solc_version = build_info.solc_version.parse::<semver::Version>()?;

            if crate::compiler::FIRST_SOLC_VERSION_SUPPORTED <= solc_version {
                Ok(Some(build_info))
            } else {
                Ok(None)
            }
        }

        fn erase<A: CompilerArtifact>(bytecode: A) -> Box<dyn CompilerArtifact> {
            Box::new(bytecode)
        }

        match self {
            BuildInfoBuffers::WithOutput(build_infos_with_output) => build_infos_with_output
                .iter()
                .map(|item| {
                    let peek: PeekableCompilerType<'_> = serde_json::from_slice(item)?;
                    let build_info = match to_compiler_type(peek.compiler_type) {
                        CompilerType::Solc => {
                            serde_json::from_slice::<BuildInfoWithOutput<SolcBytecode>>(item)?
                                .map_artifact(erase)
                        }
                        CompilerType::Solx => {
                            serde_json::from_slice::<BuildInfoWithOutput<SolxBytecode>>(item)?
                                .map_artifact(erase)
                        }
                    };
                    filter_on_solc_version(build_info)
                })
                .flatten_ok()
                .collect(),
            BuildInfoBuffers::SeparateInputOutput(separate_output) => separate_output
                .iter()
                .map(|item| {
                    let peek: PeekableCompilerType<'_> = serde_json::from_slice(item.build_info)?;
                    let input: BuildInfo = serde_json::from_slice(item.build_info)?;
                    let build_info = match to_compiler_type(peek.compiler_type) {
                        CompilerType::Solc => {
                            let output: BuildInfoOutput<SolcBytecode> =
                                serde_json::from_slice(item.output)?;
                            if input.id != output.id {
                                return Err(BuildInfoConfigError::InputOutputMismatch {
                                    input_id: input.id,
                                    output_id: output.id,
                                });
                            }
                            BuildInfoWithOutput {
                                _format: input._format,
                                id: input.id,
                                solc_version: input.solc_version,
                                solc_long_version: input.solc_long_version,
                                input: input.input,
                                output: output.output,
                            }
                            .map_artifact(erase)
                        }
                        CompilerType::Solx => {
                            let output: BuildInfoOutput<SolxBytecode> =
                                serde_json::from_slice(item.output)?;
                            if input.id != output.id {
                                return Err(BuildInfoConfigError::InputOutputMismatch {
                                    input_id: input.id,
                                    output_id: output.id,
                                });
                            }
                            BuildInfoWithOutput {
                                _format: input._format,
                                id: input.id,
                                solc_version: input.solc_version,
                                solc_long_version: input.solc_long_version,
                                input: input.input,
                                output: output.output,
                            }
                            .map_artifact(erase)
                        }
                    };
                    filter_on_solc_version(build_info)
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

impl<ArtifactT: CompilerArtifact> BuildInfoWithOutput<ArtifactT> {
    /// Convert the artifact type by mapping every bytecode through
    /// `conversion_fn`. Threads through the nested generic types.
    pub fn map_artifact<Fn, NewArtifactT>(
        self,
        conversion_fn: Fn,
    ) -> BuildInfoWithOutput<NewArtifactT>
    where
        Fn: FnMut(ArtifactT) -> NewArtifactT,
        NewArtifactT: CompilerArtifact,
    {
        BuildInfoWithOutput {
            _format: self._format,
            id: self.id,
            solc_version: self.solc_version,
            solc_long_version: self.solc_long_version,
            input: self.input,
            output: self.output.map_artifact(conversion_fn),
        }
    }
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

impl<ArtifactT: CompilerArtifact> CompilerOutput<ArtifactT> {
    /// Convert the artifact type across every contract in this output.
    pub fn map_artifact<Fn, NewArtifactT>(
        self,
        mut conversion_fn: Fn,
    ) -> CompilerOutput<NewArtifactT>
    where
        Fn: FnMut(ArtifactT) -> NewArtifactT,
        NewArtifactT: CompilerArtifact,
    {
        CompilerOutput {
            sources: self.sources,
            contracts: self
                .contracts
                .into_iter()
                .map(|(source_name, contracts)| {
                    let contracts = contracts
                        .into_iter()
                        .map(|(name, contract)| (name, contract.map_artifact(&mut conversion_fn)))
                        .collect();
                    (source_name, contracts)
                })
                .collect(),
        }
    }
}

/// The output of a contract compilation.
#[derive(Clone, Debug, Deserialize)]
pub struct CompilerOutputContract<ArtifactT: CompilerArtifact> {
    pub abi: Vec<ContractAbiEntry>,
    pub evm: CompilerOutputEvm<ArtifactT>,
}

impl<ArtifactT: CompilerArtifact> CompilerOutputContract<ArtifactT> {
    pub fn map_artifact<Fn, NewArtifactT>(
        self,
        conversion_fn: &mut Fn,
    ) -> CompilerOutputContract<NewArtifactT>
    where
        Fn: FnMut(ArtifactT) -> NewArtifactT,
        NewArtifactT: CompilerArtifact,
    {
        CompilerOutputContract {
            abi: self.abi,
            evm: self.evm.map_artifact(conversion_fn),
        }
    }
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

impl<ArtifactT: CompilerArtifact> CompilerOutputEvm<ArtifactT> {
    pub fn map_artifact<Fn, NewArtifactT>(
        self,
        conversion_fn: &mut Fn,
    ) -> CompilerOutputEvm<NewArtifactT>
    where
        Fn: FnMut(ArtifactT) -> NewArtifactT,
        NewArtifactT: CompilerArtifact,
    {
        CompilerOutputEvm {
            bytecode: conversion_fn(self.bytecode),
            deployed_bytecode: conversion_fn(self.deployed_bytecode),
            method_identifiers: self.method_identifiers,
        }
    }
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
