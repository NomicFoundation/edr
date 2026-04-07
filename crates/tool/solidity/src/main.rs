use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use edr_instrument::coverage::instrument_code;
use foundry_compilers::{
    artifacts::{output_selection::EvmOutputSelection, solc::CompactContractRef},
    compilers::{multi::MultiCompiler, solc::SolcCompiler},
    multi::MultiCompilerSettings,
    solc::SolcSettings,
    Project, ProjectPathsConfig, VyperSettings,
};
use semver::Version;

const COVERAGE_LIBRARY_SOL: &str = include_str!("../../../../data/contracts/coverage.sol");

/// Source identifier used for the coverage library in instrumented imports.
const COVERAGE_LIBRARY_SOURCE_ID: &str = "coverage_lib.sol";

/// Compile Solidity contracts with optional coverage instrumentation. Deployed
/// bytecodes are printed to stdout, or written to files with `--output-dir`.
///
/// Examples:
///
///   # Compile with an explicit import:
///   `cargo run -p edr_tool_compile_solidity -- data/contracts/increment.sol
/// -i` data/contracts/coverage.sol
///
///   # Compile with coverage instrumentation:
///   `cargo run -p edr_tool_compile_solidity -- --instrument
/// data/contracts/test/CoverageTest.sol`
///
///   # Write bytecodes to files:
///   `cargo run -p edr_tool_compile_solidity -- -o data/deployed_bytecode
/// data/contracts/increment.sol -i data/contracts/coverage.sol`
#[derive(Parser)]
#[clap(name = "compile-solidity")]
struct Args {
    /// Path to the Solidity source file to compile.
    source: PathBuf,

    /// Output directory for bytecode files. If omitted, bytecodes are printed
    /// to stdout.
    #[clap(long, short)]
    output_dir: Option<PathBuf>,

    /// Additional Solidity source files to include (e.g. imported libraries).
    #[clap(long, short = 'i')]
    include: Vec<PathBuf>,

    /// Instrument the source with coverage probes before compiling.
    /// This also includes the coverage library in the compilation.
    #[clap(long)]
    instrument: bool,

    /// Only instrument the source (no compilation). Outputs the instrumented
    /// Solidity source to stdout. `--output-dir` is not supported.
    #[clap(long)]
    instrument_only: bool,

    /// The Solidity version to target for instrumentation (e.g. `0.8.26`).
    /// Only used with `--instrument`. The actual compiler version is
    /// auto-detected from the source pragma.
    #[clap(long, default_value = "0.8.26")]
    version: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let source_code = fs::read_to_string(&args.source).context("failed to read source file")?;

    let source_id = args
        .source
        .file_name()
        .context("source path has no filename")?
        .to_string_lossy()
        .to_string();

    let final_source = if args.instrument || args.instrument_only {
        let version = Version::parse(&args.version).context("invalid --version")?;
        let instrumented = instrument_code(
            &source_code,
            &source_id,
            version,
            COVERAGE_LIBRARY_SOURCE_ID,
        )
        .context("instrumentation failed")?;
        instrumented.source
    } else {
        source_code
    };

    if args.instrument_only {
        let header = format!(
            "// Auto-generated from {} — do not edit manually.\n\
             // Regenerate with:\n\
             //   cargo run -p edr_tool_solidity -- --instrument-only {}\n",
            args.source.display(),
            args.source.display(),
        );
        print!("{header}{final_source}");
        return Ok(());
    }

    // Write sources to a temp directory for compilation.
    let project_dir = tempfile::TempDir::new()?;
    let root = project_dir.path().to_path_buf();

    fs::write(root.join(&source_id), &final_source)?;

    if args.instrument {
        fs::write(root.join(COVERAGE_LIBRARY_SOURCE_ID), COVERAGE_LIBRARY_SOL)?;
    }

    // Copy any additional source files into the project root.
    for include in &args.include {
        let name = include
            .file_name()
            .context("include path has no filename")?;
        fs::copy(include, root.join(name))
            .with_context(|| format!("failed to copy {}", include.display()))?;
    }

    let output = compile_project(&root)?;

    // Extract and output deployed bytecodes.
    if let Some(ref output_dir) = args.output_dir {
        fs::create_dir_all(output_dir)?;
    }

    for (file, name, contract) in output.output().contracts_with_files_iter() {
        // When instrumenting, skip contracts from the auto-included coverage
        // library — it's never deployed.
        if args.instrument && file.to_string_lossy().contains(COVERAGE_LIBRARY_SOURCE_ID) {
            continue;
        }

        println!("\n--- {name} ---");

        if let Some(evm) = &contract.evm
            && !evm.method_identifiers.is_empty()
        {
            println!("method identifiers:");
            for (signature, selector) in &evm.method_identifiers {
                println!("  {signature} => 0x{selector}");
            }
        }

        let compact = CompactContractRef::from(contract);
        if let Some(bytecode) = compact.bytecode()
            && !bytecode.is_empty()
        {
            let hex = format!("0x{}", hex::encode(bytecode));
            println!();
            println!("bytecode: {hex}");

            if let Some(ref output_dir) = args.output_dir {
                let out_path = output_dir.join(format!("{name}.in"));
                fs::write(&out_path, &hex)?;
                println!("  written to: {}", out_path.display());
            }
        }
    }

    Ok(())
}

fn compile_project(root: &Path) -> Result<foundry_compilers::ProjectCompileOutput> {
    let paths = ProjectPathsConfig::builder()
        .sources(root)
        .build_with_root(root);

    let compiler = MultiCompiler {
        solc: Some(SolcCompiler::AutoDetect),
        vyper: None,
    };
    let settings = foundry_compilers::artifacts::Settings::default()
        .with_extra_output(vec![EvmOutputSelection::MethodIdentifiers.into()]);
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
        .build(compiler)?;

    let output = project.compile()?;
    if output.has_compiler_errors() {
        bail!("Solidity compilation errors:\n{output}");
    }

    Ok(output)
}
