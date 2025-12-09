use std::sync::Arc;

use edr_block_api::{sync::SyncBlock, GenesisBlockFactory, GenesisBlockOptions};
use edr_block_header::{blob_params_for_hardfork, BlockConfig, BlockHeader, HeaderAndEvmSpec};
use edr_block_local::EthLocalBlock;
use edr_block_remote::FetchRemoteReceiptError;
use edr_chain_config::ChainConfig;
use edr_chain_l1::{
    block::EthBlockBuilder,
    receipt::L1BlockReceipt,
    rpc::{call::L1CallRequest, transaction::L1RpcTransactionRequest},
    L1ChainSpec, L1_GENESIS_BLOCK_EXTRA_DATA,
};
use edr_chain_spec::{
    BlobExcessGasAndPrice, BlockEnvChainSpec, BlockEnvConstructor, BlockEnvForHardfork,
    BlockEnvTrait, ChainSpec, ContextChainSpec, EvmSpecId, HardforkChainSpec,
    TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{
    handler::EthInstructions, CfgEnv, Context, ContextForChainSpec, Database, Evm, EvmChainSpec,
    ExecuteEvm as _, ExecutionResultAndState, InspectEvm as _, Inspector, InterpreterResult,
    Journal, JournalTrait as _, LocalContext, PrecompileProvider, TransactionError,
};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_chain_spec_receipt::ReceiptChainSpec;
use edr_chain_spec_rpc::{RpcBlockChainSpec, RpcChainSpec};
use edr_eip1559::BaseFeeParams;
use edr_eip7892::ScheduledBlobParams;
use edr_primitives::{Address, Bytes, HashMap, B256, U256};
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, TransactionFailureReason};
use edr_receipt::{log::FilterLog, ExecutionReceiptChainSpec};
use edr_state_api::StateDiff;

use crate::{
    eip2718::TypedEnvelope,
    receipt::GenericExecutionReceiptBuilder,
    rpc::{
        block::GenericRpcBlock, receipt::GenericRpcTransactionReceipt,
        transaction::GenericRpcTransactionWithSignature,
    },
    GenericChainSpec,
};

pub struct HeaderAndEvmSpecWithFallback<'header, BlockHeaderT: BlockEnvForHardfork<EvmSpecId>> {
    inner: HeaderAndEvmSpec<'header, BlockHeaderT, EvmSpecId>,
}

impl<'header, BlockHeaderT: BlockEnvForHardfork<EvmSpecId>>
    BlockEnvConstructor<EvmSpecId, &'header BlockHeaderT>
    for HeaderAndEvmSpecWithFallback<'header, BlockHeaderT>
{
    fn new_block_env(
        header: &'header BlockHeaderT,
        hardfork: EvmSpecId,
        scheduled_blob_params: Option<ScheduledBlobParams>,
    ) -> Self {
        Self {
            inner: HeaderAndEvmSpec::new_block_env(header, hardfork, scheduled_blob_params),
        }
    }
}

impl<'header, BlockHeaderT: BlockEnvForHardfork<EvmSpecId>> BlockEnvTrait
    for HeaderAndEvmSpecWithFallback<'header, BlockHeaderT>
{
    fn number(&self) -> U256 {
        self.inner.number()
    }

    fn beneficiary(&self) -> Address {
        self.inner.beneficiary()
    }

    fn timestamp(&self) -> U256 {
        self.inner.timestamp()
    }

    fn gas_limit(&self) -> u64 {
        self.inner.gas_limit()
    }

    fn basefee(&self) -> u64 {
        self.inner.basefee()
    }

    fn difficulty(&self) -> U256 {
        self.inner.difficulty()
    }

    fn prevrandao(&self) -> Option<B256> {
        self.inner.prevrandao()
    }

    fn blob_excess_gas_and_price(&self) -> Option<BlobExcessGasAndPrice> {
        self.inner.blob_excess_gas_and_price().or_else(|| {
            // If the hardfork requires it, set ExcessGasAndPrice default value
            // see https://github.com/NomicFoundation/edr/issues/947
            if self.inner.hardfork >= edr_chain_l1::Hardfork::CANCUN {
                // FIXME: pass proper timestamp
                let blob_params = blob_params_for_hardfork(
                    self.inner.hardfork,
                    0,
                    self.inner.scheduled_blob_params.as_ref(),
                );

                let update_fraction = blob_params
                    .update_fraction
                    .try_into()
                    .expect("blob update fraction is too large");

                Some(BlobExcessGasAndPrice::new(0u64, update_fraction))
            } else {
                None
            }
        })
    }
}

