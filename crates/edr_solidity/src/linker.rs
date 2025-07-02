//! EVM bytecode linker based on Foundry's linker: <https://github.com/foundry-rs/foundry/blob/5101a32b50a71741741730d351834cb190927b51/crates/linking/src/lib.rs>

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    str::FromStr,
};

use alloy_primitives::{Address, Bytes, B256};
use foundry_compilers::{
    artifacts::{CompactContractBytecodeCow, Libraries},
    Artifact,
};
use semver::Version;

use crate::artifacts::ArtifactId;

/// A map of artifact identifiers to their corresponding bytecode.
///
/// This type is used throughout the linker to store and manipulate contract
/// artifacts that may need to be linked with libraries.
pub type ArtifactContracts<'a> = BTreeMap<ArtifactId, CompactContractBytecodeCow<'a>>;

/// Errors that can occur during linking.
#[derive(Debug, thiserror::Error)]
pub enum LinkerError {
    /// Error that occurs when bytecode cannot be extracted as bytes.
    #[error("failed to extract bytecode as bytes for artifact '{artifact_name}'")]
    BytecodeExtractionFailed {
        /// The name of the artifact.
        artifact_name: String,
    },

    /// Error that occurs when an artifact is missing its bytecode.
    #[error("artifact '{artifact_name}' is missing bytecode")]
    MissingBytecode {
        /// The name of the artifact missing bytecode.
        artifact_name: String,
    },

    /// Error that occurs when a library artifact cannot be found at the
    /// specified file path with the given name.
    #[error("wasn't able to find artifact for library '{library_name}' at '{library_file_path}' when linking '{contract_name}' at '{contract_file_path}'")]
    MissingLibraryArtifact {
        /// The file path of the contract being linked
        contract_file_path: String,
        /// The name of the contract being linked
        contract_name: String,
        /// The file path of the library being linked
        library_file_path: String,
        /// The name of the library being linked
        library_name: String,
    },

    /// Error that occurs when the target artifact to link is not present in the
    /// provided artifacts set.
    #[error("target artifact is not present in provided artifacts set")]
    MissingTargetArtifact,

    /// Error that occurs when an invalid Ethereum address is provided for
    /// linking.
    #[error(transparent)]
    InvalidAddress(<Address as std::str::FromStr>::Err),

    /// Error that occurs when a cyclic dependency is detected, which prevents
    /// successful CREATE2-based linking.
    #[error("cyclic dependency found, can't link libraries via CREATE2")]
    CyclicDependency,
}

/// A library for resolving and linking EVM bytecode dependencies.
///
/// The `Linker` manages the resolution and linking of contract dependencies,
/// particularly for Solidity libraries that are referenced in contract
/// bytecode. It supports both regular CREATE-based deployment and CREATE2-based
/// deployment strategies.
pub struct Linker<'a> {
    /// Root of the project, used to determine whether artifact/library path can
    /// be stripped.
    pub root: PathBuf,
    /// Compilation artifacts containing the contracts to be linked.
    pub contracts: ArtifactContracts<'a>,
}

/// Output produced by the linking process.
///
/// Contains the resolved library addresses and the libraries that need to be
/// deployed.
pub struct LinkOutput {
    /// Resolved library addresses. Contains both user-provided and newly
    /// deployed libraries. It will always contain library paths with
    /// stripped path prefixes.
    pub libraries: Libraries,
    /// Vector of libraries that need to be deployed from sender address.
    /// The order in which they appear in the vector is the order in which they
    /// should be deployed.
    pub libs_to_deploy: Vec<Bytes>,
}

