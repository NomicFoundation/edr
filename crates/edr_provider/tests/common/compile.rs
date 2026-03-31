use std::{collections::HashMap, path::PathBuf};

use edr_instrument::coverage::{instrument_code, InstrumentationMetadata};
use edr_primitives::Bytes;
use edr_test_utils::svm_global_lock;
use foundry_compilers::{
    artifacts::{
        solc::{sources::Source, CompactContractRef, SolcInput, SolcLanguage, Sources},
        Settings,
    },
    solc::Solc,
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

/// Returns a `Solc` instance for the required version, installing it via svm if
/// needed. Acquires the global SVM lock to prevent concurrent installations
/// from different test crates during `cargo test --workspace`.
fn get_solc() -> Solc {
    let version = Version::parse(SOLC_VERSION).expect("valid semver");

    let mut lock = svm_global_lock();
    let guard = lock.write().unwrap();
    let solc = Solc::find_or_install(&version).expect("failed to find or install solc");
    drop(guard);

    solc
}

/// Instruments a Solidity source file with coverage probes and compiles it.
///
/// Returns the compiled contracts (keyed by contract name) and the
/// instrumentation metadata (tags for each instrumented statement).
pub fn instrument_and_compile(source_code: &str, source_id: &str) -> InstrumentAndCompileResult {
    let version = Version::parse(SOLC_VERSION).expect("valid semver");

    // 1. Instrument
    let instrumented = instrument_code(source_code, source_id, version, COVERAGE_LIBRARY_SOURCE_ID)
        .expect("instrumentation should succeed");

    // 2. Build sources
    let mut sources = Sources::new();
    sources.insert(PathBuf::from(source_id), Source::new(&instrumented.source));
    sources.insert(
        PathBuf::from(COVERAGE_LIBRARY_SOURCE_ID),
        Source::new(COVERAGE_LIBRARY_SOL),
    );

    let mut input = SolcInput::new(SolcLanguage::Solidity, sources, Settings::default());
    input.settings.evm_version = None;

    // 3. Compile
    let solc = get_solc();
    let output = solc.compile(&input).expect("solc compilation failed");

    assert!(
        !output.has_error(),
        "Solidity compilation errors:\n{}",
        output
            .errors
            .iter()
            .filter(|e| e.severity.is_error())
            .map(|e| e.formatted_message.as_deref().unwrap_or(&e.message))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // 4. Extract contracts
    let contracts = output
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
