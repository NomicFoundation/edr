/// Types and constants for Ethereum improvements proposals (EIPs)
pub mod eips;

use std::{collections::BTreeMap, fmt::Debug, marker::PhantomData, num::NonZeroU64, sync::Arc};

use derive_where::derive_where;
use edr_block_api::{
    validate_next_block, Block, BlockAndTotalDifficulty, BlockValidityError, EmptyBlock,
    EthBlockData, FetchBlockReceipts, LocalBlock,
};
use edr_block_header::BlockConfig;
use edr_block_remote::RemoteBlock;
use edr_block_storage::{
    InsertBlockAndReceiptsError, InsertBlockError, ReservableSparseBlockStorage,
};
use edr_blockchain_api::{
    utils::compute_state_at_block, BlockHashByNumber, BlockchainMetadata, GetBlockchainBlock,
    GetBlockchainLogs, InsertBlock, ReceiptByTransactionHash, ReserveBlocks, RevertToBlock,
    StateAtBlock, TotalDifficultyByBlockHash,
};
use edr_blockchain_remote::{FetchRemoteBlockError, FetchRemoteReceiptError, RemoteBlockchain};
use edr_chain_config::{ChainConfig, HardforkActivations};
use edr_chain_spec::{EvmSpecId, ExecutableTransaction};
use edr_chain_spec_rpc::{RpcBlockChainSpec, RpcEthBlock, RpcTransaction};
use edr_eip1559::BaseFeeParams;
use edr_eip7892::ScheduledBlobParams;
use edr_eth::{
    block::{largest_safe_block_number, safe_block_depth, LargestSafeBlockNumberArgs},
    BlockSpec, PreEip1898BlockSpec,
};
use edr_primitives::{Address, ChainId, HashMap, HashSet, B256, U256};
use edr_receipt::{log::FilterLog, ExecutionReceipt, ReceiptTrait};
use edr_rpc_eth::{
    client::{EthRpcClient, RpcClientError},
    fork::ForkMetadata,
};
use edr_state_api::{
    account::{Account, AccountStatus},
    irregular::IrregularState,
    DynState, StateDiff, StateOverride,
};
use edr_state_fork::ForkedState;
use edr_utils::{random::RandomHashGenerator, CastArcFrom, CastArcInto};
use parking_lot::Mutex;
use tokio::runtime;

use crate::eips::{
    eip2935::{
        add_history_storage_contract_to_state_diff, history_storage_contract,
        HISTORY_STORAGE_ADDRESS,
    },
    eip4788::{
        add_beacon_roots_contract_to_state_diff, beacon_roots_contract, BEACON_ROOTS_ADDRESS,
    },
};

/// An error that occurs upon creation of a [`ForkedBlockchain`].
#[derive(Debug, thiserror::Error)]
pub enum ForkedBlockchainCreationError<HardforkT> {
    /// JSON-RPC error
    #[error(transparent)]
    RpcClientError(#[from] RpcClientError),
    /// The requested block number does not exist
    #[error(
        "Trying to initialize a provider with block {fork_block_number} but the current block is {latest_block_number}"
    )]
    InvalidBlockNumber {
        /// Requested fork block number
        fork_block_number: u64,
        /// Latest block number
        latest_block_number: u64,
    },
    /// The detected hardfork is not supported
    #[error(
        "Cannot fork {chain_name} from block {fork_block_number}. The hardfork must be at least Spurious Dragon, but {hardfork:?} was detected."
    )]
    InvalidHardfork {
        /// Requested fork block number
        fork_block_number: u64,
        /// Chain name
        chain_name: String,
        /// Detected hardfork
        hardfork: HardforkT,
    },
    /// Unsupported storage overrides
    #[error(
        "Storage overrides are not supported for forked blocks yet. See https://github.com/NomicFoundation/edr/issues/911"
    )]
    StorageOverridesUnsupported,
}

