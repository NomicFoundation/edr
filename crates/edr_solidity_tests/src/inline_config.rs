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
//! The work flows through the submodules as a pipeline:
//!
//! ```text
//!   - parse      locate contract/function definitions via Slang
//!   - natspec    scan the NatSpec comment blocks above each definition
//!   - directives parse a block's lines into a config
//!   - overrides  compose the above into a source's per-contract overrides
//!   - provider   cache the overrides and serve them
//! ```

mod directives;
mod error;
mod natspec;
mod overrides;
mod parse;
mod provider;
mod resolver;

pub use self::{
    directives::is_test_function,
    error::{
        InlineConfigCollectError, InlineConfigError, InlineConfigErrorItem, InlineConfigErrors,
        InlineConfigProblem,
    },
    overrides::{ContractInlineConfig, FunctionOverride},
    provider::{CachedInlineConfigProvider, InlineConfigRoot, SharedInlineConfigProvider},
    resolver::ImportResolver,
};
