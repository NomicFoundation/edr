//! Resolving Solidity imports to on-disk files, and reading those files for
//! Slang's compilation builder.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use slang_solidity_v2::compilation::CompilationBuilderConfig;

use crate::compilation::file_id_of;

/// Resolves Solidity imports.
///
/// `./` and `../` imports resolve against the importing file. Every other
/// import path is looked up in the map provided on construction. Either way
/// the result is the absolute, normalized file id the unit holds.
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
            Ok(file_id_of(&parent.join(import_path)))
        } else {
            self.import_map
                .get(import_path)
                .map(|path| file_id_of(path))
                .ok_or_else(|| format!("import '{import_path}' not found in import mappings"))
        }
    }
}

/// What the provider records while Slang builds a compilation unit, until
/// [`Self::finish`] separates the roots it read from the ones it could not.
#[derive(Debug)]
pub(crate) struct SourceRecorder {
    roots: HashMap<String, RootRead>,
    imports: HashMap<String, Vec<String>>,
}

#[derive(Debug)]
enum RootRead {
    Pending,
    Read(String),
    Unreadable(std::io::Error),
}

impl SourceRecorder {
    /// Retains the text of each root in `root_ids`. [`Self::finish`] fails for
    /// any of them Slang never reads.
    pub(crate) fn expecting(root_ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            roots: root_ids
                .into_iter()
                .map(|root_id| (root_id, RootRead::Pending))
                .collect(),
            imports: HashMap::new(),
        }
    }

    /// Splits the record into the roots that were read and those that could
    /// not be. Fails when an expected root was never asked for, which means
    /// the caller did not add it to the builder.
    pub(crate) fn finish(self) -> Result<ProvidedSources, PendingRoots> {
        let mut root_contents = HashMap::new();
        let mut unreadable_roots = HashMap::new();
        let mut pending = Vec::new();
        for (root_id, read) in self.roots {
            match read {
                RootRead::Read(content) => {
                    root_contents.insert(root_id, content);
                }
                RootRead::Unreadable(error) => {
                    unreadable_roots.insert(root_id, error);
                }
                RootRead::Pending => pending.push(root_id),
            }
        }

        if !pending.is_empty() {
            pending.sort_unstable();
            return Err(PendingRoots(pending));
        }

        Ok(ProvidedSources {
            root_contents,
            unreadable_roots,
            imports: self.imports,
        })
    }
}

/// Roots the recorder expected but Slang never asked for.
#[derive(Debug, thiserror::Error)]
#[error("roots never read: {}", .0.join(", "))]
pub(crate) struct PendingRoots(pub(crate) Vec<String>);

/// What the provider recorded while Slang built a compilation unit.
///
/// It holds the text of every root it read, why the other roots could not be
/// read, and the import graph it resolved.
///
/// A unit can span several roots, so its consumers need the import graph to
/// tell which files a given root reaches.
#[derive(Debug)]
pub struct ProvidedSources {
    root_contents: HashMap<String, String>,
    unreadable_roots: HashMap<String, std::io::Error>,
    imports: HashMap<String, Vec<String>>,
}

impl ProvidedSources {
    /// The text of every root that could be read, by file id.
    pub fn root_contents(&self) -> &HashMap<String, String> {
        &self.root_contents
    }

    /// Why each root that could not be read failed, by file id.
    pub fn unreadable_roots(&self) -> &HashMap<String, std::io::Error> {
        &self.unreadable_roots
    }

    /// The file ids reachable from `file_id` through resolved imports,
    /// including `file_id` itself.
    ///
    /// An import that did not resolve is no edge, so it narrows the closure
    /// exactly as it would in a unit built over this root alone.
    pub fn reachable_from<'a>(&'a self, file_id: &'a str) -> HashSet<&'a str> {
        let mut reachable = HashSet::from([file_id]);
        let mut pending = vec![file_id];

        while let Some(current) = pending.pop() {
            let Some(imported) = self.imports.get(current) else {
                continue;
            };

            for target in imported {
                if reachable.insert(target) {
                    pending.push(target);
                }
            }
        }

        reachable
    }
}

/// Reads every file from disk and records the import edges it resolves. The
/// text of each expected root, or why it could not be read, is retained.
///
/// Do not let the provider own the recorder. Slang's builder consumes the
/// provider on `build`, and the record would go with it.
pub(crate) struct SourceProvider<'a> {
    import_resolver: &'a ImportResolver,
    recorder: &'a mut SourceRecorder,
    /// Every import proven readable, holding its text until
    /// [`CompilationBuilderConfig::read_file`] takes it. A key without text is
    /// a file already handed over, which a further importer of it must not
    /// read again.
    import_texts: HashMap<String, Option<String>>,
}

