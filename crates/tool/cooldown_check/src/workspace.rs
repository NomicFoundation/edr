use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use cargo_metadata::{camino::Utf8PathBuf, Metadata, Node, Package, PackageId};
use clap_cargo::{Features, Manifest};

use crate::{allowlist::Allowlist, config::Config};

pub const COOLDOWN_FILE_CONFIG: &str = "cooldown.toml";
pub const ALLOWLIST_FILE_CONFIG: &str = "cooldown-allowlist.toml";

pub struct Workspace {
    pub packages: HashMap<PackageId, Package>,
    pub root_path: PathBuf,
    pub config: Config,
    pub allowlist: Allowlist,
    pub nodes: Vec<Node>,
}

impl Workspace {
    pub fn load() -> anyhow::Result<Self> {
        let features = {
            let mut features = Features::default();
            features.all_features = true;
            features
        };
        let manifest = Manifest::default();
        let metadata = read_metadata(&manifest, &features)?;
        let config_file_path =
            cargo_config_file_path(&metadata.workspace_root, COOLDOWN_FILE_CONFIG);
        let config = Config::load(&config_file_path)?;
        let allowlist_file_path =
            cargo_config_file_path(&metadata.workspace_root, ALLOWLIST_FILE_CONFIG);
        let allowlist = Allowlist::load(&allowlist_file_path)?;

        let nodes = metadata
            .resolve
            .context("cargo metadata output did not include a resolved dependency graph")?
            .nodes;

        let packages = metadata
            .packages
            .into_iter()
            .map(|pkg| (pkg.id.clone(), pkg))
            .collect();

        let root_path = metadata.workspace_root.into();

        Ok(Self {
            packages,
            root_path,
            config,
            allowlist,
            nodes,
        })
    }
}

fn cargo_config_file_path(workspace_root_path: &Utf8PathBuf, filename: &str) -> PathBuf {
    let mut path = PathBuf::from(workspace_root_path);
    path.push(".cargo");
    path.push(filename);
    path
}

fn read_metadata(manifest: &Manifest, features: &Features) -> anyhow::Result<Metadata> {
    let mut command = manifest.metadata();
    features.forward_metadata(&mut command);
    let metadata = command.exec()?;
    Ok(metadata)
}
