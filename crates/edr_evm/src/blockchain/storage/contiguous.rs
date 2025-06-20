//! A more cache-friendly block storage implementation.
//!
//! While this does not support block reservations, which we require[^1], it
//! still may be useful for applications that do not.
//!
//! [^1]: for that, we internally use the sparse implementation via
//! [`SparseBlockchainStorage`](super::sparse::SparseBlockchainStorage).

use std::marker::PhantomData;

use derive_where::derive_where;
use edr_eth::{receipt::ReceiptTrait, transaction::ExecutableTransaction, HashMap, B256, U256};

use super::InsertError;
use crate::{Block, EmptyBlock, LocalBlock};

/// A storage solution for storing a Blockchain's blocks contiguously in-memory.
#[derive_where(Clone, Debug, Default; BlockReceiptT, BlockT)]
pub struct ContiguousBlockchainStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT> {
    blocks: Vec<BlockT>,
    hash_to_block: HashMap<B256, BlockT>,
    total_difficulties: Vec<U256>,
    transaction_hash_to_block: HashMap<B256, BlockT>,
    transaction_hash_to_receipt: HashMap<B256, BlockReceiptT>,
    phantom: PhantomData<(HardforkT, SignedTransactionT)>,
}

impl<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
    ContiguousBlockchainStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Retrieves the instance's blocks.
    pub fn blocks(&self) -> &[BlockT] {
        &self.blocks
    }

    /// Retrieves a block by its hash.
    pub fn block_by_hash(&self, hash: &B256) -> Option<&BlockT> {
        self.hash_to_block.get(hash)
    }

    /// Retrieves the block that contains the transaction with the provided
    /// hash, if it exists.
    pub fn block_by_transaction_hash(&self, transaction_hash: &B256) -> Option<&BlockT> {
        self.transaction_hash_to_block.get(transaction_hash)
    }

    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    pub fn receipt_by_transaction_hash(&self, transaction_hash: &B256) -> Option<&BlockReceiptT> {
        self.transaction_hash_to_receipt.get(transaction_hash)
    }

    /// Retrieves the instance's total difficulties.
    pub fn total_difficulties(&self) -> &[U256] {
        &self.total_difficulties
    }
}

impl<BlockReceiptT, BlockT: Block<SignedTransactionT>, HardforkT, SignedTransactionT>
    ContiguousBlockchainStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Reverts to the block with the provided number, deleting all later
    /// blocks.
    pub fn revert_to_block(&mut self, block_number: &U256) -> bool {
        let block_number = usize::try_from(block_number)
            .expect("No blocks with a number larger than usize::MAX are inserted");

        let block_index = if let Some(first_block) = self.blocks.first() {
            let first_block_number = usize::try_from(first_block.header().number)
                .expect("No blocks with a number larger than usize::MAX are inserted");

            if block_number < first_block_number {
                return false;
            }

            block_number - first_block_number
        } else {
            return false;
        };

        if block_index >= self.blocks.len() {
            return false;
        }

        self.total_difficulties.truncate(block_index + 1);
        let removed_blocks = self.blocks.split_off(block_index + 1);

        for block in removed_blocks {
            let block_hash = block.block_hash();

            self.hash_to_block.remove(block_hash);
            self.transaction_hash_to_block.remove(block_hash);
            self.transaction_hash_to_receipt.remove(block_hash);
        }

        true
    }

    /// Retrieves the total difficulty of the block with the provided hash.
    pub fn total_difficulty_by_hash(&self, hash: &B256) -> Option<&U256> {
        self.hash_to_block.get(hash).map(|block| {
            let first_block_number = self
                .blocks
                .first()
                .expect("A block must exist since we found one")
                .header()
                .number;

            let local_block_number = usize::try_from(block.header().number - first_block_number)
                .expect("No blocks with a number larger than usize::MAX are inserted");

            // SAFETY: A total difficulty is inserted for each block
            unsafe { self.total_difficulties.get_unchecked(local_block_number) }
        })
    }
}

impl<
        BlockReceiptT: Clone + ReceiptTrait,
        BlockT: Block<SignedTransactionT> + EmptyBlock<HardforkT> + LocalBlock<BlockReceiptT> + Clone,
        HardforkT,
        SignedTransactionT: ExecutableTransaction,
    > ContiguousBlockchainStorage<BlockReceiptT, BlockT, HardforkT, SignedTransactionT>
{
    /// Constructs a new instance with the provided block.
    pub fn with_block(block: BlockT, total_difficulty: U256) -> Self {
        let block_hash = *block.block_hash();

        let transaction_hash_to_receipt = block
            .transaction_receipts()
            .iter()
            .map(|receipt| (*receipt.transaction_hash(), receipt.clone()))
            .collect();

        let transaction_hash_to_block = block
            .transactions()
            .iter()
            .map(|transaction| (*transaction.transaction_hash(), block.clone()))
            .collect();

        let mut hash_to_block = HashMap::new();
        hash_to_block.insert(block_hash, block.clone());

        Self {
            total_difficulties: vec![total_difficulty],
            blocks: vec![block],
            hash_to_block,
            transaction_hash_to_block,
            transaction_hash_to_receipt,
            phantom: PhantomData,
        }
    }

    /// Inserts a block, failing if a block with the same hash already exists.
    pub fn insert_block(
        &mut self,
        block: BlockT,
        total_difficulty: U256,
    ) -> Result<&BlockT, InsertError> {
        let block_hash = block.block_hash();

        // As blocks are contiguous, we are guaranteed that the block number won't exist
        // if its hash is not present.
        if self.hash_to_block.contains_key(block_hash) {
            return Err(InsertError::DuplicateBlock {
                block_hash: *block_hash,
                block_number: block.header().number,
            });
        }

        if let Some(transaction) = block.transactions().iter().find(|transaction| {
            self.transaction_hash_to_block
                .contains_key(transaction.transaction_hash())
        }) {
            return Err(InsertError::DuplicateTransaction {
                hash: *transaction.transaction_hash(),
            });
        }

        // SAFETY: We checked that block hash doesn't exist yet
        Ok(unsafe { self.insert_block_unchecked(block, total_difficulty) })
    }

    /// Inserts a block without checking its validity.
    ///
    /// # Safety
    ///
    /// Ensure that the instance does not contain a block with the same hash,
    /// nor any transactions with the same hash.
    pub unsafe fn insert_block_unchecked(
        &mut self,
        block: BlockT,
        total_difficulty: U256,
    ) -> &BlockT {
        self.transaction_hash_to_receipt.extend(
            block
                .transaction_receipts()
                .iter()
                .map(|receipt| (*receipt.transaction_hash(), receipt.clone())),
        );

        self.transaction_hash_to_block.extend(
            block
                .transactions()
                .iter()
                .map(|transaction| (*transaction.transaction_hash(), block.clone())),
        );

        self.blocks.push(block.clone());
        self.total_difficulties.push(total_difficulty);

        // SAFETY: The safety concern is propagated in the function signature.
        unsafe {
            self.hash_to_block
                .insert_unique_unchecked(*block.block_hash(), block)
        }
        .1
    }
}
