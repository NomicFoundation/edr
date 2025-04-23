mod builder;

use edr_evm::EthLocalBlockForChainSpec;

pub use self::builder::Builder;
use crate::OpChainSpec;

/// Local block type for OP.
pub type LocalBlock = EthLocalBlockForChainSpec<OpChainSpec>;
