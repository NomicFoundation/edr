//! Matching source files against user-provided globs, to keep some of them
//! out of a collection.

use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::compilation::file_id_of;

/// The separator the globs are written with and the paths are matched with,
/// whatever the host platform spells its own paths with.
const GLOB_SEPARATOR: &str = "/";

/// A glob that could not be compiled.
#[derive(Clone, Debug, thiserror::Error)]
#[error("invalid source exclusion glob `{glob}`: {reason}")]
pub struct InvalidExclusionGlob {
    /// The glob as it was written.
    pub glob: String,
    /// What is wrong with it.
    pub reason: String,
}

impl From<globset::Error> for InvalidExclusionGlob {
    fn from(error: globset::Error) -> Self {
        Self {
            glob: error.glob().unwrap_or_default().to_owned(),
            reason: error.kind().to_string(),
        }
    }
}

/// A set of globs matched against source files, excluding those that match.
///
/// A file is matched by its path relative to the project root, spelled with
/// forward slashes, and a file outside the root by its absolute path. `*` does
/// not cross a separator, while `**` does.
#[derive(Clone, Debug, Default)]
pub struct SourceExclusions {
    /// Normalized the way [`file_id_of`] normalizes the paths matched against
    /// it, so a root spelled differently still strips.
    project_root: PathBuf,
    globs: GlobSet,
}

impl SourceExclusions {
    /// Compiles `globs`, to be matched relative to `project_root`.
    pub fn new<'glob>(
        project_root: &Path,
        globs: impl IntoIterator<Item = &'glob str>,
    ) -> Result<Self, InvalidExclusionGlob> {
        let mut builder = GlobSetBuilder::new();
        for glob in globs {
            builder.add(
                GlobBuilder::new(glob)
                    .literal_separator(true)
                    .build()
                    .map_err(InvalidExclusionGlob::from)?,
            );
        }

        Ok(Self {
            project_root: PathBuf::from(file_id_of(project_root)),
            globs: builder.build().map_err(InvalidExclusionGlob::from)?,
        })
    }

    /// Whether the file held under `file_id` is excluded.
    pub fn excludes(&self, file_id: &str) -> bool {
        self.globs.is_match(self.match_path(file_id))
    }

    /// Spells `file_id` the way the globs are written: relative to the project
    /// root, or absolute when it lies outside the root.
    fn match_path(&self, file_id: &str) -> String {
        let path = Path::new(file_id);
        let path = path.strip_prefix(&self.project_root).unwrap_or(path);

        with_forward_slashes(path)
    }
}

/// Rewrites a path with forward slashes, which is a no-op where the platform
/// already separates with them.
fn with_forward_slashes(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, GLOB_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project root spelled as the platform spells an absolute path.
    fn project_root() -> PathBuf {
        PathBuf::from(file_id_of(Path::new("/project")))
    }

    /// The file id of `relative_path` within [`project_root`].
    fn file_in_root(relative_path: &str) -> String {
        file_id_of(&project_root().join(relative_path))
    }

    fn exclusions(globs: &[&str]) -> SourceExclusions {
        SourceExclusions::new(&project_root(), globs.iter().copied()).expect("valid globs")
    }

    #[test]
    fn a_file_is_matched_by_its_path_relative_to_the_project_root() {
        let exclusions = exclusions(&["contracts/vendor/**"]);

        assert!(exclusions.excludes(&file_in_root("contracts/vendor/Permit.sol")));
        assert!(!exclusions.excludes(&file_in_root("contracts/Permit.sol")));
    }

    #[test]
    fn a_dependency_under_node_modules_is_matched() {
        let exclusions = exclusions(&["node_modules/@uniswap/permit2/**"]);

        assert!(exclusions.excludes(&file_in_root(
            "node_modules/@uniswap/permit2/src/interfaces/IPermit2.sol"
        )));
        assert!(!exclusions.excludes(&file_in_root("node_modules/@openzeppelin/Permit.sol")));
    }

    #[test]
    fn a_file_outside_the_project_root_is_matched_by_its_absolute_path() {
        let outside = file_id_of(Path::new("/elsewhere/vendor/Permit.sol"));
        let absolute = with_forward_slashes(Path::new(&outside));

        assert!(exclusions(&[&absolute]).excludes(&outside));
        assert!(!exclusions(&["vendor/**"]).excludes(&outside));
    }

    #[test]
    fn a_single_star_does_not_cross_a_separator() {
        let exclusions = exclusions(&["contracts/*.sol"]);

        assert!(exclusions.excludes(&file_in_root("contracts/Permit.sol")));
        assert!(!exclusions.excludes(&file_in_root("contracts/vendor/Permit.sol")));
    }

    #[test]
    fn a_double_star_crosses_a_separator() {
        let exclusions = exclusions(&["contracts/**/*.sol"]);

        assert!(exclusions.excludes(&file_in_root("contracts/vendor/deep/Permit.sol")));
    }

    #[test]
    fn an_empty_set_matches_nothing() {
        let exclusions = exclusions(&[]);

        assert!(!exclusions.excludes(&file_in_root("contracts/vendor/Permit.sol")));
    }

    #[test]
    fn an_unparseable_glob_is_rejected_by_name() {
        let error = SourceExclusions::new(
            &project_root(),
            ["contracts/[".to_string()].iter().map(String::as_str),
        )
        .expect_err("an unclosed character class is not a glob");

        assert_eq!(error.glob, "contracts/[");
        assert!(error.to_string().contains("contracts/["), "{error}");
    }
}
