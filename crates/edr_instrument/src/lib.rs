use semver::Version;
use slang_solidity::utils::LanguageFacts;

/// Types for instrumenting code for the purpose of code coverage.
pub mod coverage;

/// The latest version of `Solidity` supported by `edr_instrument`.
pub const LATEST_SUPPORTED_SOLIDITY_VERSION: Version = LanguageFacts::LATEST_VERSION;
