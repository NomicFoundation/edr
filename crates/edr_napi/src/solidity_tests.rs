pub mod artifact;
pub mod cheatcode_errors;
pub mod config;
pub mod factory;
pub mod inline_config;
pub mod l1;
#[cfg(feature = "op")]
pub mod op;
pub mod runner;
pub mod test_results;

use std::{borrow::Cow, path::Path};

use edr_napi_core::solidity::config::BuildInfoAndOutput;
use edr_primitives::Bytes;
use edr_solidity::{
    linker::{LinkOutput, Linker},
    project::{default_build_info_dir, find_build_info_files, load_artifacts, LoadedArtifact},
};
use edr_solidity_tests::{constants::LIBRARY_DEPLOYER, contracts::ContractsByArtifact};
use foundry_compilers::artifacts::{CompactContractBytecodeCow, Libraries};

use crate::solidity_tests::artifact::{Artifact, TestSuiteReference};

/// Compilation artifacts as consumed by the linker, keyed by artifact id.
pub(crate) type ArtifactContracts = Vec<(
    edr_artifact::ArtifactId,
    CompactContractBytecodeCow<'static>,
)>;

pub(crate) struct LinkingOutput {
    pub libs_to_deploy: Vec<Bytes>,
    pub known_contracts: ContractsByArtifact,
}

impl LinkingOutput {
    pub fn link(project_root: &Path, artifact_contracts: ArtifactContracts) -> napi::Result<Self> {
        let linker = Linker::new(project_root, artifact_contracts);

        let LinkOutput {
            libraries,
            libs_to_deploy,
        } = linker
            .link_with_nonce_or_address(
                Libraries::default(),
                LIBRARY_DEPLOYER,
                0,
                linker.contracts.keys(),
            )
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        let linked_contracts = linker
            .get_linked_artifacts(&libraries)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        let known_contracts = ContractsByArtifact::new(linked_contracts);

        Ok(LinkingOutput {
            libs_to_deploy,
            known_contracts,
        })
    }
}

/// Converts NAPI artifacts into linker inputs.
pub(crate) fn artifact_contracts_from_napi(
    artifacts: Vec<Artifact>,
) -> napi::Result<ArtifactContracts> {
    artifacts
        .into_iter()
        .map(|artifact| Ok((artifact.id.try_into()?, artifact.contract.try_into()?)))
        .collect()
}

/// The test runner inputs loaded from artifact directories on disk.
pub(crate) struct ProjectInputs {
    /// All of the project's compilation artifacts.
    pub artifact_contracts: ArtifactContracts,
    /// The resolved ids of the test suites to execute.
    pub test_suites: Vec<edr_artifact::ArtifactId>,
    /// The build infos used for stack trace generation.
    pub build_infos: Vec<BuildInfoAndOutput>,
}

/// Loads all artifacts and build infos from the provided artifact directories
/// and resolves the test suite references against them.
pub(crate) fn load_project_inputs(
    artifacts_directories: &[String],
    test_suites: Vec<TestSuiteReference>,
) -> napi::Result<ProjectInputs> {
    let mut artifacts: Vec<LoadedArtifact> = Vec::new();
    let mut build_infos = Vec::new();

    for artifacts_directory in artifacts_directories {
        let artifacts_directory = Path::new(artifacts_directory);

        artifacts.extend(
            load_artifacts(artifacts_directory)
                .map_err(|error| napi::Error::from_reason(error.to_string()))?,
        );

        let build_info_dir = default_build_info_dir(artifacts_directory);
        for build_info in find_build_info_files(&build_info_dir)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?
        {
            let output_path = build_info.output_path.ok_or_else(|| {
                napi::Error::from_reason(format!(
                    "Missing output file for build info '{}' in '{}'",
                    build_info.id,
                    build_info_dir.display()
                ))
            })?;

            let build_info_buffer =
                std::fs::read(&build_info.build_info_path).map_err(|error| {
                    napi::Error::from_reason(format!(
                        "Failed to read '{}': {error}",
                        build_info.build_info_path.display()
                    ))
                })?;
            let output_buffer = std::fs::read(&output_path).map_err(|error| {
                napi::Error::from_reason(format!(
                    "Failed to read '{}': {error}",
                    output_path.display()
                ))
            })?;

            build_infos.push(BuildInfoAndOutput {
                build_info: build_info_buffer.into(),
                output: output_buffer.into(),
            });
        }
    }

    let test_suites = resolve_test_suites(&artifacts, test_suites)?;

    let artifact_contracts = artifacts
        .into_iter()
        .map(|artifact| {
            let contract = CompactContractBytecodeCow {
                abi: artifact.contract.abi.map(Cow::Owned),
                bytecode: artifact.contract.bytecode.map(Cow::Owned),
                deployed_bytecode: artifact.contract.deployed_bytecode.map(Cow::Owned),
            };

            (artifact.id, contract)
        })
        .collect();

    Ok(ProjectInputs {
        artifact_contracts,
        test_suites,
        build_infos,
    })
}

/// Resolves test suite references to the ids of the loaded artifacts. The
/// reference's source may be either the user-facing source name or the
/// compiler input source name.
fn resolve_test_suites(
    artifacts: &[LoadedArtifact],
    test_suites: Vec<TestSuiteReference>,
) -> napi::Result<Vec<edr_artifact::ArtifactId>> {
    test_suites
        .into_iter()
        .map(|test_suite| {
            artifacts
                .iter()
                .find(|artifact| {
                    artifact.id.name == test_suite.name
                        && (artifact.user_source_name == test_suite.source
                            || artifact.id.source == Path::new(&test_suite.source))
                })
                .map(|artifact| artifact.id.clone())
                .ok_or_else(|| {
                    napi::Error::new(
                        napi::Status::GenericFailure,
                        format!(
                            "Unknown test suite contract: {}:{}",
                            test_suite.source, test_suite.name
                        ),
                    )
                })
        })
        .collect()
}
