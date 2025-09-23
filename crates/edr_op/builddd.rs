use std::collections::HashMap;
// use std::env;
use std::fs::{create_dir_all, File};
use std::io::Stderr;
use std::net;
use std::{
    error::Error,
    ffi::OsString,
    fs::{self, DirEntry},
    io,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use git2::Repository;
use tempfile::tempdir; // Required for `fmt::Write` trait

fn main() -> Result<(), OpImporterError> {
    // let out_dir = env::var_os("OUT_DIR").unwrap();
    // println!("cargo::warning=Out dir: {}", out_dir.into_string().unwrap());
    let modules_dir = Path::new("src/hardfork/generated");
    create_dir_all(modules_dir)?;

    let generated_module_path = Path::new("src/hardfork/generated.rs");
    let mut generated_module = File::create(generated_module_path)?;

    println!(
        "cargo::warning=Modules dir: {}",
        modules_dir.to_str().unwrap()
    );
    // Create a temporary directory that will be automatically deleted on drop
    let temp_dir = tempdir()?;
    let repo_path = temp_dir.path();
    // let path = Path::new("./repository");

    let repo_url = "https://github.com/ethereum-optimism/superchain-registry.git";

    // println!("Cloning {} into temporary directory: {:?}", repo_url, path);

    // Clone the repository into the temporary directory
    Repository::clone(repo_url, repo_path)?;
    // // println!("Repository cloned successfully.");

    let networks = ["mainnet", "sepolia"];
    let mut config_path = PathBuf::from(repo_path);
    config_path.push("superchain/configs");

    // map that contains filename - [networks], so that we can generate one module per filename
    let mut files_to_generate = HashMap::<String, Vec<String>>::new();

    for network in networks {
        let mut network_config_path = PathBuf::from(&config_path);
        network_config_path.push(network);

        for entry in fs::read_dir(network_config_path)? {
            let filename = entry?.file_name().to_string_lossy().to_string();
            if files_to_generate.contains_key(&filename) {
                files_to_generate
                    .get_mut(&filename)
                    .unwrap()
                    .push(network.to_string());
            } else {
                files_to_generate.insert(filename, vec![network.to_string()]);
            }
        }
    }

    println!("cargo::warning=files to generate: {:?}", files_to_generate);

    write!(
        generated_module,
        "
    use std::{{collections::HashMap, sync::OnceLock}};
    use edr_evm::hardfork::ChainConfig;
    use crate::Hardfork;
    "
    )?;
    // module name, network
    let mut chains_by_module = Vec::<(String, String)>::new(); //HashMap::<String, Vec<String>>::new();

    for (file_name, networks) in files_to_generate.into_iter() {
        let chain_module: Result<String, OpImporterError> =
            build_hardfork_for_chain(&config_path, file_name, &networks, modules_dir);
        match chain_module {
            Ok(name) => {
                writeln!(generated_module, "pub mod {};", &name)?;
                networks
                    .into_iter()
                    .for_each(|network| chains_by_module.push((name.clone(), network)));
            }
            Err(_) => (),
        };
    }
    update_generated_module(&mut generated_module, chains_by_module)?;
    Command::new("rustfmt")
        .arg("+nightly")
        .arg(generated_module_path)
        .output()?;

    println!("cargo::rerun-if-changed=build.rs");
    Ok(())
}

fn update_generated_module(
    generated_module: &mut File,
    chains_by_module: Vec<(String, String)>,
) -> Result<(), OpImporterError> {
    write!(
        generated_module,
        "
    fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<Hardfork>> {{

        static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<Hardfork>>> = OnceLock::new();

        CONFIGS.get_or_init(|| {{
            let mut hardforks = HashMap::new();
    "
    )?;

    for (module, network) in chains_by_module.into_iter() {
        write!(
            generated_module,
            "
            hardforks.insert({}, &*{});
        ",
            module_attribute(&module, &chain_id_name(&network)),
            module_attribute(&module, &config_name(&network))
        )?;
    }

    write!(
        generated_module,
        "
            hardforks
        }})
    }}
    "
    )?;

    Ok(())
}

fn module_attribute(module: &str, attribute: &str) -> String {
    format!("{}::{}", module, attribute)
}
fn chain_id_name(network: &str) -> String {
    format!("{}_CHAIN_ID", network.to_uppercase())
}
fn config_name(network: &str) -> String {
    format!("{}_CONFIG", network.to_uppercase())
}
fn build_hardfork_for_chain(
    config_path: &PathBuf,
    // dir_entry: String,
    file_name: String,
    networks: &[String],
    output_path: &Path,
) -> Result<String, OpImporterError> {
    if networks.is_empty() {
        return Err(OpImporterError {
            message: "No networks for chain".to_string(),
        });
    }
    let module_name = {
        match file_name.clone().replace('-', "_").split_once(".") {
            Some((name, _extension)) => String::from(name),
            None => {
                return Err(OpImporterError {
                    message: format!("could not file_name {:?}", file_name),
                })
            }
        }
    };
    let module_path = {
        let mut path = PathBuf::from(output_path);
        path.push(format!("{}.rs", module_name));
        path
    };
    let mut module = File::create(module_path.clone())?;

    write!(
        &mut module,
        "
    use std::{{str::FromStr, sync::LazyLock}};
    
    use edr_evm::hardfork::{{self, Activations, ChainConfig, ForkCondition}};
    use op_revm::OpSpecId;
    "
    )?; // TODO: is there a way for knowing the import names depending on how those types get imported in this script file?
    for network in networks {
        let mut chain_config_path = PathBuf::from(config_path);
        chain_config_path.push(network.clone());
        chain_config_path.push(file_name.clone());

        let file_contents = fs::read_to_string(chain_config_path)?;
        let chain_config: OpChainConfig = toml::from_str(&file_contents)?;
        write!(
            &mut module,
            "
    pub const {}: u64 = {};
    ",
            chain_id_name(&network),
            format!("0x{:X}", chain_config.chain_id),
        )?;
        write!(
            &mut module,
            "
    pub static {}: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {{
        name: \"{}\".into(),
        hardfork_activations: Activations::new( vec![
        ",
            config_name(&network),
            chain_config.name
        )?;
        for hardfork in chain_config.hardforks.iter() {
            write!(
                &mut module,
                "
            hardfork::Activation {{
                condition: ForkCondition::Timestamp({}),
                hardfork: OpSpecId::from_str(\"{}\").unwrap(),
            }},
    ",
                hardfork.1,
                hardfork
                    .0
                    .split_once("_time")
                    .expect("hardfork activation is not time based")
                    .0
            )?;
        }
        write!(&mut module, "   ]),}});")?;
    }

    Command::new("rustfmt")
        .arg("+nightly")
        .arg(module_path)
        .output()?;

    Ok(module_name)
}

#[derive(Debug)]
struct OpImporterError {
    message: String,
}

impl From<io::Error> for OpImporterError {
    fn from(value: io::Error) -> Self {
        OpImporterError {
            message: value.to_string(),
        }
    }
}
impl From<git2::Error> for OpImporterError {
    fn from(value: git2::Error) -> Self {
        OpImporterError {
            message: value.to_string(),
        }
    }
}

impl From<toml::de::Error> for OpImporterError {
    fn from(value: toml::de::Error) -> Self {
        OpImporterError {
            message: value.to_string(),
        }
    }
}

impl From<std::fmt::Error> for OpImporterError {
    fn from(value: std::fmt::Error) -> Self {
        OpImporterError {
            message: value.to_string(),
        }
    }
}

impl From<OsString> for OpImporterError {
    fn from(value: OsString) -> Self {
        OpImporterError {
            message: format!("Could not convert OsString {:?} to string", value),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct OpChainConfig {
    name: String,
    chain_id: i64,
    hardforks: toml::Table,
}
