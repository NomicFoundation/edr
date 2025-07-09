use crate::{
    block::Header, spec::EthHeaderConstants, trie::KECCAK_RLP_EMPTY_ARRAY, EvmSpecId, U256,
};

fn bomb_delay(spec_id: EvmSpecId) -> u64 {
    match spec_id {
        EvmSpecId::FRONTIER
        | EvmSpecId::FRONTIER_THAWING
        | EvmSpecId::HOMESTEAD
        | EvmSpecId::DAO_FORK
        | EvmSpecId::TANGERINE
        | EvmSpecId::SPURIOUS_DRAGON => 0,
        EvmSpecId::BYZANTIUM => 3000000,
        EvmSpecId::CONSTANTINOPLE | EvmSpecId::PETERSBURG | EvmSpecId::ISTANBUL => 5000000,
        EvmSpecId::MUIR_GLACIER | EvmSpecId::BERLIN | EvmSpecId::LONDON => 9000000,
        // SpecId::LONDON => 9500000, // EIP-3554
        EvmSpecId::ARROW_GLACIER => 10700000,
        EvmSpecId::GRAY_GLACIER => 11400000,
        _ => {
            unreachable!("Post-merge hardforks don't have a bomb delay")
        }
    }
}

/// Calculates the mining difficulty of a block.
pub fn calculate_ethash_canonical_difficulty<ChainSpecT: EthHeaderConstants>(
    spec_id: EvmSpecId,
    parent: &Header,
    block_number: u64,
    block_timestamp: u64,
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

    difficulty.max(U256::from(ChainSpecT::MIN_ETHASH_DIFFICULTY))
}
