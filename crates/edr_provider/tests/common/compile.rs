use std::collections::HashMap;

use edr_instrument::coverage::{instrument_code, InstrumentationMetadata};
use edr_primitives::Bytes;
use foundry_compilers::{
    artifacts::{solc::CompactContractRef, EvmVersion, Settings},
    compilers::{multi::MultiCompiler, solc::SolcCompiler},
    multi::MultiCompilerSettings,
    solc::SolcSettings,
    Project, ProjectPathsConfig, VyperSettings,
};
use semver::Version;

const COVERAGE_LIBRARY_SOL: &str = include_str!("../../../../data/contracts/coverage.sol");

/// Path to the coverage library as it will appear in the solcjs sources map.
/// Must match the path passed to `instrument_code` as `coverage_library_path`.
const COVERAGE_LIBRARY_SOURCE_ID: &str = "coverage_lib.sol";

const SOLC_VERSION: &str = "0.8.26";

pub struct CompiledContract {
    pub name: String,
    pub bytecode: Bytes,
}

pub struct InstrumentAndCompileResult {
    pub contracts: HashMap<String, CompiledContract>,
    pub metadata: Vec<InstrumentationMetadata>,
}

/// Instruments a Solidity source file with coverage probes and compiles it
/// using a `Project`-based compilation with auto-detect solc, matching the
/// configuration used by `edr_solidity_tests`. This avoids interfering with
/// the global SVM state used by other test crates during
/// `cargo test --workspace`.
pub fn instrument_and_compile(source_code: &str, source_id: &str) -> InstrumentAndCompileResult {
    let version = Version::parse(SOLC_VERSION).expect("valid semver");

    // 1. Instrument
    let instrumented = instrument_code(source_code, source_id, version, COVERAGE_LIBRARY_SOURCE_ID)
        .expect("instrumentation should succeed");

    // 2. Write sources to a temp directory (flat layout so bare imports resolve).
    let project_dir = tempfile::TempDir::new().expect("failed to create temp dir");
    let root = project_dir.path();

    std::fs::write(root.join(source_id), &instrumented.source)
        .expect("failed to write source file");
    std::fs::write(root.join(COVERAGE_LIBRARY_SOURCE_ID), COVERAGE_LIBRARY_SOL)
        .expect("failed to write coverage library");

    // 3. Build project with auto-detect solc (same approach as edr_solidity_tests)
    let paths = ProjectPathsConfig::builder()
        .sources(root)
        .build_with_root(root);

    let mut settings = Settings {
        evm_version: Some(EvmVersion::Paris),
        ..Default::default()
    };
    settings.optimizer.enabled = Some(true);
    settings.optimizer.runs = Some(200);

    let compiler = MultiCompiler {
        solc: Some(SolcCompiler::AutoDetect),
        vyper: None,
    };
    let compiler_settings = MultiCompilerSettings {
        solc: SolcSettings {
            settings,
            ..SolcSettings::default()
        },
        vyper: VyperSettings::default(),
    };

    let project = Project::builder()
        .paths(paths)
        .settings(compiler_settings)
        .set_cached(false)
        .set_no_artifacts(true)
        .build(compiler)
        .expect("failed to build project");

    // 4. Compile
    let output = project.compile().expect("solc compilation failed");
    assert!(
        !output.has_compiler_errors(),
        "Solidity compilation errors:\n{output}",
    );

    // 5. Extract contracts
    let contracts = output
        .output()
        .contracts_iter()
        .map(|(name, contract)| {
            let compact = CompactContractRef::from(contract);
            let bytecode = compact
                .bytecode()
                .unwrap_or_else(|| panic!("no bytecode for {name}"));

            (
                name.clone(),
                CompiledContract {
                    name: name.clone(),
                    bytecode: Bytes::copy_from_slice(bytecode),
                },
            )
        })
        .collect();

    InstrumentAndCompileResult {
        contracts,
        metadata: instrumented.metadata,
    }
}
