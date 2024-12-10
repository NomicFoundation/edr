mod builder;
mod local;

use edr_eth::log::FilterLog;
use edr_evm::{RemoteBlock, SyncBlock};
use revm_optimism::L1BlockInfo;

pub use self::{
    builder::{BlockReceiptFactory, Builder},
    local::LocalBlock,
};
use crate::{eip2718::TypedEnvelope, receipt, transaction, OptimismChainSpec};