impl BlockChainSpec for GenericChainSpec {
    type Block =
        dyn SyncBlock<Arc<Self::Receipt>, Self::SignedTransaction, Error = Self::FetchReceiptError>;

    type BlockBuilder<'builder, BlockchainErrorT: 'builder + std::error::Error> = EthBlockBuilder<
        'builder,
        Self::Receipt,
        Self::Block,
        BlockchainErrorT,
        Self,
        Self::ExecutionReceiptBuilder,
        Self,
        Self::LocalBlock,
    >;

    type FetchReceiptError =
        FetchRemoteReceiptError<<Self::Receipt as TryFrom<Self::RpcReceipt>>::Error>;
}

impl BlockEnvChainSpec for GenericChainSpec {
    type BlockEnv<'header, BlockHeaderT>
        = HeaderAndEvmSpecWithFallback<'header, BlockHeaderT>
    where
        BlockHeaderT: 'header + BlockEnvForHardfork<Self::Hardfork>;
}

impl ChainSpec for GenericChainSpec {
    type HaltReason = edr_chain_l1::HaltReason;
    type SignedTransaction = crate::transaction::SignedTransactionWithFallbackToPostEip155;
}

impl ContextChainSpec for GenericChainSpec {
    type Context = ();
}

impl EvmChainSpec for GenericChainSpec {
    type PrecompileProvider<BlockT: BlockEnvTrait, DatabaseT: Database> =
        <L1ChainSpec as EvmChainSpec>::PrecompileProvider<BlockT, DatabaseT>;

    fn dry_run<
        BlockT: BlockEnvTrait,
        DatabaseT: Database,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockT,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let context = Context {
            block,
            tx: transaction,
            journaled_state: Journal::new(database),
            cfg,
            chain: (),
            local: LocalContext::default(),
            error: Ok(()),
        };

        let mut evm = Evm::new(context, EthInstructions::default(), precompile_provider);

        evm.replay().map_err(TransactionError::from)
    }

    fn dry_run_with_inspector<
        BlockT: BlockEnvTrait,
        DatabaseT: Database,
        InspectorT: Inspector<ContextForChainSpec<Self, BlockT, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockT,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
        inspector: InspectorT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let context = Context {
            block,
            // We need to pass a transaction here to properly initialize the context.
            // This default transaction is immediately overridden by the actual transaction passed
            // to `InspectEvm::inspect_tx`, so its values do not affect the inspection
            // process.
            tx: Self::SignedTransaction::default(),
            cfg,
            journaled_state: Journal::new(database),
            chain: (),
            local: LocalContext::default(),
            error: Ok(()),
        };

        let mut evm = Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::default(),
            precompile_provider,
        );

        evm.inspect_tx(transaction).map_err(TransactionError::from)
    }
}

impl ExecutionReceiptChainSpec for GenericChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_receipt::Execution<LogT>>;
}

impl GenesisBlockFactory for GenericChainSpec {
    type GenesisBlockCreationError =
        <L1ChainSpec as GenesisBlockFactory>::GenesisBlockCreationError;

    type LocalBlock = EthLocalBlock<
        <Self as ReceiptChainSpec>::Receipt,
        <Self as BlockChainSpec>::FetchReceiptError,
        Self::Hardfork,
        <Self as ChainSpec>::SignedTransaction,
    >;

    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: BlockConfig<'_, Self::Hardfork>,
        mut options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::GenesisBlockCreationError> {
        // If no option is provided, use the default extra data for L1 Ethereum.
        options.extra_data = Some(
            options
                .extra_data
                .unwrap_or(Bytes::copy_from_slice(L1_GENESIS_BLOCK_EXTRA_DATA)),
        );

        EthLocalBlock::with_genesis_state(genesis_diff.into(), block_config, options)
    }
}

impl HardforkChainSpec for GenericChainSpec {
    type Hardfork = edr_chain_l1::Hardfork;
}

