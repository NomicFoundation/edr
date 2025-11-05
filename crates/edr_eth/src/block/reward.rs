use edr_chain_spec::EvmSpecId;

/// Retrieves the miner reward for the provided hardfork.
pub fn miner_reward(spec_id: EvmSpecId) -> Option<u128> {
    match spec_id {
        EvmSpecId::FRONTIER
        | EvmSpecId::FRONTIER_THAWING
        | EvmSpecId::HOMESTEAD
        | EvmSpecId::DAO_FORK
        | EvmSpecId::TANGERINE
        | EvmSpecId::SPURIOUS_DRAGON => Some(5_000_000_000_000_000_000u128),
        EvmSpecId::BYZANTIUM => Some(3_000_000_000_000_000_000u128),
        EvmSpecId::CONSTANTINOPLE
        | EvmSpecId::PETERSBURG
        | EvmSpecId::ISTANBUL
        | EvmSpecId::MUIR_GLACIER
        | EvmSpecId::BERLIN
        | EvmSpecId::LONDON
        | EvmSpecId::ARROW_GLACIER
        | EvmSpecId::GRAY_GLACIER => Some(2_000_000_000_000_000_000u128),
        _ => None,
    }
}
