use std::sync::Arc;

use edr_chain_l1::L1ChainSpec;
use edr_eip1559::BaseFeeParams;
use edr_eth::{
    block::{self, BlobGas, BlockChainCondition, Header, PartialHeader},
    eips::eip4844::{self, blob_base_fee_update_fraction, BlobExcessGasAndPrice},
    Bytes, U256,
};
use edr_evm::{
    evm::{EthFrame, Evm},
    hardfork::Activations,
    inspector::{Inspector, NoOpInspector},
    interpreter::{EthInstructions, EthInterpreter, InterpreterResult},
    precompile::{EthPrecompiles, PrecompileProvider},
    spec::{
        BlockEnvConstructor, ContextForChainSpec, ExecutionReceiptTypeConstructorForChainSpec,
        GenesisBlockFactory, RuntimeSpec, EXTRA_DATA,
    },
    state::{Database, DatabaseComponentError},
    transaction::{TransactionError, TransactionErrorForChainSpec},
    BlockReceipts, EthBlockBuilder, EthBlockReceiptFactory, EthLocalBlock,
    EthLocalBlockForChainSpec, RemoteBlock, SyncBlock,
};
use edr_evm_spec::{
    ChainHardfork, ChainSpec, EthHeaderConstants, EvmSpecId, TransactionValidation,
};
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, TransactionFailureReason};
use edr_receipt::{log::FilterLog, BlockReceipt};

use crate::GenericChainSpec;

impl ChainHardfork for GenericChainSpec {
    type Hardfork = edr_chain_l1::Hardfork;
}

impl ChainSpec for GenericChainSpec {
    type BlockEnv = edr_chain_l1::BlockEnv;
    type Context = ();
    type HaltReason = edr_chain_l1::HaltReason;
    type SignedTransaction = crate::transaction::SignedWithFallbackToPostEip155;
}

fn blob_excess_gas_and_price(
    blob_gas: &Option<BlobGas>,
    hardfork: edr_chain_l1::Hardfork,
) -> Option<BlobExcessGasAndPrice> {
    let update_fraction = blob_base_fee_update_fraction(hardfork);

    blob_gas
        .as_ref()
        .map(|BlobGas { excess_gas, .. }| {
            eip4844::BlobExcessGasAndPrice::new(*excess_gas, update_fraction)
        })
        .or_else(|| {
            // If the hardfork requires it, set ExcessGasAndPrice default value
            // see https://github.com/NomicFoundation/edr/issues/947
            if hardfork >= edr_chain_l1::Hardfork::CANCUN {
                Some(eip4844::BlobExcessGasAndPrice::new(0u64, update_fraction))
            } else {
                None
            }
        })
}

impl BlockEnvConstructor<block::Header> for GenericChainSpec {
    fn new_block_env(header: &Header, hardfork: EvmSpecId) -> Self::BlockEnv {
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
            blob_excess_gas_and_price: blob_excess_gas_and_price(&header.blob_gas, hardfork),
        }
    }
}

impl BlockEnvConstructor<PartialHeader> for GenericChainSpec {
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
            blob_excess_gas_and_price: blob_excess_gas_and_price(&header.blob_gas, hardfork),
        }
    }
}

impl EthHeaderConstants for GenericChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = L1ChainSpec::MIN_ETHASH_DIFFICULTY;
}

impl GenesisBlockFactory for GenericChainSpec {
    type CreationError = <L1ChainSpec as GenesisBlockFactory>::CreationError;

    type LocalBlock = <Self as RuntimeSpec>::LocalBlock;

    fn genesis_block(
        genesis_diff: edr_evm::state::StateDiff,
        chain_condition: BlockChainCondition<'_, Self::Hardfork>,
        mut options: edr_evm::GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::CreationError> {
        // If no option is provided, use the default extra data for L1 Ethereum.
        options.extra_data = Some(
            options
                .extra_data
                .unwrap_or(Bytes::copy_from_slice(EXTRA_DATA)),
        );

        EthLocalBlockForChainSpec::<Self>::with_genesis_state::<Self>(
            genesis_diff,
            chain_condition,
            options,
        )
    }
}

impl RuntimeSpec for GenericChainSpec {
    type Block = dyn SyncBlock<
        Arc<Self::BlockReceipt>,
        Self::SignedTransaction,
        Error = <Self::LocalBlock as BlockReceipts<Arc<Self::BlockReceipt>>>::Error,
    >;

    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + std::error::Error + Send,
        StateErrorT: 'builder + std::error::Error + Send,
    > = EthBlockBuilder<'builder, BlockchainErrorT, Self, StateErrorT>;

    type BlockReceipt = BlockReceipt<Self::ExecutionReceipt<FilterLog>>;

