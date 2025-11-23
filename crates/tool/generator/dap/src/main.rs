use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use anyhow::bail;
use git2::{build::RepoBuilder, FetchOptions, Oid};
use tempfile::tempdir;
use typify::{TypeSpace, TypeSpaceSettings};

const REPO_URL: &str = "https://github.com/microsoft/debug-adapter-protocol";
const SCHEMA_PATH: &str = "debugAdapterProtocol.json";
const TARGET_CRATE_DIR: &str = "crates/debugger/protocol";

fn main() -> anyhow::Result<()> {
    let schema = clone_schema()?;
    write_schema_to_disk(schema)?;

    Ok(())
}

struct Schema {
    content: String,
    commit_sha: Oid,
}

fn clone_schema() -> anyhow::Result<Schema> {
    let temp_dir = tempdir()?;

    let mut fetch_options = FetchOptions::new();
    fetch_options.depth(1);

    let repo = RepoBuilder::new()
        .fetch_options(fetch_options)
        .clone(REPO_URL, temp_dir.path())?;
    let commit_sha = repo
        .head()?
        .target()
        .ok_or_else(|| anyhow::anyhow!("Failed to get HEAD target"))?;

    let schema_path = temp_dir.path().join(SCHEMA_PATH);
    let content = fs::read_to_string(&schema_path)?;

    Ok(Schema {
        content,
        commit_sha,
    })
}

fn crate_name(crate_dir: &Path) -> anyhow::Result<String> {
    let cargo_toml = fs::read_to_string(crate_dir.join("Cargo.toml"))?;
    let cargo_toml = toml::Value::from_str(&cargo_toml)?;

    if let Some(name) = cargo_toml
        .get("package")
        .and_then(|pkg| pkg.get("name"))
        .and_then(|name| name.as_str())
    {
        Ok(name.to_string())
    } else {
        anyhow::bail!("Failed to find crate name in Cargo.toml");
    }
}

fn find_workspace_root() -> anyhow::Result<PathBuf> {
    let mut current_dir = PathBuf::from_str(env!("CARGO_MANIFEST_DIR"))?;

    while let Some(parent) = current_dir.parent() {
        let cargo_toml = current_dir.join("Cargo.toml");
        if cargo_toml.exists()
            && let Ok(contents) = fs::read_to_string(&cargo_toml)
            && contents.contains("[workspace]")
        {
            return Ok(current_dir.clone());
        }

        current_dir = parent.to_path_buf();
    }

    anyhow::bail!("Workspace root not found")
}

fn format_file(file_path: &Path) -> anyhow::Result<()> {
    Command::new("rustfmt")
        .arg("+nightly")
        .arg(file_path)
        .output()?;

    Ok(())
}

fn validate_crate(crate_dir: &Path) -> anyhow::Result<()> {
    let crate_name = crate_name(crate_dir)?;

    let output = Command::new("cargo")
        .arg("clippy")
        .arg("-p")
        .arg(crate_name)
        .output()?;

    if !output.status.success() {
        bail!("Cargo clippy failed: {}", String::from_utf8(output.stderr)?);
    }

    Ok(())
}

fn write_schema_to_disk(
    Schema {
        content,
        commit_sha,
    }: Schema,
) -> anyhow::Result<()> {
    let mut settings = TypeSpaceSettings::default();
    settings.with_derive("Default".to_owned());

    let mut type_space = TypeSpace::new(&settings);

    let schema = serde_json::from_str(&content)?;
    type_space.add_root_schema(schema)?;

    let workspace_root = find_workspace_root()?;
    let dap_crate_path = workspace_root.join(TARGET_CRATE_DIR);
    let dap_file_path = dap_crate_path.join("src/lib.rs");

    let mut file = File::create(&dap_file_path)?;
    write!(
        file,
        r#"
// WARNING: This file is auto-generated. Do not edit manually.
// source: {REPO_URL}/tree/{commit_sha}/{SCHEMA_PATH}
//
// To update, run `cargo gen-dap` inside the workspace.

"#
    )?;
    write!(file, "{}", type_space.to_stream())?;

    format_file(&dap_file_path)?;
    validate_crate(&dap_crate_path)?;

    Ok(())
}