/// Error type for [`ForkedBlockchain`].
#[derive(Debug, thiserror::Error)]
pub enum ForkedBlockchainError<HardforkT, RpcBlockConversionErrorT, RpcReceiptConversionErrorT> {
    /// Remote blocks cannot be deleted
    #[error("Cannot delete remote block.")]
    CannotDeleteRemote,
    /// Failed to fetch a remote block.
    #[error(transparent)]
    FetchRemoteBlock(#[from] FetchRemoteBlockError<RpcBlockConversionErrorT>),
    /// Failed to fetch a remote receipt.
    #[error(transparent)]
    FetchRemoteReceipt(#[from] FetchRemoteReceiptError<RpcReceiptConversionErrorT>),
    /// Failed to insert a block.
    #[error(transparent)]
    InsertBlock(#[from] InsertBlockError),
    /// Failed to insert a block and its receipts.
    #[error(transparent)]
    InsertBlockAndReceipts(#[from] InsertBlockAndReceiptsError),
    /// The next block is invalid.
    #[error(transparent)]
    InvalidNextBlock(#[from] BlockValidityError),
    /// Rpc client error
    #[error(transparent)]
    RpcClient(#[from] RpcClientError),
    /// Missing hardfork activation history
    #[error(
        "No known hardfork for execution on historical block {block_number} (relative to fork block number {fork_block_number}) in chain with id {chain_id}. The node was not configured with a hardfork activation history."
    )]
    MissingHardforkActivations {
        /// Block number
        block_number: u64,
        /// Fork block number
        fork_block_number: u64,
        /// Chain id
        chain_id: u64,
    },
    /// Missing transaction receipts for a remote block
    #[error("Missing receipts for block {block_hash}")]
    MissingReceipts {
        /// The block hash
        block_hash: B256,
    },
    /// Block number does not exist in blockchain
    #[error("Unknown block number")]
    UnknownBlockNumber,
    /// No hardfork found for block
    #[error(
        "Could not find a hardfork to run for block {block_number}, after having looked for one in the hardfork activation history, which was: {hardfork_activations:?}."
    )]
    UnknownBlockSpec {
        /// Block number
        block_number: u64,
        /// Hardfork activation history
        hardfork_activations: HardforkActivations<HardforkT>,
    },
}

/// A blockchain that forked from a remote blockchain.
#[derive_where(Debug; BlockT, HardforkT, LocalBlockT)]
pub struct ForkedBlockchain<
    BlockReceiptT: Debug + ReceiptTrait,
    BlockT: ?Sized + Block<SignedTransactionT>,
    FetchReceiptErrorT,
    HardforkT,
    LocalBlockT,
    RpcBlockChainSpecT: RpcBlockChainSpec,
    RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
    RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
    SignedTransactionT: Debug + ExecutableTransaction,
> {
    local_storage: ReservableSparseBlockStorage<
        Arc<BlockReceiptT>,
        Arc<LocalBlockT>,
        HardforkT,
        SignedTransactionT,
    >,
    // We can force caching here because we only fork from a safe block number.
    #[allow(clippy::type_complexity)]
    remote: RemoteBlockchain<
        BlockReceiptT,
        Arc<
            RemoteBlock<
                BlockReceiptT,
                FetchReceiptErrorT,
                RpcBlockChainSpecT,
                RpcReceiptT,
                RpcTransactionT,
                SignedTransactionT,
            >,
        >,
        FetchReceiptErrorT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
        true,
    >,
    state_root_generator: Arc<Mutex<RandomHashGenerator>>,
    fork_block_number: u64,
    base_fee_params: BaseFeeParams<HardforkT>,
    /// The chan id of the forked blockchain is either the local chain id
    /// override or the chain id of the remote blockchain.
    chain_id: u64,
    /// The chain id of the remote blockchain. It might deviate from `chain_id`.
    remote_chain_id: u64,
    network_id: u64,
    hardfork: HardforkT,
    hardfork_activations: Option<HardforkActivations<HardforkT>>,
    min_ethash_difficulty: u64,
    scheduled_blob_params: Option<ScheduledBlobParams>,
    _phantom: PhantomData<fn() -> BlockT>,
}

impl<
        BlockReceiptT: Debug + ReceiptTrait,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT: Clone + Into<EvmSpecId>,
        LocalBlockT,
        RpcBlockChainSpecT: RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    >
    ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    /// Constructs a new instance.
    ///
    /// If the remote chain ID is found in the provided `chain_configs`, the
    /// corresponding `ChainConfig` is used to determine hardfork activations
    /// and base fee parameters.
    ///
    /// Otherwise, the base fee parameters from the [`BlockConfig`] will be used
    /// as a default.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        block_config: BlockConfig<'_, HardforkT>,
        runtime: runtime::Handle,
        rpc_client: Arc<EthRpcClient<RpcBlockChainSpecT, RpcReceiptT, RpcTransactionT>>,
        irregular_state: &mut IrregularState,
        state_root_generator: Arc<Mutex<RandomHashGenerator>>,
        chain_configs: &HashMap<ChainId, ChainConfig<HardforkT>>,
        fork_block_number: Option<u64>,
        chain_id_override: Option<u64>,
    ) -> Result<Self, ForkedBlockchainCreationError<HardforkT>> {
        let BlockConfig {
            base_fee_params: default_base_fee_params,
            hardfork,
            min_ethash_difficulty,
            scheduled_blob_params,
        } = block_config;

        let ForkMetadata {
            chain_id: remote_chain_id,
            network_id,
            latest_block_number,
        } = rpc_client.fork_metadata().await?;

        let recommended_block_number =
            recommended_fork_block_number(RecommendedForkBlockNumberArgs {
                chain_id: remote_chain_id,
                latest_block_number,
            });

        let fork_block_number = if let Some(fork_block_number) = fork_block_number {
            if fork_block_number > latest_block_number {
                return Err(ForkedBlockchainCreationError::InvalidBlockNumber {
                    fork_block_number,
                    latest_block_number,
                });
            }

            if fork_block_number > recommended_block_number {
                let num_confirmations = latest_block_number - fork_block_number + 1;
                let required_confirmations = safe_block_depth(remote_chain_id) + 1;
                let missing_confirmations = required_confirmations - num_confirmations;

                log::warn!(
                    "You are forking from block {fork_block_number} which has less than {required_confirmations} confirmations, and will affect Hardhat Network's performance. Please use block number {recommended_block_number} or wait for the block to get {missing_confirmations} more confirmations."
                );
            }

            fork_block_number
        } else {
            recommended_block_number
        };

        // Dispatch block timestamp request
        let fork_timestamp_future =
            rpc_client.get_block_by_number(PreEip1898BlockSpec::Number(fork_block_number));

        let chain_config = chain_configs.get(&remote_chain_id);
        let base_fee_params = chain_config.map_or_else(
            || default_base_fee_params.clone(),
            |config| config.base_fee_params.clone(),
        );
        let hardfork_activations = chain_config.as_ref().and_then(
            |ChainConfig {
                 hardfork_activations,
                 ..
             }| {
                // Ignore empty hardfork activations
                if hardfork_activations.is_empty() {
                    None
                } else {
                    Some(hardfork_activations.clone())
                }
            },
        );

        let fork_timestamp = fork_timestamp_future
            .await?
            .expect("Block must exist since block number is less than the latest block number.")
            .timestamp();

        if let Some(remote_hardfork) =
            hardfork_activations
                .as_ref()
                .and_then(|hardfork_activations| {
                    hardfork_activations.hardfork_at_block(fork_block_number, fork_timestamp)
                })
        {
            let remote_evm_spec_id = remote_hardfork.clone().into();
            if remote_evm_spec_id < EvmSpecId::SPURIOUS_DRAGON {
                return Err(ForkedBlockchainCreationError::InvalidHardfork {
                    chain_name: chain_config
                        .map_or("unknown".to_string(), |config| config.name.clone()),
                    fork_block_number,
                    hardfork: remote_hardfork,
                });
            }

            let local_evm_spec_id = hardfork.clone().into();
            if remote_evm_spec_id < EvmSpecId::PRAGUE && local_evm_spec_id >= EvmSpecId::PRAGUE {
                let state_root = state_root_generator.lock().next_value();

                irregular_state
                    .state_override_at_block_number(fork_block_number)
                    .and_modify(|state_override| {
                        add_beacon_roots_contract_to_state_diff(&mut state_override.diff);
                        add_history_storage_contract_to_state_diff(&mut state_override.diff);
                    })
                    .or_insert_with(|| {
                        let accounts: HashMap<Address, Account> = [
                            (
                                BEACON_ROOTS_ADDRESS,
                                Account {
                                    info: beacon_roots_contract(),
                                    status: AccountStatus::Created | AccountStatus::Touched,
                                    storage: HashMap::default(),
                                    transaction_id: 0,
                                },
                            ),
                            (
                                HISTORY_STORAGE_ADDRESS,
                                Account {
                                    info: history_storage_contract(),
                                    status: AccountStatus::Created | AccountStatus::Touched,
                                    storage: HashMap::default(),
                                    transaction_id: 0,
                                },
                            ),
                        ]
                        .into_iter()
                        .collect();

                        StateOverride {
                            diff: StateDiff::from(accounts),
                            state_root,
                        }
                    });
            } else if remote_evm_spec_id < EvmSpecId::CANCUN
                && local_evm_spec_id >= EvmSpecId::CANCUN
            {
                let state_root = state_root_generator.lock().next_value();

                irregular_state
                    .state_override_at_block_number(fork_block_number)
                    .and_modify(|state_override| {
                        add_beacon_roots_contract_to_state_diff(&mut state_override.diff);
                    })
                    .or_insert_with(|| {
                        let accounts: HashMap<Address, Account> = [(
                            BEACON_ROOTS_ADDRESS,
                            Account {
                                info: beacon_roots_contract(),
                                status: AccountStatus::Created | AccountStatus::Touched,
                                storage: HashMap::default(),
                                transaction_id: 0,
                            },
                        )]
                        .into_iter()
                        .collect();

                        StateOverride {
                            diff: StateDiff::from(accounts),
                            state_root,
                        }
                    });
            }
        }

        Ok(Self {
            local_storage: ReservableSparseBlockStorage::empty(fork_block_number),
            remote: RemoteBlockchain::new(rpc_client, runtime),
            state_root_generator,
            base_fee_params,
            chain_id: chain_id_override.unwrap_or(remote_chain_id),
            remote_chain_id,
            fork_block_number,
            network_id,
            hardfork,
            hardfork_activations,
            min_ethash_difficulty,
            _phantom: PhantomData,
            scheduled_blob_params,
        })
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT: RpcBlockChainSpec,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    >
    ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    /// Returns the chain id of the remote blockchain.
    pub fn remote_chain_id(&self) -> u64 {
        self.remote_chain_id
    }

    fn runtime(&self) -> &runtime::Handle {
        self.remote.runtime()
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT> + EmptyBlock<HardforkT> + LocalBlock<Arc<BlockReceiptT>>,
        RpcBlockChainSpecT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock + TryInto<EthBlockData<SignedTransactionT>>,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > BlockHashByNumber
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        if block_number <= self.fork_block_number {
            tokio::task::block_in_place(move || {
                self.runtime()
                    .block_on(self.remote.block_by_number(block_number))
            })
            .map(|block| Ok(*block.block_hash()))?
        } else {
            self.local_storage
                .block_by_number(block_number)?
                .map(|block| *block.block_hash())
                .ok_or(ForkedBlockchainError::UnknownBlockNumber)
        }
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT: Clone,
        LocalBlockT,
        RpcBlockChainSpecT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock + TryInto<EthBlockData<SignedTransactionT>>,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > BlockchainMetadata<HardforkT>
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    fn base_fee_params(&self) -> &BaseFeeParams<HardforkT> {
        &self.base_fee_params
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn chain_id_at_block_number(&self, block_number: u64) -> Result<u64, Self::Error> {
        if block_number > self.last_block_number() {
            return Err(ForkedBlockchainError::UnknownBlockNumber);
        }

        if block_number <= self.fork_block_number {
            Ok(self.remote_chain_id())
        } else {
            Ok(self.chain_id())
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, Self::Error> {
        if block_number > self.last_block_number() {
            return Err(ForkedBlockchainError::UnknownBlockNumber);
        }

        if block_number <= self.fork_block_number {
            tokio::task::block_in_place(move || {
                self.runtime()
                    .block_on(self.remote.block_by_number(block_number))
                    .map_err(ForkedBlockchainError::from)
            })
            .and_then(|block| {
                if let Some(hardfork_activations) = &self.hardfork_activations {
                    let header = block.block_header();
                    hardfork_activations
                        .hardfork_at_block(header.number, header.timestamp)
                        .ok_or(ForkedBlockchainError::UnknownBlockSpec {
                            block_number,
                            hardfork_activations: hardfork_activations.clone(),
                        })
                } else {
                    Err(ForkedBlockchainError::MissingHardforkActivations {
                        block_number,
                        fork_block_number: self.fork_block_number,
                        chain_id: self.remote_chain_id,
                    })
                }
            })
        } else {
            Ok(self.hardfork.clone())
        }
    }

    fn hardfork(&self) -> HardforkT {
        self.hardfork.clone()
    }

    fn last_block_number(&self) -> u64 {
        self.local_storage.last_block_number()
    }

    fn min_ethash_difficulty(&self) -> u64 {
        self.min_ethash_difficulty
    }

    fn network_id(&self) -> u64 {
        self.network_id
    }
    
    fn scheduled_blob_params(&self) -> Option< &ScheduledBlobParams>  {
        self.scheduled_blob_params.as_ref()
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized
            + Block<SignedTransactionT>
            + CastArcFrom<LocalBlockT>
            + CastArcFrom<
                RemoteBlock<
                    BlockReceiptT,
                    FetchReceiptErrorT,
                    RpcBlockChainSpecT,
                    RpcReceiptT,
                    RpcTransactionT,
                    SignedTransactionT,
                >,
            >,
        FetchReceiptErrorT,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT> + EmptyBlock<HardforkT> + LocalBlock<Arc<BlockReceiptT>>,
        RpcBlockChainSpecT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock + TryInto<EthBlockData<SignedTransactionT>>,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: RpcTransaction + serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > GetBlockchainBlock<BlockT, HardforkT>
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<BlockT>>, Self::Error> {
        if let Some(local_block) = self.local_storage.block_by_hash(hash) {
            Ok(Some(local_block.cast_arc_into()))
        } else {
            let remote_block = tokio::task::block_in_place(move || {
                self.runtime().block_on(self.remote.block_by_hash(hash))
            })?;

            Ok(remote_block.map(CastArcInto::cast_arc_into))
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_number(&self, number: u64) -> Result<Option<Arc<BlockT>>, Self::Error> {
        if number <= self.fork_block_number {
            let remote_block = tokio::task::block_in_place(move || {
                self.runtime().block_on(self.remote.block_by_number(number))
            })?;

            Ok(Some(remote_block.cast_arc_into()))
        } else {
            let local_block = self.local_storage.block_by_number(number)?;

            Ok(local_block.map(CastArcInto::cast_arc_into))
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockT>>, Self::Error> {
        if let Some(local_block) = self
            .local_storage
            .block_by_transaction_hash(transaction_hash)
        {
            Ok(Some(CastArcFrom::cast_arc_from(local_block)))
        } else {
            let remote_block = tokio::task::block_in_place(move || {
                self.runtime()
                    .block_on(self.remote.block_by_transaction_hash(transaction_hash))
            })?;

            Ok(remote_block.map(CastArcFrom::cast_arc_from))
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn last_block(&self) -> Result<Arc<BlockT>, Self::Error> {
        let last_block_number = self.last_block_number();
        if self.fork_block_number < last_block_number {
            let local_block = self
                .local_storage
                .block_by_number(last_block_number)?
                .expect("Block must exist since block number is less than the last block number");

            Ok(CastArcFrom::cast_arc_from(local_block))
        } else {
            let remote_block = tokio::task::block_in_place(move || {
                self.runtime()
                    .block_on(self.remote.block_by_number(self.fork_block_number))
            })?;

            Ok(CastArcFrom::cast_arc_from(remote_block))
        }
    }
}

impl<
        BlockReceiptT: Debug + ExecutionReceipt<Log = FilterLog> + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT: FetchBlockReceipts<Arc<BlockReceiptT>, Error: Debug>,
        RpcBlockChainSpecT: RpcBlockChainSpec<RpcBlock<RpcTransactionT>: TryInto<EthBlockData<SignedTransactionT>>>,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > GetBlockchainLogs
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, Self::Error> {
        if from_block <= self.fork_block_number {
            let (to_block, mut local_logs) = if to_block <= self.fork_block_number {
                (to_block, Vec::new())
            } else {
                let local_logs = self.local_storage.try_fetch_logs(
                    self.fork_block_number + 1,
                    to_block,
                    addresses,
                    normalized_topics,
                ).expect(
                    "Trait bound guarantees fetching of receipts from local storage is infallible",
                );

                (self.fork_block_number, local_logs)
            };

            let mut remote_logs = tokio::task::block_in_place(move || {
                self.runtime().block_on(self.remote.logs(
                    BlockSpec::Number(from_block),
                    BlockSpec::Number(to_block),
                    addresses,
                    normalized_topics,
                ))
            })?;

            remote_logs.append(&mut local_logs);
            Ok(remote_logs)
        } else {
            Ok(self
                .local_storage
                .try_fetch_logs(from_block, to_block, addresses, normalized_topics)
                .expect(
                    "Trait bound guarantees fetching of receipts from local storage is infallible",
                ))
        }
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT, Error: Debug>,
        BlockT: ?Sized
            + Block<SignedTransactionT>
            + CastArcFrom<LocalBlockT>
            + CastArcFrom<
                RemoteBlock<
                    BlockReceiptT,
                    FetchReceiptErrorT,
                    RpcBlockChainSpecT,
                    RpcReceiptT,
                    RpcTransactionT,
                    SignedTransactionT,
                >,
            >,
        FetchReceiptErrorT,
        HardforkT: Clone + Debug + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT> + EmptyBlock<HardforkT> + LocalBlock<Arc<BlockReceiptT>>,
        RpcBlockChainSpecT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock
                                           + TryInto<EthBlockData<SignedTransactionT>, Error: Debug>,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: RpcTransaction + serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > InsertBlock<BlockT, LocalBlockT, SignedTransactionT>
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn insert_block(
        &mut self,
        block: LocalBlockT,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>, Self::Error> {
        let last_block = self.last_block()?;

        validate_next_block(self.hardfork.clone(), &last_block, &block)?;

        let previous_total_difficulty = self
            .total_difficulty_by_hash(last_block.block_hash())
            .expect("No error can occur as it is stored locally")
            .expect("Must exist as its block is stored");

        let total_difficulty = previous_total_difficulty + block.block_header().difficulty;

        let block = self.local_storage.insert_block_and_receipts(
            Arc::new(block),
            state_diff,
            total_difficulty,
        )?;

        Ok(BlockAndTotalDifficulty::new(
            CastArcFrom::cast_arc_from(block.clone()),
            Some(total_difficulty),
        ))
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT: Clone,
        LocalBlockT,
        RpcBlockChainSpecT: RpcBlockChainSpec<RpcBlock<RpcTransactionT>: TryInto<EthBlockData<SignedTransactionT>>>,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > ReceiptByTransactionHash<BlockReceiptT>
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, Self::Error> {
        if let Some(receipt) = self
            .local_storage
            .receipt_by_transaction_hash(transaction_hash)
        {
            Ok(Some(receipt))
        } else {
            Ok(tokio::task::block_in_place(move || {
                self.runtime()
                    .block_on(self.remote.receipt_by_transaction_hash(transaction_hash))
            })?)
        }
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized
            + Block<SignedTransactionT>
            + CastArcFrom<LocalBlockT>
            + CastArcFrom<
                RemoteBlock<
                    BlockReceiptT,
                    FetchReceiptErrorT,
                    RpcBlockChainSpecT,
                    RpcReceiptT,
                    RpcTransactionT,
                    SignedTransactionT,
                >,
            >,
        FetchReceiptErrorT,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT> + EmptyBlock<HardforkT> + LocalBlock<Arc<BlockReceiptT>>,
        RpcBlockChainSpecT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock + TryInto<EthBlockData<SignedTransactionT>>,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: RpcTransaction + serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > ReserveBlocks
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), Self::Error> {
        let additional = if let Some(additional) = NonZeroU64::new(additional) {
            additional
        } else {
            return Ok(()); // nothing to do
        };

        let last_block = self.last_block()?;
        let previous_total_difficulty = self
            .total_difficulty_by_hash(last_block.block_hash())?
            .expect("Must exist as its block is stored");

        let last_header = last_block.block_header();
        self.local_storage.reserve_blocks(
            additional,
            interval,
            last_header.base_fee_per_gas,
            last_header.state_root,
            previous_total_difficulty,
            BlockConfig {
                base_fee_params: &self.base_fee_params,
                hardfork: self.hardfork.clone(),
                min_ethash_difficulty: self.min_ethash_difficulty,
                scheduled_blob_params: self.scheduled_blob_params.clone(),
            },
        );

        Ok(())
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT: Block<SignedTransactionT>,
        RpcBlockChainSpecT: RpcBlockChainSpec<RpcBlock<RpcTransactionT>: TryInto<EthBlockData<SignedTransactionT>>>,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > RevertToBlock
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error> {
        match block_number.cmp(&self.fork_block_number) {
            std::cmp::Ordering::Less => Err(ForkedBlockchainError::CannotDeleteRemote),
            std::cmp::Ordering::Equal => {
                self.local_storage = ReservableSparseBlockStorage::empty(self.fork_block_number);

                Ok(())
            }
            std::cmp::Ordering::Greater => {
                if self.local_storage.revert_to_block(block_number) {
                    Ok(())
                } else {
                    Err(ForkedBlockchainError::UnknownBlockNumber)
                }
            }
        }
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized
            + Block<SignedTransactionT>
            + CastArcFrom<LocalBlockT>
            + CastArcFrom<
                RemoteBlock<
                    BlockReceiptT,
                    FetchReceiptErrorT,
                    RpcBlockChainSpecT,
                    RpcReceiptT,
                    RpcTransactionT,
                    SignedTransactionT,
                >,
            >,
        FetchReceiptErrorT,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT> + EmptyBlock<HardforkT> + LocalBlock<Arc<BlockReceiptT>>,
        RpcBlockChainSpecT: 'static
            + RpcBlockChainSpec<
                RpcBlock<RpcTransactionT>: RpcEthBlock + TryInto<EthBlockData<SignedTransactionT>>,
            >
            + RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
        RpcReceiptT: 'static + serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: 'static + RpcTransaction + serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > StateAtBlock
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type BlockchainError = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn state_at_block_number(
        &self,
        block_number: u64,
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn DynState>, Self::BlockchainError> {
        if block_number > self.last_block_number() {
            return Err(ForkedBlockchainError::UnknownBlockNumber);
        }

        let state_root = if let Some(state_override) = state_overrides.get(&block_number) {
            state_override.state_root
        } else {
            self.block_by_number(block_number)?
                .expect(
                    "Block must exist since block number is less than equal the last block number.",
                )
                .block_header()
                .state_root
        };

        let mut state = ForkedState::new(
            self.runtime().clone(),
            self.remote.client().clone(),
            self.state_root_generator.clone(),
            block_number,
            state_root,
        );

        let (first_block_number, last_block_number) =
            match block_number.cmp(&self.fork_block_number) {
                // Only override the state at the forked block
                std::cmp::Ordering::Less => (block_number, block_number),
                // Override blocks between the forked block and the requested block
                std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {
                    (self.fork_block_number, block_number)
                }
            };

        compute_state_at_block(
            &mut state,
            &self.local_storage,
            first_block_number,
            last_block_number,
            state_overrides,
        );

        // Override the state root in case the local state was modified
        state.set_state_root(state_root);

        Ok(Box::new(state))
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT>,
        BlockT: ?Sized + Block<SignedTransactionT>,
        FetchReceiptErrorT,
        HardforkT: Clone,
        LocalBlockT,
        RpcBlockChainSpecT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock + TryInto<EthBlockData<SignedTransactionT>>,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > TotalDifficultyByBlockHash
    for ForkedBlockchain<
        BlockReceiptT,
        BlockT,
        FetchReceiptErrorT,
        HardforkT,
        LocalBlockT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = ForkedBlockchainError<
        HardforkT,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
        <BlockReceiptT as TryFrom<RpcReceiptT>>::Error,
    >;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::Error> {
        if let Some(difficulty) = self.local_storage.total_difficulty_by_hash(hash) {
            Ok(Some(difficulty))
        } else {
            Ok(tokio::task::block_in_place(move || {
                self.runtime()
                    .block_on(self.remote.total_difficulty_by_hash(hash))
            })?)
        }
    }
}

/// Arguments for the `recommended_fork_block_number` function.
/// The purpose of this struct is to prevent mixing up the `u64` arguments.
struct RecommendedForkBlockNumberArgs {
    /// The chain id
    pub chain_id: u64,
    /// The latest known block number
    pub latest_block_number: u64,
}

impl<'a> From<&'a RecommendedForkBlockNumberArgs> for LargestSafeBlockNumberArgs {
    fn from(value: &'a RecommendedForkBlockNumberArgs) -> Self {
        Self {
            chain_id: value.chain_id,
            latest_block_number: value.latest_block_number,
        }
    }
}

/// Determines the recommended block number for forking a specific chain based
/// on the latest block number.
///
/// # Design
///
/// If there is no safe block number, then the latest block number will be used.
/// This decision is based on the assumption that a forked blockchain with a
/// `safe_block_depth` larger than the `latest_block_number` has a high
/// probability of being a devnet.
fn recommended_fork_block_number(args: RecommendedForkBlockNumberArgs) -> u64 {
    largest_safe_block_number(LargestSafeBlockNumberArgs::from(&args))
        .unwrap_or(args.latest_block_number)
}

#[cfg(test)]
mod tests {
    const ROPSTEN_CHAIN_ID: u64 = 3;

    use super::*;

    #[test]
    fn recommended_fork_block_number_with_safe_blocks() {
        const LATEST_BLOCK_NUMBER: u64 = 1_000;

        let safe_block_depth = safe_block_depth(ROPSTEN_CHAIN_ID);
        let args = RecommendedForkBlockNumberArgs {
            chain_id: ROPSTEN_CHAIN_ID,
            latest_block_number: LATEST_BLOCK_NUMBER,
        };
        assert_eq!(
            recommended_fork_block_number(args),
            LATEST_BLOCK_NUMBER - safe_block_depth
        );
    }

    #[test]
    fn recommended_fork_block_number_all_blocks_unsafe() {
        const LATEST_BLOCK_NUMBER: u64 = 50;

        let args = RecommendedForkBlockNumberArgs {
            chain_id: ROPSTEN_CHAIN_ID,
            latest_block_number: LATEST_BLOCK_NUMBER,
        };
        assert_eq!(recommended_fork_block_number(args), LATEST_BLOCK_NUMBER);
    }
}
