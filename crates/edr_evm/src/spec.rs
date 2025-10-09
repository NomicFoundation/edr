use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use alloy_eips::eip7840::BlobParams;
use edr_block_api::{Block, BlockReceipts, EmptyBlock, LocalBlock};
use edr_block_header::{
    calculate_next_base_fee_per_gas, BlobGas, BlockConfig, BlockHeader, PartialHeader,
};
use edr_chain_l1::L1ChainSpec;
use edr_chain_spec::{
    BlobExcessGasAndPrice, ChainHardfork, ChainSpec, EvmSpecId, EvmTransactionValidationError,
    ExecutableTransaction, TransactionValidation,
};
use edr_database_components::DatabaseComponentError;
use edr_eip1559::BaseFeeParams;
use edr_primitives::{Bytes, B256, U256};
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    BlockReceipt, ExecutionReceipt, MapReceiptLogs, ReceiptFactory, ReceiptTrait,
};
use edr_rpc_spec::{RpcEthBlock, RpcSpec, RpcTypeFrom};
use edr_state_api::{EvmState, StateDiff};
use edr_transaction::TransactionType;
use edr_utils::types::TypeConstructor;
use revm::{inspector::NoOpInspector, ExecuteEvm, InspectEvm, Inspector};
use revm_context::{JournalTr as _, LocalContext};
pub use revm_context_interface::ContextTr as ContextTrait;
use revm_handler::{instructions::EthInstructions, EthFrame, PrecompileProvider};
use revm_interpreter::{interpreter::EthInterpreter, InterpreterResult};

use crate::{
    block::{transaction::TransactionAndBlockForChainSpec, LocalCreationError},
    config::CfgEnv,
    evm::Evm,
    hardfork::{self, ChainConfig},
    journal::Journal,
    precompile::EthPrecompiles,
    receipt::{self, ExecutionReceiptBuilder},
    result::{EVMErrorForChain, ExecutionResult},
    state::Database,
    transaction::{remote::RpcTransaction, TransactionError, TransactionErrorForChainSpec},
    BlockBuilder, EthBlockBuilder, EthBlockData, EthBlockReceiptFactory, EthLocalBlock,
    EthLocalBlockForChainSpec, GenesisBlockOptions, RemoteBlock, RemoteBlockConversionError,
    SyncBlock,
};

/// Returns the corresponding base fee params configured for the given chain ID.
/// If it's not defined in the defined chain specification it fallbacks to the
/// chain spec default.
pub fn base_fee_params_for<ChainSpecT: RuntimeSpec>(
    chain_id: u64,
) -> &'static BaseFeeParams<ChainSpecT::Hardfork> {
    ChainSpecT::chain_config(chain_id).map_or(ChainSpecT::default_base_fee_params(), |config| {
        &config.base_fee_params
    })
}

/// A supertrait for [`RuntimeSpec`] that is safe to send between threads.
pub trait SyncRuntimeSpec:
    RuntimeSpec<
        BlockReceipt: Send + Sync,
        ExecutionReceipt<FilterLog>: Send + Sync,
        HaltReason: Send + Sync,
        Hardfork: Send + Sync,
        LocalBlock: Send + Sync,
        RpcBlockConversionError: Send + Sync,
        RpcReceiptConversionError: Send + Sync,
        SignedTransaction: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
    > + Send
    + Sync
    + 'static
{
}

impl<ChainSpecT> SyncRuntimeSpec for ChainSpecT where
    ChainSpecT: RuntimeSpec<
            BlockReceipt: Send + Sync,
            ExecutionReceipt<FilterLog>: Send + Sync,
            HaltReason: Send + Sync,
            Hardfork: Send + Sync,
            LocalBlock: Send + Sync,
            RpcBlockConversionError: Send + Sync,
            RpcReceiptConversionError: Send + Sync,
            SignedTransaction: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
        > + Send
        + Sync
        + 'static
{
}

impl RuntimeSpec for L1ChainSpec {
    type Block = dyn SyncBlock<
        Arc<Self::BlockReceipt>,
        Self::SignedTransaction,
        Error = <Self::LocalBlock as BlockReceipts<Arc<Self::BlockReceipt>>>::Error,
    >;

    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + Send + std::error::Error,
        StateErrorT: 'builder + Send + std::error::Error,
    > = EthBlockBuilder<'builder, BlockchainErrorT, Self, StateErrorT>;

