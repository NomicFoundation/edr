mod builder;

pub use builder::decode_base_params;
use edr_block_local::EthLocalBlock;
use edr_chain_spec::ChainSpec;
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_receipt::ReceiptChainSpec;

pub use self::builder::OpBlockBuilder;
use crate::{Hardfork, OpChainSpec};

/// Local block type for OP.
pub type LocalBlock = EthLocalBlock<
    <OpChainSpec as ReceiptChainSpec>::Receipt,
    <OpChainSpec as BlockChainSpec>::FetchReceiptError,
    Hardfork,
    <OpChainSpec as ChainSpec>::SignedTransaction,
>;
