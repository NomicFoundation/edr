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
//! ```
//!
//! The test runner drives extraction through
//! `crate::test_sources::collect_test_sources`, which parses each test
//! source once and extracts both its inline configuration (entering the
//! pipeline at `overrides`) and its EIP-712 struct definitions from the same
//! compilation unit.

mod directives;
pub mod error;
mod natspec;
mod overrides;
mod parse;

pub use edr_solidity_parser_slang::ImportResolver;

pub use self::overrides::{ContractInlineConfig, FunctionOverride};
pub(crate) use self::{
    directives::is_test_function,
    overrides::{collect_source_overrides_from_unit, SourceOverrides},
};
