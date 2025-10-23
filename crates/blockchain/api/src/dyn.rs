use core::marker::PhantomData;

use edr_eip1559::BaseFeeParams;
use edr_primitives::B256;

use crate::{BlockHashByNumber, Blockchain, BlockchainMetadata};

/// Wrapper struct for dynamic dispatch of a blockchain implementation.
///
/// Error types are converted into `Box<dyn std::error::Error>` for dynamic
/// dispatch.
pub struct DynBlockchain<
    BlockReceiptT,
    BlockT: ?Sized,
    BlockchainErrorT: Into<Box<dyn std::error::Error>>,
    BlockchainT: Blockchain<BlockReceiptT, BlockT, BlockchainErrorT, HardforkT, LocalBlockT, SignedTransactionT>,
    HardforkT,
    LocalBlockT,
    SignedTransactionT,
> {
    inner: BlockchainT,
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<
        fn() -> (
            BlockReceiptT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
            // only the last element of a tuple may have a dynamically sized type
            BlockT,
        ),
    >,
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: Into<Box<dyn std::error::Error>>,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
    DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    /// Constructs a new instance.
    pub fn new(inner: BlockchainT) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: Into<Box<dyn std::error::Error>>,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > BlockHashByNumber
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = Box<dyn std::error::Error>;

    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        self.inner
            .block_hash_by_number(block_number)
            .map_err(Into::into)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: Into<Box<dyn std::error::Error>>,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > BlockchainMetadata<HardforkT>
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = Box<dyn std::error::Error>;

    fn base_fee_params(&self) -> &BaseFeeParams<HardforkT> {
        self.inner.base_fee_params()
    }

    fn chain_id(&self) -> u64 {
        self.inner.chain_id()
    }

    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, Self::Error> {
        self.inner
            .spec_at_block_number(block_number)
            .map_err(Into::into)
    }

    fn hardfork(&self) -> HardforkT {
        self.inner.hardfork()
    }

    fn last_block_number(&self) -> u64 {
        self.inner.last_block_number()
    }

    fn min_ethash_difficulty(&self) -> u64 {
        self.inner.min_ethash_difficulty()
    }

    fn network_id(&self) -> u64 {
        self.inner.network_id()
    }
}