impl<'a> Linker<'a> {
    /// Creates a new Linker instance.
    ///
    /// # Parameters
    /// - `root`: The root path of the project, used to normalize paths
    /// - `contracts`: An iterator of artifact IDs and their corresponding
    ///   bytecode
    ///
    /// # Returns
    /// A new `Linker` instance with the specified root path and contracts
    pub fn new(
        root: impl Into<PathBuf>,
        contracts: impl IntoIterator<Item = (ArtifactId, CompactContractBytecodeCow<'a>)>,
    ) -> Self {
        let root = root.into();
        let contracts = contracts
            .into_iter()
            .map(|(id, contract)| (id.with_stripped_file_prefixes(&root), contract))
            .collect();
        Linker { root, contracts }
    }

    /// Helper method to convert [`ArtifactId`] to the format in which libraries
    /// are stored in [`Libraries`] object.
    ///
    /// Strips project root path from source file path.
    fn convert_artifact_id_to_lib_path(&self, id: &ArtifactId) -> (PathBuf, String) {
        let path = id
            .source
            .strip_prefix(self.root.as_path())
            .unwrap_or(&id.source);
        // name is either {LibName} or {LibName}.{version}
        let name = id
            .name
            .split('.')
            .next()
            .expect("split never returns empty iterator");

        (path.to_path_buf(), name.to_owned())
    }

    /// Finds an [`ArtifactId`] object in the given [`ArtifactContracts`] keys
    /// which corresponds to the library path in the form of
    /// "./path/to/Lib.sol:Lib".
    ///
    /// If there are multiple matching artifacts based on the name and the path,
    /// returns the one that has the same version as the contract being linked.
    /// If there isn't, returns the latest one.
    fn find_artifact_id_by_library_path(
        &'a self,
        file: &str,
        name: &str,
        version: &Version,
    ) -> Option<&'a ArtifactId> {
        // Find all the matching artifacts
        let matching_artifacts = self
            .contracts
            .keys()
            .filter(|id| {
                let (artifact_path, artifact_name) = self.convert_artifact_id_to_lib_path(id);

                artifact_name == *name && artifact_path == Path::new(file)
            })
            .collect::<Vec<_>>();

        if matching_artifacts.len() < 2 {
            // If there's only one matching artifact, return that. Return `None` if there
            // are no matching artifacts.
            matching_artifacts.into_iter().next_back()
        } else {
            // If there's more than one, use the one that has the same version as the
            // contract being linked. If there isn't, use the latest one.
            matching_artifacts
                .iter()
                .copied()
                .find(|&id| &id.version == version)
                .or_else(|| matching_artifacts.into_iter().max_by_key(|id| &id.version))
        }
    }

    /// Performs DFS on the graph of link references, and populates `deps` with
    /// all found libraries.
    fn collect_dependencies(
        &'a self,
        target: &'a ArtifactId,
        deps: &mut BTreeSet<&'a ArtifactId>,
    ) -> Result<(), LinkerError> {
        let contract = self
            .contracts
            .get(target)
            .ok_or(LinkerError::MissingTargetArtifact)?;

        let mut references = BTreeMap::new();
        if let Some(bytecode) = &contract.bytecode {
            references.extend(bytecode.link_references.clone());
        }
        if let Some(deployed_bytecode) = &contract.deployed_bytecode {
            if let Some(bytecode) = &deployed_bytecode.bytecode {
                references.extend(bytecode.link_references.clone());
            }
        }

        for (file_path, libs) in &references {
            for contract in libs.keys() {
                let id = self
                    .find_artifact_id_by_library_path(file_path, contract, &target.version)
                    .ok_or_else(|| LinkerError::MissingLibraryArtifact {
                        contract_file_path: target.source.to_string_lossy().to_string(),
                        contract_name: target.name.clone(),
                        library_file_path: file_path.clone(),
                        library_name: contract.clone(),
                    })?;
                if deps.insert(id) {
                    self.collect_dependencies(id, deps)?;
                }
            }
        }

        Ok(())
    }

