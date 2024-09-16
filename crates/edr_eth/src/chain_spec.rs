use alloy_rlp::RlpEncodable;
pub use revm_primitives::{ChainSpec, EvmWiring, HaltReasonTrait, HardforkTrait};

use crate::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    transaction,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type ChainContext = ();
    type Block = revm_primitives::BlockEnv;
    type Transaction = transaction::Signed;
    type Hardfork = revm_primitives::SpecId;
    type HaltReason = revm_primitives::HaltReason;
}

/// Constants for constructing Ethereum headers.
pub trait EthHeaderConstants: ChainSpec<Hardfork: 'static + PartialOrd> {
    /// Parameters for the EIP-1559 base fee calculation.
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork>;

    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;
}

impl EthHeaderConstants for L1ChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}
