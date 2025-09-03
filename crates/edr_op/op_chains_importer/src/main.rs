use git2::Repository;
use indexmap::IndexMap;
use std::fmt::Write;
use std::{
    collections::HashMap,
    error::Error,
    fs::{self, DirEntry},
    io,
    path::{Path, PathBuf},
};
use tempfile::tempdir; // Required for `fmt::Write` trait

fn main() -> Result<(), Box<dyn Error>> {
    // Create a temporary directory that will be automatically deleted on drop
    // let temp_dir = tempdir()?;
    // let path = temp_dir.path();
    let path = Path::new("./repository");

    let repo_url = "https://github.com/ethereum-optimism/superchain-registry.git";

    println!("Cloning {} into temporary directory: {:?}", repo_url, path);

    // Clone the repository into the temporary directory
    // let _repo = Repository::clone(repo_url, path)?;
    // println!("Repository cloned successfully.");

    let mut chains_path = PathBuf::from(path);
    chains_path.push("superchain/configs/mainnet");
    let entries = fs::read_dir(chains_path)?;
    entries.into_iter().for_each(|e| {
        let res = match e {
            Ok(file) => build_hardfork_for_chain(file),
            Err(_) => Ok(()),
        };
        println!("{:?}", res);
    });

    // The temporary directory and its contents will be automatically cleaned up
    // when `temp_dir` goes out of scope.

    Ok(())
}

fn build_hardfork_for_chain(dirEntry: DirEntry) -> Result<(), OpImporterError> {
    println!("Parsing {:?}", dirEntry.file_name());
    let file_contents = fs::read_to_string(dirEntry.path())?;
    let chain_config: OpChainConfig = toml::from_str(&file_contents)?;
    let mut representation = String::new();
    write!(
        &mut representation,
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
            &mut representation,
"
        hardfork::Activation {{
            condition: ForkCondition::Timestamp({}),
            hardfork: OpSpecId::from_str(\"{}\").unwrap(),
        }},
",
            hardfork.1, hardfork.0.split_once("_time").unwrap().0
        )?;
    }
    write!(&mut representation, 
"   ]),
}});")?;

    println!("{}", representation);
    println!("------------------");

    Ok(())
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
#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct OpChainConfig {
    name: String,
    chain_id: i64,
    hardforks: toml::Table,
}
