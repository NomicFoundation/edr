use foundry_compilers::artifacts::Libraries;
use napi_derive::napi;

/// A test suite is a contract and its test methods.
#[derive(Clone)]
#[napi(object)]
pub struct TestSuite {
    /// The identifier of the test suite.
    pub id: ArtifactId,
    /// The test contract.
    pub contract: TestContract,
}

/// The identifier of a Solidity test contract.
#[derive(Clone)]
#[napi(object)]
pub struct ArtifactId {
    /// The name of the contract.
    pub name: String,
    /// Original source file path.
    pub source: String,
    /// The solc semver string.
    pub solc_version: String,
    /// The artifact cache path. Currently unused.
    pub artifact_cache_path: String,
}

impl TryFrom<ArtifactId> for foundry_compilers::ArtifactId {
    type Error = napi::Error;

    fn try_from(value: ArtifactId) -> napi::Result<Self> {
        Ok(foundry_compilers::ArtifactId {
            path: value.artifact_cache_path.parse().map_err(|_err| {
                napi::Error::new(napi::Status::GenericFailure, "Invalid artifact cache path")
            })?,
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
#[derive(Clone)]
#[napi(object)]
pub struct TestContract {
    /// The contract ABI as a JSON string.
    pub abi: String,
    /// The contract bytecode including all libraries as a hex string.
    pub bytecode: String,
    /// Vector of library bytecodes to deploy as hex string.
    pub libs_to_deploy: Vec<String>,
    /// Vector of library specifications of the form corresponding to libs to
    /// deploy, example item:
    /// `"src/DssSpell.sol:DssExecLib:
    /// 0xfD88CeE74f7D78697775aBDAE53f9Da1559728E4"`
    pub libraries: Vec<String>,
}

impl TryFrom<TestContract> for forge::multi_runner::TestContract {
    type Error = napi::Error;

    fn try_from(contract: TestContract) -> napi::Result<Self> {
        Ok(forge::multi_runner::TestContract {
            abi: serde_json::from_str(&contract.abi).map_err(|_err| {
                napi::Error::new(napi::Status::GenericFailure, "Invalid JSON ABI")
            })?,
            bytecode: contract.bytecode.parse().map_err(|_err| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "Invalid hex bytecode for test contract",
                )
            })?,
            // Hardhat builds all libraries into the contract bytecode, so we don't need to link any
            // other libraries.
            libs_to_deploy: contract
                .libs_to_deploy
                .into_iter()
                .map(|lib| {
                    lib.parse().map_err(|_err| {
                        napi::Error::new(
                            napi::Status::GenericFailure,
                            "Invalid hex bytecode for library",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            libraries: Libraries::parse(&contract.libraries).map_err(|_err| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "Invalid library specifications",
                )
            })?,
        })
    }
}
