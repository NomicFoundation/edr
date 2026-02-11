use std::{path::PathBuf, process::Command, sync::OnceLock};

use anyhow::{anyhow, bail};

static WORKSPACE_ROOT_PATH: OnceLock<anyhow::Result<String>> = OnceLock::new();
pub const COOLDOWN_FILE_CONFIG: &str = "cooldown.toml";
pub const ALLOWLIST_FILE_CONFIG: &str = "cooldown-allowlist.toml";

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

pub fn root_path() -> anyhow::Result<&'static String> {
    WORKSPACE_ROOT_PATH
        .get_or_init(workspace_root)
        .as_ref()
        .map_err(|error| anyhow!("Could not determine EDR root path: {error}"))
}

fn cargo_config_path() -> anyhow::Result<PathBuf> {
    let mut path = PathBuf::from(root_path()?);
    path.push(".cargo");

    log::debug!("project config path: {}", path.to_string_lossy());

    Ok(path)
}

pub fn config_file_path(filename: &str) -> anyhow::Result<PathBuf> {
    let path_buf = cargo_config_path()?;
    cargo_config_path()?.push(filename);
    Ok(path_buf)
}
