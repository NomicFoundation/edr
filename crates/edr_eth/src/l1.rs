use alloy_rlp::RlpEncodable;
pub use revm_context::{BlockEnv, TxEnv};
pub use revm_context_interface::result::{
    HaltReason, InvalidHeader, InvalidTransaction, OutOfGasError,
};
pub use revm_primitives::hardfork::{self, SpecId};

use crate::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    spec::{ChainHardfork, ChainSpec, EthHeaderConstants},
    transaction,
};

/// L1 Ethereum chain type
pub const CHAIN_TYPE: &str = "L1";
const _BASE_FEE_PARAMS: BaseFeeParams<SpecId> =
    BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainHardfork for L1ChainSpec {
    type Hardfork = SpecId;
}

impl ChainSpec for L1ChainSpec {
    type BlockEnv = BlockEnv;
    type Context = ();
    type HaltReason = HaltReason;
    type SignedTransaction = transaction::Signed;
}

impl EthHeaderConstants for L1ChainSpec {
    fn base_fee_params() -> &'static BaseFeeParams<Self::Hardfork> {
        &_BASE_FEE_PARAMS
    }

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}