    type BlockReceiptFactory = EthBlockReceiptFactory<Self::ExecutionReceipt<FilterLog>>;

    type Evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    > = Evm<
        ContextForChainSpec<Self, DatabaseT>,
        InspectorT,
        EthInstructions<EthInterpreter, ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT,
        EthFrame<EthInterpreter>,
    >;

    type LocalBlock = EthLocalBlock<
        Self::RpcBlockConversionError,
        Self::BlockReceipt,
        ExecutionReceiptTypeConstructorForChainSpec<Self>,
        Self::Hardfork,
        Self::RpcReceiptConversionError,
        Self::SignedTransaction,
    >;

    type PrecompileProvider<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    > = EthPrecompiles;

    type ReceiptBuilder = crate::receipt::execution::Builder;
    type RpcBlockConversionError = crate::rpc::block::ConversionError<Self>;
    type RpcReceiptConversionError = crate::rpc::receipt::ConversionError;
    type RpcTransactionConversionError = crate::rpc::transaction::ConversionError;

    fn cast_local_block(local_block: Arc<Self::LocalBlock>) -> Arc<Self::Block> {
        local_block
    }

    fn cast_remote_block(remote_block: Arc<RemoteBlock<Self>>) -> Arc<Self::Block> {
        remote_block
    }

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::SignedTransaction as TransactionValidation>::ValidationError,
    ) -> TransactionErrorForChainSpec<BlockchainErrorT, Self, StateErrorT> {
        // Can't use L1ChainSpec impl here as the TransactionError is generic
        // over the specific chain spec rather than just the validation error.
        // Instead, we copy the impl here.
        match error {
            edr_chain_l1::InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self::Hardfork>> {
        L1ChainSpec::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        L1ChainSpec::chain_name(chain_id)
    }

    fn evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, NoOpInspector, PrecompileProviderT, StateErrorT>
    {
        Self::evm_with_inspector(context, NoOpInspector {}, precompile_provider)
    }

    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        inspector: InspectorT,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, InspectorT, PrecompileProviderT, StateErrorT> {
        Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::default(),
            precompile_provider,
        )
    }

    fn chain_base_fee_params(chain_id: u64) -> &'static BaseFeeParams<Self::Hardfork> {
        L1ChainSpec::chain_base_fee_params(chain_id)
    }

    fn next_base_fee_per_gas(
        header: &Header,
        chain_id: u64,
        hardfork: Self::Hardfork,
        base_fee_params_overrides: Option<&BaseFeeParams<Self::Hardfork>>,
    ) -> u128 {
        L1ChainSpec::next_base_fee_per_gas(header, chain_id, hardfork, base_fee_params_overrides)
    }
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for GenericChainSpec {
    type PooledTransaction = edr_chain_l1::PooledTransaction;
    type TransactionRequest = crate::transaction::Request;

    fn cast_halt_reason(reason: Self::HaltReason) -> TransactionFailureReason<Self::HaltReason> {
        <L1ChainSpec as ProviderSpec<TimerT>>::cast_halt_reason(reason)
    }
}

#[cfg(test)]
mod tests {
    use edr_eth::{
        block::{BlobGas, Header},
        eips::eip4844,
        Address, Bloom, Bytes, B256, B64, U256,
    };
    use edr_evm::spec::BlockEnvConstructor as _;

    use crate::spec::GenericChainSpec;

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
    fn generic_block_constructor_should_default_excess_blob_gas_for_cancun() {
        let header = build_block_header(None); // No blob gas information
        let spec_id = edr_chain_l1::Hardfork::CANCUN;

        let block = GenericChainSpec::new_block_env(&header, spec_id);
        assert_eq!(
            block.blob_excess_gas_and_price,
            Some(eip4844::BlobExcessGasAndPrice::new(
                0u64,
                eip4844::blob_base_fee_update_fraction(spec_id)
            ))
        );
    }

    #[test]
    fn generic_block_constructor_should_default_excess_blob_gas_after_cancun() {
        let header = build_block_header(None); // No blob gas information
        let spec_id = edr_chain_l1::Hardfork::PRAGUE;

        let block = GenericChainSpec::new_block_env(&header, spec_id);
        assert_eq!(
            block.blob_excess_gas_and_price,
            Some(eip4844::BlobExcessGasAndPrice::new(
                0u64,
                eip4844::blob_base_fee_update_fraction(spec_id)
            ))
        );
    }

    #[test]
    fn generic_block_constructor_should_not_default_excess_blob_gas_before_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = GenericChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::SHANGHAI);
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
        let spec_id = edr_chain_l1::Hardfork::CANCUN;

        let block = GenericChainSpec::new_block_env(&header, spec_id);

        let blob_excess_gas = block
            .blob_excess_gas_and_price
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }
}
