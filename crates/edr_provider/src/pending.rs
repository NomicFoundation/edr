use core::marker::PhantomData;
use std::sync::Arc;

use edr_block_api::Block;
use edr_blockchain_api::{
    r#dyn::DynBlockchainError, BlockHashByNumber, BlockHashByNumberAndScheduledBlobParams,
    BlockchainScheduledBlobParams,
};
use edr_primitives::B256;

/// A blockchain with a pending block.
pub(crate) struct BlockchainWithPending<'blockchain, LocalBlockT, SignedTransactionT> {
    blockchain: &'blockchain dyn BlockHashByNumberAndScheduledBlobParams<DynBlockchainError>,
    pending_block: Arc<LocalBlockT>,
    _phantom: PhantomData<SignedTransactionT>,
}

impl<'blockchain, LocalBlockT, SignedTransactionT>
    BlockchainWithPending<'blockchain, LocalBlockT, SignedTransactionT>
{
    /// Constructs a new instance with the provided blockchain and pending
    /// block.
    pub fn new(
        blockchain: &'blockchain dyn BlockHashByNumberAndScheduledBlobParams<DynBlockchainError>,
        pending_block: LocalBlockT,
    ) -> Self {
        Self {
            blockchain,
            pending_block: pending_block.into(),
            _phantom: PhantomData,
        }
    }

    /// Returns the last block (i.e. the pending block).
    pub fn last_block(&self) -> &Arc<LocalBlockT> {
        &self.pending_block
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

impl<LocalBlockT: Block<SignedTransactionT>, SignedTransactionT> BlockchainScheduledBlobParams
    for BlockchainWithPending<'_, LocalBlockT, SignedTransactionT>
{
    fn scheduled_blob_params(&self) -> Option<&edr_eip7892::ScheduledBlobParams> {
        self.blockchain.scheduled_blob_params()
    }
}
