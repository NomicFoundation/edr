//! [`CompilationBuilderConfig`] implementation that reads Solidity sources from
//! disk and resolves imports.

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

/// Reads files from disk and resolves imports.
pub(crate) struct SourceProvider<'resolver> {
    import_resolver: &'resolver ImportResolver,
}

impl<'resolver> SourceProvider<'resolver> {
    pub fn new(import_resolver: &'resolver ImportResolver) -> Self {
        Self { import_resolver }
    }
}

impl CompilationBuilderConfig for SourceProvider<'_> {
    fn read_file(&mut self, file_id: &str) -> Result<String, String> {
        std::fs::read_to_string(Path::new(file_id)).map_err(|error| error.to_string())
    }

    fn resolve_import(
        &mut self,
        source_file_id: &str,
        import_path: &str,
    ) -> Result<String, String> {
        self.import_resolver
            .resolve_import(source_file_id, import_path)
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
    fn unmapped_imports_error() {
        let resolver = ImportResolver::default();
        assert!(resolver
            .resolve_import("/project/A.sol", "@oz/contracts/token/ERC20.sol")
            .is_err());
    }
}