impl ProviderChainSpec for GenericChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = L1ChainSpec::MIN_ETHASH_DIFFICULTY;

    fn chain_configs() -> &'static HashMap<u64, ChainConfig<Self::Hardfork>> {
        L1ChainSpec::chain_configs()
    }

    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork> {
        L1ChainSpec::default_base_fee_params()
    }

    fn next_base_fee_per_gas(
        header: &BlockHeader,
        hardfork: Self::Hardfork,
        default_base_fee_params: &BaseFeeParams<Self::Hardfork>,
    ) -> u128 {
        L1ChainSpec::next_base_fee_per_gas(header, hardfork, default_base_fee_params)
    }
}

impl ReceiptChainSpec for GenericChainSpec {
    type ExecutionReceiptBuilder = GenericExecutionReceiptBuilder;

    type Receipt = L1BlockReceipt<<Self as ExecutionReceiptChainSpec>::ExecutionReceipt<FilterLog>>;
}

impl RpcBlockChainSpec for GenericChainSpec {
    type RpcBlock<DataT>
        = GenericRpcBlock<DataT>
    where
        DataT: serde::de::DeserializeOwned + serde::Serialize;
}

impl RpcChainSpec for GenericChainSpec {
    type RpcCallRequest = L1CallRequest;
    type RpcReceipt = GenericRpcTransactionReceipt;
    type RpcTransaction = GenericRpcTransactionWithSignature;
    type RpcTransactionRequest = L1RpcTransactionRequest;
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for GenericChainSpec {
    type PooledTransaction = edr_chain_l1::L1PooledTransaction;
    type TransactionRequest = crate::transaction::GenericTransactionRequest;

    fn cast_halt_reason(reason: Self::HaltReason) -> TransactionFailureReason<Self::HaltReason> {
        <L1ChainSpec as ProviderSpec<TimerT>>::cast_halt_reason(reason)
    }
}

#[cfg(test)]
mod tests {
    use alloy_eips::eip7840::BlobParams;
    use edr_block_header::BlobGas;
    use edr_primitives::{Address, Bloom, Bytes, B256, B64, U256};

    use super::*;
    use crate::spec::GenericChainSpec;

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
    fn generic_block_constructor_should_default_excess_blob_gas_for_cancun() {
        let header = build_block_header(None); // No blob gas information
        let spec_id = edr_chain_l1::Hardfork::CANCUN;

        let block = <GenericChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header, spec_id, None,
        );
        assert_eq!(
            block.blob_excess_gas_and_price(),
            Some(BlobExcessGasAndPrice::new(
                0u64,
                BlobParams::cancun()
                    .update_fraction
                    .try_into()
                    .expect("blob update fraction is too large")
            ))
        );
    }

    #[test]
    fn generic_block_constructor_should_default_excess_blob_gas_for_prague() {
        let header = build_block_header(None); // No blob gas information
        let spec_id = edr_chain_l1::Hardfork::PRAGUE;

        let block = <GenericChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header, spec_id, None,
        );
        assert_eq!(
            block.blob_excess_gas_and_price(),
            Some(BlobExcessGasAndPrice::new(
                0u64,
                BlobParams::prague()
                    .update_fraction
                    .try_into()
                    .expect("blob update fraction is too large")
            ))
        );
    }

    #[test]
    fn generic_block_constructor_should_default_excess_blob_gas_for_osaka() {
        let header = build_block_header(None); // No blob gas information
        let spec_id = edr_chain_l1::Hardfork::OSAKA;

        let block = <GenericChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header, spec_id, None,
        );
        assert_eq!(
            block.blob_excess_gas_and_price(),
            Some(BlobExcessGasAndPrice::new(
                0u64,
                BlobParams::osaka()
                    .update_fraction
                    .try_into()
                    .expect("blob update fraction is too large")
            ))
        );
    }

    #[test]
    fn generic_block_constructor_should_not_default_excess_blob_gas_before_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = <GenericChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header,
            edr_chain_l1::Hardfork::SHANGHAI,
            None,
        );
        assert_eq!(block.blob_excess_gas_and_price(), None);
    }

    #[test]
    fn generic_block_constructor_should_use_existing_excess_blob_gas() {
        let excess_gas = 0x80000u64;
        let blob_gas = BlobGas {
            excess_gas,
            gas_used: 0x80000u64,
        };
        let header = build_block_header(Some(blob_gas)); // blob gas present
        let spec_id = edr_chain_l1::Hardfork::CANCUN;

        let block =
            <GenericChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(&header, spec_id, None);

        let blob_excess_gas = block
            .blob_excess_gas_and_price()
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }
}
