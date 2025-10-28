use std::time::Instant;

use edr_chain_spec::ExecutableTransaction;
use edr_primitives::Address;
use edr_runtime::MemPool;
use edr_state_api::irregular::IrregularState;
use edr_utils::random::RandomHashGenerator;
use rpds::HashTrieMapSync;

use crate::data::StateId;

pub(crate) struct Snapshot<SignedTransactionT: ExecutableTransaction> {
    pub block_number: u64,
    pub block_number_to_state_id: HashTrieMapSync<u64, StateId>,
    pub block_time_offset_seconds: i64,
    pub coinbase: Address,
    pub irregular_state: IrregularState,
    pub mem_pool: MemPool<SignedTransactionT>,
    pub next_block_base_fee_per_gas: Option<u128>,
    pub next_block_timestamp: Option<u64>,
    pub parent_beacon_block_root_generator: RandomHashGenerator,
    pub prev_randao_generator: RandomHashGenerator,
    pub time: Instant,
}
