mod builder;
mod local;

use edr_eth::log::FilterLog;
use edr_evm::SyncBlock;
use revm_optimism::L1BlockInfo;

pub use self::{builder::Builder, local::LocalBlock};
use crate::{eip2718::TypedEnvelope, receipt, transaction};

/// Trait for Optimism-specific block information.
pub trait OptimismBlock {
    /// Retrieves the block's L1 block info.
    fn l1_block_info(&self) -> &L1BlockInfo;
}

pub trait SyncOptimismBlock:
    OptimismBlock + SyncBlock<TypedEnvelope<receipt::Execution<FilterLog>>, transaction::Signed>
{
}