    /// Links given artifacts with either given library addresses or computes
    /// addresses from sender and nonce.
    ///
    /// This method resolves all library dependencies for the specified targets
    /// and either uses provided library addresses or computes new ones
    /// based on the sender's address and nonce for CREATE-based
    /// deployments.
    ///
    /// # Parameters
    /// - `deployed_libraries`: Already deployed libraries with their addresses
    /// - `sender`: The address that will deploy the libraries
    /// - `nonce`: The starting nonce to use for computing library addresses
    /// - `targets`: Artifacts to link libraries for
    ///
    /// # Returns
    /// A `LinkOutput` containing resolved library addresses and libraries that
    /// need deployment
    ///
    /// # Errors
    /// Returns a `LinkerError` if library artifacts are missing or addresses
    /// are invalid
    ///
    /// # Notes
    /// Each key in `deployed_libraries` should either be a global path or
    /// relative to project root. All remappings should be resolved.
    ///
    /// When calling for `target` being an external library itself, you should
    /// check that `target` does not appear in `libs_to_deploy` to avoid
    /// deploying it twice. It may happen in cases when there is a
    /// dependency cycle including `target`.
    pub fn link_with_nonce_or_address(
        &'a self,
        deployed_libraries: Libraries,
        sender: Address,
        mut nonce: u64,
        targets: impl IntoIterator<Item = &'a ArtifactId>,
    ) -> Result<LinkOutput, LinkerError> {
        // Library paths in `link_references` keys are always stripped, so we have to
        // strip user-provided paths to be able to match them correctly.
        let mut libraries = deployed_libraries.with_stripped_file_prefixes(self.root.as_path());

        let mut needed_libraries = BTreeSet::new();
        for target in targets {
            self.collect_dependencies(target, &mut needed_libraries)?;
        }

        let mut libs_to_deploy = Vec::new();

        // If `libraries` does not contain needed dependency, compute its address and
        // add to `libs_to_deploy`.
        for id in needed_libraries {
            let (lib_path, lib_name) = self.convert_artifact_id_to_lib_path(id);

            libraries
                .libs
                .entry(lib_path)
                .or_default()
                .entry(lib_name)
                .or_insert_with(|| {
                    let address = sender.create(nonce);
                    libs_to_deploy.push((id, address));
                    nonce += 1;

                    address.to_checksum(None)
                });
        }

        // Link and collect bytecodes for `libs_to_deploy`.
        let libs_to_deploy = libs_to_deploy
            .into_iter()
            .map(|(id, _address)| {
                let linked_contract = self.link(id, &libraries)?;
                let bytecode_bytes = linked_contract.get_bytecode_bytes().ok_or_else(|| {
                    LinkerError::BytecodeExtractionFailed {
                        artifact_name: id.name.clone(),
                    }
                })?;
                Ok(bytecode_bytes.into_owned())
            })
            .collect::<Result<Vec<_>, LinkerError>>()?;

        Ok(LinkOutput {
            libraries,
            libs_to_deploy: libs_to_deploy,
        })
    }

    /// Links libraries using CREATE2 deployment method.
    ///
    /// This method resolves all library dependencies for the specified target
    /// and either uses provided library addresses or computes new ones
    /// based on CREATE2 deployment with the specified sender and salt.
    ///
    /// # Parameters
    /// - `deployed_libraries`: Already deployed libraries with their addresses
    /// - `sender`: The address that will deploy the libraries
    /// - `salt`: The salt to use for CREATE2 deployment
    /// - `target`: The artifact to link libraries for
    ///
    /// # Returns
    /// A `LinkOutput` containing resolved library addresses and libraries that
    /// need deployment
    ///
    /// # Errors
    /// Returns a `LinkerError` if library artifacts are missing, addresses are
    /// invalid, or a cyclic dependency is found (CREATE2 cannot handle
    /// cyclic dependencies)
    pub fn link_with_create2(
        &'a self,
        deployed_libraries: Libraries,
        sender: Address,
        salt: B256,
        target: &'a ArtifactId,
    ) -> Result<LinkOutput, LinkerError> {
        // Library paths in `link_references` keys are always stripped, so we have to
        // strip user-provided paths to be able to match them correctly.
        let mut libraries = deployed_libraries.with_stripped_file_prefixes(self.root.as_path());

        let mut needed_libraries = BTreeSet::new();
        self.collect_dependencies(target, &mut needed_libraries)?;

        let mut needed_libraries = needed_libraries
            .into_iter()
            .filter(|id| {
                // Filter out already provided libraries.
                let (file, name) = self.convert_artifact_id_to_lib_path(id);
                !libraries.libs.contains_key(&file) || !libraries.libs[&file].contains_key(&name)
            })
            .map(|id| {
                // Link library with provided libs and extract bytecode object (possibly
                // unlinked).
                let linked_contract = self.link(id, &libraries)?;
                let bytecode =
                    linked_contract
                        .bytecode
                        .ok_or_else(|| LinkerError::MissingBytecode {
                            artifact_name: id.name.clone(),
                        })?;
                Ok((id, bytecode))
            })
            .collect::<Result<Vec<_>, LinkerError>>()?;

        let mut libs_to_deploy = Vec::new();

        // Iteratively compute addresses and link libraries until we have no unlinked
        // libraries left.
        while !needed_libraries.is_empty() {
            // Find any library which is fully linked.
            let deployable = needed_libraries
                .iter()
                .enumerate()
                .find(|(_, (_, bytecode))| !bytecode.object.is_unlinked());

            // If we haven't found any deployable library, it means we have a cyclic
            // dependency.
            let Some((index, &(id, _))) = deployable else {
                return Err(LinkerError::CyclicDependency);
            };
            let (_, bytecode) = needed_libraries.swap_remove(index);
            let code = bytecode
                .bytes()
                .ok_or_else(|| LinkerError::BytecodeExtractionFailed {
                    artifact_name: id.name.clone(),
                })?;
            let address = sender.create2_from_code(salt, code);
            libs_to_deploy.push(code.clone());

            let (file, name) = self.convert_artifact_id_to_lib_path(id);

            for (_, bytecode) in &mut needed_libraries {
                bytecode
                    .to_mut()
                    .link(&file.to_string_lossy(), &name, address);
            }

            libraries
                .libs
                .entry(file)
                .or_default()
                .insert(name, address.to_checksum(None));
        }

        Ok(LinkOutput {
            libraries,
            libs_to_deploy,
        })
    }

