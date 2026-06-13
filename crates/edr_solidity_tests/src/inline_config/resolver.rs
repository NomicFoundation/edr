//! [`CompilationBuilderConfig`] implementation that reads Solidity sources from
//! disk and resolves imports.

use std::path::Path;

pub use edr_solidity_collector_eip712::ImportResolver;
use slang_solidity_v2::compilation::CompilationBuilderConfig;

/// Reads files from disk and resolves imports.
pub(super) struct SourceProvider<'resolver> {
    import_resolver: &'resolver ImportResolver,
}

impl<'resolver> SourceProvider<'resolver> {
    pub(super) fn new(import_resolver: &'resolver ImportResolver) -> Self {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
