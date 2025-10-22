//! Test utilities for replaying blocks from remote Ethereum-compatible
//! nodes and comparing the results with locally mined blocks.
#![warn(missing_docs)]

use core::fmt::Debug;
use std::sync::Arc;

use anyhow::anyhow;
use edr_block_api::{Block, BlockReceipts, EthBlockData, LocalBlock as _};
use edr_block_builder_api::{BlockBuilder as _, BlockInputs};
use edr_block_header::{BlockConfig, BlockHeader, HeaderOverrides, PartialHeader, Withdrawal};
use edr_block_remote::RemoteBlock;
use edr_blockchain_api::{BlockchainMetadata as _, StateAtBlock as _};
use edr_blockchain_fork::ForkedBlockchain;
use edr_chain_spec::{
    ChainSpec, EvmSpecId, ExecutableTransaction, HardforkChainSpec, TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_eth::{block::miner_reward, PreEip1898BlockSpec};
use edr_evm_spec::config::EvmConfig;
use edr_primitives::{HashMap, B256};
use edr_receipt::{log::FilterLog, AsExecutionReceipt, ExecutionReceipt as _, ReceiptTrait};
use edr_receipt_spec::ReceiptChainSpec;
use edr_rpc_eth::{client::EthRpcClient, RpcBlockChainSpec};
use edr_rpc_spec::{RpcChainSpec, RpcEthBlock};
use edr_state_api::irregular::IrregularState;
use edr_utils::random::RandomHashGenerator;

type ForkedStateAndBlockchainForChainSpec<ChainSpecT> = ForkedStateAndBlockchain<
    <ChainSpecT as ReceiptChainSpec>::Receipt,
    <ChainSpecT as BlockChainSpec>::Block,
    <ChainSpecT as HardforkChainSpec>::Hardfork,
    <ChainSpecT as BlockChainSpec>::LocalBlock,
    ChainSpecT,
    <ChainSpecT as RpcChainSpec>::RpcReceipt,
    <ChainSpecT as RpcChainSpec>::RpcTransaction,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

struct ForkedStateAndBlockchain<
    BlockReceiptT: Debug + ReceiptTrait,
    BlockT: ?Sized + Block<SignedTransactionT>,
    HardforkT: Clone + Into<EvmSpecId>,
    LocalBlockT,
    RpcBlockChainSpecT: RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
    RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
    RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
    SignedTransactionT: Debug + ExecutableTransaction,
> {
    pub expected_block: RemoteBlock<
        BlockReceiptT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >,
    pub prior_blockchain: ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >,
    pub prior_irregular_state: IrregularState,
}

/// Creates forked state at the previous block and returns the corresponding
/// `ForkedStateAndBlockchain`
async fn get_fork_state<
    ChainSpecT: ProviderChainSpec<Hardfork: 'static + Debug, SignedTransaction: Debug>
        + ProviderChainSpec<
            Hardfork: Send + Sync,
            RpcBlock<<ChainSpecT as RpcChainSpec>::RpcTransaction>: TryInto<
                EthBlockData<ChainSpecT::SignedTransaction>,
                Error: 'static,
            >,
        >,
>(
    runtime: tokio::runtime::Handle,
    url: String,
    block_number: u64,
) -> anyhow::Result<ForkedStateAndBlockchainForChainSpec<ChainSpecT>> {
    let rpc_client =
        EthRpcClient::<ChainSpecT, ChainSpecT::RpcReceipt, ChainSpecT::RpcTransaction>::new(
            &url,
            edr_defaults::CACHE_DIR.into(),
            None,
        )?;
    let chain_id = rpc_client.chain_id().await?;

    let rpc_client = Arc::new(rpc_client);
    let replay_block = {
        let block = rpc_client
            .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(block_number))
            .await?;

        RemoteBlock::new(block, rpc_client.clone(), runtime.clone())?
    };

    let mut irregular_state = IrregularState::default();
    let state_root_generator = Arc::new(parking_lot::Mutex::new(RandomHashGenerator::with_seed(
        edr_defaults::STATE_ROOT_HASH_SEED,
    )));

    let chain_configs = ChainSpecT::chain_configs();
    let chain_config = chain_configs.get(&chain_id);

    let hardfork_activations = chain_config
        .as_ref()
        .map(|config| config.hardfork_activations.clone())
        .ok_or(anyhow!("Unsupported chain id"))?;

    let replay_header = replay_block.block_header();
    let hardfork = hardfork_activations
        .hardfork_at_block(block_number, replay_header.timestamp)
        .ok_or(anyhow!("Unsupported block number"))?;

    let base_fee_params = chain_config.as_ref().map_or_else(
        || ChainSpecT::default_base_fee_params(),
        |config| &config.base_fee_params,
    );

    let block_config = BlockConfig {
        base_fee_params,
        hardfork,
        min_ethash_difficulty: ChainSpecT::MIN_ETHASH_DIFFICULTY,
    };

    let blockchain = ForkedBlockchain::new(
        block_config,
        runtime.clone(),
        rpc_client,
        Some(block_number - 1),
        &mut irregular_state,
        state_root_generator,
        chain_configs,
        Some(chain_id),
    )
    .await?;

    Ok(ForkedStateAndBlockchain {
        expected_block: replay_block,
        prior_blockchain: blockchain,
        prior_irregular_state: irregular_state,
    })
}

