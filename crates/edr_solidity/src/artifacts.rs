//! Represents artifacts of the Solidity compiler input and output in the
//! Standard JSON format.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A BuildInfo is a file that contains all the information of a solc run. It
/// includes all the necessary information to recreate that exact same run, and
/// all of its output.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    _format: String,
    id: String,
    solc_version: String,
    solc_long_version: String,
    input: CompilerInput,
    output: CompilerOutput,
}

/// References: of source name -> library name -> link references.
#[derive(Debug, Deserialize, Serialize)]
pub struct LinkReferences(HashMap<String, HashMap<String, Vec<LinkReference>>>);

/// The source code of a contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct Source {
    content: String,
}

/// The main input to the Solidity compiler.
#[derive(Debug, Deserialize, Serialize)]
pub struct CompilerInput {
    language: String,
    sources: HashMap<String, Source>,
    settings: CompilerSettings,
}

/// Additional settings like the optimizer, metadata, etc.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerSettings {
    #[serde(rename = "viaIR")]
    via_ir: Option<bool>,
    optimizer: OptimizerSettings,
    metadata: Option<MetadataSettings>,
    output_selection: HashMap<String, HashMap<String, Vec<String>>>,
    evm_version: Option<String>,
    libraries: Option<HashMap<String, HashMap<String, String>>>,
    remappings: Option<Vec<String>>,
}

/// Specifies the optimizer settings.
#[derive(Debug, Deserialize, Serialize)]
pub struct OptimizerSettings {
    runs: Option<u32>,
    enabled: Option<bool>,
    details: Option<OptimizerDetails>,
}

/// Specifies the optimizer details.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizerDetails {
    yul_details: YulDetails,
}

/// Yul-specific optimizer details.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YulDetails {
    optimizer_steps: String,
}

/// Specifies the metadata settings.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSettings {
    use_literal_content: bool,
}

/// The main output of the Solidity compiler.
#[derive(Debug, Deserialize, Serialize)]
pub struct CompilerOutput {
    sources: HashMap<String, CompilerOutputSource>,
    contracts: HashMap<String, HashMap<String, CompilerOutputContract>>,
}

/// The output of a contract compilation.
#[derive(Debug, Deserialize, Serialize)]
pub struct CompilerOutputContract {
    abi: serde_json::Value,
    evm: CompilerOutputEvm,
}

/// The EVM-specific output of a contract compilation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerOutputEvm {
    bytecode: CompilerOutputBytecode,
    deployed_bytecode: CompilerOutputBytecode,
    method_identifiers: HashMap<String, String>,
}

/// The ID and the AST of the compiled sources.
#[derive(Debug, Deserialize, Serialize)]
pub struct CompilerOutputSource {
    id: u32,
    ast: serde_json::Value,
}

/// The bytecode output for a given compiled contract.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerOutputBytecode {
    object: String,
    opcodes: String,
    source_map: String,
    link_references: HashMap<String, HashMap<String, Vec<LinkReference>>>,
    immutable_references: Option<HashMap<String, Vec<ImmutableReference>>>,
}

/// A reference to a library.
#[derive(Debug, Deserialize, Serialize)]
pub struct LinkReference {
    start: u32,
    length: u32,
}

/// A reference to an immutable value.
#[derive(Debug, Deserialize, Serialize)]
pub struct ImmutableReference {
    start: u32,
    length: u32,
}