    type BlockReceipt = BlockReceipt<Self::ExecutionReceipt<FilterLog>>;
    type BlockReceiptFactory = EthBlockReceiptFactory<Self::ExecutionReceipt<FilterLog>>;

    type LocalBlock = EthLocalBlock<
        Self::RpcBlockConversionError,
        Self::BlockReceipt,
        ExecutionReceiptTypeConstructorForChainSpec<Self>,
        Self::Hardfork,
        Self::RpcReceiptConversionError,
        Self::SignedTransaction,
    >;

    type ReceiptBuilder = receipt::Builder;
    type RpcBlockConversionError = RemoteBlockConversionError<Self::RpcTransactionConversionError>;
    type RpcReceiptConversionError = edr_chain_l1::rpc::receipt::ConversionError;
    type RpcTransactionConversionError = edr_chain_l1::rpc::transaction::ConversionError;

    fn cast_local_block(local_block: Arc<Self::LocalBlock>) -> Arc<Self::Block> {
        local_block
    }

    fn cast_remote_block(remote_block: Arc<RemoteBlock<Self>>) -> Arc<Self::Block> {
        remote_block
    }

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::SignedTransaction as TransactionValidation>::ValidationError,
    ) -> TransactionErrorForChainSpec<BlockchainErrorT, Self, StateErrorT> {
        match error {
            EvmTransactionValidationError::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn next_base_fee_per_gas(
        header: &BlockHeader,
        chain_id: u64,
        hardfork: Self::Hardfork,
        base_fee_params_overrides: Option<&BaseFeeParams<Self::Hardfork>>,
    ) -> u128 {
        calculate_next_base_fee_per_gas(
            header,
            base_fee_params_overrides.unwrap_or(base_fee_params_for::<Self>(chain_id)),
            hardfork,
        )
    }

    fn chain_config(chain_id: u64) -> Option<&'static ChainConfig<Self::Hardfork>> {
        hardfork::l1::chain_config(chain_id)
    }

    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork> {
        hardfork::l1::default_base_fee_params()
    }
}

impl BlockEnvConstructor<PartialHeader> for L1ChainSpec {
    fn new_block_env(header: &PartialHeader, hardfork: EvmSpecId) -> Self::BlockEnv {
        edr_chain_l1::BlockEnv {
            number: U256::from(header.number),
            beneficiary: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= EvmSpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    let blob_params = if hardfork >= EvmSpecId::PRAGUE {
                        BlobParams::prague()
                    } else {
                        BlobParams::cancun()
                    };

                    BlobExcessGasAndPrice::new(
                        *excess_gas,
                        blob_params
                            .update_fraction
                            .try_into()
                            .expect("blob update fraction is too large"),
                    )
                },
            ),
        }
    }
}

impl BlockEnvConstructor<BlockHeader> for L1ChainSpec {
    fn new_block_env(header: &BlockHeader, hardfork: EvmSpecId) -> Self::BlockEnv {
        edr_chain_l1::BlockEnv {
            number: U256::from(header.number),
            beneficiary: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee_per_gas.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= EvmSpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    let blob_params = if hardfork >= EvmSpecId::PRAGUE {
                        BlobParams::prague()
                    } else {
                        BlobParams::cancun()
                    };

                    BlobExcessGasAndPrice::new(
                        *excess_gas,
                        blob_params
                            .update_fraction
                            .try_into()
                            .expect("blob update fraction is too large"),
                    )
                },
            ),
        }
    }
}

#[cfg(test)]
mod l1_chain_spec_tests {
    use edr_block_header::{BlobGas, BlockHeader};
    use edr_primitives::{Address, Bloom, Bytes, B256, B64, U256};

    use crate::spec::{BlockEnvConstructor as _, L1ChainSpec};

    fn build_block_header(blob_gas: Option<BlobGas>) -> BlockHeader {
        BlockHeader {
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
    fn l1_block_constructor_should_not_default_excess_blob_gas_for_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::CANCUN);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn l1_block_constructor_should_not_default_excess_blob_gas_before_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::SHANGHAI);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn l1_block_constructor_should_not_default_excess_blob_gas_after_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::PRAGUE);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn l1_block_constructor_should_use_existing_excess_blob_gas() {
        let excess_gas = 0x80000u64;
        let blob_gas = BlobGas {
            excess_gas,
            gas_used: 0x80000u64,
        };
        let header = build_block_header(Some(blob_gas)); // blob gas present

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::CANCUN);

        let blob_excess_gas = block
            .blob_excess_gas_and_price
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }
}
