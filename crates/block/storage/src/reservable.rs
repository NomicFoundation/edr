use core::fmt::Debug;
use std::num::NonZeroU64;

use edr_block_api::{Block, BlockReceipts, EmptyBlock, FetchBlockReceipts, LocalBlock};
use edr_block_header::{BlockConfig, HeaderOverrides, PartialHeader};
use edr_chain_spec::{EvmSpecId, ExecutableTransaction};
use edr_eip1559::BaseFeeParams;
use edr_primitives::{Address, HashMap, HashSet, B256, U256};
use edr_receipt::{log::FilterLog, ExecutionReceipt, ReceiptTrait};
use edr_state_api::StateDiff;
use parking_lot::{RwLock, RwLockUpgradableReadGuard, RwLockWriteGuard};

use super::{sparse, InsertBlockError, SparseBlockStorage};
use crate::InsertBlockAndReceiptsError;

/// A reservation for a sequence of blocks that have not yet been inserted into
/// storage.
#[derive(Clone, Debug)]
struct Reservation<HardforkT> {
    first_number: u64,
    last_number: u64,
    interval: u64,
    previous_base_fee_per_gas: Option<u128>,
    previous_state_root: B256,
    previous_total_difficulty: U256,
    previous_diff_index: usize,
    hardfork: HardforkT,
    base_fee_params: BaseFeeParams<HardforkT>,
    min_ethash_difficulty: u64,
}

/// A storage solution for storing a subset of a blockchain's blocks in-memory,
/// while lazily loading blocks that have been reserved.
#[derive(Debug)]
pub struct ReservableSparseBlockStorage<
    BlockReceiptT: ReceiptTrait,
    BlockT,
    HardforkT,
    SignedTransactionT,
> {
    reservations: RwLock<Vec<Reservation<HardforkT>>>,
    storage: RwLock<SparseBlockStorage<BlockReceiptT, BlockT, SignedTransactionT>>,
    // We can store the state diffs contiguously, as reservations don't contain any diffs.
    // Diffs are a mapping from one state to the next, so the genesis block contains the initial
    // state.
    state_diffs: Vec<(u64, StateDiff)>,
    number_to_diff_index: HashMap<u64, usize>,
    last_block_number: u64,
}

impl<BlockReceiptT: ReceiptTrait, BlockT, HardforkT, SignedTransactionT>
    ReservableSparseBlockStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Constructs a new instance with no blocks.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn empty(last_block_number: u64) -> Self {
        Self {
            reservations: RwLock::new(Vec::new()),
            storage: RwLock::new(SparseBlockStorage::default()),
            state_diffs: Vec::new(),
            number_to_diff_index: HashMap::default(),
            last_block_number,
        }
    }
}

impl<
        BlockReceiptT: ReceiptTrait,
        BlockT: Block<SignedTransactionT> + Clone,
        HardforkT,
        SignedTransactionT: ExecutableTransaction,
    > ReservableSparseBlockStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Constructs a new instance with the provided block as genesis block.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn with_genesis_block(block: BlockT, diff: StateDiff, total_difficulty: U256) -> Self {
        Self {
            reservations: RwLock::new(Vec::new()),
            storage: RwLock::new(SparseBlockStorage::with_block(block, total_difficulty)),
            state_diffs: vec![(0, diff)],
            number_to_diff_index: std::iter::once((0, 0)).collect(),
            last_block_number: 0,
        }
    }

    /// Reverts to the block with the provided number, deleting all later
    /// blocks.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn revert_to_block(&mut self, block_number: u64) -> bool {
        if block_number > self.last_block_number {
            return false;
        }

        self.last_block_number = block_number;

        self.storage.get_mut().revert_to_block(block_number);

        if block_number == 0 {
            // Reservations and state diffs can only occur after the genesis block,
            // so we can clear them all
            self.reservations.get_mut().clear();

            // Keep the genesis block's diff
            self.state_diffs.truncate(1);

            // Keep the genesis block's mapping
            self.number_to_diff_index.clear();
            self.number_to_diff_index.insert(0, 0);
        } else {
            // Only retain reservations that are not fully reverted
            self.reservations.get_mut().retain_mut(|reservation| {
                if reservation.last_number <= block_number {
                    true
                } else if reservation.first_number <= block_number {
                    reservation.last_number = block_number;
                    true
                } else {
                    false
                }
            });

            // Remove all diffs that are newer than the reverted block
            let diff_index = self
                .number_to_diff_index
                .get(&block_number)
                .copied()
                .unwrap_or_else(|| {
                    let reservations = self.reservations.get_mut();

                    find_reservation(reservations, block_number)
                    .expect("There must either be a block or a reservation matching the block number").previous_diff_index
                });

            self.state_diffs.truncate(diff_index + 1);

            self.number_to_diff_index
                .retain(|number, _| *number <= block_number);
        }

        true
    }
}
impl<
        BlockReceiptT: ExecutionReceipt<Log = FilterLog> + ReceiptTrait,
        BlockT: BlockReceipts<BlockReceiptT>,
        HardforkT,
        SignedTransactionT,
    > ReservableSparseBlockStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Retrieves the logs that match the provided filter.
    pub fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Vec<FilterLog> {
        let storage = self.storage.read();
        sparse::logs(&storage, from_block, to_block, addresses, normalized_topics)
    }
}

