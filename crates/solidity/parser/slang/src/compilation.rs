//! Building Slang compilation units over on-disk Solidity sources.

use std::path::Path;

use edr_common::fs::normalize_path;
use semver::Version;
use slang_solidity_v2::compilation::{CompilationBuilder, CompilationUnit};

use super::LanguageVersion;
use crate::resolver::{ImportResolver, ProvidedSources, SourceProvider, SourceRecorder};

/// Maps a solc version to the Slang grammar that parses it, or `None` when it
/// predates the oldest grammar Slang ships.
///
/// A version newer than the newest grammar clamps down to it, and build and
/// pre-release metadata is ignored: a nightly parses as its release.
pub fn language_version_for_solc(solc_version: &Version) -> Option<LanguageVersion> {
    let release = Version::new(solc_version.major, solc_version.minor, solc_version.patch);
    let latest: Version = LanguageVersion::LATEST.into();
    if release > latest {
        return Some(LanguageVersion::LATEST);
    }

    LanguageVersion::try_from(release).ok()
}

/// The file id a compilation unit holds a file under, which is its absolute,
/// normalized path.
///
/// Imports resolve to such ids, so a root spelled relatively or with `.` and
/// `..` segments still shares the file an import reaches. Normalization is
/// lexical, so a `..` across a symlinked directory is not resolved the way the
/// OS would.
pub fn file_id_of(path: &Path) -> String {
    // Normalizing is lexical and drops a leading `..`, so a path that cannot
    // be made absolute is kept as spelled.
    match std::path::absolute(path) {
        Ok(absolute) => normalize_path(&absolute).to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// A compilation unit built over one or more root files, together with what
/// the provider recorded while building it.
pub struct MultiRootCompilation {
    /// The unit over the roots and whatever of their import closures resolved.
    pub unit: CompilationUnit,
    /// The roots' texts and the resolved import graph. The unit retains
    /// neither comments nor import edges, so NatSpec directives and per-root
    /// reachability have to come from here.
    pub sources: ProvidedSources,
}

/// Builds one Slang compilation unit over every path in `root_paths`, resolving
/// their imports via `import_resolver` and reading every file from disk.
///
/// Roots sharing an import parse and analyse it once. The unit does not
/// separate the roots' name resolution, so a caller scopes a query to one root
/// through [`ProvidedSources`]'s import graph.
///
/// An unreadable root, a file that does not parse and an unresolvable import
/// all surface as diagnostics on the unit. Building therefore never fails, and
/// whatever still resolves is available. Why a root could not be read is also
/// recorded on [`ProvidedSources`].
pub fn build_multi_root_compilation_unit<'path>(
    root_paths: impl IntoIterator<Item = &'path Path>,
    language_version: LanguageVersion,
    import_resolver: &ImportResolver,
) -> MultiRootCompilation {
    let root_ids: Vec<String> = root_paths.into_iter().map(file_id_of).collect();

    let mut recorder = SourceRecorder::expecting(root_ids.iter().cloned());
    let unit = {
        let provider = SourceProvider::new(import_resolver, &mut recorder);
        let mut builder = CompilationBuilder::create(language_version, provider);
        for root_id in root_ids {
            builder.add_file(root_id);
        }

        builder.build()
    };

    let sources = recorder
        .finish()
        .expect("Slang reads every file added to the builder");

    MultiRootCompilation { unit, sources }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use slang_solidity_v2::diagnostics::{
        kinds::compilation::CompilationDiagnosticKind, DiagnosticKind,
    };

    use super::*;

    #[test]
    fn inheriting_from_a_missing_import_reports_it_as_unresolved() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("Root.sol");
        std::fs::write(
            &root,
            "pragma solidity ^0.8.0;
import {Base} from \"./Missing.sol\";
contract Derived is Base {}
",
        )
        .expect("write root");

        let unit = build_multi_root_compilation_unit(
            [root.as_path()],
            LanguageVersion::LATEST,
            &ImportResolver::default(),
        )
        .unit;

        assert!(
            unit.diagnostics().iter().any(|diagnostic| matches!(
                diagnostic.kind(),
                DiagnosticKind::Compilation(CompilationDiagnosticKind::UnresolvedImport(_))
            )),
            "{:?}",
            unit.diagnostics()
        );
    }

    #[test]
    fn retains_the_root_text_it_parsed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("Root.sol");
        let source =
            "pragma solidity ^0.8.0;\n/// forge-config: default.fuzz.runs = 1\ncontract C {}\n";
        std::fs::write(&root, source).expect("write root");

        let MultiRootCompilation { unit, sources } = build_multi_root_compilation_unit(
            [root.as_path()],
            LanguageVersion::LATEST,
            &ImportResolver::default(),
        );

        let root_id = file_id_of(&root);
        assert_eq!(
            sources.root_contents().get(&root_id).map(String::as_str),
            Some(source)
        );
        assert!(unit.file(&root_id).is_some());
    }

    #[test]
    fn records_why_a_missing_root_could_not_be_read() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("Missing.sol");

        let MultiRootCompilation { sources, .. } = build_multi_root_compilation_unit(
            [root.as_path()],
            LanguageVersion::LATEST,
            &ImportResolver::default(),
        );

        let error = sources
            .unreadable_roots()
            .get(&file_id_of(&root))
            .expect("an unreadable root is recorded");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn a_relative_root_keeps_its_leading_parent_segment() {
        let current_dir = std::env::current_dir().expect("current dir");
        let sibling = current_dir
            .parent()
            .expect("the current dir has a parent")
            .join("Sibling.sol");

        assert_eq!(
            file_id_of(Path::new("../Sibling.sol")),
            sibling.to_string_lossy()
        );
    }

    #[test]
    fn a_root_spelled_with_dot_segments_shares_the_file_an_import_reaches() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().join("Base.sol");
        std::fs::write(&base, "pragma solidity ^0.8.0;\ncontract Base {}\n").expect("write base");
        let importer = dir.path().join("Importer.sol");
        std::fs::write(
            &importer,
            "pragma solidity ^0.8.0;\nimport \"./Base.sol\";\ncontract Importer is Base {}\n",
        )
        .expect("write importer");
        let base_with_dots = dir.path().join(".").join("sub").join("..").join("Base.sol");

        let MultiRootCompilation { unit, .. } = build_multi_root_compilation_unit(
            [importer.as_path(), base_with_dots.as_path()],
            LanguageVersion::LATEST,
            &ImportResolver::default(),
        );

        assert_eq!(unit.file_ids().len(), 2, "{:?}", unit.file_ids());
    }

    #[test]
    fn multi_root_unit_records_each_roots_text_closure_and_read_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let shared = dir.path().join("Shared.sol");
        std::fs::write(
            &shared,
            "pragma solidity ^0.8.0;\nstruct Shared { uint256 a; }\n",
        )
        .expect("write shared");
        let only_first = dir.path().join("OnlyFirst.sol");
        std::fs::write(
            &only_first,
            "pragma solidity ^0.8.0;\nstruct OnlyFirst { uint256 a; }\n",
        )
        .expect("write dependency");

        let first = dir.path().join("First.sol");
        std::fs::write(
            &first,
            "pragma solidity ^0.8.0;\nimport \"./Shared.sol\";\nimport \"./OnlyFirst.sol\";\ncontract First {}\n",
        )
        .expect("write first root");
        let second = dir.path().join("Second.sol");
        std::fs::write(
            &second,
            "pragma solidity ^0.8.0;\nimport \"./Shared.sol\";\ncontract Second {}\n",
        )
        .expect("write second root");
        let missing = dir.path().join("Missing.sol");

        let MultiRootCompilation { unit, sources } = build_multi_root_compilation_unit(
            [first.as_path(), second.as_path(), missing.as_path()],
            LanguageVersion::LATEST,
            &ImportResolver::default(),
        );

        let first_id = file_id_of(&first);
        let second_id = file_id_of(&second);
        let shared_id = file_id_of(&shared);
        let only_first_id = file_id_of(&only_first);

        assert!(unit.file(&first_id).is_some());
        assert!(unit.file(&second_id).is_some());

        assert!(sources.root_contents()[&first_id].contains("First"));
        assert!(sources.root_contents()[&second_id].contains("Second"));

        assert_eq!(
            sources.reachable_from(&first_id),
            HashSet::from([
                first_id.as_str(),
                shared_id.as_str(),
                only_first_id.as_str()
            ])
        );
        assert_eq!(
            sources.reachable_from(&second_id),
            HashSet::from([second_id.as_str(), shared_id.as_str()])
        );

        let error = sources
            .unreadable_roots()
            .get(&file_id_of(&missing))
            .expect("an unreadable root is recorded");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn exact_supported_version() {
        assert_eq!(
            language_version_for_solc(&Version::new(0, 8, 24)),
            Some(LanguageVersion::V0_8_24)
        );
    }

    #[test]
    fn clamps_newer_versions_to_latest() {
        assert_eq!(
            language_version_for_solc(&Version::new(0, 9, 0)),
            Some(LanguageVersion::LATEST)
        );
    }

    #[test]
    fn versions_older_than_0_8_0_have_no_grammar() {
        assert_eq!(language_version_for_solc(&Version::new(0, 7, 6)), None);
    }

    #[test]
    fn ignores_build_and_prerelease_metadata() {
        let version =
            Version::parse("0.8.24-nightly.2024.1.1+commit.abcdef").expect("valid semver");

        assert_eq!(
            language_version_for_solc(&version),
            Some(LanguageVersion::V0_8_24)
        );
    }
}
