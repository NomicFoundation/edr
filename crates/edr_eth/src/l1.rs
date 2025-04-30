use alloy_rlp::RlpEncodable;
pub use revm_context::BlockEnv;
pub use revm_context_interface::result::{
    HaltReason, InvalidHeader, InvalidTransaction, OutOfGasError,
};
pub use revm_primitives::hardfork::{self, SpecId};

use crate::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    spec::{ChainSpec, EthHeaderConstants},
    transaction,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type BlockEnv = BlockEnv;
    type Context = ();
    type HaltReason = HaltReason;
    type Hardfork = SpecId;
    type SignedTransaction = transaction::Signed;
}

impl EthHeaderConstants for L1ChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = 131072;

    fn chain_base_fee_params(_chain_id: u64) -> BaseFeeParams<Self::Hardfork> {
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum())
    }
}