impl<
        BlockReceiptT: ExecutionReceipt<Log = FilterLog> + ReceiptTrait,
        BlockT: FetchBlockReceipts<BlockReceiptT>,
        HardforkT,
        SignedTransactionT,
    > ReservableSparseBlockStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Tries to fetch the logs that match the provided filter.
    pub fn try_fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, BlockT::Error> {
        let storage = self.storage.read();
        sparse::try_fetch_logs(&storage, from_block, to_block, addresses, normalized_topics)
    }
}

impl<BlockReceiptT: Clone + ReceiptTrait, BlockT: Clone, HardforkT: Clone, SignedTransactionT>
    ReservableSparseBlockStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Retrieves the block by hash, if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn block_by_hash(&self, hash: &B256) -> Option<BlockT> {
        self.storage.read().block_by_hash(hash).cloned()
    }

    /// Retrieves the block that contains the transaction with the provided
    /// hash, if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn block_by_transaction_hash(&self, transaction_hash: &B256) -> Option<BlockT> {
        self.storage
            .read()
            .block_by_transaction_hash(transaction_hash)
            .cloned()
    }

    /// Retrieves whether a block with the provided number exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn contains_block_number(&self, number: u64) -> bool {
        self.storage.read().contains_block_number(number)
    }

    /// Retrieves the last block number.
    pub fn last_block_number(&self) -> u64 {
        self.last_block_number
    }

    /// Retrieves the sequence of diffs from the genesis state to the state of
    /// the block with the provided number, if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn state_diffs_until_block(&self, block_number: u64) -> Option<&[(u64, StateDiff)]> {
        let diff_index = self
            .number_to_diff_index
            .get(&block_number)
            .copied()
            .or_else(|| {
                let reservations = self.reservations.read();
                find_reservation(&reservations, block_number)
                    .map(|reservation| reservation.previous_diff_index)
            })?;

        Some(
            self.state_diffs
                .get(0..=diff_index)
                .expect("diff_index is valid"),
        )
    }

    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn receipt_by_transaction_hash(&self, transaction_hash: &B256) -> Option<BlockReceiptT> {
        self.storage
            .read()
            .receipt_by_transaction_hash(transaction_hash)
            .cloned()
    }

    /// Reserves the provided number of blocks, starting from the next block
    /// number.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn reserve_blocks(
        &mut self,
        additional: NonZeroU64,
        interval: u64,
        previous_base_fee: Option<u128>,
        previous_state_root: B256,
        previous_total_difficulty: U256,
        block_config: BlockConfig<'_, HardforkT>,
    ) {
        let reservation = Reservation {
            first_number: self.last_block_number + 1,
            last_number: self.last_block_number + additional.get(),
            interval,
            previous_base_fee_per_gas: previous_base_fee,
            previous_state_root,
            previous_total_difficulty,
            previous_diff_index: self.state_diffs.len() - 1,
            hardfork: block_config.hardfork,
            base_fee_params: (*block_config.base_fee_params).clone(),
            min_ethash_difficulty: block_config.min_ethash_difficulty,
        };

        self.reservations.get_mut().push(reservation);
        self.last_block_number += additional.get();
    }

    /// Retrieves the total difficulty of the block with the provided hash.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn total_difficulty_by_hash(&self, hash: &B256) -> Option<U256> {
        self.storage.read().total_difficulty_by_hash(hash).cloned()
    }
}

