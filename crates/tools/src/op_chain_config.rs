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

const GENERATED_FILE_WARNING_MESSAGE: &str = "
    // WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
    // Any changes made to this file will be overwritten the next time it is generated.
    // To make changes, update the generator script instead in `tools/src/op_chain_config.rs`.
";
const EDR_SUPPORTED_NETWORKS: [&str; 2] = ["mainnet", "sepolia"];
const SUPERCHAIN_REGISTRY_REPO_URL: &'static str =
    "https://github.com/ethereum-optimism/superchain-registry.git";
const REPO_CONFIGS_PATH: &str = "superchain/configs";
const GENERATED_MODULE_PATH: &str = "./crates/edr_op/src/hardfork/generated.rs";
/// These hardforks are not included in OpSpecId since they only impacted the
/// conscensus layer
const KNOWN_IGNORED_HARDFORKS: [&str; 2] = ["delta", "pectra_blob_schedule"];

pub fn import_op_chain_configs() -> Result<(), anyhow::Error> {
    // Create a temporary directory that will be automatically deleted on drop
    let superchain_registry_repo_dir = tempdir()?;
    Repository::clone(
        SUPERCHAIN_REGISTRY_REPO_URL,
        superchain_registry_repo_dir.path(),
    )?;

    let modules_dir = Path::new("./crates/edr_op/src/hardfork/generated");
    create_dir_all(modules_dir)?;

    println!(
        "Generated modules dir: {}",
        modules_dir.canonicalize()?.to_str().unwrap()
    );

    let chains_to_configure =
        fetch_op_stack_chains_to_configure(superchain_registry_repo_dir.path())?;

    let chains_by_module: Vec<(String, String)> = generate_chain_modules(
        superchain_registry_repo_dir.path(),
        modules_dir,
        chains_to_configure,
    );

    generate_generated_module(chains_by_module)?;

    let generated_files_path = {
        let mut path_buf = modules_dir.to_path_buf();
        path_buf.push("*");
        path_buf.as_os_str().to_owned()
    };

    println!("Formatting generated files...");
    Command::new("rustfmt")
        .arg("+nightly")
        .arg(GENERATED_MODULE_PATH)
        .arg(generated_files_path)
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

fn generate_chain_modules(
    repo_path: &Path,
    modules_dir: &Path,
    chains_and_networks: HashMap<String, Vec<String>>,
) -> Vec<(String, String)> {
    let mut gnerated_chain_configs = Vec::<(String, String)>::new();

    for (chain_file_name, networks) in chains_and_networks {
        let chain_module = build_chain_module(
            &repo_config_path_buf(repo_path),
            chain_file_name.clone(),
            &networks,
            modules_dir,
        );
        match chain_module {
            Ok(module_name) => {
                for network in networks {
                    gnerated_chain_configs.push((module_name.clone(), network));
                }
            }
            Err(error) => {
                println!("Skipping {chain_file_name} chain module generation due to {error}",)
            }
        };
    }
    gnerated_chain_configs
}

/// Based on SuperChain registry repository, idetifies all the chains and their
/// corresponding networks to create configs for Returns a map groupping
/// networks by chain, since in EDR we want to create a single module per chain
fn fetch_op_stack_chains_to_configure(
    repo_path: &Path,
) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let config_path = repo_config_path_buf(repo_path);

    let mut networks_by_chain = HashMap::<String, Vec<String>>::new();

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
    Ok(networks_by_chain)
}

fn repo_config_path_buf(repo_path: &Path) -> PathBuf {
    let mut path = PathBuf::from(repo_path);
    path.push(REPO_CONFIGS_PATH);
    path
}
fn generate_generated_module(
    mut chains_by_module: Vec<(String, String)>,
) -> Result<(), anyhow::Error> {
    let generated_module_path = Path::new(GENERATED_MODULE_PATH);

    let mut generated_module: File = File::create(generated_module_path)?;

    chains_by_module.sort();
    let module_imports = chains_by_module
        .iter()
        .map(|(module, _)| module)
        .unique()
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

    let insert_lines =
        chains_by_module
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

            {insert_lines}
            
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
fn build_chain_module(
    repo_config_path: &PathBuf,
    file_name_in_repo: String,
    networks: &[String],
    output_path: &Path,
) -> Result<String, anyhow::Error> {
    if networks.is_empty() {
        return Err(OpImporterError {
            message: "No networks for chain".to_string(),
        }
        .into());
    }
    let chain_module_name = {
        match file_name_in_repo.clone().replace('-', "_").split_once(".") {
            Some((name, _extension)) => String::from(name),
            None => {
                return Err(OpImporterError {
                    message: format!("could not define module filename: {file_name_in_repo}"),
                }
                .into())
            }
        }
    };

    let networks_config = {
        let mut networks_config = String::new();
        for network in networks {
            let chain_config_path = {
                let mut path = PathBuf::from(repo_config_path);
                path.push(network.clone());
                path.push(file_name_in_repo.clone());
                path
            };

            let file_contents = fs::read_to_string(chain_config_path)?;
            let chain_config: OpChainConfig = toml::from_str(&file_contents)?;

            let chain_id_const_name = chain_id_name(network);
            let chain_id_hex = format!("0x{:X}", chain_config.chain_id);
            let chain_config_function = network_config_function(network);
            let chain_bame = chain_config.name;
            let chain_base_fee_params: String = build_base_fee_params(chain_config.optimism);
            let chain_hardfork_activations = build_hardfork_activations_for(
                format!("{chain_module_name} {network}"),
                chain_config.hardforks,
            );

            write!(
                &mut networks_config,
                "
                /// `{chain_module_name}` {network} chain id
                pub const {chain_id_const_name}: u64 = {chain_id_hex};
                
                /// `{chain_module_name}` {network} chain configuration
                pub(crate) fn {chain_config_function} -> ChainConfig<OpSpecId>{{ ChainConfig {{
                    name: \"{chain_bame}\".into(),
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
