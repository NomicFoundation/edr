use alloy_rlp::RlpEncodable;
pub use revm_context::{BlockEnv, TxEnv};
pub use revm_context_interface::result::{
    HaltReason, InvalidHeader, InvalidTransaction, OutOfGasError,
};
pub use revm_primitives::hardfork::{self, SpecId};

use crate::{
    block::{BlobGas, Header, PartialHeader},
    eips::{
        eip1559::{BaseFeeParams, ConstantBaseFeeParams},
        eip4844,
    },
    spec::{BlockEnvConstructor, ChainHardfork, ChainSpec, EthHeaderConstants},
    transaction,
};

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
    type BlockConstructor = L1BlockConstructor;
}

impl EthHeaderConstants for L1ChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}

/// L1 definition for `BlockEnvConstructor`
pub struct L1BlockConstructor;

impl BlockEnvConstructor<PartialHeader, BlockEnv> for L1BlockConstructor {
    fn build_from_header(header: &PartialHeader, hardfork: SpecId) -> BlockEnv {
        BlockEnv {
            number: header.number,
            beneficiary: header.beneficiary,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            basefee: header.base_fee.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= SpecId::PRAGUE)
                },
            ),
        }
    }
}

impl BlockEnvConstructor<Header, BlockEnv> for L1BlockConstructor {
    fn build_from_header(header: &Header, hardfork: SpecId) -> BlockEnv {
        BlockEnv {
            number: header.number,
            beneficiary: header.beneficiary,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            basefee: header.base_fee_per_gas.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= SpecId::PRAGUE)
                },
            ),
        }
    }
}
