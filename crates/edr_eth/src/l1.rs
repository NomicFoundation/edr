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
    fn new_block_env(header: &PartialHeader, hardfork: SpecId) -> BlockEnv {
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
    fn new_block_env(header: &Header, hardfork: SpecId) -> BlockEnv {
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

#[cfg(test)]
mod tests {
    use revm_primitives::hardfork::SpecId;

    use crate::{
        block::{BlobGas, Header},
        l1::L1BlockConstructor,
        spec::BlockEnvConstructor,
        Address, Bloom, Bytes, B256, B64, U256,
    };

    fn build_block_header(blob_gas: Option<BlobGas>) -> Header {
        Header {
            parent_hash: B256::default(),
            ommers_hash: B256::default(),
            beneficiary: Address::default(),
            state_root: B256::default(),
            transactions_root: B256::default(),
            receipts_root: B256::default(),
            logs_bloom: Bloom::default(),
            difficulty: U256::default(),
            number: 124,
            gas_limit: u64::default(),
            gas_used: 1337,
            timestamp: 0,
            extra_data: Bytes::default(),
            mix_hash: B256::default(),
            nonce: B64::from(99u64),
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas,
            parent_beacon_block_root: None,
            requests_hash: Some(B256::random()),
        }
    }

    #[test]
    fn generic_block_constructor_should_not_default_excess_blob_gas_for_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1BlockConstructor::new_block_env(&header, SpecId::CANCUN);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn generic_block_constructor_should_not_default_excess_blob_gas_below_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1BlockConstructor::new_block_env(&header, SpecId::SHANGHAI);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn generic_block_constructor_should_not_default_excess_blob_gas_above_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1BlockConstructor::new_block_env(&header, SpecId::PRAGUE);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn generic_block_constructor_should_use_existing_excess_blob_gas() {
        let excess_gas = 0x80000u64;
        let blob_gas = BlobGas {
            excess_gas,
            gas_used: 0x80000u64,
        };
        let header = build_block_header(Some(blob_gas)); // blob gas present

        let block = L1BlockConstructor::new_block_env(&header, SpecId::CANCUN);

        let blob_excess_gas = block
            .blob_excess_gas_and_price
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }
}
