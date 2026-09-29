//! Resolving Solidity imports to on-disk files, and reading those files for
//! Slang's compilation builder.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use edr_common::fs::normalize_path;
use slang_solidity_v2::compilation::CompilationBuilderConfig;

/// Resolves Solidity imports.
///
/// `./` and `../` are normalized. Every other import path is looked up in the
/// map of import source name to absolute path provided on construction.
#[derive(Clone, Debug, Default)]
pub struct ImportResolver {
    import_map: HashMap<String, PathBuf>,
}

impl ImportResolver {
    /// Constructs a new instance.
    pub fn new(import_map: HashMap<String, PathBuf>) -> Self {
        Self { import_map }
    }

    /// Tries to resolve a `Solidity` import.
    pub fn resolve_import(
        &self,
        source_file_id: &str,
        import_path: &str,
    ) -> Result<String, String> {
        if is_relative_import(import_path) {
            let parent = Path::new(source_file_id)
                .parent()
                .unwrap_or_else(|| Path::new(""));
            let normalized = normalize_path(&parent.join(import_path));
            Ok(normalized.to_string_lossy().into_owned())
        } else {
            self.import_map
                .get(import_path)
                .map(|path| normalize_path(path).to_string_lossy().into_owned())
                .ok_or_else(|| format!("import '{import_path}' not found in import mappings"))
        }
    }
}

/// The root file's id and its already-read text.
pub(crate) struct RootSource<'root> {
    pub id: &'root str,
    pub content: &'root str,
}

/// Serves the root from the text the caller read, reads every other file from
/// disk, and resolves imports.
pub(crate) struct SourceProvider<'a> {
    import_resolver: &'a ImportResolver,
    root: RootSource<'a>,
}

impl<'a> SourceProvider<'a> {
    pub fn new(import_resolver: &'a ImportResolver, root: RootSource<'a>) -> Self {
        Self {
            import_resolver,
            root,
        }
    }
}

impl CompilationBuilderConfig for SourceProvider<'_> {
    /// Keep `fs_permissions` out of this path. The file ids come from the
    /// paths the test runner was configured with, never from paths a test
    /// controls, so this reads a project source exactly as the compiler does.
    fn read_file(&mut self, file_id: &str) -> Result<String, String> {
        if file_id == self.root.id {
            return Ok(self.root.content.to_owned());
        }

        std::fs::read_to_string(Path::new(file_id)).map_err(|error| error.to_string())
    }

    /// Reports an import whose resolved path holds no file as unresolved,
    /// because Slang's binder panics when a contract inherits from an import
    /// it resolved but could not read. An unresolved import only yields a
    /// diagnostic.
    fn resolve_import(
        &mut self,
        source_file_id: &str,
        import_path: &str,
    ) -> Result<String, String> {
        let resolved = self
            .import_resolver
            .resolve_import(source_file_id, import_path)?;

        if Path::new(&resolved).is_file() {
            Ok(resolved)
        } else {
            Err(format!(
                "import '{import_path}' resolves to '{resolved}', which is not a file"
            ))
        }
    }
}

/// Whether an import path is relative (resolved against the importer) rather
/// than mapped (npm-style, resolved via the import map).
fn is_relative_import(import_path: &str) -> bool {
    import_path.starts_with("./") || import_path.starts_with("../")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts a resolution result equals `expected`, comparing as paths so the
    /// platform's separator doesn't matter (`normalize_path` emits `\` on
    /// Windows; the resolved string is only ever handed to
    /// `fs::read_to_string`, which accepts either separator).
    fn assert_resolves(result: Result<String, String>, expected: &str) {
        assert_eq!(result.map(PathBuf::from), Ok(PathBuf::from(expected)));
    }

    #[test]
    fn resolves_relative_imports_against_the_importer() {
        let resolver = ImportResolver::default();
        assert_resolves(
            resolver.resolve_import("/project/contracts/A.sol", "./lib/B.sol"),
            "/project/contracts/lib/B.sol",
        );
        assert_resolves(
            resolver.resolve_import("/project/contracts/lib/B.sol", "../A.sol"),
            "/project/contracts/A.sol",
        );
    }

    #[test]
    fn resolves_mapped_imports() {
        let resolver = ImportResolver::new(
            [(
                "@oz/contracts/token/ERC20.sol".to_owned(),
                PathBuf::from("/deps/@oz/contracts/token/ERC20.sol"),
            )]
            .into(),
        );
        assert_resolves(
            resolver.resolve_import("/project/A.sol", "@oz/contracts/token/ERC20.sol"),
            "/deps/@oz/contracts/token/ERC20.sol",
        );
    }

    #[test]
    fn provider_reports_a_missing_file_as_unresolved() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("Dep.sol"), "").expect("write dependency");
        let root_id = dir.path().join("Root.sol").to_string_lossy().into_owned();

        let resolver = ImportResolver::default();
        let mut provider = SourceProvider::new(
            &resolver,
            RootSource {
                id: &root_id,
                content: "",
            },
        );

        assert!(provider.resolve_import(&root_id, "./Dep.sol").is_ok());
        assert!(provider.resolve_import(&root_id, "./Missing.sol").is_err());
    }

    #[test]
    fn provider_serves_the_root_from_memory_and_imports_from_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("Dep.sol");
        std::fs::write(&dep, "on disk").expect("write dependency");
        let root_id = dir.path().join("Root.sol").to_string_lossy().into_owned();

        let resolver = ImportResolver::default();
        let mut provider = SourceProvider::new(
            &resolver,
            RootSource {
                id: &root_id,
                content: "in memory",
            },
        );

        assert_eq!(provider.read_file(&root_id), Ok("in memory".to_owned()));
        assert_eq!(
            provider.read_file(&dep.to_string_lossy()),
            Ok("on disk".to_owned())
        );
    }

    #[test]
    fn unmapped_imports_error() {
        let resolver = ImportResolver::default();
        assert!(resolver
            .resolve_import("/project/A.sol", "@oz/contracts/token/ERC20.sol")
            .is_err());
    }
}
