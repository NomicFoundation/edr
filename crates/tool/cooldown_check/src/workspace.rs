use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use cargo_metadata::{camino::Utf8PathBuf, Metadata, Node, PackageId};
use clap_cargo::{Features, Manifest};

use crate::{allowlist::Allowlist, config::Config, metadata::read_metadata};

pub const COOLDOWN_FILE_CONFIG: &str = "cooldown.toml";
pub const ALLOWLIST_FILE_CONFIG: &str = "cooldown-allowlist.toml";

pub struct Workspace {
    metadata: Metadata,
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
        let config = Config::load(config_file_path)?;
        let allowlist_file_path =
            cargo_config_file_path(&metadata.workspace_root, ALLOWLIST_FILE_CONFIG);
        let allowlist = Allowlist::load(allowlist_file_path)?;

        let nodes = metadata
            .resolve
            .clone()
            .context("cargo metadata output did not include a resolved dependency graph")?
            .nodes;

        Ok(Self {
            metadata,
            config,
            allowlist,
            nodes,
        })
    }

    pub fn packages(&self) -> HashMap<PackageId, cargo_metadata::Package> {
        self.metadata
            .packages
            .iter()
            .cloned()
            .map(|pkg| (pkg.id.clone(), pkg))
            .collect()
    }
}

fn cargo_config_file_path(workspace_root_path: &Utf8PathBuf, filename: &str) -> PathBuf {
    let mut path = PathBuf::from(workspace_root_path);
    path.push(".cargo");
    path.push(filename);
    path
}
