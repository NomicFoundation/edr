use revm_primitives::EthSpecId;

use crate::U256;

/// Retrieves the miner reward for the provided hardfork.
pub fn miner_reward(spec_id: EthSpecId) -> Option<U256> {
    match spec_id {
        EthSpecId::FRONTIER
        | EthSpecId::FRONTIER_THAWING
        | EthSpecId::HOMESTEAD
        | EthSpecId::DAO_FORK
        | EthSpecId::TANGERINE
        | EthSpecId::SPURIOUS_DRAGON => Some(U256::from(5_000_000_000_000_000_000u128)),
        EthSpecId::BYZANTIUM => Some(U256::from(3_000_000_000_000_000_000u128)),
        EthSpecId::CONSTANTINOPLE
        | EthSpecId::PETERSBURG
        | EthSpecId::ISTANBUL
        | EthSpecId::MUIR_GLACIER
        | EthSpecId::BERLIN
        | EthSpecId::LONDON
        | EthSpecId::ARROW_GLACIER
        | EthSpecId::GRAY_GLACIER => Some(U256::from(2_000_000_000_000_000_000u128)),
        _ => None,
    }
}
