use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context as _};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Overwrite,
    #[allow(unused)]
    Verify,
}

pub fn update(path: &Path, contents: &str, mode: Mode) -> anyhow::Result<()> {
    let contents = contents.replace("\r\n", "\n");
    let old_contents = match fs::read_to_string(path) {
        Ok(old_contents) => old_contents.replace("\r\n", "\n"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && mode == Mode::Overwrite => {
            eprintln!("creating {}", path.display());
            fs::write(path, &contents).with_context(|| format!("creating {}", path.display()))?;
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", path.display()));
        }
    };
    if old_contents == contents {
        return Ok(());
    }

    if mode == Mode::Verify {
        let changes = difference::Changeset::new(&old_contents, &contents, "\n");
        bail!("`{}` is not up-to-date:\n{}", path.display(), changes,);
    }
    eprintln!("updating {}", path.display());
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn project_root() -> PathBuf {
    // Must match this crate's depth below the repo root
    // (`crates/tool/cli` -> three ancestors up).
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf()
}
