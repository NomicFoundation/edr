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
//! A directive above a contract definition applies to every test the contract
//! runs (including inherited ones), with function-level directives taking
//! per-key precedence:
//!
//! ```solidity
//! /// forge-config: default.fuzz.runs = 50
//! contract MyTest is Test { /* ... */ }
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
//!   - natspec    scan the NatSpec comment blocks above each definition
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

pub(crate) use self::directives::is_test_function;
pub use self::{
    error::{
        InlineConfigCollectError, InlineConfigError, InlineConfigErrorItem, InlineConfigErrors,
        InlineConfigProblem, InlineConfigProfilesError,
    },
    overrides::{ContractInlineConfig, FunctionOverride},
    profiles::{InlineConfigProfiles, DEFAULT_PROFILE},
    provider::{CachedInlineConfigProvider, InlineConfigRoot, SharedInlineConfigProvider},
    resolver::ImportResolver,
};
