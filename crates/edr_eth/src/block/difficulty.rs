use crate::{block::Header, trie::KECCAK_RLP_EMPTY_ARRAY, EthSpecId, U256};

fn bomb_delay(spec_id: EthSpecId) -> u64 {
    match spec_id {
        EthSpecId::FRONTIER
        | EthSpecId::FRONTIER_THAWING
        | EthSpecId::HOMESTEAD
        | EthSpecId::DAO_FORK
        | EthSpecId::TANGERINE
        | EthSpecId::SPURIOUS_DRAGON => 0,
        EthSpecId::BYZANTIUM => 3000000,
        EthSpecId::CONSTANTINOPLE | EthSpecId::PETERSBURG | EthSpecId::ISTANBUL => 5000000,
        EthSpecId::MUIR_GLACIER | EthSpecId::BERLIN | EthSpecId::LONDON => 9000000,
        // EthSpecId::LONDON => 9500000, // EIP-3554
        EthSpecId::ARROW_GLACIER => 10700000,
        EthSpecId::GRAY_GLACIER => 11400000,
        _ => {
            unreachable!("Post-merge hardforks don't have a bomb delay")
        }
    }
}

/// Calculates the mining difficulty of a block.
pub fn calculate_ethash_canonical_difficulty(
    spec_id: EthSpecId,
    parent: &Header,
    block_number: u64,
    block_timestamp: u64,
) -> U256 {
    // TODO: Create a custom config that prevents usage of older hardforks
    assert!(
        spec_id >= EthSpecId::BYZANTIUM,
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

    let min_difficulty = U256::from(131072);
    difficulty.max(min_difficulty)
}
