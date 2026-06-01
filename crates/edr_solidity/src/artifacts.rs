//! Represents artifacts of the Solidity compiler input and output in the
//! Standard JSON format.
//!
//! See <https://docs.soliditylang.org/en/latest/using-the-compiler.html#compiler-input-and-output-json-description>.
#![allow(missing_docs)]

use std::{collections::HashMap, sync::Arc};

use indexmap::IndexMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

/// Compiler that produced a Hardhat build-info. Absent on older build-infos
/// and the EDR in-process flow; absent is treated as `Solc`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, strum::Display, strum::EnumString)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum CompilerType {
    /// Reference Solidity compiler; uses `evm.{deployed,}Bytecode.sourceMap`.
    #[default]
    Solc,
    /// solx compiler; uses `evm.{deployed,}Bytecode.debugInfo`.
    Solx,
}

/// Unknown compilerType (e.g. from a 3rd-party plugin EDR doesn't know yet)
/// → log warn + fall back to `Solc` so deserialization doesn't hard-fail.
fn deserialize_compiler_type_graceful<'de, D>(deserializer: D) -> Result<CompilerType, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use std::str::FromStr;

    let raw = String::deserialize(deserializer)?;
    CompilerType::from_str(&raw).or_else(|_| {
        log::warn!("Unknown build-info compilerType {raw:?}; treating as \"solc\".");
        Ok(CompilerType::default())
    })
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
#[derive(Clone, Debug, Default, Deserialize)]
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
                        compiler_type: input.compiler_type,
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

/// A `BuildInfoWithOutput` contains all the information of a compiler run. It
/// includes all the necessary information to recreate that exact same run, and
/// the output of the run.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoWithOutput {
    #[serde(rename = "_format")]
    pub _format: String,
    pub id: String,
    pub solc_version: String,
    pub solc_long_version: String,
    /// Producing compiler. Defaults to [`CompilerType::Solc`] when the
    /// field is absent (older builds, Hardhat 2 flow) or holds an
    /// unknown value (a 3rd-party plugin EDR doesn't recognise).
    #[serde(default, deserialize_with = "deserialize_compiler_type_graceful")]
    pub compiler_type: CompilerType,
    pub input: CompilerInput,
    pub output: CompilerOutput,
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
    /// See [`BuildInfoWithOutput::compiler_type`].
    #[serde(default, deserialize_with = "deserialize_compiler_type_graceful")]
    pub compiler_type: CompilerType,
    pub input: CompilerInput,
}

/// A `BuildInfoOutput` contains all the output of a compiler run.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfoOutput {
    /// Mirrored from the input file (canonical source). See
    /// [`BuildInfoWithOutput::compiler_type`].
    #[serde(default, deserialize_with = "deserialize_compiler_type_graceful")]
    pub compiler_type: CompilerType,
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
#[derive(Clone, Debug, Deserialize)]
pub struct CompilerOutput {
    // Retain the order of the sources as emitted by the compiler.
    // Our post processing relies on this order to build the codebase model.
    pub sources: IndexMap<String, CompilerOutputSource>,
    pub contracts: HashMap<String, HashMap<String, CompilerOutputContract>>,
}

/// The output of a contract compilation.
#[derive(Clone, Debug, Deserialize)]
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
#[derive(Clone, Debug, Deserialize)]
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

/// Bytecode output for a compiled contract. Wraps an `Arc<dyn CompilerArtifact>`
/// so the stack-trace pipeline dispatches dynamically over compiler-specific
/// implementations ([`SolcBytecode`], [`SolxBytecode`]).
#[derive(Clone, Debug)]
pub struct CompilerOutputBytecode(pub Arc<dyn crate::debug_info::CompilerArtifact>);

impl CompilerOutputBytecode {
    pub fn as_artifact(&self) -> &dyn crate::debug_info::CompilerArtifact {
        self.0.as_ref()
    }
}

