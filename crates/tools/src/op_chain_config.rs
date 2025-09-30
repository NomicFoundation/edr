use core::fmt;
use std::{
    collections::HashMap,
    error::Error,
    fs::{self, create_dir_all, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use git2::Repository;
use op_revm::OpSpecId;
use tempfile::tempdir; // Required for `fmt::Write` trait

const GENERATED_FILE_WARNING_MESSAGE: &str = "
    // WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
    // Any changes made to this file will be overwritten the next time it is generated.
    // To make changes, update the generator script instead in `tools/src/op_chain_config.rs`.
";

pub fn import_op_chain_configs() -> Result<(), anyhow::Error> {
    let modules_dir = Path::new("./crates/edr_op/src/hardfork/generated");
    create_dir_all(modules_dir)?;

    let generated_module_path = Path::new("./crates/edr_op/src/hardfork/generated.rs");
    let mut generated_module = File::create(generated_module_path)?;

    println!(
        "Generated modules dir: {}",
        modules_dir.canonicalize()?.to_str().unwrap()
    );
    // Create a temporary directory that will be automatically deleted on drop
    let temp_dir = tempdir()?;
    let repo_path = temp_dir.path();

    let repo_url = "https://github.com/ethereum-optimism/superchain-registry.git";

    // Clone the repository into the temporary directory
    Repository::clone(repo_url, repo_path)?;

    let networks = ["mainnet", "sepolia"];
    let mut config_path = PathBuf::from(repo_path);
    config_path.push("superchain/configs");

    // map that contains filename - [networks], so that we can generate one module
    // per filename
    let mut files_to_generate = HashMap::<String, Vec<String>>::new();

    for network in networks {
        let mut network_config_path = PathBuf::from(&config_path);
        network_config_path.push(network);

        for entry in fs::read_dir(network_config_path)? {
            let filename = entry?.file_name().to_string_lossy().to_string();
            files_to_generate
                .entry(filename)
                .and_modify(|entry| entry.push(network.to_string()))
                .or_insert(vec![network.to_string()]);
        }
    }

    write!(
        generated_module,
        "
        {GENERATED_FILE_WARNING_MESSAGE}
        
        use edr_primitives::HashMap;
        use edr_evm::hardfork::ChainConfig;
        use crate::Hardfork;

    "
    )?;

    let mut chains_by_module = Vec::<(String, String)>::new();

    for (file_name, networks) in files_to_generate {
        let chain_module: Result<String, anyhow::Error> =
            build_chain_module(&config_path, file_name.clone(), &networks, modules_dir);
        match chain_module {
            Ok(name) => {
                writeln!(
                    generated_module,
                    "/// `{}` chain configuration module;",
                    &name
                )?;
                writeln!(generated_module, "pub mod {};", &name)?;
                for network in networks {
                    chains_by_module.push((name.clone(), network));
                }
            }
            Err(error) => println!("Skipping {file_name} chain module generation due to {error}",),
        };
    }
    update_generated_module(&mut generated_module, &mut chains_by_module)?;
    Command::new("rustfmt")
        .arg("+nightly")
        .arg(generated_module_path)
        .output()?;

    println!("Running `cargo check`...");
    let cargo_check_output = Command::new("cargo").arg("check").output()?;
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

fn update_generated_module(
    generated_module: &mut File,
    chains_by_module: &mut Vec<(String, String)>,
) -> Result<(), anyhow::Error> {
    write!(
        generated_module,
        "
    pub(crate) fn chain_configs() -> HashMap<u64, &'static ChainConfig<Hardfork>> {{

        let mut hardforks = HashMap::new();
    "
    )?;

    chains_by_module.sort();
    for (module, network) in chains_by_module {
        write!(
            generated_module,
            "
            hardforks.insert({}, &*{});
        ",
            module_attribute(module, &chain_id_name(network)),
            module_attribute(module, &config_name(network))
        )?;
    }

    write!(
        generated_module,
        "
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
fn config_name(network: &str) -> String {
    format!("{}_CONFIG", network.to_uppercase())
}
fn build_chain_module(
    config_path: &PathBuf,
    file_name: String,
    networks: &[String],
    output_path: &Path,
) -> Result<String, anyhow::Error> {
    if networks.is_empty() {
        return Err(OpImporterError {
            message: "No networks for chain".to_string(),
        }
        .into());
    }
    let chain = {
        match file_name.clone().replace('-', "_").split_once(".") {
            Some((name, _extension)) => String::from(name),
            None => {
                return Err(OpImporterError {
                    message: format!("could not define module filename: {file_name}"),
                }
                .into())
            }
        }
    };
    let module_path = {
        let mut path = PathBuf::from(output_path);
        path.push(format!("{chain}.rs"));
        path
    };
    let mut module = File::create(module_path.clone())?;

    write!(
        &mut module,
        "
    {GENERATED_FILE_WARNING_MESSAGE}
    
    use std::sync::LazyLock;
    
    use edr_eip1559::{{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams}};
    use edr_evm::hardfork::{{self, Activations, ChainConfig, ForkCondition}};
    use op_revm::OpSpecId;
    "
    )?;
    for network in networks {
        let mut chain_config_path = PathBuf::from(config_path);
        chain_config_path.push(network.clone());
        chain_config_path.push(file_name.clone());

        let file_contents = fs::read_to_string(chain_config_path)?;
        let chain_config: OpChainConfig = toml::from_str(&file_contents)?;
        let chain_base_fee_params: String = build_base_fee_params(chain_config.optimism);
        write!(
            &mut module,
            "
    /// `{chain}` {network} chain id
    pub const {}: u64 = 0x{:X};
    ",
            chain_id_name(network),
            chain_config.chain_id,
        )?;
        write!(
            &mut module,
            "
    /// `{chain}` {network} chain configuration
    pub static {}: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {{
        name: \"{}\".into(),
        base_fee_params: {chain_base_fee_params}, 
        hardfork_activations: Activations::new( vec![
        ",
            config_name(network),
            chain_config.name
        )?;
        'activations: for activations in chain_config.hardforks.iter() {
            let hardfork_name = activations
                .0
                .split_once("_time")
                .map(|(before_match, _)| before_match);

            let hardfork_name = match hardfork_name {
                None => {
                    println!(
                        "{chain} {network}: ignoring activation - activation is not time based: {}",
                        activations.0
                    );
                    continue 'activations;
                }
                Some(name) => capitalize_first_letter(name),
            };

            let hardfork = match OpSpecId::from_str(hardfork_name.as_str()) {
                Err(_) => {
                    println!(
                        "{chain} {network}: ignoring activation - hardfork name is not supported: {hardfork_name}",
                    );
                    continue 'activations;
                }
                Ok(hardfork) => hardfork,
            };
            let hardfork_str: &'static str = hardfork.into();
            write!(
                &mut module,
                "
            hardfork::Activation {{
                condition: ForkCondition::Timestamp({}),
                hardfork: OpSpecId::{},
            }},
    ",
                activations.1,
                hardfork_str.to_uppercase()
            )?;
        }
        write!(&mut module, "   ]),}});")?;
    }

    Command::new("rustfmt")
        .arg("+nightly")
        .arg(module_path)
        .output()?;

    Ok(chain)
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