impl<
        BlockReceiptT: Clone + ReceiptTrait,
        BlockT: Block<SignedTransactionT> + Clone + EmptyBlock<HardforkT> + LocalBlock<BlockReceiptT>,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        SignedTransactionT: ExecutableTransaction,
    > ReservableSparseBlockStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Retrieves the block by number, if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn block_by_number(&self, number: u64) -> Result<Option<BlockT>, InsertBlockError> {
        Ok(self
            .try_fulfilling_reservation(number)?
            .or_else(|| self.storage.read().block_by_number(number).cloned()))
    }

    /// Insert a block into the storage. Errors if a block with the same hash or
    /// number already exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn insert_block_and_receipts(
        &mut self,
        block: BlockT,
        state_diff: StateDiff,
        total_difficulty: U256,
    ) -> Result<&BlockT, InsertBlockAndReceiptsError> {
        self.last_block_number = block.block_header().number;
        self.number_to_diff_index
            .insert(self.last_block_number, self.state_diffs.len());

        self.state_diffs.push((self.last_block_number, state_diff));

        let receipts: Vec<_> = block.transaction_receipts().to_vec();
        self.storage.get_mut().insert_receipts(receipts)?;

        self.storage
            .get_mut()
            .insert_block(block, total_difficulty)
            .map_err(InsertBlockAndReceiptsError::from)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn try_fulfilling_reservation(
        &self,
        block_number: u64,
    ) -> Result<Option<BlockT>, InsertBlockError> {
        let reservations = self.reservations.upgradable_read();

        reservations
            .iter()
            .enumerate()
            .find_map(|(idx, reservation)| {
                if reservation.first_number <= block_number
                    && block_number <= reservation.last_number
                {
                    Some(idx)
                } else {
                    None
                }
            })
            .map(|idx| {
                let mut reservations = RwLockUpgradableReadGuard::upgrade(reservations);
                let reservation = reservations.remove(idx);

                if block_number != reservation.first_number {
                    reservations.push(Reservation {
                        last_number: block_number - 1,
                        ..reservation.clone()
                    });
                }

                if block_number != reservation.last_number {
                    reservations.push(Reservation {
                        first_number: block_number + 1,
                        ..reservation.clone()
                    });
                }

                let reservations = RwLockWriteGuard::downgrade(reservations);
                let storage = self.storage.upgradable_read();

                let timestamp = calculate_timestamp_for_reserved_block(
                    &storage,
                    &reservations,
                    &reservation,
                    block_number,
                );

                let block = BlockT::empty(
                    reservation.hardfork.clone(),
                    PartialHeader::new(
                        BlockConfig {
                            base_fee_params: &reservation.base_fee_params,
                            hardfork: reservation.hardfork,
                            min_ethash_difficulty: reservation.min_ethash_difficulty,
                        },
                        HeaderOverrides {
                            number: Some(block_number),
                            state_root: Some(reservation.previous_state_root),
                            base_fee: reservation.previous_base_fee_per_gas,
                            timestamp: Some(timestamp),
                            ..HeaderOverrides::default()
                        },
                        None,
                        &Vec::new(),
                        None,
                    ),
                );

                {
                    let mut storage = RwLockUpgradableReadGuard::upgrade(storage);
                    Ok(storage
                        .insert_block(block, reservation.previous_total_difficulty)?
                        .clone())
                }
            })
            .transpose()
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
fn calculate_timestamp_for_reserved_block<
    BlockReceiptT: ReceiptTrait,
    BlockT: Block<SignedTransactionT>,
    HardforkT,
    SignedTransactionT,
>(
    storage: &SparseBlockStorage<BlockReceiptT, BlockT, SignedTransactionT>,
    reservations: &Vec<Reservation<HardforkT>>,
    reservation: &Reservation<HardforkT>,
    block_number: u64,
) -> u64 {
    let previous_block_number = reservation.first_number - 1;
    let previous_timestamp =
        if let Some(previous_reservation) = find_reservation(reservations, previous_block_number) {
            calculate_timestamp_for_reserved_block(
                storage,
                reservations,
                previous_reservation,
                previous_block_number,
            )
        } else {
            let previous_block = storage
                .block_by_number(previous_block_number)
                .expect("Block must exist");

            previous_block.block_header().timestamp
        };

    previous_timestamp + reservation.interval * (block_number - reservation.first_number + 1)
}

fn find_reservation<HardforkT>(
    reservations: &[Reservation<HardforkT>],
    number: u64,
) -> Option<&Reservation<HardforkT>> {
    reservations
        .iter()
        .find(|reservation| reservation.first_number <= number && number <= reservation.last_number)
}