    /// Links a specific artifact with given libraries.
    ///
    /// This method performs the actual linking of a contract's bytecode with
    /// the specified library addresses.
    ///
    /// # Parameters
    /// - `target`: The artifact to link
    /// - `libraries`: The libraries with their addresses to link into the
    ///   bytecode
    ///
    /// # Returns
    /// The contract bytecode with libraries linked
    ///
    /// # Errors
    /// Returns a `LinkerError` if the target artifact is not found or if
    /// library addresses are invalid
    pub fn link(
        &self,
        target: &ArtifactId,
        libraries: &Libraries,
    ) -> Result<CompactContractBytecodeCow<'a>, LinkerError> {
        let mut contract = self
            .contracts
            .get(target)
            .ok_or(LinkerError::MissingTargetArtifact)?
            .clone();
        for (file, libs) in &libraries.libs {
            for (name, address) in libs {
                let address = Address::from_str(address).map_err(LinkerError::InvalidAddress)?;
                if let Some(bytecode) = contract.bytecode.as_mut() {
                    let bytecode_mut = bytecode.to_mut();
                    if !bytecode_mut.link(&file.to_string_lossy(), name, address) {
                        // If we didn't link, there is nothing to link. By calling `resolve()` we
                        // make sure that the `BytecodeObject::Unlinked` is turned into
                        // `BytecodeObject:Bytecode`.
                        bytecode_mut.object.resolve();
                    }
                }
                if let Some(deployed_bytecode) = contract
                    .deployed_bytecode
                    .as_mut()
                    .and_then(|b| b.to_mut().bytecode.as_mut())
                {
                    if !deployed_bytecode.link(&file.to_string_lossy(), name, address) {
                        // If we didn't link, there is nothing to link. By calling `resolve()` we
                        // make sure that the `BytecodeObject::Unlinked` is turned into
                        // `BytecodeObject:Bytecode`.
                        deployed_bytecode.object.resolve();
                    }
                }
            }
        }
        Ok(contract)
    }

    /// Gets all artifacts with libraries linked.
    ///
    /// Links all artifacts in the linker's collection with the provided
    /// libraries.
    ///
    /// # Parameters
    /// - `libraries`: The libraries with their addresses to link into all
    ///   artifacts
    ///
    /// # Returns
    /// A map of artifact IDs to their linked bytecode
    ///
    /// # Errors
    /// Returns a `LinkerError` if any artifact is not found or if library
    /// addresses are invalid
    pub fn get_linked_artifacts(
        &self,
        libraries: &Libraries,
    ) -> Result<ArtifactContracts<'_>, LinkerError> {
        self.contracts
            .keys()
            .map(|id| Ok((id.clone(), self.link(id, libraries)?)))
            .collect()
    }

    /// Gets all artifacts with libraries linked, preserving the lifetime of the
    /// bytecode.
    ///
    /// Similar to `get_linked_artifacts`, but preserves the lifetime of the
    /// original bytecode.
    ///
    /// # Parameters
    /// - `libraries`: The libraries with their addresses to link into all
    ///   artifacts
    ///
    /// # Returns
    /// A map of artifact IDs to their linked bytecode with preserved lifetime
    ///
    /// # Errors
    /// Returns a `LinkerError` if any artifact is not found or if library
    /// addresses are invalid
    pub fn get_linked_artifacts_cow(
        &self,
        libraries: &Libraries,
    ) -> Result<ArtifactContracts<'a>, LinkerError> {
        self.contracts
            .keys()
            .map(|id| Ok((id.clone(), self.link(id, libraries)?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{fixed_bytes, map::HashMap};
    use foundry_compilers::{
        multi::MultiCompiler,
        solc::{Solc, SolcCompiler},
        Project, ProjectCompileOutput, ProjectPathsConfig,
    };

    use super::*;

    struct LinkerTest {
        project: Project,
        output: ProjectCompileOutput,
        dependency_assertions: HashMap<String, Vec<(String, Address)>>,
    }

    impl LinkerTest {
        fn new(path: impl Into<PathBuf>, strip_prefixes: bool) -> Self {
            let path = path.into();
            let paths = ProjectPathsConfig::builder()
                .root("../../testdata")
                .lib("../../testdata/lib")
                .sources(path.clone())
                .tests(path)
                .build()
                .unwrap();

            let solc = Solc::find_or_install(&Version::new(0, 8, 18)).unwrap();
            let project = Project::builder()
                .paths(paths)
                .ephemeral()
                .no_artifacts()
                .build(MultiCompiler {
                    solc: Some(SolcCompiler::Specific(solc)),
                    vyper: None,
                })
                .unwrap();

            let mut output = project.compile().unwrap();

            if strip_prefixes {
                output = output.with_stripped_file_prefixes(project.root());
            }

            Self {
                project,
                output,
                dependency_assertions: HashMap::default(),
            }
        }

        fn artifact_contracts(&self) -> ArtifactContracts<'_> {
            self.output
                .artifact_ids()
                .map(|(id, artifact)| {
                    let id = ArtifactId {
                        name: id.name,
                        source: id.source,
                        version: id.version,
                    };
                    (id, artifact.into())
                })
                .collect()
        }

        fn assert_dependencies(
            mut self,
            artifact_id: String,
            deps: Vec<(String, Address)>,
        ) -> Self {
            self.dependency_assertions.insert(artifact_id, deps);
            self
        }

        fn test_with_sender_and_nonce(self, sender: Address, initial_nonce: u64) {
            let linker = Linker::new(self.project.root(), self.artifact_contracts());
            for (id, identifier) in self.iter_linking_targets(&linker) {
                let output = linker
                    .link_with_nonce_or_address(Libraries::default(), sender, initial_nonce, [id])
                    .expect("Linking failed");
                self.validate_assertions(identifier, output);
            }
        }

        fn test_with_create2(self, sender: Address, salt: B256) {
            let linker = Linker::new(self.project.root(), self.artifact_contracts());
            for (id, identifier) in self.iter_linking_targets(&linker) {
                let output = linker
                    .link_with_create2(Libraries::default(), sender, salt, id)
                    .expect("Linking failed");
                self.validate_assertions(identifier, output);
            }
        }

        fn iter_linking_targets<'a>(
            &'a self,
            linker: &'a Linker<'_>,
        ) -> impl IntoIterator<Item = (&'a ArtifactId, String)> + 'a {
            linker.contracts.keys().filter_map(move |id| {
                // If we didn't strip paths, artifacts will have absolute paths.
                // That's expected and we want to ensure that only `libraries` object has
                // relative paths, artifacts should be kept as is.
                let source = id
                    .source
                    .strip_prefix(self.project.root())
                    .unwrap_or(&id.source)
                    .to_string_lossy();
                let identifier = format!("{source}:{}", id.name);

                // Skip ds-test as it always has no dependencies etc. (and the path is outside
                // root so is not sanitized)
                if identifier.contains("DSTest") {
                    return None;
                }

                Some((id, identifier))
            })
        }

        fn validate_assertions(&self, identifier: String, output: LinkOutput) {
            let LinkOutput {
                libs_to_deploy,
                libraries,
            } = output;

            let assertions = self
                .dependency_assertions
                .get(&identifier)
                .unwrap_or_else(|| panic!("Unexpected artifact: {identifier}"));

            assert_eq!(
                libs_to_deploy.len(),
                assertions.len(),
                "artifact {identifier} has more/less dependencies than expected ({} vs {}): {:#?}",
                libs_to_deploy.len(),
                assertions.len(),
                libs_to_deploy
            );

            for (dep_identifier, address) in assertions {
                let (file, name) = dep_identifier.split_once(':').unwrap();
                if let Some(lib_address) = libraries
                    .libs
                    .get(Path::new(file))
                    .and_then(|libs| libs.get(name))
                {
                    assert_eq!(
                        *lib_address,
                        address.to_string(),
                        "incorrect library address for dependency {dep_identifier} of {identifier}"
                    );
                } else {
                    panic!("Library {dep_identifier} not found");
                }
            }
        }
    }

    fn link_test(path: impl Into<PathBuf>, test_fn: impl Fn(LinkerTest)) {
        let path = path.into();
        test_fn(LinkerTest::new(path.clone(), true));
        test_fn(LinkerTest::new(path, false));
    }

    #[test]
    fn link_simple() {
        link_test("../../testdata/default/linking/simple", |linker| {
            linker
                .assert_dependencies(
                    "default/linking/simple/Simple.t.sol:Lib".to_string(),
                    vec![],
                )
                .assert_dependencies(
                    "default/linking/simple/Simple.t.sol:LibraryConsumer".to_string(),
                    vec![(
                        "default/linking/simple/Simple.t.sol:Lib".to_string(),
                        Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3").unwrap(),
                    )],
                )
                .assert_dependencies(
                    "default/linking/simple/Simple.t.sol:SimpleLibraryLinkingTest".to_string(),
                    vec![(
                        "default/linking/simple/Simple.t.sol:Lib".to_string(),
                        Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3").unwrap(),
                    )],
                )
                .test_with_sender_and_nonce(Address::default(), 1);
        });
    }

    #[test]
    fn link_nested() {
        link_test("../../testdata/default/linking/nested", |linker| {
            linker
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:Lib".to_string(),
                    vec![],
                )
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:NestedLib".to_string(),
                    vec![(
                        "default/linking/nested/Nested.t.sol:Lib".to_string(),
                        Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3").unwrap(),
                    )],
                )
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:LibraryConsumer".to_string(),
                    vec![
                        // Lib shows up here twice, because the linker sees it twice, but it should
                        // have the same address and nonce.
                        (
                            "default/linking/nested/Nested.t.sol:Lib".to_string(),
                            Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3")
                                .unwrap(),
                        ),
                        (
                            "default/linking/nested/Nested.t.sol:NestedLib".to_string(),
                            Address::from_str("0x47e9Fbef8C83A1714F1951F142132E6e90F5fa5D")
                                .unwrap(),
                        ),
                    ],
                )
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:NestedLibraryLinkingTest".to_string(),
                    vec![
                        (
                            "default/linking/nested/Nested.t.sol:Lib".to_string(),
                            Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3")
                                .unwrap(),
                        ),
                        (
                            "default/linking/nested/Nested.t.sol:NestedLib".to_string(),
                            Address::from_str("0x47e9fbef8c83a1714f1951f142132e6e90f5fa5d")
                                .unwrap(),
                        ),
                    ],
                )
                .test_with_sender_and_nonce(Address::default(), 1);
        });
    }

    #[test]
    fn link_duplicate() {
        link_test("../../testdata/default/linking/duplicate", |linker| {
            linker
                .assert_dependencies(
                    "default/linking/duplicate/Duplicate.t.sol:A".to_string(),
                    vec![],
                )
                .assert_dependencies(
                    "default/linking/duplicate/Duplicate.t.sol:B".to_string(),
                    vec![],
                )
                .assert_dependencies(
                    "default/linking/duplicate/Duplicate.t.sol:C".to_string(),
                    vec![(
                        "default/linking/duplicate/Duplicate.t.sol:A".to_string(),
                        Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3").unwrap(),
                    )],
                )
                .assert_dependencies(
                    "default/linking/duplicate/Duplicate.t.sol:D".to_string(),
                    vec![(
                        "default/linking/duplicate/Duplicate.t.sol:B".to_string(),
                        Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3").unwrap(),
                    )],
                )
                .assert_dependencies(
                    "default/linking/duplicate/Duplicate.t.sol:E".to_string(),
                    vec![
                        (
                            "default/linking/duplicate/Duplicate.t.sol:A".to_string(),
                            Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:C".to_string(),
                            Address::from_str("0x47e9fbef8c83a1714f1951f142132e6e90f5fa5d")
                                .unwrap(),
                        ),
                    ],
                )
                .assert_dependencies(
                    "default/linking/duplicate/Duplicate.t.sol:LibraryConsumer".to_string(),
                    vec![
                        (
                            "default/linking/duplicate/Duplicate.t.sol:A".to_string(),
                            Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:B".to_string(),
                            Address::from_str("0x47e9fbef8c83a1714f1951f142132e6e90f5fa5d")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:C".to_string(),
                            Address::from_str("0x8be503bcded90ed42eff31f56199399b2b0154ca")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:D".to_string(),
                            Address::from_str("0x47c5e40890bce4a473a49d7501808b9633f29782")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:E".to_string(),
                            Address::from_str("0x29b2440db4a256b0c1e6d3b4cdcaa68e2440a08f")
                                .unwrap(),
                        ),
                    ],
                )
                .assert_dependencies(
                    "default/linking/duplicate/Duplicate.t.sol:DuplicateLibraryLinkingTest"
                        .to_string(),
                    vec![
                        (
                            "default/linking/duplicate/Duplicate.t.sol:A".to_string(),
                            Address::from_str("0x5a443704dd4b594b382c22a083e2bd3090a6fef3")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:B".to_string(),
                            Address::from_str("0x47e9fbef8c83a1714f1951f142132e6e90f5fa5d")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:C".to_string(),
                            Address::from_str("0x8be503bcded90ed42eff31f56199399b2b0154ca")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:D".to_string(),
                            Address::from_str("0x47c5e40890bce4a473a49d7501808b9633f29782")
                                .unwrap(),
                        ),
                        (
                            "default/linking/duplicate/Duplicate.t.sol:E".to_string(),
                            Address::from_str("0x29b2440db4a256b0c1e6d3b4cdcaa68e2440a08f")
                                .unwrap(),
                        ),
                    ],
                )
                .test_with_sender_and_nonce(Address::default(), 1);
        });
    }

    #[test]
    fn link_cycle() {
        link_test("../../testdata/default/linking/cycle", |linker| {
            linker
                .assert_dependencies(
                    "default/linking/cycle/Cycle.t.sol:Foo".to_string(),
                    vec![
                        (
                            "default/linking/cycle/Cycle.t.sol:Foo".to_string(),
                            Address::from_str("0x47e9Fbef8C83A1714F1951F142132E6e90F5fa5D")
                                .unwrap(),
                        ),
                        (
                            "default/linking/cycle/Cycle.t.sol:Bar".to_string(),
                            Address::from_str("0x5a443704dd4B594B382c22a083e2BD3090A6feF3")
                                .unwrap(),
                        ),
                    ],
                )
                .assert_dependencies(
                    "default/linking/cycle/Cycle.t.sol:Bar".to_string(),
                    vec![
                        (
                            "default/linking/cycle/Cycle.t.sol:Foo".to_string(),
                            Address::from_str("0x47e9Fbef8C83A1714F1951F142132E6e90F5fa5D")
                                .unwrap(),
                        ),
                        (
                            "default/linking/cycle/Cycle.t.sol:Bar".to_string(),
                            Address::from_str("0x5a443704dd4B594B382c22a083e2BD3090A6feF3")
                                .unwrap(),
                        ),
                    ],
                )
                .test_with_sender_and_nonce(Address::default(), 1);
        });
    }

    #[test]
    fn link_create2_nested() {
        link_test("../../testdata/default/linking/nested", |linker| {
            linker
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:Lib".to_string(),
                    vec![],
                )
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:NestedLib".to_string(),
                    vec![(
                        "default/linking/nested/Nested.t.sol:Lib".to_string(),
                        Address::from_str("0xddb1Cd2497000DAeA687CEa3dc34Af44084BEa74").unwrap(),
                    )],
                )
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:LibraryConsumer".to_string(),
                    vec![
                        // Lib shows up here twice, because the linker sees it twice, but it should
                        // have the same address and nonce.
                        (
                            "default/linking/nested/Nested.t.sol:Lib".to_string(),
                            Address::from_str("0xddb1Cd2497000DAeA687CEa3dc34Af44084BEa74")
                                .unwrap(),
                        ),
                        (
                            "default/linking/nested/Nested.t.sol:NestedLib".to_string(),
                            Address::from_str("0xfebE2F30641170642f317Ff6F644Cee60E7Ac369")
                                .unwrap(),
                        ),
                    ],
                )
                .assert_dependencies(
                    "default/linking/nested/Nested.t.sol:NestedLibraryLinkingTest".to_string(),
                    vec![
                        (
                            "default/linking/nested/Nested.t.sol:Lib".to_string(),
                            Address::from_str("0xddb1Cd2497000DAeA687CEa3dc34Af44084BEa74")
                                .unwrap(),
                        ),
                        (
                            "default/linking/nested/Nested.t.sol:NestedLib".to_string(),
                            Address::from_str("0xfebE2F30641170642f317Ff6F644Cee60E7Ac369")
                                .unwrap(),
                        ),
                    ],
                )
                .test_with_create2(
                    Address::default(),
                    fixed_bytes!(
                        "19bf59b7b67ae8edcbc6e53616080f61fa99285c061450ad601b0bc40c9adfc9"
                    ),
                );
        });
    }

    #[test]
    fn find_artifact_id_by_library_path_with_multiple_artifacts_same_version() {
        let root = PathBuf::from("/path/to/project");
        let lib_path = PathBuf::from("/path/to/project/contracts/Lib.sol");
        let other_path = PathBuf::from("/path/to/project/contracts/OtherLib.sol");
        let version = Version::new(0, 8, 18);

        let lib_artifact = ArtifactId {
            name: "Lib".to_string(),
            source: lib_path.clone(),
            version: version.clone(),
        };

        let other_artifact = ArtifactId {
            name: "OtherLib".to_string(),
            source: other_path.clone(),
            version: version.clone(),
        };

        let contracts = BTreeMap::from([
            (lib_artifact.clone(), CompactContractBytecodeCow::default()),
            (
                other_artifact.clone(),
                CompactContractBytecodeCow::default(),
            ),
        ]);

        let linker = Linker { root, contracts };

        let result = linker.find_artifact_id_by_library_path("contracts/Lib.sol", "Lib", &version);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), &lib_artifact);
    }

    #[test]
    fn find_artifact_id_by_library_path_returns_none_when_no_match() {
        let root = PathBuf::from("/path/to/project");
        let artifact_path = PathBuf::from("/path/to/project/contracts/Lib.sol");
        let version = Version::new(0, 8, 18);

        let artifact_id = ArtifactId {
            name: "Lib".to_string(),
            source: artifact_path.clone(),
            version: version.clone(),
        };

        let contracts =
            BTreeMap::from([(artifact_id.clone(), CompactContractBytecodeCow::default())]);

        let linker = Linker { root, contracts };

        let result =
            linker.find_artifact_id_by_library_path("contracts/OtherLib.sol", "Lib", &version);
        assert!(result.is_none());

        let result =
            linker.find_artifact_id_by_library_path("contracts/Lib.sol", "OtherLib", &version);
        assert!(result.is_none());
    }

    #[test]
    fn find_artifact_id_by_library_path_with_multiple_versions() {
        let root = PathBuf::from("/path/to/project");
        let artifact_path = PathBuf::from("/path/to/project/contracts/Lib.sol");

        let version_1_0_0 = Version::new(1, 0, 0);
        let version_1_1_0 = Version::new(1, 1, 0);
        let version_1_2_0 = Version::new(1, 2, 0);

        let artifact_id_1_0_0 = ArtifactId {
            name: "Lib".to_string(),
            source: artifact_path.clone(),
            version: version_1_0_0.clone(),
        };

        let artifact_id_1_1_0 = ArtifactId {
            name: "Lib".to_string(),
            source: artifact_path.clone(),
            version: version_1_1_0.clone(),
        };

        let artifact_id_1_2_0 = ArtifactId {
            name: "Lib".to_string(),
            source: artifact_path.clone(),
            version: version_1_2_0.clone(),
        };

        let contracts = BTreeMap::from([
            (
                artifact_id_1_0_0.clone(),
                CompactContractBytecodeCow::default(),
            ),
            (
                artifact_id_1_1_0.clone(),
                CompactContractBytecodeCow::default(),
            ),
            (
                artifact_id_1_2_0.clone(),
                CompactContractBytecodeCow::default(),
            ),
        ]);

        let linker = Linker { root, contracts };

        let result =
            linker.find_artifact_id_by_library_path("contracts/Lib.sol", "Lib", &version_1_1_0);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &artifact_id_1_1_0);

        let non_existent_version = Version::new(1, 3, 0);
        let result = linker.find_artifact_id_by_library_path(
            "contracts/Lib.sol",
            "Lib",
            &non_existent_version,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &artifact_id_1_2_0);
    }
}
