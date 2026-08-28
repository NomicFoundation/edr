//! Parses inline configuration for Solidity tests from NatSpec comments.
//!
//! Solidity tests support per-test configuration written as NatSpec comments
//! above test functions, e.g.:
//!
//! ```solidity
//! /// forge-config: default.fuzz.runs = 100
//! function testFoo(uint256 x) public { /* ... */ }
//! ```
//!
//! Both the `forge-config:` and `hardhat-config:` prefixes are recognized.
//!
//! A directive may be scoped to a test profile by prefixing its key with the
//! profile's name. A prefixed directive applies only under that profile, an
//! unprefixed one under every profile, and where both set the same key the
//! prefixed one wins, whatever order they appear in. A prefix must name a
//! declared profile — see [`InlineConfigProfiles`].
//!
//! ```solidity
//! /// forge-config: fuzz.runs = 3       // under every profile
//! /// forge-config: ci.fuzz.runs = 500  // only under `ci`
//! function testFoo(uint256 x) public { /* ... */ }
//! ```
//!
//! The work flows through the submodules as a pipeline:
//!
//! ```text
//!   - parse      locate contract/function definitions via Slang
//!   - natspec    scan the NatSpec comment blocks above each function
//!   - directives parse a block's lines into a config
//!   - overrides  compose the above into a source's per-contract overrides
//!   - provider   cache the overrides and serve them
//! ```
//!
//! `profiles` carries the run's selected and declared profiles through that
//! pipeline, so `directives` can scope each one.

mod directives;
mod error;
mod natspec;
mod overrides;
mod parse;
mod profiles;
mod provider;
mod resolver;

use alloy_json_abi::JsonAbi;

pub use self::{
    error::{
        InlineConfigCollectError, InlineConfigError, InlineConfigErrorItem, InlineConfigErrors,
        InlineConfigProblem, InlineConfigProfilesError,
    },
    overrides::FunctionOverride,
    profiles::{InlineConfigProfiles, DEFAULT_PROFILE},
    provider::{CachedInlineConfigProvider, InlineConfigRoot, SharedInlineConfigProvider},
    resolver::ImportResolver,
};

/// Resolves the 4-byte selector (as a `0x`-prefixed hex string) of the first
/// function in `abi` named `function_name`. Returns `None` if no such function
/// exists.
///
/// The returned string matches the format used to build
/// `TestFunctionIdentifier` in the test runner (`func.selector().to_string()`).
pub fn resolve_selector(abi: &JsonAbi, function_name: &str) -> Option<String> {
    abi.functions()
        .find(|function| function.name == function_name)
        .map(|function| function.selector().to_string())
}

#[cfg(test)]
mod tests {
    use alloy_json_abi::JsonAbi;

    use super::*;

    #[test]
    fn resolve_selector_matches_abi() {
        let abi: JsonAbi = serde_json::from_str(
            r#"[{"type":"function","name":"testFoo","inputs":[],"outputs":[],"stateMutability":"nonpayable"}]"#,
        )
        .unwrap();
        let selector = resolve_selector(&abi, "testFoo").expect("selector");
        assert!(selector.starts_with("0x"));
        assert_eq!(selector.len(), 10);
        assert_eq!(resolve_selector(&abi, "missing"), None);
    }
}
