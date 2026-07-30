use edr_chain_spec::EvmSpecId;
use edr_primitives::{KECCAK_RLP_EMPTY_ARRAY, U256};

use crate::BlockHeader;

fn bomb_delay(spec_id: EvmSpecId) -> u64 {
    // Note: revm removed the EVM-equivalent `SpecId` variants (Frontier
    // Thawing, DAO Fork, Constantinople, Muir Glacier, Arrow Glacier, and Gray
    // Glacier), so the bomb delays that were introduced by the removed
    // glacier hardforks (Muir Glacier: 9M, Arrow Glacier: 10.7M, Gray
    // Glacier: 11.4M) can no longer be distinguished by `spec_id` alone.
    match spec_id {
        EvmSpecId::FRONTIER
        | EvmSpecId::HOMESTEAD
        | EvmSpecId::TANGERINE
        | EvmSpecId::SPURIOUS_DRAGON => 0,
        EvmSpecId::BYZANTIUM => 3000000,
        EvmSpecId::PETERSBURG | EvmSpecId::ISTANBUL => 5000000,
        EvmSpecId::BERLIN | EvmSpecId::LONDON => 9000000,
        // SpecId::LONDON => 9500000, // EIP-3554
        _ => {
            unreachable!("Post-merge hardforks don't have a bomb delay")
        }
    }
}

/// Calculates the mining difficulty of a block.
pub fn calculate_ethash_canonical_difficulty(
    spec_id: EvmSpecId,
    parent: &BlockHeader,
    block_number: u64,
    block_timestamp: u64,
    min_ethash_difficulty: u64,
) -> U256 {
    // TODO: Create a custom config that prevents usage of older hardforks
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
        .checked_sub(bomb_delay(spec_id))
        .and_then(|num| (num / 100000).checked_sub(2))
    {
        difficulty += U256::from(2u64).pow(U256::from(exp));
    }

    difficulty.max(U256::from(min_ethash_difficulty))
}
