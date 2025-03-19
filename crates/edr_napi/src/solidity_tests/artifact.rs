use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};

use foundry_compilers::artifacts::CompactContractBytecodeCow;
use napi_derive::napi;

/// A compilation artifact.
#[derive(Clone, Debug)]
#[napi(object)]
pub struct Artifact {
    /// The identifier of the artifact.
    pub id: ArtifactId,
    /// The test contract.
    pub contract: ContractData,
}

/// The identifier of a Solidity contract.
#[derive(Clone, Debug)]
#[napi(object)]
pub struct ArtifactId {
    pub path: String,
    /// The name of the contract.
    pub name: String,
    /// Original source file path.
    pub source: String,
    /// The solc semver string.
    pub solc_version: String,
}

impl From<edr_solidity::artifacts::ArtifactId> for ArtifactId {
    fn from(value: edr_solidity::artifacts::ArtifactId) -> Self {
        Self {
            path: todo!(),
            name: value.name,
            source: value.source.to_string_lossy().to_string(),
            solc_version: value.version.to_string(),
        }
    }
}

impl TryFrom<ArtifactId> for edr_solidity::artifacts::ArtifactId {
    type Error = napi::Error;

    fn try_from(value: ArtifactId) -> napi::Result<Self> {
        Ok(edr_solidity::artifacts::ArtifactId {
            name: value.name,
            source: value.source.parse().map_err(|_err| {
                napi::Error::new(napi::Status::GenericFailure, "Invalid source path")
            })?,
            version: value.solc_version.parse().map_err(|_err| {
                napi::Error::new(napi::Status::GenericFailure, "Invalid solc semver string")
            })?,
        })
    }
}

/// A test contract to execute.
#[derive(Clone, Debug)]
#[napi(object)]
pub struct ContractData {
    /// Contract ABI as json string.
    pub abi: String,
    /// Contract creation code as hex string. It can be missing if the contract
    /// is ABI only.
    pub bytecode: Option<String>,
    /// The link references of the deployment bytecode.
    pub link_references: Option<HashMap<String, HashMap<String, Vec<LinkReference>>>>,
    /// Contract runtime code as hex string. It can be missing if the contract
    /// is ABI only.
    pub deployed_bytecode: Option<String>,
    /// The link references of the deployed bytecode.
    pub deployed_link_references: Option<HashMap<String, HashMap<String, Vec<LinkReference>>>>,
}

impl TryFrom<ContractData> for foundry_compilers::artifacts::CompactContractBytecode {
    type Error = napi::Error;

    fn try_from(contract: ContractData) -> napi::Result<Self> {
        Ok(foundry_compilers::artifacts::CompactContractBytecode {
            abi: Some(serde_json::from_str(&contract.abi).map_err(|_err| {
                napi::Error::new(napi::Status::GenericFailure, "Invalid JSON ABI")
            })?),
            bytecode: contract.bytecode.map(|bytecode| {
                foundry_compilers::artifacts::CompactBytecode {
                    object: foundry_compilers::artifacts::BytecodeObject::Unlinked(bytecode),
                    source_map: None,
                    link_references: convert_link_references(
                        contract.link_references.unwrap_or_default(),
                    ),
                }
            }),
            deployed_bytecode: contract.deployed_bytecode.map(|deployed_bytecode| {
                let compact_bytecode = foundry_compilers::artifacts::CompactBytecode {
                    object: foundry_compilers::artifacts::BytecodeObject::Unlinked(
                        deployed_bytecode,
                    ),
                    source_map: None,
                    link_references: convert_link_references(
                        contract.deployed_link_references.unwrap_or_default(),
                    ),
                };
                foundry_compilers::artifacts::CompactDeployedBytecode {
                    bytecode: Some(compact_bytecode),
                    immutable_references: BTreeMap::default(),
                }
            }),
        })
    }
}

impl TryFrom<ContractData> for foundry_compilers::artifacts::CompactContractBytecodeCow<'static> {
    type Error = napi::Error;

    fn try_from(contract: ContractData) -> napi::Result<Self> {
        let c: foundry_compilers::artifacts::CompactContractBytecode = contract.try_into()?;
        Ok(CompactContractBytecodeCow {
            abi: c.abi.map(Cow::Owned),
            bytecode: c.bytecode.map(Cow::Owned),
            deployed_bytecode: c.deployed_bytecode.map(Cow::Owned),
        })
    }
}

fn convert_link_references(
    link_references: HashMap<String, HashMap<String, Vec<LinkReference>>>,
) -> BTreeMap<String, BTreeMap<String, Vec<foundry_compilers::artifacts::Offsets>>> {
    link_references
        .into_iter()
        .map(|(file, libraries)| {
            let lib_map = libraries
                .into_iter()
                .map(|(library_name, references)| {
                    let offsets = references.into_iter().map(Into::into).collect();
                    (library_name, offsets)
                })
                .collect();
            (file, lib_map)
        })
        .collect()
}

#[derive(Clone, Debug)]
#[napi(object)]
pub struct LinkReference {
    pub start: u32,
    pub length: u32,
}

impl From<LinkReference> for foundry_compilers::artifacts::Offsets {
    fn from(value: LinkReference) -> Self {
        Self {
            start: value.start,
            length: value.length,
        }
    }
}