impl<'de> Deserialize<'de> for CompilerOutputBytecode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Peek at the JSON value to discriminate. Solx artifacts carry
        // a mandatory `debugInfo` field; solc artifacts don't.
        let value = serde_json::Value::deserialize(deserializer)?;
        let artifact: Arc<dyn crate::debug_info::CompilerArtifact> =
            if value.get("debugInfo").is_some() {
                let b: SolxBytecode =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Arc::new(b)
            } else {
                let b: SolcBytecode =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Arc::new(b)
            };
        Ok(Self(artifact))
    }
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
        // these were taken from a run of TypeScript function compileLiteral
        let compiler_input_json = include_str!("../fixtures/compiler_input.json");

        let _compiler_input: CompilerInput = serde_json::from_str(compiler_input_json).unwrap();
    }

    #[test]
    fn serde_solc_output() {
        // these were taken from a run of TypeScript function compileLiteral
        let compiler_output_json = include_str!("../fixtures/compiler_output.json");
        let output: CompilerOutput = serde_json::from_str(compiler_output_json).unwrap();
        // Solc artifacts must construct as the SolcBytecode concrete type.
        if let Some((_, contract)) = output.contracts.values().flat_map(|m| m.iter()).next() {
            assert!(contract
                .evm
                .bytecode
                .as_artifact()
                .as_any()
                .downcast_ref::<SolcBytecode>()
                .is_some());
            assert!(contract
                .evm
                .deployed_bytecode
                .as_artifact()
                .as_any()
                .downcast_ref::<SolcBytecode>()
                .is_some());
        }
    }

    #[test]
    fn solx_compiler_output_carries_debug_info() {
        let compiler_output_json = include_str!("../fixtures/solx_compiler_output.json");
        let output: CompilerOutput = serde_json::from_str(compiler_output_json).unwrap();
        let contract = output
            .contracts
            .get("Counter.sol")
            .and_then(|m| m.get("Counter"))
            .expect("Counter.sol::Counter should be in the solx fixture");
        let creation = contract
            .evm
            .bytecode
            .as_artifact()
            .as_any()
            .downcast_ref::<SolxBytecode>()
            .expect("expected SolxBytecode for creation");
        let runtime = contract
            .evm
            .deployed_bytecode
            .as_artifact()
            .as_any()
            .downcast_ref::<SolxBytecode>()
            .expect("expected SolxBytecode for runtime");
        // \x7fELF magic, hex-encoded.
        assert!(creation.debug_info.starts_with("7f454c46"));
        assert!(runtime.debug_info.starts_with("7f454c46"));
        assert!(creation.debug_info.len() >= 200 && runtime.debug_info.len() >= 200);
    }

    #[test]
    fn build_info_with_compiler_type_round_trips() {
        let with_type = serde_json::json!({
            "_format": "hh3-sol-build-info-1",
            "id": "solc-0_8_34-solx-deadbeef",
            "solcVersion": "0.8.34",
            "solcLongVersion": "0.8.34+solx",
            "compilerType": "solx",
            "input": {
                "language": "Solidity",
                "sources": {},
                "settings": null
            }
        });
        let bi: BuildInfo = serde_json::from_value(with_type).unwrap();
        assert_eq!(bi.compiler_type, CompilerType::Solx);
        let round_tripped = serde_json::to_value(&bi).unwrap();
        assert_eq!(round_tripped["compilerType"], "solx");
    }

    #[test]
    fn build_info_with_solc_compiler_type_round_trips() {
        let solc = serde_json::json!({
            "_format": "hh3-sol-build-info-1",
            "id": "solc-0_8_34-deadbeef",
            "solcVersion": "0.8.34",
            "solcLongVersion": "0.8.34+commit.abc",
            "compilerType": "solc",
            "input": {
                "language": "Solidity",
                "sources": {},
                "settings": null
            }
        });
        let bi: BuildInfo = serde_json::from_value(solc).unwrap();
        assert_eq!(bi.compiler_type, CompilerType::Solc);
        let round_tripped = serde_json::to_value(&bi).unwrap();
        assert_eq!(round_tripped["compilerType"], "solc");
    }

    #[test]
    fn build_info_without_compiler_type_defaults_to_none() {
        let without_type = serde_json::json!({
            "_format": "hh3-sol-build-info-1",
            "id": "solc-0_8_31-deadbeef",
            "solcVersion": "0.8.31",
            "solcLongVersion": "0.8.31+commit.abc",
            "input": {
                "language": "Solidity",
                "sources": {},
                "settings": null
            }
        });
        let bi: BuildInfo = serde_json::from_value(without_type).unwrap();
        assert_eq!(bi.compiler_type, CompilerType::Solc);
    }
}
