use std::{fmt::Debug, num::NonZeroU64, sync::Arc};

use anyhow::anyhow;
use edr_eth::{
    account::AccountInfo,
    block::{self, miner_reward, HeaderOverrides},
    log::FilterLog,
    receipt::{AsExecutionReceipt, ExecutionReceipt as _, ReceiptTrait as _},
    transaction::{ExecutableTransaction, TransactionValidation},
    withdrawal::Withdrawal,
    Address, HashMap, PreEip1898BlockSpec,
};
use edr_rpc_eth::client::EthRpcClient;

use crate::{
    blockchain::{Blockchain as _, BlockchainErrorForChainSpec, ForkedBlockchain},
    config::CfgEnv,
    spec::SyncRuntimeSpec,
    state::{AccountTrie, IrregularState, StateError, TrieState},
    Block, BlockBuilder, BlockInputs, BlockReceipts, EvmInvalidTransaction, LocalBlock as _,
    MemPool, MemPoolAddTransactionError, RandomHashGenerator, RemoteBlock,
};

/// A test fixture for `MemPool`.
pub struct MemPoolTestFixture<SignedTransactionT: ExecutableTransaction> {
    /// The mem pool.
    pub mem_pool: MemPool<SignedTransactionT>,
    /// The state.
    pub state: TrieState,
}

impl<SignedTransactionT: ExecutableTransaction> MemPoolTestFixture<SignedTransactionT> {
    /// Constructs an instance with the provided accounts.
    pub fn with_accounts(accounts: &[(Address, AccountInfo)]) -> Self {
        let accounts = accounts.iter().cloned().collect::<HashMap<_, _>>();
        let trie = AccountTrie::with_accounts(&accounts);

        MemPoolTestFixture {
            // SAFETY: literal is non-zero
            mem_pool: MemPool::new(unsafe { NonZeroU64::new_unchecked(10_000_000u64) }),
            state: TrieState::with_accounts(trie),
        }
    }

    /// Sets the block gas limit.
    pub fn set_block_gas_limit(&mut self, block_gas_limit: NonZeroU64) -> Result<(), StateError> {
        self.mem_pool
            .set_block_gas_limit(&self.state, block_gas_limit)
    }

    /// Updates the mem pool.
    pub fn update(&mut self) -> Result<(), StateError> {
        self.mem_pool.update(&self.state)
    }
}

impl<SignedTransactionT: Clone + ExecutableTransaction> MemPoolTestFixture<SignedTransactionT> {
    /// Tries to add the provided transaction to the mem pool.
    pub fn add_transaction(
        &mut self,
        transaction: SignedTransactionT,
    ) -> Result<(), MemPoolAddTransactionError<StateError>> {
        self.mem_pool.add_transaction(&self.state, transaction)
    }
}

/// Runs a full remote block, asserting that the mined block matches the remote
/// block.
pub async fn run_full_block<
    ChainSpecT: Debug
        + SyncRuntimeSpec<
            BlockReceipt: AsExecutionReceipt<
                ExecutionReceipt = ChainSpecT::ExecutionReceipt<FilterLog>,
            >,
            ExecutionReceipt<FilterLog>: PartialEq,
            LocalBlock: BlockReceipts<
                Arc<ChainSpecT::BlockReceipt>,
                Error = BlockchainErrorForChainSpec<ChainSpecT>,
            >,
            SignedTransaction: TransactionValidation<
                ValidationError: From<EvmInvalidTransaction> + Send + Sync,
            >,
        >,
>(
    url: String,
    block_number: u64,
    header_overrides_constructor: impl FnOnce(&block::Header) -> HeaderOverrides,
) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Handle::current();

    let rpc_client = EthRpcClient::<ChainSpecT>::new(&url, edr_defaults::CACHE_DIR.into(), None)?;
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
    let hardfork_activation_overrides = HashMap::new();

    let hardfork_activations =
        ChainSpecT::chain_hardfork_activations(chain_id).ok_or(anyhow!("Unsupported chain id"))?;

    let replay_header = replay_block.header();
    let hardfork = hardfork_activations
        .hardfork_at_block(block_number, replay_header.timestamp)
        .ok_or(anyhow!("Unsupported block number"))?;

    let blockchain = ForkedBlockchain::new(
        runtime.clone(),
        Some(chain_id),
        hardfork,
        rpc_client,
        Some(block_number - 1),
        &mut irregular_state,
        state_root_generator,
        &hardfork_activation_overrides,
    )
    .await?;

    let mut cfg = CfgEnv::<ChainSpecT::Hardfork>::new_with_spec(hardfork);
    cfg.chain_id = chain_id;
    cfg.disable_eip3607 = true;

    let state =
        blockchain.state_at_block_number(block_number - 1, irregular_state.state_overrides())?;

    let custom_precompiles = HashMap::new();

    let mut builder = ChainSpecT::BlockBuilder::new_block_builder(
        &blockchain,
        state,
        cfg,
        BlockInputs {
            ommers: Vec::new(),
            withdrawals: replay_block.withdrawals().map(<[Withdrawal]>::to_vec),
        },
        header_overrides_constructor(replay_header),
        &custom_precompiles,
    )?;

    assert_eq!(replay_header.base_fee_per_gas, builder.header().base_fee);

    for transaction in replay_block.transactions() {
        builder.add_transaction(transaction.clone())?;
    }

    let rewards = vec![(
        replay_header.beneficiary,
        miner_reward(hardfork.into()).unwrap_or(0),
    )];
    let mined_block = builder.finalize(rewards)?;

    let mined_header = mined_block.block.header();
    for (expected, actual) in replay_block
        .fetch_transaction_receipts()?
        .into_iter()
        .zip(mined_block.block.transaction_receipts().iter())
    {
        debug_assert_eq!(
            expected.block_number(),
            actual.block_number(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
        debug_assert_eq!(
            expected.transaction_hash(),
            actual.transaction_hash(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
        debug_assert_eq!(
            expected.transaction_index(),
            actual.transaction_index(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
        debug_assert_eq!(
            expected.from(),
            actual.from(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
        debug_assert_eq!(
            expected.to(),
            actual.to(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
        debug_assert_eq!(
            expected.contract_address(),
            actual.contract_address(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
        debug_assert_eq!(
            expected.gas_used(),
            actual.gas_used(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
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
            replay_block.transactions()[expected.transaction_index() as usize]
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
                    replay_block.transactions()[expected.transaction_index as usize]
                );
                debug_assert_eq!(
                    expected.inner.topics(),
                    actual.inner.topics(),
                    "{:?}",
                    replay_block.transactions()[expected.transaction_index as usize]
                );
                debug_assert_eq!(
                    expected.inner.data.data,
                    actual.inner.data.data,
                    "{:?}",
                    replay_block.transactions()[expected.transaction_index as usize]
                );
            }
        }
        debug_assert_eq!(
            expected.root_or_status(),
            actual.root_or_status(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
        debug_assert_eq!(
            expected.as_execution_receipt(),
            actual.as_execution_receipt(),
            "{:?}",
            replay_block.transactions()[expected.transaction_index() as usize]
        );
    }

    assert_eq!(replay_header, mined_header);

    Ok(())
}

/// Implements full block tests for the provided chain specs.
/// ```no_run
/// use edr_eth::{block::{self, HeaderOverrides}, l1::L1ChainSpec};
/// use edr_evm::impl_full_block_tests;
/// use edr_test_utils::env::get_alchemy_url;
///
/// fn timestamp_overrides(replay_header: &block::Header) -> HeaderOverrides {
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
