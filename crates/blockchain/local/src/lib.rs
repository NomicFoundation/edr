use std::{collections::BTreeMap, fmt::Debug, num::NonZeroU64, sync::Arc};

use edr_block_api::{Block as _, BlockAndTotalDifficulty, BlockReceipts};
use edr_block_header::BlockConfig;
use edr_block_storage::ReservableSparseBlockStorage;
use edr_blockchain_api::{utils::compute_state_at_block, Blockchain};
use edr_evm_spec::EvmSpecId;
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::{log::FilterLog, ReceiptTrait};
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
    storage:
        ReservableSparseBlockStorage<BlockReceiptT, HardforkT, LocalBlockT, SignedTransactionT>,
    chain_id: u64,
    hardfork: HardforkT,
}

impl<
        BlockReceiptT: ReceiptTrait,
        HardforkT,
        LocalBlockT: BlockReceipts<Arc<BlockReceiptT>>,
        SignedTransactionT,
    > LocalBlockchain<BlockReceiptT, LocalBlockT, HardforkT, SignedTransactionT>
{
    /// Constructs a new instance with the provided genesis block, validating a
    /// zero block number.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new(
        genesis_block: LocalBlockT,
        genesis_diff: StateDiff,
        chain_id: u64,
        hardfork: HardforkT,
    ) -> Result<Self, InvalidGenesisBlock> {
        let genesis_header = genesis_block.header();

        if genesis_header.number != 0 {
            return Err(InvalidGenesisBlock::InvalidBlockNumber {
                actual: genesis_header.number,
            });
        }

        if hardfork.into() >= EvmSpecId::SHANGHAI && genesis_header.withdrawals_root.is_none() {
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
            chain_id,
            hardfork,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LocalBlockchainError {
    /// Block number does not exist in blockchain
    #[error("Unknown block number")]
    UnknownBlockNumber,
}

impl<
        BlockReceiptT: ReceiptTrait,
        BlockT: ?Sized,
        HardforkT,
        LocalBlockT: BlockReceipts<Arc<BlockReceiptT>> + CastArc<BlockT>,
        SignedTransactionT,
    > Blockchain<BlockT, BlockReceiptT, HardforkT>
    for LocalBlockchain<BlockReceiptT, LocalBlockT, HardforkT, SignedTransactionT>
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
        self.storage
            .logs(from_block, to_block, addresses, normalized_topics)
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

        Ok(self.hardfork)
    }

    fn hardfork(&self) -> HardforkT {
        self.hardfork
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

impl<ChainSpecT: SyncRuntimeSpec>
    BlockchainMut<ChainSpecT::Block, ChainSpecT::LocalBlock, ChainSpecT::SignedTransaction>
    for LocalBlockchain<ChainSpecT>
where
    ChainSpecT::LocalBlock: BlockReceipts<
        Arc<ChainSpecT::BlockReceipt>,
        Error = BlockchainErrorForChainSpec<ChainSpecT>,
    >,
{
    type Error = BlockchainErrorForChainSpec<ChainSpecT>;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn insert_block(
        &mut self,
        block: ChainSpecT::LocalBlock,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficultyForChainSpec<ChainSpecT>, Self::Error> {
        let last_block = self.last_block()?;

        validate_next_block::<ChainSpecT>(self.hardfork, &last_block, &block)?;

        let previous_total_difficulty = self
            .total_difficulty_by_hash(last_block.block_hash())
            .expect("No error can occur as it is stored locally")
            .expect("Must exist as its block is stored");

        let total_difficulty = previous_total_difficulty + block.header().difficulty;

        let block = self
            .storage
            .insert_block(Arc::new(block), state_diff, total_difficulty)?;

        Ok(BlockAndTotalDifficulty::new(
            ChainSpecT::cast_local_block(block.clone()),
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
                hardfork: self.hardfork,
                base_fee_params: base_fee_params_for::<ChainSpecT>(self.chain_id),
            },
        );

        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error> {
        if self.storage.revert_to_block(block_number) {
            Ok(())
        } else {
            Err(BlockchainError::UnknownBlockNumber)
        }
    }
}

impl<ChainSpecT: SyncRuntimeSpec> BlockHash for LocalBlockchain<ChainSpecT> {
    type Error = BlockchainErrorForChainSpec<ChainSpecT>;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        self.storage
            .block_by_number::<ChainSpecT>(block_number)?
            .map(|block| *block.block_hash())
            .ok_or(BlockchainError::UnknownBlockNumber)
    }
}

#[cfg(test)]
mod tests {
    use edr_chain_l1::L1ChainSpec;
    use edr_primitives::HashMap;
    use edr_state_api::account::{Account, AccountInfo, AccountStatus};

    use super::*;
    use crate::{spec::GenesisBlockFactory as _, state::IrregularState, GenesisBlockOptions};

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
                hardfork: edr_chain_l1::Hardfork::SHANGHAI,
                base_fee_params: base_fee_params_for::<edr_chain_l1::L1ChainSpec>(1),
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
