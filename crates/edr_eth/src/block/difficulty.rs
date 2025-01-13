use crate::{block::Header, l1, spec::EthHeaderConstants, trie::KECCAK_RLP_EMPTY_ARRAY, U256};

fn bomb_delay(spec_id: l1::Hardfork) -> u64 {
    match spec_id {
        l1::Hardfork::Byzantium => 3000000,
        l1::Hardfork::Constantinople | l1::Hardfork::Petersburg | l1::Hardfork::Istanbul => 5000000,
        l1::Hardfork::MuirGlacier | l1::Hardfork::Berlin | l1::Hardfork::London => 9000000,
        // l1::Hardfork::London => 9500000, // EIP-3554
        l1::Hardfork::ArrowGlacier => 10700000,
        l1::Hardfork::GrayGlacier => 11400000,
        _ => {
            unreachable!("Post-merge hardforks don't have a bomb delay")
        }
    }
}

/// Calculates the mining difficulty of a block.
pub fn calculate_ethash_canonical_difficulty<ChainSpecT: EthHeaderConstants>(
    hardfork: ChainSpecT::Hardfork,
    parent: &Header,
    block_number: u64,
    block_timestamp: u64,
) -> U256 {
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
        .checked_sub(bomb_delay(hardfork.into()))
        .and_then(|num| (num / 100000).checked_sub(2))
    {
        difficulty += U256::from(2u64).pow(U256::from(exp));
    }

    difficulty.max(U256::from(ChainSpecT::MIN_ETHASH_DIFFICULTY))
}
