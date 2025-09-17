use auto_impl::auto_impl;
use edr_primitives::B256;

/// Trait for retrieving a block's hash by number.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait BlockHash {
    /// The blockchain's error type.
    type Error;

    /// Retrieves the block hash at the provided number.
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error>;
}
