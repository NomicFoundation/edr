use std::{
    collections::HashMap,
    fmt::Write as _,
    fs::{self, create_dir_all, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::OnceLock,
};

use anyhow::{anyhow, bail};
use git2::Repository;
use itertools::Itertools;
use op_revm::OpSpecId;
use tempfile::tempdir;

/// These hardforks are not included in `OpSpecId` since they only impacted the
/// consensus layer.
const KNOWN_IGNORED_HARDFORKS: [&str; 2] = ["delta", "pectra_blob_schedule"];
const SUPERCHAIN_REGISTRY_REPO_URL: &str =
    "https://github.com/ethereum-optimism/superchain-registry.git";
const REPO_CONFIGS_PATH: &str = "superchain/configs";
const EDR_SUPPORTED_NETWORKS: [&str; 2] = ["mainnet", "sepolia"];
const GENERATED_FILE_WARNING_MESSAGE: &str = "
// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is generated.     
// To make changes, update the generator script instead in `tools/src/op_chain_config.rs`.";

static WORKSPACE_ROOT_PATH: OnceLock<anyhow::Result<String>> = OnceLock::new();

fn generated_module_path() -> anyhow::Result<String> {
    let root_path = WORKSPACE_ROOT_PATH.get_or_init(workspace_root);

    root_path
        .as_ref()
        .map_err(|error| anyhow!("Could not determine EDR root path: {error}"))
        .map(|workspace_root_path| {
            format!("{workspace_root_path}/crates/edr_op/src/hardfork/generated")
        })
}

fn generate_warning_message(commit_sha: String) -> String {
    format!("
    {GENERATED_FILE_WARNING_MESSAGE}
    //
    // source: https://github.com/ethereum-optimism/superchain-registry/tree/{commit_sha}/{REPO_CONFIGS_PATH}
")
}

fn get_current_commit_sha(repo_path: &Path) -> Result<String, git2::Error> {
    let repo = Repository::open(repo_path)?;

    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    let commit_id = commit.id();

    Ok(commit_id.to_string())
}

pub fn import_op_chain_configs() -> anyhow::Result<()> {
    // Create a temporary directory that will be automatically deleted on drop
    let superchain_registry_repo_dir = tempdir()?;
    Repository::clone(
        SUPERCHAIN_REGISTRY_REPO_URL,
        superchain_registry_repo_dir.path(),
    )?;

    let generated_module_path = generated_module_path()?;
    let modules_dir = Path::new(generated_module_path.as_str());
    create_dir_all(modules_dir)?;

    println!(
        "Generated modules dir: {}",
        modules_dir.canonicalize()?.to_str().unwrap()
    );

    let chains_to_generate =
        fetch_op_stack_chains_to_configure(superchain_registry_repo_dir.path())?;

    let generated_modules = generate_chain_modules(
        superchain_registry_repo_dir.path(),
        modules_dir,
        chains_to_generate,
    );

    write_generated_module_file(generated_modules)?;

    let generated_files_path = {
        let mut path_buf = modules_dir.to_path_buf();
        path_buf.push("*");
        path_buf.as_os_str().to_owned()
    };

    println!("Formatting generated files...");
    Command::new("rustfmt")
        .arg("+nightly")
        .arg(generated_files_path)
        .arg(format!("{generated_module_path}.rs"))
        .output()?;

    println!("Running `cargo check`...");
    let cargo_check_output = Command::new("cargo")
        .arg("check")
        .arg("-p")
        .arg("edr_op")
        .output()?;
    if cargo_check_output.status.success() {
        println!("Success!");
        Ok(())
    } else {
        io::stderr().write_all(&cargo_check_output.stderr)?;
        bail!("Cargo check fails after generation");
    }
}

/// Generates a new file module for every element in `chain_configurations`
/// Returns a `Vec` containing the chain config specs within EDR repo.
fn generate_chain_modules(
    repo_path: &Path,
    modules_dir: &Path,
    chain_configurations: Vec<ChainConfigSpec>,
) -> Vec<ChainConfigSpec> {
    chain_configurations
        .into_iter()
        .filter_map(|chain_config| {
            let result = write_chain_module(repo_path, &chain_config, modules_dir);
            match result {
                Ok(module_name) => Some(ChainConfigSpec {
                    file_name: module_name,
                    networks: chain_config.networks,
                }),
                Err(error) => {
                    println!(
                        "Skipping {} chain module generation due to {error}",
                        chain_config.file_name
                    );
                    None
                }
            }
        })
        .collect()
}

/// Based on superchain registry repository, identifies all the chains and their
/// corresponding networks to create configs for
/// Returns a `Vec` with the chain config spec within superchain registry.
fn fetch_op_stack_chains_to_configure(repo_path: &Path) -> anyhow::Result<Vec<ChainConfigSpec>> {
    let config_path = repo_config_path_buf(repo_path);

    let mut networks_by_chain = HashMap::<String, Vec<String>>::new();

    // Superchain repo configurations are organized by network
    for network in EDR_SUPPORTED_NETWORKS {
        let mut network_config_path = config_path.clone();
        network_config_path.push(network);

        for entry in fs::read_dir(network_config_path)? {
            let filename = entry?.file_name().to_string_lossy().to_string();
            networks_by_chain
                .entry(filename)
                .and_modify(|entry| entry.push(network.to_string()))
                .or_insert(vec![network.to_string()]);
        }
    }

    Ok(networks_by_chain.into_iter().map(Into::into).collect())
}

fn repo_config_path_buf(repo_path: &Path) -> PathBuf {
    let mut path = PathBuf::from(repo_path);
    path.push(REPO_CONFIGS_PATH);
    path
}
/// Creates or updates the `generated.rs` module file
/// declares all the submodules and defines a `chain_configs()` function that
/// returns a map containing all the generated configs
fn write_generated_module_file(generated_chains: Vec<ChainConfigSpec>) -> anyhow::Result<()> {
    let generated_module_file_name = format!("{}.rs", generated_module_path()?);
    let generated_module_path = Path::new(&generated_module_file_name);

    let mut generated_module: File = File::create(generated_module_path)?;

    let module_imports: String = Itertools::intersperse(
        generated_chains
            .iter()
            .map(|op_chain_config| op_chain_config.file_name.as_str())
            .map(|module| {
                format!(
                    "/// Chain configuration module for `{module}`
                    pub mod {module};"
                )
            }),
        String::from("\n"),
    )
    .collect();

    let sorted_chain_networks = generated_chains
        .into_iter()
        .flat_map(|chain_config| {
            chain_config
                .networks
                .into_iter()
                .map(move |network| (chain_config.file_name.clone(), network))
        })
        .sorted();

    let config_tuples: String = Itertools::intersperse(
        sorted_chain_networks.map(|(module, network)| {
            let chain_id_name = module_attribute(&module, &chain_id_name(&network));
            let chain_config = module_attribute(&module, &network_config_function(&network));
            format!("({chain_id_name}, {chain_config}),")
        }),
        String::from("\n"),
    )
    .collect();

    write!(
        generated_module,
        "
        {GENERATED_FILE_WARNING_MESSAGE}

        use edr_primitives::HashMap;
        use edr_evm::hardfork::ChainConfig;
        use crate::Hardfork;

        {module_imports}

        pub(super) fn chain_configs() -> HashMap<u64, ChainConfig<Hardfork>> {{

            HashMap::from([

                {config_tuples}
            
            ])
        }}
        "
    )?;
    Ok(())
}

fn module_attribute(module: &str, attribute: &str) -> String {
    format!("{module}::{attribute}")
}
fn chain_id_name(network: &str) -> String {
    format!("{}_CHAIN_ID", network.to_uppercase())
}
fn network_config_function(network: &str) -> String {
    format!("{}_config()", network.to_lowercase())
}
/// Creates a rust module file for the given chain config spec
/// The module defines a `ChainConfig` and id for every specified network
///
/// Returns the name of the new module file
/// Overwrites the file if it already exists
fn write_chain_module(
    repo_path: &Path,
    repo_chain_config: &ChainConfigSpec,
    output_path: &Path,
) -> anyhow::Result<String> {
    let repo_config_path = repo_config_path_buf(repo_path);
    if repo_chain_config.networks.is_empty() {
        bail!("No networks for chain");
    }
    let chain_module_name = if let Some((name, _extension)) = repo_chain_config
        .file_name
        .replace('-', "_")
        .split_once(".")
    {
        String::from(name)
    } else {
        bail!(
            "could not define module filename from {}",
            repo_chain_config.file_name
        );
    };

    let networks_config = {
        let mut networks_config = String::new();
        for network in repo_chain_config.networks.iter() {
            let chain_config_path = {
                let mut path = repo_config_path.clone();
                path.push(network.clone());
                path.push(repo_chain_config.file_name.clone());
                path
            };
            let file_contents = fs::read_to_string(chain_config_path)?;
            let chain_config: OpChainConfig = toml::from_str(&file_contents)?;

            let chain_id_const_name = chain_id_name(network);
            let chain_id_hex = format!("0x{:X}", chain_config.chain_id);
            let chain_config_function = network_config_function(network);
            let chain_name = chain_config.name;

            let chain_base_fee_params: String = generate_base_fee_params(chain_config.optimism);
            let chain_hardfork_activations = generate_hardfork_activations_for(
                format!("{chain_module_name} {network}"),
                chain_config.hardforks,
            );

            write!(
                &mut networks_config,
                "
                /// `{chain_name}` chain id
                pub const {chain_id_const_name}: u64 = {chain_id_hex};
                
                /// `{chain_name}` chain configuration
                pub(super) fn {chain_config_function} -> ChainConfig<OpSpecId>{{ ChainConfig {{
                    name: \"{chain_name}\".into(),
                    base_fee_params: {chain_base_fee_params}, 
                    hardfork_activations: {chain_hardfork_activations}
                    }}
                }}
                ",
            )?;
        }
        networks_config
    };

    let module_path = {
        let mut path = PathBuf::from(output_path);
        path.push(format!("{chain_module_name}.rs"));
        path
    };
    let mut module = File::create(module_path.clone())?;

    let file_warning_message = {
        let head_sha = get_current_commit_sha(repo_path)?;
        generate_warning_message(head_sha)
    };

    write!(
        &mut module,
        "
        {file_warning_message}
        
        use edr_eip1559::{{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams}};
        use edr_evm::hardfork::{{self, Activations, ChainConfig, ForkCondition}};
        use op_revm::OpSpecId;

        {networks_config}
        "
    )?;

    Ok(chain_module_name)
}

fn generate_hardfork_activations_for(
    chain_name: String,
    hardforks: toml::map::Map<String, toml::Value>,
) -> String {
    let superchain_activations = hardforks.into_iter().filter_map(
        |(hardfork, activation_value)| match get_op_hardfork_from(&hardfork) {
            Err(error) => {
                println!("{chain_name}: ignoring activation - {error}");
                None
            }
            Ok(opt_hardfork) => opt_hardfork
                .and_then(|hardfork| activation_value.as_integer().map(|value| (hardfork, value))),
        },
    );

    // Superchain registry lists hardforks starting from Canyon, but there are two
    // previous OpSpec hardforks before: bedrock and regolith. We are adding
    // those hardforks to make sure that the blockchain hardfork list is complete.
    let previous_hardforks = [(OpSpecId::BEDROCK, 0), (OpSpecId::REGOLITH, 0)];
    let activations_str: String = previous_hardforks
        .into_iter()
        .chain(superchain_activations)
        .map(|(hardfork, activation)| {
            let hardfork_str: &'static str = hardfork.into();

            format!(
                "
            hardfork::Activation {{
                condition: ForkCondition::Timestamp({}),
                hardfork: OpSpecId::{},
            }},",
                activation,
                hardfork_str.to_uppercase()
            )
        })
        .collect();
    format!("Activations::new(vec![{activations_str}]),")
}

fn get_op_hardfork_from(hardfork_str: &str) -> anyhow::Result<Option<OpSpecId>> {
    let hardfork_name = hardfork_str
        .split_once("_time")
        .map(|(before_match, _)| before_match)
        .ok_or(anyhow!("activation is not time based: {hardfork_str}"))?;

    match OpSpecId::from_str(&capitalize_first_letter(hardfork_name)) {
        Err(_) => {
            if !KNOWN_IGNORED_HARDFORKS.contains(&hardfork_name) {
                bail!("hardfork name is not supported: {hardfork_name}")
            } else {
                Ok(None)
            }
        }
        Ok(hardfork) => Ok(Some(hardfork)),
    }
}
fn generate_base_fee_params(params: OpHardforkBaseFeeParams) -> String {
    let original_denominator = params.eip1559_denominator;
    let canyon_denominator = params.eip1559_denominator_canyon;
    let elasticity = params.eip1559_elasticity;

    format!(
        "BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
        (
            BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
            ConstantBaseFeeParams::new({original_denominator}, {elasticity}),
        ),
        (
            BaseFeeActivation::Hardfork(OpSpecId::CANYON),
            ConstantBaseFeeParams::new({canyon_denominator}, {elasticity}),
        )
]))"
    )
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct OpChainConfig {
    name: String,
    chain_id: i64,
    hardforks: toml::value::Table,
    optimism: OpHardforkBaseFeeParams,
}

struct ChainConfigSpec {
    file_name: String,
    networks: Vec<String>,
}

impl From<(String, Vec<String>)> for ChainConfigSpec {
    fn from((file_name, networks): (String, Vec<String>)) -> Self {
        ChainConfigSpec {
            file_name,
            networks,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct OpHardforkBaseFeeParams {
    eip1559_elasticity: u64,
    eip1559_denominator: u64,
    eip1559_denominator_canyon: u64,
}

fn capitalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first_char) => first_char.to_uppercase().chain(chars).collect(),
    }
}

fn workspace_root() -> anyhow::Result<String> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .output()?;

    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8(output.stdout)?;
    let metadata: serde_json::Value = serde_json::from_str(&stdout)?;

    metadata
        .get("workspace_root")
        .and_then(|value| value.as_str().map(String::from))
        .ok_or(anyhow!("Could not find workspace_root in cargo metadata"))
}
