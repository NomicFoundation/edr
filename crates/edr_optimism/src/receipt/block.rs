use edr_eth::log::FilterLog;
use revm_optimism::L1BlockInfo;

use crate::{eip2718::TypedEnvelope, receipt};

pub struct Block {
    pub eth: edr_eth::receipt::BlockReceipt<TypedEnvelope<receipt::Execution<FilterLog>>>,
    pub l1_block_info: L1BlockInfo,
}
