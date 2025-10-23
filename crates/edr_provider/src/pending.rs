use core::marker::PhantomData;
use std::sync::Arc;

use edr_block_api::Block;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_primitives::B256;
use edr_state_api::StateDiff;

/// A blockchain with a pending block.
pub(crate) struct BlockchainWithPending<'blockchain, LocalBlockT, SignedTransactionT> {
    blockchain: &'blockchain dyn BlockHashByNumber<Error = DynBlockchainError>,
    pending_block: Arc<LocalBlockT>,
    pending_state_diff: StateDiff,
    _phantom: PhantomData<SignedTransactionT>,
}

impl<'blockchain, LocalBlockT, SignedTransactionT>
    BlockchainWithPending<'blockchain, LocalBlockT, SignedTransactionT>
{
    /// Constructs a new instance with the provided blockchain and pending
    /// block.
    pub fn new(
        blockchain: &'blockchain dyn BlockHashByNumber<Error = DynBlockchainError>,
        pending_block: LocalBlockT,
        pending_state_diff: StateDiff,
    ) -> Self {
        Self {
            blockchain,
            pending_block: pending_block.into(),
            pending_state_diff,
            _phantom: PhantomData,
        }
    }
}

impl<LocalBlockT: Block<SignedTransactionT>, SignedTransactionT> BlockHashByNumber
    for BlockchainWithPending<'_, LocalBlockT, SignedTransactionT>
{
    type Error = DynBlockchainError;

    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        if block_number == self.pending_block.block_header().number {
            Ok(*self.pending_block.block_hash())
        } else {
            self.blockchain.block_hash_by_number(block_number)
        }
    }
}