/// Runs a full remote block, asserting that the mined block matches the remote
/// block.
pub async fn run_full_block<
    ChainSpecT: 'static
        + ProviderChainSpec<
            ExecutionReceipt<FilterLog>: Debug + PartialEq,
            Hardfork: 'static + Debug + Send + Sync,
            Receipt: AsExecutionReceipt<ExecutionReceipt = ChainSpecT::ExecutionReceipt<FilterLog>>,
            RpcBlock<<ChainSpecT as RpcChainSpec>::RpcTransaction>: TryInto<
                EthBlockData<ChainSpecT::SignedTransaction>,
                Error: 'static,
            >,
            SignedTransaction: Debug + TransactionValidation<ValidationError: Send + Sync>,
        >,
>(
    runtime: tokio::runtime::Handle,
    url: String,
    block_number: u64,
    header_overrides_constructor: impl FnOnce(&BlockHeader) -> HeaderOverrides<ChainSpecT::Hardfork>,
) -> anyhow::Result<()> {
    let ForkedStateAndBlockchain {
        expected_block,
        prior_blockchain,
        prior_irregular_state,
    } = get_fork_state::<ChainSpecT>(runtime, url, block_number).await?;

    let replay_header = expected_block.block_header();
    let hardfork = prior_blockchain.hardfork();

    let evm_config = EvmConfig {
        chain_id: prior_blockchain.chain_id(),
        disable_eip3607: true,
        limit_contract_code_size: None,
    };

    let state = prior_blockchain
        .state_at_block_number(block_number - 1, prior_irregular_state.state_overrides())?;

    let custom_precompiles = HashMap::new();

    let mut builder = ChainSpecT::BlockBuilder::new_block_builder(
        &prior_blockchain,
        state,
        &evm_config,
        BlockInputs::new(hardfork),
        header_overrides_constructor(replay_header),
        &custom_precompiles,
    )?;
    assert_eq!(replay_header.base_fee_per_gas, builder.header().base_fee);

    for transaction in expected_block.transactions() {
        builder.add_transaction(transaction.clone())?;
    }

    let rewards = vec![(
        replay_header.beneficiary,
        miner_reward(hardfork.into()).unwrap_or(0),
    )];
    let mined_block = builder.finalize_block(rewards)?;

    let mined_header = mined_block.block.block_header();
    for (expected, actual) in expected_block
        .fetch_transaction_receipts()?
        .into_iter()
        .zip(mined_block.block.transaction_receipts().iter())
    {
        debug_assert_eq!(
            expected.block_number(),
            actual.block_number(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        debug_assert_eq!(
            expected.transaction_hash(),
            actual.transaction_hash(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        debug_assert_eq!(
            expected.transaction_index(),
            actual.transaction_index(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        debug_assert_eq!(
            expected.from(),
            actual.from(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        debug_assert_eq!(
            expected.to(),
            actual.to(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        debug_assert_eq!(
            expected.contract_address(),
            actual.contract_address(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        debug_assert_eq!(
            expected.gas_used(),
            actual.gas_used(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        // Skip effective gas price check because Hardhat doesn't include it pre-London
        // debug_assert_eq!(
        //     expected.effective_gas_price,
        //     actual.effective_gas_price,
        //     "{:?}",
        //     replay_block.transactions()[expected.transaction_index as usize]
        // );
        debug_assert_eq!(
            expected.cumulative_gas_used(),
            actual.cumulative_gas_used(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        if expected.logs_bloom() != actual.logs_bloom() {
            for (expected, actual) in expected
                .transaction_logs()
                .iter()
                .zip(actual.transaction_logs().iter())
            {
                debug_assert_eq!(
                    expected.inner.address,
                    actual.inner.address,
                    "{:?}",
                    expected_block
                        .transactions()
                        .get(expected.transaction_index as usize)
                        .expect("transaction index is valid")
                );
                debug_assert_eq!(
                    expected.inner.topics(),
                    actual.inner.topics(),
                    "{:?}",
                    expected_block
                        .transactions()
                        .get(expected.transaction_index as usize)
                        .expect("transaction index is valid")
                );
                debug_assert_eq!(
                    expected.inner.data.data,
                    actual.inner.data.data,
                    "{:?}",
                    expected_block
                        .transactions()
                        .get(expected.transaction_index as usize)
                        .expect("transaction index is valid")
                );
            }
        }
        debug_assert_eq!(
            expected.root_or_status(),
            actual.root_or_status(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
        debug_assert_eq!(
            expected.as_execution_receipt(),
            actual.as_execution_receipt(),
            "{:?}",
            expected_block
                .transactions()
                .get(expected.transaction_index() as usize)
                .expect("transaction index is valid")
        );
    }

    assert_eq!(replay_header, mined_header);

    Ok(())
}

/// Forks the block at the provided block number and compares it with the
/// locally mined block header for that block without transactions.
///
/// It does not add the transactions of the block being replayed to keep it
/// lightweight. If you need to compare header values that change depending on
/// the transactions included in the block use `run_full_block` instead.
pub async fn assert_replay_header<
    ChainSpecT: 'static
        + ProviderChainSpec<
            ExecutionReceipt<FilterLog>: Debug + PartialEq,
            Hardfork: 'static + Debug + Send + Sync,
            Receipt: AsExecutionReceipt<ExecutionReceipt = ChainSpecT::ExecutionReceipt<FilterLog>>,
            RpcBlock<<ChainSpecT as RpcChainSpec>::RpcTransaction>: TryInto<
                EthBlockData<ChainSpecT::SignedTransaction>,
                Error: 'static,
            >,
            SignedTransaction: Debug + TransactionValidation<ValidationError: Send + Sync>,
        >, /*
            * + SyncRuntimeSpec< BlockReceipt: AsExecutionReceipt< ExecutionReceipt =
            *   ChainSpecT::ExecutionReceipt<FilterLog>, >, ExecutionReceipt<FilterLog>:
            *   PartialEq, LocalBlock: BlockReceipts< Arc<ChainSpecT::BlockReceipt>, Error =
            *   BlockchainErrorForChainSpec<ChainSpecT>, >, SignedTransaction:
            *   TransactionValidation< ValidationError: From<EvmTransactionValidationError> +
            *   Send + Sync, >,
            * >, */
>(
    runtime: tokio::runtime::Handle,
    url: String,
    block_number: u64,
    header_overrides_constructor: impl FnOnce(&BlockHeader) -> HeaderOverrides<ChainSpecT::Hardfork>,
    header_validation: impl FnOnce(&BlockHeader, &PartialHeader) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let ForkedStateAndBlockchain {
        expected_block,
        prior_blockchain,
        prior_irregular_state,
    } = get_fork_state::<ChainSpecT>(runtime, url, block_number).await?;

    let replay_header = expected_block.block_header();

    let evm_config = EvmConfig {
        chain_id: prior_blockchain.chain_id(),
        disable_eip3607: true,
        limit_contract_code_size: None,
    };

    let state = prior_blockchain
        .state_at_block_number(block_number - 1, prior_irregular_state.state_overrides())?;

    let custom_precompiles = HashMap::new();

    let builder = ChainSpecT::BlockBuilder::new_block_builder(
        &prior_blockchain,
        state,
        &evm_config,
        BlockInputs {
            ommers: Vec::new(),
            withdrawals: expected_block.withdrawals().map(<[Withdrawal]>::to_vec),
        },
        header_overrides_constructor(replay_header),
        &custom_precompiles,
    )?;
    header_validation(replay_header, builder.header())
}

/// Implements full block tests for the provided chain specs.
/// ```no_run
/// use edr_block_header::{BlockHeader, HeaderOverrides};
/// use edr_chain_l1::L1ChainSpec;
/// use edr_evm::impl_full_block_tests;
/// use edr_test_utils::env::get_alchemy_url;
///
/// fn timestamp_overrides<HardforkT: Default>(replay_header: &BlockHeader) -> HeaderOverrides<HardforkT> {
///     HeaderOverrides {
///         timestamp: Some(replay_header.timestamp),
///         ..HeaderOverrides::default()
///     }
/// }
///
/// impl_full_block_tests! {
///     mainnet_byzantium => L1ChainSpec {
///         block_number: 4_370_001,
///         url: get_alchemy_url(),
///         header_overrides_constructor: timestamp_overrides,
///     },
/// }
/// ```
#[macro_export]
macro_rules! impl_full_block_tests {
    ($(
        $name:ident => $chain_spec:ident {
            block_number: $block_number:expr,
            url: $url:expr,
            header_overrides_constructor: $header_overrides_constructor:expr,
        },
    )+) => {
        $(
            paste::item! {
                #[serial_test::serial]
                #[tokio::test(flavor = "multi_thread")]
                async fn [<full_block_ $name>]() -> anyhow::Result<()> {
                    let url = $url;

                    $crate::test_utils::run_full_block::<$chain_spec>(url, $block_number, $header_overrides_constructor).await
                }
            }
        )+
    }
}
