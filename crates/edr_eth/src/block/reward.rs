use crate::l1::SpecId;

/// Retrieves the miner reward for the provided hardfork.
pub fn miner_reward(spec_id: SpecId) -> Option<u128> {
    match spec_id {
        SpecId::FRONTIER
        | SpecId::FRONTIER_THAWING
        | SpecId::HOMESTEAD
        | SpecId::DAO_FORK
        | SpecId::TANGERINE
        | SpecId::SPURIOUS_DRAGON => Some(5_000_000_000_000_000_000u128),
        SpecId::BYZANTIUM => Some(3_000_000_000_000_000_000u128),
        SpecId::CONSTANTINOPLE
        | SpecId::PETERSBURG
        | SpecId::ISTANBUL
        | SpecId::MUIR_GLACIER
        | SpecId::BERLIN
        | SpecId::LONDON
        | SpecId::ARROW_GLACIER
        | SpecId::GRAY_GLACIER => Some(2_000_000_000_000_000_000u128),
        _ => None,
    }
}
