use std::{collections::BTreeMap, fmt::Debug, num::NonZeroU64, sync::Arc};

use derive_where::derive_where;
use edr_evm_spec::EvmSpecId;
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::log::FilterLog;

use super::{
    compute_state_at_block,
    storage::{ReservableSparseBlockchainStorage, ReservableSparseBlockchainStorageForChainSpec},
    validate_next_block, BlockHash, Blockchain, BlockchainError, BlockchainErrorForChainSpec,
    BlockchainMut,
};
use crate::{
    spec::SyncRuntimeSpec,
    state::{StateDiff, StateError, StateOverride, SyncState, TrieState},
    Block as _, BlockAndTotalDifficulty, BlockAndTotalDifficultyForChainSpec, BlockReceipts,
};

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
#[derive_where(Debug; ChainSpecT::Hardfork)]
pub struct LocalBlockchain<ChainSpecT>
where
    ChainSpecT: SyncRuntimeSpec,
{
    storage: ReservableSparseBlockchainStorageForChainSpec<ChainSpecT>,
    chain_id: u64,
    hardfork: ChainSpecT::Hardfork,
}

impl<ChainSpecT> LocalBlockchain<ChainSpecT>
where
    ChainSpecT: SyncRuntimeSpec<
        LocalBlock: BlockReceipts<
            Arc<ChainSpecT::BlockReceipt>,
            Error = BlockchainErrorForChainSpec<ChainSpecT>,
        >,
    >,
{
    /// Constructs a new instance with the provided genesis block, validating a
    /// zero block number.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new(
        genesis_block: ChainSpecT::LocalBlock,
        genesis_diff: StateDiff,
        chain_id: u64,
        hardfork: ChainSpecT::Hardfork,
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
        let storage = ReservableSparseBlockchainStorage::with_genesis_block(
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

impl<ChainSpecT: SyncRuntimeSpec> Blockchain<ChainSpecT> for LocalBlockchain<ChainSpecT>
where
    ChainSpecT::LocalBlock: BlockReceipts<
        Arc<ChainSpecT::BlockReceipt>,
        Error = BlockchainErrorForChainSpec<ChainSpecT>,
    >,
{
    type BlockchainError = BlockchainErrorForChainSpec<ChainSpecT>;

    type StateError = StateError;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<Arc<ChainSpecT::Block>>, Self::BlockchainError> {
        let local_block = self.storage.block_by_hash(hash);

        Ok(local_block.map(ChainSpecT::cast_local_block))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_number(
        &self,
        number: u64,
    ) -> Result<Option<Arc<ChainSpecT::Block>>, Self::BlockchainError> {
        let local_block = self.storage.block_by_number::<ChainSpecT>(number)?;

        Ok(local_block.map(ChainSpecT::cast_local_block))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    #[allow(clippy::type_complexity)]
    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<ChainSpecT::Block>>, Self::BlockchainError> {
        let local_block = self.storage.block_by_transaction_hash(transaction_hash);

        Ok(local_block.map(ChainSpecT::cast_local_block))
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn last_block(&self) -> Result<Arc<ChainSpecT::Block>, Self::BlockchainError> {
        let local_block = self
            .storage
            .block_by_number::<ChainSpecT>(self.storage.last_block_number())?
            .expect("Block must exist");

        Ok(ChainSpecT::cast_local_block(local_block))
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
    ) -> Result<Option<Arc<ChainSpecT::BlockReceipt>>, Self::BlockchainError> {
        Ok(self.storage.receipt_by_transaction_hash(transaction_hash))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn spec_at_block_number(
        &self,
        block_number: u64,
    ) -> Result<ChainSpecT::Hardfork, Self::BlockchainError> {
        if block_number > self.last_block_number() {
            return Err(BlockchainError::UnknownBlockNumber);
        }

        Ok(self.hardfork)
    }

    fn hardfork(&self) -> ChainSpecT::Hardfork {
        self.hardfork
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn state_at_block_number(
        &self,
        block_number: u64,
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn SyncState<Self::StateError>>, Self::BlockchainError> {
        if block_number > self.last_block_number() {
            return Err(BlockchainError::UnknownBlockNumber);
        }

        let mut state = TrieState::default();
        compute_state_at_block(&mut state, &self.storage, 0, block_number, state_overrides);

        Ok(Box::new(state))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::BlockchainError> {
        Ok(self.storage.total_difficulty_by_hash(hash))
    }
}

impl<ChainSpecT: SyncRuntimeSpec> BlockchainMut<ChainSpecT> for LocalBlockchain<ChainSpecT>
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
            self.hardfork,
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
    use edr_state::account::{Account, AccountInfo, AccountStatus};

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
            edr_chain_l1::Hardfork::SHANGHAI,
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
