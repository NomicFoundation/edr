use git2::Repository;
use std::ffi::OsString;
// use std::env;
use std::fs::File;
use std::io::Write;
use std::{
    error::Error,
    fs::{self, DirEntry},
    io,
    path::{Path, PathBuf},
};
use tempfile::tempdir; // Required for `fmt::Write` trait

fn main() -> Result<(), Box<dyn Error>> {
    // let out_dir = env::var_os("OUT_DIR").unwrap();
    // println!("cargo::warning=Out dir: {}", out_dir.into_string().unwrap());
    let modules_dir = Path::new("src/hardfork/generated").canonicalize().unwrap();
    let mut generated_module = File::create(Path::new("src/hardfork/generated.rs"))?;

    println!(
        "cargo::warning=Modules dir: {}",
        modules_dir.to_str().unwrap()
    );
    // Create a temporary directory that will be automatically deleted on drop
    let temp_dir = tempdir()?;
    let path = temp_dir.path();
    // let path = Path::new("./repository");

    let repo_url = "https://github.com/ethereum-optimism/superchain-registry.git";

    // println!("Cloning {} into temporary directory: {:?}", repo_url, path);

    // Clone the repository into the temporary directory
    Repository::clone(repo_url, path)?;
    // // println!("Repository cloned successfully.");

    let mut chains_path = PathBuf::from(path);
    chains_path.push("superchain/configs/mainnet");
    let entries = fs::read_dir(chains_path)?;
    entries.into_iter().for_each(|e| {
        let res = match e {
            Ok(file) => {
                let module_generated = build_hardfork_for_chain(file, &modules_dir);
                match module_generated {
                    Ok(name) => writeln!(generated_module, "pub mod {};", name),
                    Err(_) => Ok(()),
                }
            }
            Err(_) => Ok(()),
        };
        println!("{:?}", res);
    });

    println!("cargo::rerun-if-changed=build.rs");
    Ok(())
}

fn build_hardfork_for_chain(
    dir_entry: DirEntry,
    output_path: &PathBuf,
) -> Result<String, OpImporterError> {
    let file_contents = fs::read_to_string(dir_entry.path())?;
    let chain_config: OpChainConfig = toml::from_str(&file_contents)?;
    let module_name = {
        match dir_entry.file_name().into_string()?.replace('-', "_").split_once(".") {
            Some((name, _extension)) => String::from(name),
            None => {
                return Err(OpImporterError {
                    message: format!("could not parse file {:?}", dir_entry.file_name()),
                })
            }
        }
    };
    let path_name = {
        let mut path = PathBuf::from(output_path);
        path.push(format!("{}.rs", module_name));
        path
    };

    let mut module = File::create(path_name)?;
    write!(
        &mut module,
        "
use std::{{str::FromStr, sync::LazyLock}};

use edr_evm::hardfork::{{self, Activations, ChainConfig, ForkCondition}};
use op_revm::OpSpecId;

pub const MAINNET_CHAIN_ID: u64 = {};

pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {{
    name: \"{}\".into(),
    hardfork_activations: Activations::new( vec![
    ",
        format!("0x{:X}", chain_config.chain_id),
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
            hardfork.0.split_once("_time").unwrap().0
        )?;
    }
    write!(
        &mut module,
        "   ]),
}});"
    )?;

    // println!("{}", module);
    // println!("------------------");

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
