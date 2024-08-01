use alloy_rlp::RlpEncodable;
pub use revm_primitives::EvmWiring;

use crate::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    transaction,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl EvmWiring for L1ChainSpec {
    type Block = revm_primitives::BlockEnv;

    type Hardfork = revm_primitives::SpecId;

    type HaltReason = revm_primitives::HaltReason;

    type Transaction = transaction::Signed;
}

impl revm::EvmWiring for L1ChainSpec {
    type Context = ();

    fn handler<'evm, EXT, DB>(hardfork: Self::Hardfork) -> revm::EvmHandler<'evm, Self, EXT, DB>
    where
        DB: revm::Database,
    {
        revm::EvmHandler::mainnet_with_spec(hardfork)
    }
}

/// Constants for constructing Ethereum headers.
pub trait EthHeaderConstants: revm_primitives::EvmWiring<Hardfork: PartialOrd> {
    /// Parameters for the EIP-1559 base fee calculation.
    const BASE_FEE_PARAMS: BaseFeeParams<Self>;

    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;
}

impl EthHeaderConstants for L1ChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}
