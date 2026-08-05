use edr_chain_spec::{EvmSpecId, ProtocolHardfork};
use edr_primitives::{KECCAK_RLP_EMPTY_ARRAY, U256};

use crate::BlockHeader;

/// Calculates the mining difficulty of a block.
pub fn calculate_ethash_canonical_difficulty<HardforkT: ProtocolHardfork>(
    hardfork: HardforkT,
    parent: &BlockHeader,
    block_number: u64,
    block_timestamp: u64,
    min_ethash_difficulty: u64,
) -> U256 {
    // TODO: Create a custom config that prevents usage of older hardforks
    let spec_id = hardfork.to_evm_spec_id();
    assert!(
        spec_id >= EvmSpecId::BYZANTIUM,
        "Hardforks older than Byzantium are not supported"
    );

    let bound_divisor = U256::from(2048);
    let offset = parent.difficulty / bound_divisor;

    let mut difficulty = {
        let uncle_addend = if parent.ommers_hash == KECCAK_RLP_EMPTY_ARRAY {
            1
        } else {
            2
        };
        let a = (block_timestamp - parent.timestamp) / 9;

        if let Some(a) = a.checked_sub(uncle_addend) {
            let a = U256::from(a.min(99));

            parent.difficulty - a * offset
        } else {
            let a = U256::from(uncle_addend - a);
            parent.difficulty + a * offset
        }
    };

    if let Some(exp) = block_number
        .checked_sub(hardfork.bomb_delay())
        .and_then(|num| (num / 100000).checked_sub(2))
    {
        difficulty += U256::from(2u64).pow(U256::from(exp));
    }

    difficulty.max(U256::from(min_ethash_difficulty))
}