impl<'a> SourceProvider<'a> {
    pub(crate) fn new(
        import_resolver: &'a ImportResolver,
        recorder: &'a mut SourceRecorder,
    ) -> Self {
        Self {
            import_resolver,
            recorder,
            import_texts: HashMap::new(),
        }
    }
}

impl CompilationBuilderConfig for SourceProvider<'_> {
    /// Keep `fs_permissions` out of this path. The file ids come from the
    /// paths the test runner was configured with, never from paths a test
    /// controls, so this reads a project source exactly as the compiler does.
    ///
    /// Slang reads every file at most once, so a root's text is retained here
    /// for whoever needs its comments. An import already read by
    /// [`Self::resolve_import`] is served from there instead of from disk.
    fn read_file(&mut self, file_id: &str) -> Result<String, String> {
        let read = match self.import_texts.get_mut(file_id).and_then(Option::take) {
            Some(text) => Ok(text),
            None => std::fs::read_to_string(Path::new(file_id)),
        };

        let Some(root) = self.recorder.roots.get_mut(file_id) else {
            return read.map_err(|error| error.to_string());
        };

        match read {
            Ok(content) => {
                *root = RootRead::Read(content.clone());

                Ok(content)
            }
            Err(error) => {
                let reason = error.to_string();
                *root = RootRead::Unreadable(error);

                Err(reason)
            }
        }
    }

    /// Reports an import whose file cannot be read as unresolved, because
    /// Slang's binder panics when a contract inherits from an import it
    /// resolved but could not read. An unresolved import only yields a
    /// diagnostic.
    ///
    /// Reading it here is what proves it readable, so the text is retained for
    /// the [`Self::read_file`] that follows rather than read twice.
    fn resolve_import(
        &mut self,
        source_file_id: &str,
        import_path: &str,
    ) -> Result<String, String> {
        let resolved = self
            .import_resolver
            .resolve_import(source_file_id, import_path)?;

        if !self.import_texts.contains_key(&resolved) {
            let text = std::fs::read_to_string(Path::new(&resolved)).map_err(|error| {
                format!(
                    "import '{import_path}' resolves to '{resolved}', which could not be read: \
                     {error}"
                )
            })?;

            self.import_texts.insert(resolved.clone(), Some(text));
        }

        self.recorder
            .imports
            .entry(source_file_id.to_owned())
            .or_default()
            .push(resolved.clone());

        Ok(resolved)
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
    fn assert_resolves(result: Result<String, String>, expected: &Path) {
        assert_eq!(result.map(PathBuf::from), Ok(expected.to_path_buf()));
    }

    /// An absolute directory that exists on every platform, so file ids are
    /// not prefixed with a drive on Windows. No file under it is read.
    fn project_dir() -> PathBuf {
        std::env::temp_dir().join("project")
    }

    #[test]
    fn resolves_relative_imports_against_the_importer() {
        let resolver = ImportResolver::default();
        let contracts = project_dir().join("contracts");
        let a = contracts.join("A.sol");
        let b = contracts.join("lib").join("B.sol");

        assert_resolves(
            resolver.resolve_import(&a.to_string_lossy(), "./lib/B.sol"),
            &b,
        );
        assert_resolves(
            resolver.resolve_import(&b.to_string_lossy(), "../A.sol"),
            &a,
        );
    }

    #[test]
    fn resolves_mapped_imports() {
        let import_path = "@oz/contracts/token/ERC20.sol";
        let mapped = std::env::temp_dir().join("deps").join(import_path);
        let resolver = ImportResolver::new([(import_path.to_owned(), mapped.clone())].into());

        assert_resolves(
            resolver.resolve_import(&project_dir().join("A.sol").to_string_lossy(), import_path),
            &mapped,
        );
    }

    #[test]
    fn provider_reports_an_unreadable_file_as_unresolved() {
        let dir = tempfile::tempdir().expect("temp dir");
        // Invalid UTF-8: the file exists, but its text cannot be read.
        std::fs::write(dir.path().join("Dep.sol"), [0xff, 0xfe]).expect("write dependency");

        let root_id = dir.path().join("Root.sol").to_string_lossy().into_owned();

        let resolver = ImportResolver::default();
        let mut recorder = SourceRecorder::expecting([root_id.clone()]);
        let mut provider = SourceProvider::new(&resolver, &mut recorder);

        assert!(provider.resolve_import(&root_id, "./Dep.sol").is_err());
    }

    /// Deleting the file between the calls is what proves no further read
    /// happened: another one would fail.
    #[test]
    fn provider_reads_an_import_once_however_many_files_import_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("Dep.sol");
        std::fs::write(&dep, "on disk").expect("write dependency");

        let first_id = dir.path().join("First.sol").to_string_lossy().into_owned();
        let second_id = dir.path().join("Second.sol").to_string_lossy().into_owned();
        let dep_id = dep.to_string_lossy().into_owned();

        let resolver = ImportResolver::default();
        let mut recorder = SourceRecorder::expecting([]);
        let mut provider = SourceProvider::new(&resolver, &mut recorder);

        assert_resolves(provider.resolve_import(&first_id, "./Dep.sol"), &dep);
        std::fs::remove_file(&dep).expect("remove dependency");

        assert_resolves(provider.resolve_import(&second_id, "./Dep.sol"), &dep);
        assert_eq!(provider.read_file(&dep_id), Ok("on disk".to_owned()));
    }

    #[test]
    fn provider_reports_a_missing_file_as_unresolved() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("Dep.sol"), "").expect("write dependency");

        let root_path = dir.path().join("Root.sol");
        std::fs::write(&root_path, "content").expect("write root");

        let root_id = root_path.to_string_lossy().into_owned();

        let resolver = ImportResolver::default();
        let mut recorder = SourceRecorder::expecting([root_id.clone()]);
        let mut provider = SourceProvider::new(&resolver, &mut recorder);

        assert!(provider.resolve_import(&root_id, "./Dep.sol").is_ok());
        assert!(provider.resolve_import(&root_id, "./Missing.sol").is_err());
    }

    #[test]
    fn provider_retains_root_texts_and_records_read_errors() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("Dep.sol");
        std::fs::write(&dep, "on disk").expect("write dependency");
        let root_path = dir.path().join("Root.sol");
        std::fs::write(&root_path, "root text").expect("write root");
        let root_id = root_path.to_string_lossy().into_owned();
        let missing_id = dir
            .path()
            .join("Missing.sol")
            .to_string_lossy()
            .into_owned();

        let resolver = ImportResolver::default();
        let mut recorder = SourceRecorder::expecting([root_id.clone(), missing_id.clone()]);
        let mut provider = SourceProvider::new(&resolver, &mut recorder);

        assert_eq!(provider.read_file(&root_id), Ok("root text".to_owned()));
        assert_eq!(
            provider.read_file(&dep.to_string_lossy()),
            Ok("on disk".to_owned())
        );
        assert!(provider.read_file(&missing_id).is_err());

        let sources = recorder.finish().expect("every expected root was read");
        assert_eq!(
            sources.root_contents(),
            &HashMap::from([(root_id, "root text".to_owned())])
        );
        assert_eq!(
            sources
                .unreadable_roots()
                .get(&missing_id)
                .expect("the read failed")
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }

    #[test]
    fn finishing_with_unread_roots_reports_them_sorted() {
        let recorder = SourceRecorder::expecting(["b.sol".to_owned(), "a.sol".to_owned()]);

        let pending = recorder.finish().expect_err("the roots were never read");
        assert_eq!(pending.0, vec!["a.sol".to_owned(), "b.sol".to_owned()]);
        assert_eq!(pending.to_string(), "roots never read: a.sol, b.sol");
    }

    #[test]
    fn reachability_follows_resolved_imports_transitively() {
        let mut recorder = SourceRecorder::expecting([]);
        recorder
            .imports
            .insert("a.sol".to_owned(), vec!["b.sol".to_owned()]);
        recorder.imports.insert(
            "b.sol".to_owned(),
            vec!["c.sol".to_owned(), "a.sol".to_owned()],
        );
        recorder
            .imports
            .insert("d.sol".to_owned(), vec!["c.sol".to_owned()]);
        let sources = recorder.finish().expect("no roots were expected");

        assert_eq!(
            sources.reachable_from("a.sol"),
            HashSet::from(["a.sol", "b.sol", "c.sol"])
        );
        assert_eq!(
            sources.reachable_from("d.sol"),
            HashSet::from(["d.sol", "c.sol"])
        );
        assert_eq!(
            sources.reachable_from("lonely.sol"),
            HashSet::from(["lonely.sol"])
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
