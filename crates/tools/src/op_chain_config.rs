use core::fmt;
use std::{
    collections::HashMap,
    error::Error,
    fmt::Write as _,
    fs::{self, create_dir_all, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use git2::Repository;
use itertools::Itertools;
use op_revm::OpSpecId;
use tempfile::tempdir;

/// These hardforks are not included in `OpSpecId` since they only impacted the
/// conscensus layer
const KNOWN_IGNORED_HARDFORKS: [&str; 2] = ["delta", "pectra_blob_schedule"];
const SUPERCHAIN_REGISTRY_REPO_URL: &str =
    "https://github.com/ethereum-optimism/superchain-registry.git";
const REPO_CONFIGS_PATH: &str = "superchain/configs";
const EDR_SUPPORTED_NETWORKS: [&str; 2] = ["mainnet", "sepolia"];
const GENERATED_MODULE_PATH: &str = "./crates/edr_op/src/hardfork/generated";
const GENERATED_FILE_WARNING_MESSAGE: &str = "
    // WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
    // Any changes made to this file will be overwritten the next time it is generated.
    // To make changes, update the generator script instead in `tools/src/op_chain_config.rs`.
";

pub fn import_op_chain_configs() -> Result<(), anyhow::Error> {
    // Create a temporary directory that will be automatically deleted on drop
    let superchain_registry_repo_dir = tempdir()?;
    Repository::clone(
        SUPERCHAIN_REGISTRY_REPO_URL,
        superchain_registry_repo_dir.path(),
    )?;

    let modules_dir = Path::new(GENERATED_MODULE_PATH);
    create_dir_all(modules_dir)?;

    println!(
        "Generated modules dir: {}",
        modules_dir.canonicalize()?.to_str().unwrap()
    );

    let chains_to_configure =
        fetch_op_stack_chains_to_configure(superchain_registry_repo_dir.path())?;

    let generated_modules = generate_chain_modules(
        superchain_registry_repo_dir.path(),
        modules_dir,
        chains_to_configure,
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
        .arg(format!("{GENERATED_MODULE_PATH}.rs"))
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
        Err(OpImporterError {
            message: "Cargo check fails after generation".to_string(),
        }
        .into())
    }
}

/// Generates a new file module for every element in `chain_configurations`
/// Returns a Vec containing the chain config specs within EDR repo
fn generate_chain_modules(
    repo_path: &Path,
    modules_dir: &Path,
    chain_configurations: Vec<ChainConfigSpec>,
) -> Vec<ChainConfigSpec> {
    chain_configurations
        .into_iter()
        .filter_map(|chain_config| {
            let result =
                write_chain_module(&repo_config_path_buf(repo_path), &chain_config, modules_dir);
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
/// Returns a Vec with the chain config spec within superchain registry
fn fetch_op_stack_chains_to_configure(repo_path: &Path) -> anyhow::Result<Vec<ChainConfigSpec>> {
    let config_path = repo_config_path_buf(repo_path);

    let mut networks_by_chain = HashMap::<String, Vec<String>>::new();

    // Superchain repo configurations are oganized by network
    for network in EDR_SUPPORTED_NETWORKS {
        let mut network_config_path = PathBuf::from(&config_path);
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
fn write_generated_module_file(
    chains_to_configure: Vec<ChainConfigSpec>,
) -> Result<(), anyhow::Error> {
    let generated_module_file_name = format!("{GENERATED_MODULE_PATH}.rs");
    let generated_module_path = Path::new(&generated_module_file_name);

    let mut generated_module: File = File::create(generated_module_path)?;

    let generated_modules = chains_to_configure
        .iter()
        .map(|op_chain_config| op_chain_config.file_name.as_str())
        .collect::<Vec<&str>>();

    let module_imports = generated_modules
        .iter()
        .fold(String::new(), |mut imports, module| {
            imports.push('\n');
            imports.push_str(
                format!(
                    "/// `{module}` chain configuration module;
                    pub mod {module};"
                )
                .as_str(),
            );
            imports
        });

    let sorted_chain_networks: Vec<(String, String)> = chains_to_configure
        .into_iter()
        .flat_map(|chain_config| {
            chain_config
                .networks
                .into_iter()
                .map(move |network| (chain_config.file_name.clone(), network))
        })
        .sorted()
        .collect();

    let insert_config_lines =
        sorted_chain_networks
            .iter()
            .fold(String::new(), |mut imports, (module, network)| {
                let chain_id_name = module_attribute(module, &chain_id_name(network));
                let chain_config = module_attribute(module, &network_config_function(network));
                imports.push('\n');
                imports.push_str(
                    format!("hardforks.insert({chain_id_name}, {chain_config});").as_str(),
                );
                imports
            });

    write!(
        generated_module,
        "
        {GENERATED_FILE_WARNING_MESSAGE}

        use edr_primitives::HashMap;
        use edr_evm::hardfork::ChainConfig;
        use crate::Hardfork;

        {module_imports}

        pub(crate) fn chain_configs() -> HashMap<u64, ChainConfig<Hardfork>> {{

            let mut hardforks = HashMap::new();

            {insert_config_lines}
            
            hardforks
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
    repo_config_path: &PathBuf,
    repo_chain_config: &ChainConfigSpec,
    output_path: &Path,
) -> Result<String, anyhow::Error> {
    if repo_chain_config.networks.is_empty() {
        return Err(OpImporterError {
            message: "No networks for chain".to_string(),
        }
        .into());
    }
    let chain_module_name = {
        match repo_chain_config
            .file_name
            .replace('-', "_")
            .split_once(".")
        {
            Some((name, _extension)) => String::from(name),
            None => {
                return Err(OpImporterError {
                    message: format!(
                        "could not define module filename from {}",
                        repo_chain_config.file_name
                    ),
                }
                .into())
            }
        }
    };

    let networks_config = {
        let mut networks_config = String::new();
        for network in repo_chain_config.networks.iter() {
            let chain_config_path = {
                let mut path = PathBuf::from(repo_config_path);
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

            let chain_base_fee_params: String = build_base_fee_params(chain_config.optimism);
            let chain_hardfork_activations = build_hardfork_activations_for(
                format!("{chain_module_name} {network}"),
                chain_config.hardforks,
            );

            write!(
                &mut networks_config,
                "
                /// `{chain_name}` chain id
                pub const {chain_id_const_name}: u64 = {chain_id_hex};
                
                /// `{chain_name}` chain configuration
                pub(crate) fn {chain_config_function} -> ChainConfig<OpSpecId>{{ ChainConfig {{
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

    write!(
        &mut module,
        "
        {GENERATED_FILE_WARNING_MESSAGE}
        
        use edr_eip1559::{{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams}};
        use edr_evm::hardfork::{{self, Activations, ChainConfig, ForkCondition}};
        use op_revm::OpSpecId;

        {networks_config}
        "
    )?;

    Ok(chain_module_name)
}

fn build_hardfork_activations_for(
    chain_name: String,
    hardforks: toml::map::Map<String, toml::Value>,
) -> String {
    let mut activations_str = String::new();

    for activations in hardforks.iter() {
        let hardfork_name = activations
            .0
            .split_once("_time")
            .map(|(before_match, _)| before_match);

        let hardfork_name = match hardfork_name {
            None => {
                println!(
                    "{chain_name}: ignoring activation - activation is not time based: {}",
                    activations.0
                );
                continue;
            }
            Some(name) => name,
        };

        let hardfork = match OpSpecId::from_str(&capitalize_first_letter(hardfork_name)) {
            Err(_) => {
                if !KNOWN_IGNORED_HARDFORKS.contains(&hardfork_name) {
                    println!(
                            "{chain_name}: ignoring activation - hardfork name is not supported: {hardfork_name}",
                        );
                }
                continue;
            }
            Ok(hardfork) => hardfork,
        };
        let hardfork_str: &'static str = hardfork.into();
        activations_str.push_str(
            format!(
                "
            hardfork::Activation {{
                condition: ForkCondition::Timestamp({}),
                hardfork: OpSpecId::{},
            }},",
                activations.1,
                hardfork_str.to_uppercase()
            )
            .as_str(),
        );
    }
    format!("Activations::new(vec![{activations_str}]),")
}
fn build_base_fee_params(params: OpHardforkBaseFeeParams) -> String {
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

#[derive(Debug)]
struct OpImporterError {
    message: String,
}

impl Error for OpImporterError {}

impl fmt::Display for OpImporterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
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
