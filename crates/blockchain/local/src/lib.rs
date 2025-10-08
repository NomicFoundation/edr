use std::{collections::BTreeMap, convert::Infallible, fmt::Debug, num::NonZeroU64, sync::Arc};

use edr_block_api::{
    validate_next_block, Block, BlockAndTotalDifficulty, BlockReceipts, BlockValidityError,
    EmptyBlock, LocalBlock,
};
use edr_block_header::BlockConfig;
use edr_block_storage::ReservableSparseBlockStorage;
use edr_blockchain_api::{utils::compute_state_at_block, BlockHash, Blockchain, BlockchainMut};
use edr_eip1559::BaseFeeParams;
use edr_evm_spec::{EvmSpecId, ExecutableTransaction};
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::{log::FilterLog, ExecutionReceipt, ReceiptTrait};
use edr_state_api::{StateDiff, StateError, StateOverride, SyncState};
use edr_state_persistent_trie::PersistentStateTrie;
use edr_utils::CastArc;

/// An error that occurs upon creation of a [`LocalBlockchain`].
#[derive(Debug, thiserror::Error)]
pub enum InvalidGenesisBlock {
    /// Invalid block number in the genesis block.
    #[error("Invalid block number: {actual}. Expected: 0")]
    InvalidBlockNumber {
        /// The actual block number.
        actual: u64,
    },
    /// Missing withdrawals for post-Shanghai blockchain
    #[error("Missing withdrawals for post-Shanghai blockchain")]
    MissingWithdrawals,
}

/// A blockchain consisting of locally created blocks.
#[derive(Debug)]
pub struct LocalBlockchain<BlockReceiptT: ReceiptTrait, HardforkT, LocalBlockT, SignedTransactionT>
{
    storage: ReservableSparseBlockStorage<
        Arc<BlockReceiptT>,
        Arc<LocalBlockT>,
        HardforkT,
        SignedTransactionT,
    >,
    base_fee_params: BaseFeeParams<HardforkT>,
    chain_id: u64,
    hardfork: HardforkT,
    min_ethash_difficulty: u64,
}

