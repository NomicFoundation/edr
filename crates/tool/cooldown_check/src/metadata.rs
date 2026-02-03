use anyhow::Result;
use cargo_metadata::Metadata;
use clap_cargo::{Features, Manifest};

pub fn read_metadata(manifest: &Manifest, features: &Features) -> Result<Metadata> {
    let mut command = manifest.metadata();
    features.forward_metadata(&mut command);
    let metadata = command.exec()?;
    Ok(metadata)
}