impl<
        BlockReceiptT: ReceiptTrait,
        HardforkT: Clone + Into<EvmSpecId>,
        LocalBlockT: Block<SignedTransactionT> + BlockReceipts<Arc<BlockReceiptT>> + Clone,
        SignedTransactionT: ExecutableTransaction,
    > LocalBlockchain<BlockReceiptT, HardforkT, LocalBlockT, SignedTransactionT>
{
    /// Constructs a new instance with the provided genesis block, validating a
    /// zero block number.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new(
        genesis_block: LocalBlockT,
        genesis_diff: StateDiff,
        chain_id: u64,
        hardfork: HardforkT,
        base_fee_params: BaseFeeParams<HardforkT>,
        min_ethash_difficulty: u64,
    ) -> Result<Self, InvalidGenesisBlock> {
        let genesis_header = genesis_block.header();

        if genesis_header.number != 0 {
            return Err(InvalidGenesisBlock::InvalidBlockNumber {
                actual: genesis_header.number,
            });
        }

        if hardfork.clone().into() >= EvmSpecId::SHANGHAI
            && genesis_header.withdrawals_root.is_none()
        {
            return Err(InvalidGenesisBlock::MissingWithdrawals);
        }

        let total_difficulty = genesis_header.difficulty;
        let storage = ReservableSparseBlockStorage::with_genesis_block(
            Arc::new(genesis_block),
            genesis_diff,
            total_difficulty,
        );

        Ok(Self {
            storage,
            base_fee_params,
            chain_id,
            hardfork,
            min_ethash_difficulty,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LocalBlockchainError {
    /// An error that occurs when trying to insert a block and its receipts
    /// into storage.
    #[error(transparent)]
    BlockAndReceiptInsertion(#[from] edr_block_storage::InsertBlockAndReceiptsError),
    /// An error that occurs when trying to insert a local block into storage.
    #[error(transparent)]
    BlockInsertion(#[from] edr_block_storage::InsertBlockError),
    /// An error that occurs when trying to insert an invalid local block.
    #[error(transparent)]
    BlockValidity(#[from] BlockValidityError),
    /// Block number does not exist in blockchain
    #[error("Unknown block number")]
    UnknownBlockNumber,
}

impl<
        BlockReceiptT: Clone + ExecutionReceipt<Log = FilterLog> + ReceiptTrait,
        BlockT: ?Sized,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT>
            + BlockReceipts<Arc<BlockReceiptT>, Error = Infallible>
            + CastArc<BlockT>
            + Clone
            + EmptyBlock<HardforkT>
            + LocalBlock<Arc<BlockReceiptT>>,
        SignedTransactionT: ExecutableTransaction,
    > Blockchain<BlockT, BlockReceiptT, HardforkT>
    for LocalBlockchain<BlockReceiptT, HardforkT, LocalBlockT, SignedTransactionT>
{
    type BlockchainError = LocalBlockchainError;

    type StateError = StateError;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<BlockT>>, Self::BlockchainError> {
        let local_block = self.storage.block_by_hash(hash);

        Ok(local_block.map(CastArc::cast_arc))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_number(&self, number: u64) -> Result<Option<Arc<BlockT>>, Self::BlockchainError> {
        let local_block = self.storage.block_by_number(number)?;

        Ok(local_block.map(CastArc::cast_arc))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockT>>, Self::BlockchainError> {
        let local_block = self.storage.block_by_transaction_hash(transaction_hash);

        Ok(local_block.map(CastArc::cast_arc))
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn last_block(&self) -> Result<Arc<BlockT>, Self::BlockchainError> {
        let local_block = self
            .storage
            .block_by_number(self.storage.last_block_number())?
            .expect("Block must exist");

        Ok(CastArc::cast_arc(local_block))
    }

    fn last_block_number(&self) -> u64 {
        self.storage.last_block_number()
    }

    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, Self::BlockchainError> {
        let logs = self
            .storage
            .logs(from_block, to_block, addresses, normalized_topics)
            .expect("BlockReceipts::logs cannot fail as error type is Infallible");

        Ok(logs)
    }

    fn network_id(&self) -> u64 {
        self.chain_id
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, Self::BlockchainError> {
        Ok(self.storage.receipt_by_transaction_hash(transaction_hash))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, Self::BlockchainError> {
        if block_number > self.last_block_number() {
            return Err(LocalBlockchainError::UnknownBlockNumber);
        }

        Ok(self.hardfork.clone())
    }

    fn hardfork(&self) -> HardforkT {
        self.hardfork.clone()
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn state_at_block_number(
        &self,
        block_number: u64,
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn SyncState<Self::StateError>>, Self::BlockchainError> {
        if block_number > self.last_block_number() {
            return Err(LocalBlockchainError::UnknownBlockNumber);
        }

        let mut state = PersistentStateTrie::default();
        compute_state_at_block(&mut state, &self.storage, 0, block_number, state_overrides);

        Ok(Box::new(state))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::BlockchainError> {
        Ok(self.storage.total_difficulty_by_hash(hash))
    }

    #[doc = " Retrieves the chain ID of the block at the provided number."]
    #[doc = " The chain ID can be different in fork mode pre- and post-fork block"]
    #[doc = " number."]
    fn chain_id_at_block_number(&self, _block_number: u64) -> Result<u64, Self::BlockchainError> {
        Ok(self.chain_id())
    }
}

impl<
        BlockReceiptT: Clone + ExecutionReceipt<Log = FilterLog> + ReceiptTrait,
        BlockT: Block<SignedTransactionT> + ?Sized,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT>
            + BlockReceipts<Arc<BlockReceiptT>, Error = Infallible>
            + CastArc<BlockT>
            + Clone
            + EmptyBlock<HardforkT>
            + LocalBlock<Arc<BlockReceiptT>>,
        SignedTransactionT: ExecutableTransaction,
    > BlockchainMut<BlockT, LocalBlockT, SignedTransactionT>
    for LocalBlockchain<BlockReceiptT, HardforkT, LocalBlockT, SignedTransactionT>
{
    type Error = LocalBlockchainError;

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

        let total_difficulty = previous_total_difficulty + block.header().difficulty;

        let block = self.storage.insert_block_and_receipts(
            Arc::new(block),
            state_diff,
            total_difficulty,
        )?;

        Ok(BlockAndTotalDifficulty::new(
            CastArc::cast_arc(block.clone()),
            Some(total_difficulty),
        ))
    }

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

        let last_header = last_block.header();

        self.storage.reserve_blocks(
            additional,
            interval,
            last_header.base_fee_per_gas,
            last_header.state_root,
            previous_total_difficulty,
            BlockConfig {
                base_fee_params: &self.base_fee_params,
                hardfork: self.hardfork.clone(),
                min_ethash_difficulty: self.min_ethash_difficulty,
            },
        );

        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error> {
        if self.storage.revert_to_block(block_number) {
            Ok(())
        } else {
            Err(LocalBlockchainError::UnknownBlockNumber)
        }
    }
}

impl<
        BlockReceiptT: Clone + ReceiptTrait,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        LocalBlockT: Block<SignedTransactionT> + Clone + EmptyBlock<HardforkT> + LocalBlock<Arc<BlockReceiptT>>,
        SignedTransactionT: ExecutableTransaction,
    > BlockHash for LocalBlockchain<BlockReceiptT, HardforkT, LocalBlockT, SignedTransactionT>
{
    type Error = LocalBlockchainError;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        self.storage
            .block_by_number(block_number)?
            .map(|block| *block.block_hash())
            .ok_or(LocalBlockchainError::UnknownBlockNumber)
    }
}

#[cfg(test)]
mod tests {
    use edr_chain_l1::L1ChainSpec;
    use edr_primitives::HashMap;
    use edr_state_api::account::{Account, AccountInfo, AccountStatus};

    use super::*;

    #[test]
    fn compute_state_after_reserve() -> anyhow::Result<()> {
        let address1 = Address::random();
        let accounts = vec![(
            address1,
            AccountInfo {
                balance: U256::from(1_000_000_000u64),
                ..AccountInfo::default()
            },
        )];

        let genesis_diff: StateDiff = accounts
            .iter()
            .map(|(address, info)| {
                (
                    *address,
                    Account {
                        info: info.clone(),
                        storage: HashMap::new(),
                        status: AccountStatus::Created | AccountStatus::Touched,
                        transaction_id: 0,
                    },
                )
            })
            .collect::<HashMap<_, _>>()
            .into();

        let genesis_block = L1ChainSpec::genesis_block(
            genesis_diff.clone(),
            BlockConfig {
                base_fee_params: base_fee_params_for::<L1ChainSpec>(1),
                hardfork: edr_chain_l1::Hardfork::SHANGHAI,
                min_ethash_difficulty: edr_chain_l1::MIN_ETHASH_DIFFICULTY,
            },
            GenesisBlockOptions {
                gas_limit: Some(6_000_000),
                mix_hash: Some(B256::random()),
                ..GenesisBlockOptions::default()
            },
        )?;

        let mut blockchain = LocalBlockchain::<L1ChainSpec>::new(
            genesis_block,
            genesis_diff,
            123,
            edr_chain_l1::Hardfork::SHANGHAI,
        )
        .unwrap();

        let irregular_state = IrregularState::default();
        let expected = blockchain.state_at_block_number(0, irregular_state.state_overrides())?;

        blockchain.reserve_blocks(1_000_000_000, 1)?;

        let actual =
            blockchain.state_at_block_number(1_000_000_000, irregular_state.state_overrides())?;

        assert_eq!(actual.state_root().unwrap(), expected.state_root().unwrap());

        for (address, expected) in accounts {
            let actual_account = actual.basic(address)?.expect("account should exist");
            assert_eq!(actual_account, expected);
        }

        Ok(())
    }
}
